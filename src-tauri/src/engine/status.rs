//! Status — the `GTGitStatusOptionInclude*` surface, porcelain-v2-equivalent
//! (docs/03 §3 `wc_status_load`, `wc_entry_detail_load`; evidence F11/F15).
//!
//! The read is built on libgit2's status machinery, which already applies the
//! repository's ignore rules (`.gitignore`, `.git/info/exclude`, the global
//! excludes file) and `.gitattributes`-driven filters. On top of it this
//! module adds the three things the evidence's status surface has that the
//! packed `git2:: Status` bitfield does not carry: rename sources, submodule
//! state, and the orthogonal assume-unchanged flag (evidence F2/F16).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use git2::{Delta, IndexEntry, Status, StatusEntry};

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::status::FileStatus;
use crate::domain::working_copy::{
    EntryConflict, EntryDetail, FileMode, SubmoduleEntryState, SubmoduleState, WorkingCopyEntry,
    WorkingCopyStatusOptions,
};

/// The assume-unchanged bit as it lives in the index entry's flags word
/// (`GIT_INDEX_ENTRY_VALID`, and the on-disk "assume-valid" bit).
pub(crate) const INDEX_ENTRY_VALID: u16 = 0x8000;

/// The one index-lookup helper the crate uses.
///
/// `git2::Index::get_path` *panics* rather than erroring on a path whose first
/// component is `.` or `..` (git2-rs unwraps `path_to_repo_path`), so a stray
/// `"."` arriving from the UI would take the process down. Such a path simply
/// is not an index entry, and answering `None` is both true and safe.
pub(crate) fn lookup(index: &git2::Index, path: &str, stage: i32) -> Option<IndexEntry> {
    if !is_lookupable(path) {
        return None;
    }
    index.get_path(Path::new(path), stage)
}

fn is_lookupable(path: &str) -> bool {
    let mut components = Path::new(path).components();
    match components.next() {
        // The first component decides whether git2-rs panics …
        Some(std::path::Component::Normal(_)) => {}
        _ => return false,
    }
    // … and `..` anywhere would be a path that leaves the repository.
    !components.any(|component| matches!(component, std::path::Component::ParentDir))
}

/// Read the working-copy status.
pub fn load(
    repo: &git2::Repository,
    opts: &WorkingCopyStatusOptions,
) -> Result<Vec<WorkingCopyEntry>, ErrorPayload> {
    ensure_working_copy(repo)?;

    let index = repo.index()?;
    let assume_unchanged = assume_unchanged_set(&index);
    let submodules = submodule_states(repo, &index);

    let mut git_opts = git2::StatusOptions::new();
    git_opts
        .include_untracked(true)
        .include_ignored(opts.include_ignored)
        // Clean rows are needed for `include_nonmodified`, and they are how
        // libgit2 surfaces assume-unchanged entries at all (the index→workdir
        // diff reports those as unmodified).
        .include_unmodified(opts.include_nonmodified || opts.include_assume_unchanged)
        .recurse_untracked_dirs(!opts.include_dirs)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo.statuses(Some(&mut git_opts))?;

    let mut entries = Vec::with_capacity(statuses.len());
    for entry in statuses.iter() {
        let Ok(path) = entry.path() else { continue };
        let status = entry.status();
        let clean = !status.intersects(TrackedChangeBits::any());
        let assume = assume_unchanged.contains(path);

        if opts.conflicted_only && !status.contains(Status::CONFLICTED) {
            continue;
        }
        // Clean rows are filtered here: they appear only when asked for, or
        // when the file carries the assume-unchanged flag that the user asked
        // to have listed.
        if clean && !opts.include_nonmodified && !(opts.include_assume_unchanged && assume) {
            continue;
        }

        entries.extend(entry_rows(&entry, path, status, assume, &submodules));
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| b.staged.cmp(&a.staged)));
    Ok(entries)
}

/// Read one entry's detail rows (`wc_entry_detail_load`).
pub fn entry_detail(repo: &git2::Repository, path: &str) -> Result<EntryDetail, ErrorPayload> {
    ensure_working_copy(repo)?;
    let workdir = workdir(repo)?;
    let full = workdir.join(path);

    let index = repo.index()?;
    // `IndexEntry` is not `Copy`, so the two facts the detail needs are read
    // out once, up front.
    let index_entry = lookup(&index, path, 0);
    let index_mode = index_entry.as_ref().map(|entry| entry.mode);
    let index_oid = index_entry.as_ref().map(|entry| entry.id);

    let (exists, size, mtime, fs_mode) = match std::fs::symlink_metadata(&full) {
        Ok(meta) => {
            let kind = fs_file_mode(&meta);
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            (true, meta.len(), mtime, Some(kind))
        }
        Err(_) => (false, 0, None, None),
    };

    let mode = fs_mode
        .or_else(|| index_mode.map(mode_from_raw))
        .unwrap_or(FileMode::Unreadable);

    let conflict = conflict_sides(&index, path);

    let submodule = if mode == FileMode::Submodule {
        index_oid.and_then(|oid| submodule_state_at(repo, path, oid))
    } else {
        None
    };

    Ok(EntryDetail {
        path: path.to_string(),
        exists,
        size,
        mode,
        mtime,
        oid: index_oid.map(|oid| oid.to_string()),
        conflict,
        submodule,
    })
}

/// Whether the path currently resolves through the ignore machinery — the
/// check `wc_entry_ignore` and explicit staging use.
pub fn is_ignored(repo: &git2::Repository, path: &str) -> Result<bool, ErrorPayload> {
    Ok(repo.status_should_ignore(Path::new(path))?)
}

/// Whether the path has an index entry (any stage).
pub fn is_tracked(repo: &git2::Repository, path: &str) -> Result<bool, ErrorPayload> {
    let index = repo.index()?;
    Ok((0..=3).any(|stage| lookup(&index, path, stage).is_some()))
}

/// Whether the path is unmerged (any conflict stage present, no stage 0).
pub fn is_conflicted(repo: &git2::Repository, path: &str) -> Result<bool, ErrorPayload> {
    let index = repo.index()?;
    Ok(has_conflict_stages(&index, path))
}

fn has_conflict_stages(index: &git2::Index, path: &str) -> bool {
    lookup(index, path, 0).is_none() && (1..=3).any(|stage| lookup(index, path, stage).is_some())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// The status bits that describe a change on one of the two tracked axes.
struct TrackedChangeBits;

impl TrackedChangeBits {
    fn any() -> Status {
        Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE
            | Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_RENAMED
            | Status::WT_TYPECHANGE
            | Status::WT_UNREADABLE
            | Status::IGNORED
            | Status::CONFLICTED
    }
}

fn ensure_working_copy(repo: &git2::Repository) -> Result<(), ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    Ok(())
}

fn workdir(repo: &git2::Repository) -> Result<&Path, ErrorPayload> {
    repo.workdir().ok_or_else(|| {
        ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
    })
}

/// Turn one packed status entry into its rows — one per axis, an unmerged
/// path producing a single row (conflicts are neither staged nor unstaged).
fn entry_rows(
    entry: &StatusEntry<'_>,
    path: &str,
    status: Status,
    assume_unchanged: bool,
    submodules: &HashMap<String, SubmoduleEntryState>,
) -> Vec<WorkingCopyEntry> {
    if status.contains(Status::CONFLICTED) {
        let row = WorkingCopyEntry::changed(path, FileStatus::Unmerged, false, assume_unchanged);
        return vec![finish_row(row, entry, submodules)];
    }

    let mut rows = Vec::with_capacity(2);
    if let Some(code) = index_code(status) {
        rows.push(WorkingCopyEntry::changed(
            path,
            code,
            true,
            assume_unchanged,
        ));
    }
    if let Some(code) = workdir_code(status) {
        rows.push(WorkingCopyEntry::changed(
            path,
            code,
            false,
            assume_unchanged,
        ));
    }
    if !TrackedChangeBits::any().intersects(status) {
        // A clean row; the ordering convention puts it on the workdir axis.
        rows.push(WorkingCopyEntry::clean(path, assume_unchanged));
    }

    rows.into_iter()
        .map(|row| finish_row(row, entry, submodules))
        .collect()
}

/// Attach the rename's real name and the submodule state to a row.
///
/// libgit2 pins a renamed status entry's `path` to the **old** name (verified
/// against the delta: `index_to_workdir().new_file().path()` is the new one).
/// The row has to point at the file that exists, so the new name becomes the
/// path and the old name travels as `old_path`.
fn finish_row(
    mut row: WorkingCopyEntry,
    entry: &StatusEntry<'_>,
    submodules: &HashMap<String, SubmoduleEntryState>,
) -> WorkingCopyEntry {
    let delta = if row.staged {
        entry.head_to_index()
    } else {
        entry.index_to_workdir()
    };
    if let Some(delta) = delta {
        if matches!(delta.status(), Delta::Renamed | Delta::Copied) {
            let old = delta
                .old_file()
                .path()
                .map(|path| path.to_string_lossy().into_owned());
            let new = delta
                .new_file()
                .path()
                .map(|path| path.to_string_lossy().into_owned());
            if let (Some(old), Some(new)) = (old, new) {
                if old != new {
                    row.path = new;
                    row.old_path = Some(old);
                }
            }
        }
    }
    let submodule = submodules.get(&row.path).cloned();
    row.with_submodule(submodule)
}

fn index_code(status: Status) -> Option<FileStatus> {
    if status.contains(Status::INDEX_NEW) {
        Some(FileStatus::Added)
    } else if status.contains(Status::INDEX_RENAMED) {
        Some(FileStatus::Renamed)
    } else if status.contains(Status::INDEX_DELETED) {
        Some(FileStatus::Deleted)
    } else if status.contains(Status::INDEX_MODIFIED) || status.contains(Status::INDEX_TYPECHANGE) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}

fn workdir_code(status: Status) -> Option<FileStatus> {
    if status.contains(Status::WT_NEW) {
        Some(FileStatus::Untracked)
    } else if status.contains(Status::WT_RENAMED) {
        Some(FileStatus::Renamed)
    } else if status.contains(Status::WT_DELETED) {
        Some(FileStatus::Deleted)
    } else if status.contains(Status::WT_MODIFIED)
        || status.contains(Status::WT_TYPECHANGE)
        // An unreadable file still shows up as changed; the UI renders it the
        // same way and the error surfaces when the content is read.
        || status.contains(Status::WT_UNREADABLE)
    {
        Some(FileStatus::Modified)
    } else if status.contains(Status::IGNORED) {
        Some(FileStatus::Ignored)
    } else {
        None
    }
}

/// The paths whose index entries carry the assume-unchanged bit.
pub(crate) fn assume_unchanged_set(index: &git2::Index) -> HashSet<String> {
    index
        .iter()
        .filter(|entry| entry.flags & INDEX_ENTRY_VALID != 0)
        .filter_map(|entry| String::from_utf8(entry.path.clone()).ok())
        .collect()
}

/// Submodule state for every gitlink entry in the index, keyed by path.
fn submodule_states(
    repo: &git2::Repository,
    index: &git2::Index,
) -> HashMap<String, SubmoduleEntryState> {
    let mut map = HashMap::new();
    for entry in index.iter() {
        if mode_from_raw(entry.mode) != FileMode::Submodule {
            continue;
        }
        let Ok(path) = String::from_utf8(entry.path.clone()) else {
            continue;
        };
        if let Some(state) = submodule_state_at(repo, &path, entry.id) {
            map.insert(path, state);
        }
    }
    map
}

/// Compare the submodule's populated state against the recorded gitlink.
pub(crate) fn submodule_state_at(
    repo: &git2::Repository,
    path: &str,
    recorded_oid: git2::Oid,
) -> Option<SubmoduleEntryState> {
    let submodule = repo.find_submodule(path).ok()?;
    let recorded = Some(recorded_oid.to_string());

    match submodule.open() {
        Ok(sub) => {
            let head = sub.head().ok().and_then(|h| h.target());
            let state = match head {
                Some(head_oid) if head_oid == recorded_oid => {
                    // At the recorded commit; local dirt is reported separately.
                    let mut opts = git2::StatusOptions::new();
                    opts.include_untracked(false).include_ignored(false);
                    let dirty = sub
                        .statuses(Some(&mut opts))
                        .map(|s| !s.is_empty())
                        .unwrap_or(false);
                    if dirty {
                        SubmoduleState::Dirty
                    } else {
                        SubmoduleState::Current
                    }
                }
                _ => SubmoduleState::NewCommits,
            };
            Some(SubmoduleEntryState {
                state,
                recorded,
                head: head.map(|oid| oid.to_string()),
            })
        }
        Err(_) => Some(SubmoduleEntryState {
            state: SubmoduleState::Uninitialized,
            recorded,
            head: None,
        }),
    }
}

/// The unmerged sides of a path, by index stage.
fn conflict_sides(index: &git2::Index, path: &str) -> Option<EntryConflict> {
    if !has_conflict_stages(index, path) {
        return None;
    }
    let staged_entry =
        |stage: i32| lookup(index, path, stage).map(|entry: IndexEntry| entry.id.to_string());
    Some(EntryConflict {
        base: staged_entry(1),
        ours: staged_entry(2),
        theirs: staged_entry(3),
    })
}

/// Map a raw git file mode onto the domain vocabulary.
pub(crate) fn mode_from_raw(raw: u32) -> FileMode {
    match raw & 0o170000 {
        0o100000 => {
            if raw & 0o111 != 0 {
                FileMode::Executable
            } else {
                FileMode::Regular
            }
        }
        0o120000 => FileMode::Symlink,
        0o160000 => FileMode::Submodule,
        0o040000 => FileMode::Tree,
        _ => FileMode::Unreadable,
    }
}

fn fs_file_mode(meta: &std::fs::Metadata) -> FileMode {
    let ft = meta.file_type();
    if ft.is_symlink() {
        FileMode::Symlink
    } else if ft.is_dir() {
        FileMode::Tree
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 != 0 {
                return FileMode::Executable;
            }
        }
        FileMode::Regular
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::index::test_support::{commit_all, scratch_repo};

    #[test]
    fn status_separates_axes_and_reports_untracked() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("tracked.txt"), "one\n").unwrap();
        crate::engine::index::stage_paths(&repo, &["tracked.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        // staged modification + further unstaged modification, one untracked file
        std::fs::write(dir.join("tracked.txt"), "two\n").unwrap();
        crate::engine::index::stage_paths(&repo, &["tracked.txt".to_string()]).unwrap();
        std::fs::write(dir.join("tracked.txt"), "three\n").unwrap();
        std::fs::write(dir.join("fresh.txt"), "new\n").unwrap();

        let entries = load(&repo, &WorkingCopyStatusOptions::default()).unwrap();
        let by = |path: &str, staged: bool| {
            entries
                .iter()
                .find(|e| e.path == path && e.staged == staged)
                .cloned()
        };
        assert_eq!(
            by("tracked.txt", true).unwrap().code,
            Some(FileStatus::Modified)
        );
        assert_eq!(
            by("tracked.txt", false).unwrap().code,
            Some(FileStatus::Modified)
        );
        assert_eq!(
            by("fresh.txt", false).unwrap().code,
            Some(FileStatus::Untracked)
        );
    }

    #[test]
    fn status_detects_workdir_renames() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        let body = "line one\nline two\nline three\n";
        std::fs::write(dir.join("before.txt"), body).unwrap();
        crate::engine::index::stage_paths(&repo, &["before.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::rename(dir.join("before.txt"), dir.join("after.txt")).unwrap();

        let entries = load(&repo, &WorkingCopyStatusOptions::default()).unwrap();
        let row = entries
            .iter()
            .find(|e| e.path == "after.txt" && !e.staged)
            .expect("renamed row");
        assert_eq!(row.code, Some(FileStatus::Renamed));
        assert_eq!(row.old_path.as_deref(), Some("before.txt"));
    }

    #[test]
    fn status_honors_ignore_and_assume_unchanged_options() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        std::fs::write(dir.join("debug.log"), "noise\n").unwrap();
        std::fs::write(dir.join("tracked.txt"), "one\n").unwrap();
        crate::engine::index::stage_paths(&repo, &["tracked.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        let default_entries = load(&repo, &WorkingCopyStatusOptions::default()).unwrap();
        assert!(default_entries.iter().all(|e| e.path != "debug.log"));

        let with_ignored = load(
            &repo,
            &WorkingCopyStatusOptions {
                include_ignored: true,
                ..Default::default()
            },
        )
        .unwrap();
        let ignored = with_ignored
            .iter()
            .find(|e| e.path == "debug.log")
            .expect("ignored row");
        assert_eq!(ignored.code, Some(FileStatus::Ignored));

        // Assume-unchanged: modify the file, suppress it, then ask for it.
        std::fs::write(dir.join("tracked.txt"), "two\n").unwrap();
        crate::engine::index::set_assume_unchanged(&repo, &["tracked.txt".to_string()], true)
            .unwrap();
        let hidden = load(&repo, &WorkingCopyStatusOptions::default()).unwrap();
        assert!(hidden.iter().all(|e| e.path != "tracked.txt"));

        let shown = load(
            &repo,
            &WorkingCopyStatusOptions {
                include_assume_unchanged: true,
                ..Default::default()
            },
        )
        .unwrap();
        let row = shown
            .iter()
            .find(|e| e.path == "tracked.txt")
            .expect("assume-unchanged row");
        assert!(row.assume_unchanged);
        assert_eq!(row.code, None);
    }

    #[test]
    fn entry_detail_reports_mode_and_conflict_sides() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("script.sh"), "echo hi\n").unwrap();
        crate::engine::index::stage_paths(&repo, &["script.sh".to_string()]).unwrap();
        commit_all(&repo, "initial");

        let detail = entry_detail(&repo, "script.sh").unwrap();
        assert!(detail.exists);
        assert_eq!(detail.size, 8);
        assert_eq!(detail.mode, FileMode::Regular);
        assert!(detail.conflict.is_none());
        assert!(detail.oid.is_some());
    }
}
