//! Index — the mutation half of the working-copy surface (docs/03 §3,
//! evidence F2/F5/F16): staging and unstaging paths and hunks, untracking,
//! the assume-unchanged flag, discarding, and the ignore rules.
//!
//! Every function takes a `&git2::Repository`, does the smallest index / work
//! tree write that expresses the operation, and returns domain values. Nothing
//! here emits events or knows about Tauri — the command layer sequences that.
//!
//! Two invariants drive the chunk operations:
//!
//! * a hunk reference that no longer matches is **refused**, never guessed at;
//! * the surrounding hunks of the file are never touched (libgit2's per-hunk
//!   apply callback selects exactly the hunk the user clicked).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use git2::{ApplyLocation, Patch, Status};

use crate::domain::commit::{CommitRequest, CommitResult, CommitSign, SHORT_OID_LEN};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::history::Signature as DomainSignature;
use crate::domain::working_copy::{
    ChunkDirection, DiffPerspective, DiscardOptions, HunkRef, IgnoreScope, WhitespaceMode,
    DEFAULT_CONTEXT_LINES,
};
use crate::engine::status::{self, INDEX_ENTRY_VALID};

use super::diff::{self, EngineDiffOptions};

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// `wc_stage_paths` (docs/03 §3): stage the named paths through
/// `Index::add_all`, which also records deletions (`Index::add_path` cannot).
///
/// An explicitly named untracked path that a `.gitignore` rule matches is
/// refused, the way `git add` refuses it — the UI offers the ignore flow
/// instead (`GTIgnoreTrackedFilesDialogueViewController`).
pub fn stage_paths(repo: &git2::Repository, paths: &[String]) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let workdir = repo.workdir().expect("checked").to_path_buf();
    {
        let index = repo.index()?;
        for path in paths {
            let tracked = status::lookup(&index, path, 0).is_some()
                || (1..=3).any(|stage| status::lookup(&index, path, stage).is_some());
            if tracked {
                continue;
            }
            if repo.status_should_ignore(Path::new(path))? {
                return Err(ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} is ignored by a gitignore rule; add it to the index only through the ignore flow"),
                )
                .with_param("path", path.clone()));
            }
            if !workdir.join(path).exists() {
                return Err(ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("No such file: {path}"),
                )
                .with_param("path", path.clone()));
            }
        }
    }

    let mut index = repo.index()?;
    index.add_all(
        paths.iter().map(String::as_str),
        git2::IndexAddOption::DEFAULT,
        None,
    )?;
    index.write()?;
    Ok(())
}

/// `wc_stage_all` (docs/03 §3): `git add .` — every tracked change, plus the
/// untracked files the ignore rules leave visible.
pub fn stage_all(repo: &git2::Repository) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let mut index = repo.index()?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    Ok(())
}

/// `wc_unstage_paths` (docs/03 §3): reset the named paths against HEAD. On an
/// unborn HEAD there is no tree to reset to, so unstaging means "remove from
/// the index" — the same end state the CLI produces.
pub fn unstage_paths(repo: &git2::Repository, paths: &[String]) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    // A `.` row means "everything"; `reset_default` cannot express that (see
    // `unstage_all`), so it takes the whole-index path.
    if paths.iter().any(|path| is_repo_root(path)) {
        return unstage_all(repo);
    }
    match repo.head() {
        Ok(head) => {
            let target = head.peel(git2::ObjectType::Commit)?;
            repo.reset_default(Some(&target), paths.iter().map(String::as_str))?;
        }
        Err(err) if is_unborn(&err) => {
            let mut index = repo.index()?;
            let mut changed = false;
            for path in paths {
                if index.remove_path(Path::new(path)).is_ok() {
                    changed = true;
                }
            }
            if changed {
                index.write()?;
            }
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

/// `wc_unstage_all` (docs/03 §3): everything back to HEAD; on an unborn HEAD
/// the index is emptied and the files stay untracked on disk.
pub fn unstage_all(repo: &git2::Repository) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    match repo.head() {
        Ok(head) => {
            let target = head.peel(git2::ObjectType::Commit)?;
            // The whole-index reset cannot go through `reset_default(HEAD,
            // ["."])`: the pathspec matching `git_reset_default` uses does not
            // understand ".", so that call silently does nothing. A mixed
            // reset — the CLI's `git reset` — is exactly "index back to HEAD,
            // working tree untouched", and the ref does not move because the
            // target *is* HEAD.
            repo.reset(&target, git2::ResetType::Mixed, None)?;
        }
        Err(err) if is_unborn(&err) => {
            let mut index = repo.index()?;
            index.clear()?;
            index.write()?;
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

/// `wc_entry_untrack` (docs/03 §3): drop the index entry, keep the file. The
/// file becomes untracked, exactly like `git rm --cached`.
pub fn untrack_paths(repo: &git2::Repository, paths: &[String]) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let mut index = repo.index()?;
    let mut changed = false;
    for path in paths {
        for stage in 0..=3 {
            if index.remove(Path::new(path), stage).is_ok() {
                changed = true;
            }
        }
    }
    if changed {
        index.write()?;
    }
    Ok(())
}

/// `wc_entry_assume_unchanged_set` (docs/03 §3).
///
/// The evidence reaches for a child `git update-index --[no-]assume-unchanged`
/// because libgit2 exposes no setter. It does not need one: the flag *is*
/// `GIT_INDEX_ENTRY_VALID` in the entry's flags word, libgit2 copies the flags
/// word verbatim into the entry it writes (`index_entry_cpy_nocache` keeps
/// `flags`; the on-disk writer does `htons(entry->flags)`), and every reader —
/// status, diff, checkout — treats a `VALID` entry as unmodified. So the flag
/// round-trips through `Index::add` + `Index::write` with no child process.
pub fn set_assume_unchanged(
    repo: &git2::Repository,
    paths: &[String],
    value: bool,
) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let mut index = repo.index()?;
    let mut touched = false;
    for path in paths {
        let Some(mut entry) = status::lookup(&index, path, 0) else {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("Not tracked, so it cannot be assume-unchanged: {path}"),
            )
            .with_param("path", path.clone()));
        };
        if value {
            entry.flags |= INDEX_ENTRY_VALID;
        } else {
            entry.flags &= !INDEX_ENTRY_VALID;
        }
        index.add(&entry)?;
        touched = true;
    }
    if touched {
        index.write()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Discard
// ---------------------------------------------------------------------------

/// `wc_paths_discard` (docs/03 §3): throw away local changes.
///
/// Staged paths go back to HEAD (index *and* working tree); unstaged paths go
/// back to the index; untracked paths are deleted — the last only with
/// `include_untracked` and an explicit confirmation token, because it is the
/// one case where `discard` can destroy data that exists nowhere else.
pub fn discard_paths(
    repo: &git2::Repository,
    paths: &[String],
    opts: &DiscardOptions,
) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let workdir = repo.workdir().expect("checked").to_path_buf();

    if opts.include_untracked
        && opts
            .confirm_token
            .as_deref()
            .map(|token| token.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Discarding untracked files requires the caller's confirmation token",
        ));
    }

    let statuses = status_map(repo)?;
    let index = repo.index()?;

    // A `.` row means "all of it": expand it to the paths the status read
    // found, so the checkout pathspecs below stay plain file paths.
    let targets: Vec<String> = if paths.iter().any(|path| is_repo_root(path)) {
        let mut all: Vec<String> = statuses.keys().cloned().collect();
        all.sort();
        all
    } else {
        paths.to_vec()
    };

    let mut to_head: Vec<String> = Vec::new();
    let mut to_index: Vec<String> = Vec::new();
    let mut to_delete: Vec<String> = Vec::new();

    for path in &targets {
        let status = statuses
            .get(path.as_str())
            .copied()
            .or_else(|| untracked_dir_status(&statuses, path))
            .unwrap_or(Status::CURRENT);

        if status.contains(Status::CONFLICTED) {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("{path} is unmerged; resolve the conflict instead of discarding it"),
            )
            .with_param("path", path.clone()));
        }

        let tracked = status::lookup(&index, path, 0).is_some();

        if status.contains(Status::WT_NEW) && !tracked {
            if opts.include_untracked {
                to_delete.push(path.clone());
            } else {
                // Returning Ok here makes the caller report "discarded" while
                // the only copy of the file is still on disk. The UI asks for
                // confirmation before it sets include_untracked; if it forgets
                // the option, fail loudly so it cannot show a false success.
                return Err(ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} is untracked; explicit confirmation is required to discard it"),
                )
                .with_param("path", path.clone()));
            }
            continue;
        }
        if status.intersects(index_bits()) {
            to_head.push(path.clone());
        } else if status.intersects(workdir_bits()) {
            to_index.push(path.clone());
        }
    }

    for path in &to_delete {
        remove_from_disk(&workdir.join(path))?;
    }

    if !to_head.is_empty() {
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        for path in &to_head {
            checkout.path(path.as_str());
        }
        repo.checkout_head(Some(&mut checkout))?;
    }

    if !to_index.is_empty() {
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        for path in &to_index {
            checkout.path(path.as_str());
        }
        repo.checkout_index(None, Some(&mut checkout))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Chunks
// ---------------------------------------------------------------------------

/// `wc_chunk_apply` (docs/03 §3): stage or unstage exactly one hunk.
///
/// Staging builds the index→workdir diff and applies the selected hunk *into
/// the index*; unstaging builds index→HEAD (the mirror image), where the
/// caller's coordinates are compared with the sides swapped.
pub fn apply_chunk(
    repo: &git2::Repository,
    hunk: &HunkRef,
    direction: ChunkDirection,
    context_lines: u32,
) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    // The chunk operations regenerate the diff with the caller's context so
    // the hunk they clicked is the hunk we match; whitespace is never ignored
    // here, or the coordinates would not line up with what was rendered.
    let opts = EngineDiffOptions::new(context_lines, WhitespaceMode::Show);
    let path = hunk.path.as_str();

    match direction {
        ChunkDirection::Stage => {
            if !status::is_tracked(repo, path)? {
                return Err(ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} is untracked; stage the whole file instead of a hunk"),
                )
                .with_param("path", path));
            }
            let diff = diff::index_to_workdir_diff(repo, Some(path), &opts)?;
            diff::apply_selected_hunk(repo, &diff, hunk, false, ApplyLocation::Index)
        }
        ChunkDirection::Unstage => {
            let diff = diff::index_to_head_diff(repo, Some(path), &opts)?;
            diff::apply_selected_hunk(repo, &diff, hunk, true, ApplyLocation::Index)?;
            prune_empty_added_entry(repo, path)
        }
    }
}

/// `wc_chunk_discard` (docs/03 §3, `discardChunk:`): throw one hunk away.
///
/// Which sides are involved follows from the perspective the hunk was taken
/// from: an unstaged hunk reverts the working tree to the index, a staged hunk
/// reverts both the index and (best effort) the working tree to HEAD, and a
/// hunk taken from the combined "all changes" view reverts the working tree to
/// HEAD and, when the path is also staged, the index too.
pub fn discard_chunk(
    repo: &git2::Repository,
    hunk: &HunkRef,
    context_lines: u32,
) -> Result<(), ErrorPayload> {
    ensure_working_copy(repo)?;
    let opts = EngineDiffOptions::new(context_lines, WhitespaceMode::Show);
    let path = hunk.path.as_str();

    match hunk.perspective {
        DiffPerspective::WorkingTreeToIndex => {
            let index_bytes = index_blob_bytes(repo, path)?.ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} is untracked; there is nothing to discard back to"),
                )
                .with_param("path", path)
            })?;
            let workdir_bytes = workdir_bytes(repo, path)?.ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} no longer exists on disk"),
                )
                .with_param("path", path)
            })?;
            // Old side = what is on disk, new side = the index: applying this
            // to the working tree puts the index content back for that hunk.
            let diff = buffer_patch(&workdir_bytes, &index_bytes, path, &opts)?;
            diff::apply_selected_hunk(repo, &diff, hunk, true, ApplyLocation::WorkDir)
        }
        DiffPerspective::IndexToBase => {
            let diff = diff::index_to_head_diff(repo, Some(path), &opts)?;
            // The working tree may have diverged from the index; reverting it
            // is best effort, while the index revert is the operation the user
            // asked for and must succeed.
            let _ = diff::apply_selected_hunk(repo, &diff, hunk, true, ApplyLocation::WorkDir);
            diff::apply_selected_hunk(repo, &diff, hunk, true, ApplyLocation::Index)
        }
        DiffPerspective::WorkingTreeToBase => {
            let head_bytes = head_blob_bytes(repo, path)?.unwrap_or_default();
            let workdir_bytes = workdir_bytes(repo, path)?.ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("{path} no longer exists on disk"),
                )
                .with_param("path", path)
            })?;
            let diff = buffer_patch(&workdir_bytes, &head_bytes, path, &opts)?;
            diff::apply_selected_hunk(repo, &diff, hunk, true, ApplyLocation::WorkDir)?;

            if status::is_tracked(repo, path)? && index_differs_from_head(repo, path)? {
                if let Ok(index_diff) = diff::index_to_head_diff(repo, Some(path), &opts) {
                    let _ = diff::apply_selected_hunk(
                        repo,
                        &index_diff,
                        hunk,
                        true,
                        ApplyLocation::Index,
                    );
                }
            }
            Ok(())
        }
        DiffPerspective::MineToMergeBase
        | DiffPerspective::MineToTheirs
        | DiffPerspective::TheirsToMergeBase => Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "A conflict chunk is resolved through the conflict flow, not discarded",
        )
        .with_param("path", hunk.path.clone())),
    }
}

/// Unstaging the last hunk of a newly added file should leave no entry at all
/// (`git reset -p` behaviour); libgit2's apply leaves an empty blob behind.
fn prune_empty_added_entry(repo: &git2::Repository, path: &str) -> Result<(), ErrorPayload> {
    if head_blob_bytes(repo, path)?.is_some() {
        return Ok(());
    }
    let empty = repo.blob(b"")?;
    let mut index = repo.index()?;
    let Some(entry) = status::lookup(&index, path, 0) else {
        return Ok(());
    };
    if entry.id == empty {
        index.remove_path(Path::new(path))?;
        index.write()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ignore rules
// ---------------------------------------------------------------------------

/// `wc_entry_ignore` (docs/03 §3): add an ignore pattern for `path` and answer
/// with the pattern added (the command shows it).
///
/// `scope = Repo` writes the repository's `.gitignore`; `scope = Global`
/// writes the user's excludes file — `core.excludesFile` when configured, the
/// XDG default `~/.config/git/ignore` otherwise, which is what git itself
/// falls back to. An existing identical line is left alone.
pub fn add_ignore_pattern(
    repo: &git2::Repository,
    path: &str,
    scope: IgnoreScope,
    pattern: Option<&str>,
) -> Result<String, ErrorPayload> {
    ensure_working_copy(repo)?;
    let pattern = match pattern.map(str::trim) {
        Some(given) if !given.is_empty() => given.to_string(),
        _ => default_pattern(repo, path),
    };

    let file = match scope {
        IgnoreScope::Repo => repo.workdir().expect("checked").join(".gitignore"),
        IgnoreScope::Global => global_excludes_file(repo)?,
    };

    append_line_if_absent(&file, &pattern)?;
    Ok(pattern)
}

/// The pattern the UI proposes for a path: a directory is ignored as a whole,
/// a file by its own name.
fn default_pattern(repo: &git2::Repository, path: &str) -> String {
    let full = repo.workdir().expect("checked").join(path);
    if full.is_dir() {
        format!("{path}/")
    } else {
        path.to_string()
    }
}

/// Where the global ignore rules live, following git's own resolution order.
pub fn global_excludes_file(repo: &git2::Repository) -> Result<PathBuf, ErrorPayload> {
    if let Ok(config) = repo.config() {
        if let Ok(configured) = config.get_string("core.excludesFile") {
            if !configured.trim().is_empty() {
                return Ok(expand_home(configured.trim()));
            }
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return Ok(PathBuf::from(xdg).join("git").join("ignore"));
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    Ok(PathBuf::from(home)
        .join(".config")
        .join("git")
        .join("ignore"))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn append_line_if_absent(file: &Path, line: &str) -> Result<(), ErrorPayload> {
    let existing = std::fs::read_to_string(file).unwrap_or_default();
    if existing.lines().any(|candidate| candidate.trim() == line) {
        return Ok(());
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("Could not create {parent:?}"))
                .with_detail(err.to_string())
        })?;
    }
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    std::fs::write(file, out).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write {file:?}"))
            .with_detail(err.to_string())
            .with_param("file", file.to_string_lossy().into_owned())
    })
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

/// `commit_create` / `commit_amend` (docs/03 §4): build the tree from the index
/// and write the commit, then move HEAD onto it.
///
/// Parents come from the repository, never from the caller:
///
/// * a merge in progress (`MERGE_HEAD` present) contributes its tips as the
///   extra parents, with HEAD first — the two-parent merge commit that
///   `git merge --continue` produces;
/// * an amend keeps the parents of the commit it replaces;
/// * an attached or detached HEAD contributes its commit as the first parent;
/// * an unborn HEAD (the initial commit) has no parent at all.
///
/// The ref is moved by hand (`Repository::reference`) instead of through
/// `Repository::commit`'s `update_ref`, because libgit2 stores the *whole*
/// commit message in the reflog; git — and the reflog view that reads it —
/// expects `commit (initial): <subject>`, `commit: <subject>` or
/// `commit (amend): <subject>`.
///
/// **Hooks are not run in this pass.** docs/03 §4 says `commit_create` should
/// run `pre-commit` / `commit-msg` / `post-commit`; that runner (executable
/// discovery under `core.hooksPath`, git-compatible environment, the
/// `commit-msg` file) is a separate piece of work and is deliberately out of
/// scope here rather than half-wired. `CommitOptions::skip_hooks` is accepted
/// and has no effect until it lands.
pub fn commit(
    repo: &git2::Repository,
    request: &CommitRequest,
) -> Result<CommitResult, ErrorPayload> {
    // Signing is not implemented: refuse a request that asks for it rather than
    // produce an unsigned commit the caller did not want.
    if matches!(
        request.opts.sign.as_ref(),
        Some(CommitSign::Gpg { .. }) | Some(CommitSign::Ssh { .. })
    ) {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Commit signing is not supported in this build",
        ));
    }

    // A conflicted tree is never committed — refuse before anything is written.
    if repo.index()?.has_conflicts() {
        return Err(ErrorPayload::new(
            ErrorCode::CheckoutConflicts,
            "The index still has unresolved conflicts; resolve them before committing",
        ));
    }

    let head = repo.head().ok();
    let head_commit = head
        .as_ref()
        .and_then(|reference| reference.peel_to_commit().ok());
    let was_merge = repo.state() == git2::RepositoryState::Merge;

    // An amend replaces HEAD; there is nothing to replace when HEAD is unborn.
    // Refuse rather than quietly create an initial commit under the amend flag.
    if request.amend && head_commit.is_none() {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "There is no commit to amend: HEAD does not point at a commit",
        ));
    }

    // --- message ----------------------------------------------------------
    let provided = request
        .message
        .as_deref()
        .filter(|message| !message.trim().is_empty());
    let mut message = match (provided, request.amend, head_commit.as_ref()) {
        (Some(message), _, _) => message.to_string(),
        (None, true, Some(head)) => String::from_utf8_lossy(head.message_bytes()).into_owned(),
        (None, true, None) => {
            return Err(ErrorPayload::new(
                ErrorCode::OperationInProgress,
                "There is no commit to amend: HEAD points at no commit",
            ))
        }
        (None, false, _) => {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "A commit message is required",
            ))
        }
    };

    // --- identities -------------------------------------------------------
    let committer = resolve_signature(repo, request.opts.committer.as_ref())?;
    let author = match request.opts.author.as_ref() {
        Some(author) => signature_from(author)?,
        // An amend keeps the author of the commit it replaces, like git.
        None if request.amend => match head_commit.as_ref() {
            Some(head) => copy_signature(&head.author())?,
            None => resolve_signature(repo, None)?,
        },
        None => resolve_signature(repo, None)?,
    };

    if request.opts.sign_off {
        message = with_sign_off(message, &committer);
    }

    // --- parents ----------------------------------------------------------
    let mut parent_oids: Vec<git2::Oid> = Vec::new();
    if request.amend {
        if let Some(head) = head_commit.as_ref() {
            parent_oids.extend(head.parent_ids());
        }
    } else {
        if let Some(head) = head_commit.as_ref() {
            parent_oids.push(head.id());
        }
        if was_merge {
            // `MERGE_HEAD` holds one oid per merged-in parent (one for a normal
            // merge, several for an octopus).
            parent_oids.extend(read_pseudoref_oids(repo, "MERGE_HEAD"));
        }
    }
    parent_oids.dedup();

    // --- tree + commit object --------------------------------------------
    let tree_oid = {
        let mut index = repo.index()?;
        index.write_tree()?
    };
    let tree = repo.find_tree(tree_oid)?;
    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid))
        .collect::<Result<_, _>>()?;
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();

    let oid = repo.commit(None, &author, &committer, &message, &tree, &parent_refs)?;

    // --- move HEAD with a git-shaped reflog -------------------------------
    // git labels the root commit `(initial)` and a replacement `(amend)`; the
    // reflog view reproduces those labels, so the wording must match.
    let summary = message_summary(&message);
    let reflog = if request.amend {
        format!("commit (amend): {summary}")
    } else if parent_oids.is_empty() {
        format!("commit (initial): {summary}")
    } else {
        format!("commit: {summary}")
    };
    let branch = update_head(repo, oid, &reflog)?;

    // Completing a merge means leaving merge state — drop MERGE_HEAD/MERGE_MSG
    // the way `git commit` does.
    if was_merge && !request.amend {
        repo.cleanup_state()?;
    }

    let stored = repo.find_commit(oid)?;
    let parents: Vec<String> = stored.parent_ids().map(|id| id.to_string()).collect();
    let detached = branch.is_none();
    Ok(CommitResult {
        oid: oid.to_string(),
        short_oid: oid.to_string().chars().take(SHORT_OID_LEN).collect(),
        parent_count: parents.len() as u32,
        parents,
        summary,
        message: String::from_utf8_lossy(stored.message_bytes()).into_owned(),
        author: signature_to_domain(stored.author()),
        committer: signature_to_domain(stored.committer()),
        branch,
        detached,
        amend: request.amend,
    })
}

/// The identity a commit is written with: an explicit override, else the
/// repository's configured `user.name` / `user.email`. A missing pair is a
/// clear refusal — never a panic, never a fallback identity.
pub fn resolve_signature(
    repo: &git2::Repository,
    override_signature: Option<&DomainSignature>,
) -> Result<git2::Signature<'static>, ErrorPayload> {
    if let Some(signature) = override_signature {
        return signature_from(signature);
    }
    let config = repo.config()?;
    let name = non_blank(config.get_string("user.name").ok());
    let email = non_blank(config.get_string("user.email").ok());
    match (name, email) {
        (Some(name), Some(email)) => {
            git2::Signature::now(&name, &email).map_err(ErrorPayload::from)
        }
        _ => Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Git identity is not configured: set user.name and user.email",
        )),
    }
}

/// Which identity key (`user.name` / `user.email`) is unset or blank, if any.
/// The commit validator asks this so an unresolvable signature is refused
/// before the engine ever runs.
pub fn missing_identity(repo: &git2::Repository) -> Result<Option<&'static str>, ErrorPayload> {
    let config = repo.config()?;
    for key in ["user.name", "user.email"] {
        if non_blank(config.get_string(key).ok()).is_none() {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

/// Do the index and HEAD differ — is there anything staged to commit? Pure: it
/// diffs, it never writes.
pub fn has_staged_changes(repo: &git2::Repository) -> Result<bool, ErrorPayload> {
    let index = repo.index()?;
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(err) if is_unborn(&err) => None,
        Err(err) => return Err(err.into()),
    };
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), None)?;
    Ok(diff.deltas().len() > 0)
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn signature_from(signature: &DomainSignature) -> Result<git2::Signature<'static>, ErrorPayload> {
    git2::Signature::new(
        &signature.name,
        &signature.email,
        &git2::Time::new(signature.time, 0),
    )
    .map_err(ErrorPayload::from)
}

fn copy_signature(
    signature: &git2::Signature<'_>,
) -> Result<git2::Signature<'static>, ErrorPayload> {
    git2::Signature::new(
        signature.name().unwrap_or(""),
        signature.email().unwrap_or(""),
        &signature.when(),
    )
    .map_err(ErrorPayload::from)
}

fn signature_to_domain(signature: git2::Signature<'_>) -> DomainSignature {
    DomainSignature {
        name: signature.name().unwrap_or("").to_string(),
        email: signature.email().unwrap_or("").to_string(),
        time: signature.when().seconds(),
    }
}

/// The first non-empty line of a message — git's commit subject, and the text a
/// reflog entry carries.
fn message_summary(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Append a `Signed-off-by:` trailer unless the message already carries one.
fn with_sign_off(message: String, signer: &git2::Signature<'_>) -> String {
    let already = message.lines().any(|line| {
        line.trim_start()
            .to_ascii_lowercase()
            .starts_with("signed-off-by:")
    });
    if already {
        return message;
    }
    let name = signer.name().unwrap_or("");
    let email = signer.email().unwrap_or("");
    let mut out = message.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&format!("Signed-off-by: {name} <{email}>"));
    out.push('\n');
    out
}

/// Read every oid in a one-oid-per-line pseudoref (`MERGE_HEAD`, …). An absent
/// file (not in that state) or an unparsable line yields nothing.
fn read_pseudoref_oids(repo: &git2::Repository, name: &str) -> Vec<git2::Oid> {
    std::fs::read_to_string(repo.path().join(name))
        .map(|raw| {
            raw.lines()
                .filter_map(|line| git2::Oid::from_str(line.trim()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Move the checked-out ref (or HEAD itself) onto `oid`, writing the reflog
/// entry git would write, and report the branch it landed on (`None` for a
/// detached HEAD).
///
/// The reflog entry's identity is the repository's configured `user.*`: libgit2
/// writes it from the repository identity and its reference API exposes no way
/// to pass the signature actually used, so a per-commit `committer` override is
/// not reflected in the reflog (a known, low-severity limitation).
fn update_head(
    repo: &git2::Repository,
    oid: git2::Oid,
    reflog: &str,
) -> Result<Option<String>, ErrorPayload> {
    match repo.head() {
        Ok(head) if head.is_branch() => {
            let refname = head
                .name()
                .map(str::to_string)
                .map_err(ErrorPayload::from)?;
            let branch = head
                .shorthand()
                .map(str::to_string)
                .map_err(ErrorPayload::from)?;
            repo.reference(&refname, oid, true, reflog)?;
            Ok(Some(branch))
        }
        // A detached HEAD is a direct reference named HEAD; move it in place.
        Ok(_) => {
            repo.reference("HEAD", oid, true, reflog)?;
            Ok(None)
        }
        // An unborn HEAD is symbolic to a branch that does not exist yet:
        // create the branch, then keep HEAD attached to it.
        Err(err) if is_unborn(&err) => {
            let target = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|head| head.symbolic_target().ok().flatten().map(str::to_string))
                .ok_or_else(|| {
                    ErrorPayload::new(
                        ErrorCode::Unclassified,
                        "HEAD does not resolve to a branch to create",
                    )
                })?;
            repo.reference(&target, oid, true, reflog)?;
            repo.set_head(&target)?;
            Ok(target.strip_prefix("refs/heads/").map(str::to_string))
        }
        Err(err) => Err(err.into()),
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn ensure_working_copy(repo: &git2::Repository) -> Result<(), ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    Ok(())
}

fn is_unborn(err: &git2::Error) -> bool {
    matches!(
        err.code(),
        git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
    )
}

/// Does the path name the whole repository? The evidence's "stage/unstage/
/// discard all" rows pass `.`, which libgit2's pathspec matching treats as a
/// literal name that matches nothing.
fn is_repo_root(path: &str) -> bool {
    let trimmed = path.trim();
    trimmed == "." || trimmed == "./"
}

fn index_bits() -> Status {
    Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE
}

fn workdir_bits() -> Status {
    Status::WT_MODIFIED
        | Status::WT_DELETED
        | Status::WT_RENAMED
        | Status::WT_TYPECHANGE
        | Status::WT_UNREADABLE
}

/// Every path libgit2 reports a status for, keyed by path. Rename detection is
/// off: discard and stage work on the paths the user selected, and a rename is
/// two paths there.
fn status_map(repo: &git2::Repository) -> Result<HashMap<String, Status>, ErrorPayload> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    let mut map = HashMap::with_capacity(statuses.len());
    for entry in statuses.iter() {
        if let Ok(path) = entry.path() {
            map.insert(path.to_string(), entry.status());
        }
    }
    Ok(map)
}

/// An untracked *directory* row (the status list was read with
/// `include_dirs`) has no entry of its own; treat it as untracked when
/// everything under it is.
fn untracked_dir_status(statuses: &HashMap<String, Status>, path: &str) -> Option<Status> {
    let prefix = format!("{path}/");
    statuses
        .iter()
        .find(|(candidate, status)| {
            candidate.starts_with(&prefix) && status.contains(Status::WT_NEW)
        })
        .map(|_| Status::WT_NEW)
}

fn remove_from_disk(full: &Path) -> Result<(), ErrorPayload> {
    match std::fs::symlink_metadata(full) {
        Err(_) => Ok(()),
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(full).map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("Could not remove {full:?}"))
                .with_detail(err.to_string())
        }),
        Ok(_) => std::fs::remove_file(full).map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("Could not remove {full:?}"))
                .with_detail(err.to_string())
        }),
    }
}

fn workdir_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    let workdir = repo.workdir().ok_or_else(|| {
        ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
    })?;
    Ok(std::fs::read(workdir.join(path)).ok())
}

fn index_blob_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    let index = repo.index()?;
    Ok(status::lookup(&index, path, 0)
        .and_then(|entry| repo.find_blob(entry.id).ok())
        .map(|blob| blob.content().to_vec()))
}

fn head_blob_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    let Some(tree) = diff::head_tree_optional(repo)? else {
        return Ok(None);
    };
    Ok(tree
        .get_path(Path::new(path))
        .ok()
        .and_then(|entry| repo.find_blob(entry.id()).ok())
        .map(|blob| blob.content().to_vec()))
}

fn index_differs_from_head(repo: &git2::Repository, path: &str) -> Result<bool, ErrorPayload> {
    let index = repo.index()?;
    let Some(entry) = status::lookup(&index, path, 0) else {
        return Ok(false);
    };
    Ok(match head_blob_bytes(repo, path)? {
        Some(head) => repo
            .find_blob(entry.id)
            .map(|blob| blob.content() != head.as_slice())
            .unwrap_or(true),
        None => true,
    })
}

/// A one-file diff built from two byte buffers (no object is written). The
/// result is parsed back into a `Diff` so `Repository::apply` — which takes a
/// `Diff`, not a `Patch` — can consume it.
fn buffer_patch<'a>(
    old_bytes: &'a [u8],
    new_bytes: &'a [u8],
    path: &'a str,
    opts: &EngineDiffOptions,
) -> Result<git2::Diff<'static>, ErrorPayload> {
    let mut patch = {
        let mut git = opts.git2(Some(path));
        Patch::from_buffers(
            old_bytes,
            Some(Path::new(path)),
            new_bytes,
            Some(Path::new(path)),
            Some(&mut git),
        )?
    };
    let buf = patch.to_buf()?;
    Ok(git2::Diff::from_buffer(&buf)?)
}

/// The chunk operations' default context when the caller names none: the same
/// default the diff view renders with.
pub const fn default_chunk_context() -> u32 {
    DEFAULT_CONTEXT_LINES
}

// ---------------------------------------------------------------------------
// Test support — shared with `engine::status` and `engine::diff`
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A freshly initialised repository under the system temp directory, with
    /// no dependency on the machine's git configuration (the identity used for
    /// commits is passed explicitly by [`commit_all`]).
    pub(crate) struct ScratchRepo {
        pub repo: git2::Repository,
        pub dir: PathBuf,
    }

    impl std::ops::Deref for ScratchRepo {
        type Target = git2::Repository;

        fn deref(&self) -> &git2::Repository {
            &self.repo
        }
    }

    impl Drop for ScratchRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    pub(crate) fn scratch_repo() -> ScratchRepo {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("meridian-test-{}-{}", std::process::id(), serial));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = git2::Repository::init(&dir).unwrap();
        ScratchRepo { repo, dir }
    }

    /// Stage everything currently on disk and commit it. Used to build the
    /// "clean starting point" every test then bends.
    pub(crate) fn commit_all(repo: &git2::Repository, message: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        drop(index);
        let tree = repo.find_tree(tree_oid).unwrap();
        let signature = git2::Signature::now("Meridian Test", "test@meridian.invalid").unwrap();
        let parents: Vec<git2::Commit<'_>> = match repo.head() {
            Ok(head) => vec![head.peel_to_commit().unwrap()],
            Err(_) => Vec::new(),
        };
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{commit_all, scratch_repo};
    use super::*;
    use crate::domain::working_copy::WorkingCopyStatusOptions;
    use crate::engine::status;

    fn entries(repo: &git2::Repository) -> Vec<crate::domain::working_copy::WorkingCopyEntry> {
        status::load(repo, &WorkingCopyStatusOptions::default()).unwrap()
    }

    fn code_of(
        repo: &git2::Repository,
        path: &str,
        staged: bool,
    ) -> Option<crate::domain::status::FileStatus> {
        entries(repo)
            .into_iter()
            .find(|entry| entry.path == path && entry.staged == staged)
            .and_then(|entry| entry.code)
    }

    fn index_text(repo: &git2::Repository, path: &str) -> String {
        index_blob_bytes(repo, path)
            .unwrap()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    }

    #[test]
    fn stage_and_unstage_paths_move_between_axes() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        assert_eq!(
            code_of(&repo, "a.txt", false),
            Some(crate::domain::status::FileStatus::Modified)
        );
        assert_eq!(code_of(&repo, "a.txt", true), None);

        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        assert_eq!(
            code_of(&repo, "a.txt", true),
            Some(crate::domain::status::FileStatus::Modified)
        );
        assert_eq!(code_of(&repo, "a.txt", false), None);

        unstage_paths(&repo, &["a.txt".to_string()]).unwrap();
        assert_eq!(code_of(&repo, "a.txt", true), None);
        assert_eq!(
            code_of(&repo, "a.txt", false),
            Some(crate::domain::status::FileStatus::Modified)
        );
        assert_eq!(index_text(&repo, "a.txt"), "one\n");
    }

    #[test]
    fn stage_all_records_deletions_and_additions() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("gone.txt"), "bye\n").unwrap();
        std::fs::write(dir.join("kept.txt"), "hi\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        std::fs::remove_file(dir.join("gone.txt")).unwrap();
        std::fs::write(dir.join("fresh.txt"), "new\n").unwrap();
        stage_all(&repo).unwrap();

        assert_eq!(
            code_of(&repo, "gone.txt", true),
            Some(crate::domain::status::FileStatus::Deleted)
        );
        assert_eq!(
            code_of(&repo, "fresh.txt", true),
            Some(crate::domain::status::FileStatus::Added)
        );

        unstage_all(&repo).unwrap();
        assert_eq!(
            code_of(&repo, "gone.txt", false),
            Some(crate::domain::status::FileStatus::Deleted)
        );
        assert_eq!(
            code_of(&repo, "fresh.txt", false),
            Some(crate::domain::status::FileStatus::Untracked)
        );
    }

    #[test]
    fn stage_paths_refuses_ignored_untracked_files() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        std::fs::write(dir.join("debug.log"), "noise\n").unwrap();

        let err = stage_paths(&repo, &["debug.log".to_string()]).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);

        // The ignore flow writes the rule instead of staging the file.
        let pattern = add_ignore_pattern(&repo, "other.log", IgnoreScope::Repo, None).unwrap();
        assert_eq!(pattern, "other.log");
        std::fs::write(dir.join("other.log"), "noise\n").unwrap();
        assert!(status::is_ignored(&repo, "other.log").unwrap());
        // Idempotent.
        add_ignore_pattern(&repo, "other.log", IgnoreScope::Repo, None).unwrap();
        let gitignore = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(gitignore.matches("other.log").count(), 1);
    }

    #[test]
    fn assume_unchanged_round_trips_through_the_index() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        set_assume_unchanged(&repo, &["a.txt".to_string()], true).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(Path::new("a.txt"), 0).unwrap();
        assert_eq!(entry.flags & INDEX_ENTRY_VALID, INDEX_ENTRY_VALID);

        set_assume_unchanged(&repo, &["a.txt".to_string()], false).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(Path::new("a.txt"), 0).unwrap();
        assert_eq!(entry.flags & INDEX_ENTRY_VALID, 0);

        // git itself must read the bit the same way; skipped where git is not
        // installed rather than failing the suite.
        set_assume_unchanged(&repo, &["a.txt".to_string()], true).unwrap();
        let output = std::process::Command::new("git")
            .arg("ls-files")
            .arg("-v")
            .current_dir(repo.workdir().unwrap())
            .output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            assert!(
                text.lines().any(|line| line.starts_with('h')),
                "git ls-files -v did not report the assume-unchanged bit: {text}"
            );
        }
    }

    #[test]
    fn untrack_keeps_the_file_and_drops_the_entry() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        untrack_paths(&repo, &["a.txt".to_string()]).unwrap();
        assert!(dir.join("a.txt").exists());
        assert_eq!(
            code_of(&repo, "a.txt", false),
            Some(crate::domain::status::FileStatus::Untracked)
        );
    }

    #[test]
    fn chunk_stage_and_unstage_touch_only_the_selected_hunk() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        let body: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.join("a.txt"), &body).unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        let mut edited: Vec<String> = (1..=30).map(|n| format!("line {n}\n")).collect();
        edited[1] = "first edit\n".to_string();
        edited[27] = "second edit\n".to_string();
        std::fs::write(dir.join("a.txt"), edited.concat()).unwrap();

        let refs = diff::hunks(
            &repo,
            "a.txt",
            DiffPerspective::WorkingTreeToIndex,
            &EngineDiffOptions::default(),
        )
        .unwrap();
        assert_eq!(refs.len(), 2);

        apply_chunk(
            &repo,
            &refs[0],
            ChunkDirection::Stage,
            DEFAULT_CONTEXT_LINES,
        )
        .unwrap();
        let staged = index_text(&repo, "a.txt");
        assert!(staged.contains("first edit"));
        assert!(!staged.contains("second edit"));
        // The rest of the file is untouched on the index side.
        assert!(staged.contains("line 3\n"));
        assert!(staged.contains("line 30\n"));

        // The second hunk is still unstaged, and the first is now staged.
        let staged_refs = diff::hunks(
            &repo,
            "a.txt",
            DiffPerspective::IndexToBase,
            &EngineDiffOptions::default(),
        )
        .unwrap();
        assert_eq!(staged_refs.len(), 1);

        apply_chunk(
            &repo,
            &staged_refs[0],
            ChunkDirection::Unstage,
            DEFAULT_CONTEXT_LINES,
        )
        .unwrap();
        assert_eq!(index_text(&repo, "a.txt"), body);
    }

    #[test]
    fn chunk_discard_reverts_only_that_hunk() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        let body: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.join("a.txt"), &body).unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        let mut edited: Vec<String> = (1..=30).map(|n| format!("line {n}\n")).collect();
        edited[1] = "first edit\n".to_string();
        edited[27] = "second edit\n".to_string();
        let edited_text = edited.concat();
        std::fs::write(dir.join("a.txt"), &edited_text).unwrap();

        let refs = diff::hunks(
            &repo,
            "a.txt",
            DiffPerspective::WorkingTreeToIndex,
            &EngineDiffOptions::default(),
        )
        .unwrap();
        discard_chunk(&repo, &refs[0], DEFAULT_CONTEXT_LINES).unwrap();

        let after = std::fs::read_to_string(dir.join("a.txt")).unwrap();
        assert!(after.contains("line 2\n"));
        assert!(after.contains("second edit"));
        assert!(!after.contains("first edit"));
    }

    #[test]
    fn stale_chunk_reference_is_refused() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");
        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();

        let stale = HunkRef {
            path: "a.txt".to_string(),
            perspective: DiffPerspective::WorkingTreeToIndex,
            index: 0,
            old_start: 40,
            old_lines: 12,
            new_start: 40,
            new_lines: 12,
            header: "@@ -40,12 +40,12 @@".to_string(),
        };
        let err =
            apply_chunk(&repo, &stale, ChunkDirection::Stage, DEFAULT_CONTEXT_LINES).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        // Nothing was staged.
        assert_eq!(code_of(&repo, "a.txt", true), None);
    }

    #[test]
    fn discard_paths_restores_head_and_needs_confirmation_for_untracked() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        std::fs::write(dir.join("a.txt"), "three\n").unwrap();
        std::fs::write(dir.join("scratch.txt"), "temp\n").unwrap();

        discard_paths(&repo, &["a.txt".to_string()], &DiscardOptions::default()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
        assert_eq!(code_of(&repo, "a.txt", false), None);

        // The default options must fail rather than report a no-op as success.
        // This is the exact payload the UI used to send after its confirmation.
        let err = discard_paths(
            &repo,
            &["scratch.txt".to_string()],
            &DiscardOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.message.contains("untracked"));
        assert!(dir.join("scratch.txt").exists());

        // The explicit include flag still needs a non-empty confirmation token.
        let err = discard_paths(
            &repo,
            &["scratch.txt".to_string()],
            &DiscardOptions {
                include_untracked: true,
                confirm_token: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(dir.join("scratch.txt").exists());

        // …and deletes once given.
        discard_paths(
            &repo,
            &["scratch.txt".to_string()],
            &DiscardOptions {
                include_untracked: true,
                confirm_token: Some("confirmed".to_string()),
            },
        )
        .unwrap();
        assert!(!dir.join("scratch.txt").exists());
    }

    #[test]
    fn discard_of_an_unstaged_path_returns_it_to_the_index() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        // Unstaged only: the index still holds HEAD, so discarding restores
        // the file from the index rather than from HEAD.
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        assert_eq!(code_of(&repo, "a.txt", true), None);

        discard_paths(&repo, &["a.txt".to_string()], &DiscardOptions::default()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
        assert_eq!(code_of(&repo, "a.txt", false), None);
    }

    #[test]
    fn discard_of_a_position_with_both_axes_goes_all_the_way_to_head() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_all(&repo).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "staged\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        std::fs::write(dir.join("a.txt"), "staged\nand unstaged\n").unwrap();

        discard_paths(&repo, &["a.txt".to_string()], &DiscardOptions::default()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
        assert_eq!(code_of(&repo, "a.txt", true), None);
        assert_eq!(code_of(&repo, "a.txt", false), None);
    }

    #[test]
    fn global_excludes_file_follows_the_configuration() {
        let repo = scratch_repo();
        let path = global_excludes_file(&repo).unwrap();
        // Either core.excludesFile from the machine's config, or the XDG
        // default; both are absolute paths ending in a file name.
        assert!(path.is_absolute());
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!name.is_empty());
    }
}

#[cfg(test)]
mod commit_tests {
    use super::test_support::{commit_all, scratch_repo};
    use super::*;
    use crate::domain::commit::{CommitOptions, CommitRequest};

    /// Give the repository a deterministic committer identity in its *local*
    /// config, so the tests never depend on the machine's global git config.
    fn seed_identity(repo: &git2::Repository) {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Committer").unwrap();
        config
            .set_str("user.email", "committer@meridian.invalid")
            .unwrap();
    }

    fn request(message: &str, amend: bool) -> CommitRequest {
        CommitRequest {
            message: Some(message.to_string()),
            amend,
            opts: CommitOptions::default(),
        }
    }

    fn branch_of(repo: &git2::Repository) -> String {
        repo.head().unwrap().shorthand().unwrap().to_string()
    }

    fn stage(repo: &git2::Repository) {
        stage_all(repo).unwrap();
    }

    fn checkout(repo: &git2::Repository, refname: &str) {
        repo.set_head(refname).unwrap();
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        repo.checkout_head(Some(&mut builder)).unwrap();
    }

    /// A branch `feature` that rewrote `path`, and a `main` that rewrote the
    /// same path — the two sides of a conflicting merge.
    fn conflicting_merge(repo: &git2::Repository) {
        std::fs::write(repo.workdir().unwrap().join("clash.txt"), "base\n").unwrap();
        stage(repo);
        let base = commit_all(repo, "base");
        let main = branch_of(repo);
        repo.branch("feature", &repo.find_commit(base).unwrap(), false)
            .unwrap();

        std::fs::write(repo.workdir().unwrap().join("clash.txt"), "ours\n").unwrap();
        stage(repo);
        commit(repo, &request("ours", false)).unwrap();

        checkout(repo, "refs/heads/feature");
        std::fs::write(repo.workdir().unwrap().join("clash.txt"), "theirs\n").unwrap();
        stage(repo);
        commit(repo, &request("theirs", false)).unwrap();

        checkout(repo, &format!("refs/heads/{main}"));
        let their = repo.refname_to_id("refs/heads/feature").unwrap();
        let annotated = repo.find_annotated_commit(their).unwrap();
        repo.merge(&[&annotated], None, None).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Merge);
    }

    #[test]
    fn creates_a_commit_on_a_branch_with_one_parent() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let base = commit_all(repo, "base");

        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        stage(repo);
        let result = commit(repo, &request("add b", false)).unwrap();

        assert_eq!(result.parents, vec![base.to_string()]);
        assert_eq!(result.parent_count, 1);
        assert_eq!(result.branch.as_deref(), Some(branch_of(repo).as_str()));
        assert!(!result.detached);
        assert!(!result.amend);
        assert_eq!(result.summary, "add b");
        assert_eq!(result.committer.email, "committer@meridian.invalid");
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            result.oid
        );

        // The branch reflog reads the way git writes it.
        let reflog = repo
            .reflog(&format!("refs/heads/{}", branch_of(repo)))
            .unwrap();
        assert_eq!(
            reflog.get(0).unwrap().message().unwrap(),
            Some("commit: add b")
        );
    }

    #[test]
    fn the_root_commit_is_labelled_initial_in_the_reflog() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let root = commit(repo, &request("base", false)).unwrap();
        assert_eq!(root.parent_count, 0);

        // git labels the first commit `(initial)`; the reflog view reproduces it.
        let reflog = repo
            .reflog(&format!("refs/heads/{}", branch_of(repo)))
            .unwrap();
        assert_eq!(
            reflog.get(0).unwrap().message().unwrap(),
            Some("commit (initial): base")
        );
    }

    #[test]
    fn amending_an_unborn_head_is_refused() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        // HEAD is unborn: there is no commit to replace, so amend must refuse
        // rather than quietly create an initial commit under the amend flag.
        let error = commit(repo, &request("amend nothing", true)).unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationInProgress);
        assert!(repo.head().is_err(), "no commit may be created");
    }

    #[test]
    fn amend_replaces_head_and_keeps_its_parents_and_author() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let first = commit_all(repo, "first");
        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        stage(repo);
        // Written with the fixed author "Meridian Test" by the helper.
        let second = commit_all(repo, "second");

        std::fs::write(dir.join("b.txt"), "amended\n").unwrap();
        stage(repo);
        let result = commit(repo, &request("second amended", true)).unwrap();

        assert!(result.amend);
        assert_ne!(result.oid, second.to_string());
        assert_eq!(result.parents, vec![first.to_string()]);
        assert_eq!(result.parent_count, 1);
        // The original author survives; the committer is the configured one.
        assert_eq!(result.author.name, "Meridian Test");
        assert_eq!(result.committer.email, "committer@meridian.invalid");
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            result.oid
        );

        let reflog = repo
            .reflog(&format!("refs/heads/{}", branch_of(repo)))
            .unwrap();
        assert_eq!(
            reflog.get(0).unwrap().message().unwrap(),
            Some("commit (amend): second amended")
        );
    }

    #[test]
    fn amend_can_reuse_the_original_message() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let base = commit_all(repo, "base");
        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        stage(repo);
        let second = commit_all(repo, "original subject");

        // No staged change and no new message: the message of the commit being
        // replaced is reused verbatim and the parent is unchanged.
        let result = commit(
            repo,
            &CommitRequest {
                message: None,
                amend: true,
                opts: CommitOptions::default(),
            },
        )
        .unwrap();

        assert_ne!(result.oid, second.to_string());
        assert_eq!(result.message.trim(), "original subject");
        assert_eq!(result.summary, "original subject");
        assert_eq!(result.parents, vec![base.to_string()]);
    }

    #[test]
    fn initial_commit_has_no_parents_and_creates_the_branch() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        assert!(
            repo.head().is_err(),
            "HEAD must be unborn before the first commit"
        );

        let result = commit(repo, &request("initial", false)).unwrap();
        assert_eq!(result.parent_count, 0);
        assert!(result.parents.is_empty());
        assert!(result.branch.is_some());
        assert!(!result.detached);
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            result.oid
        );

        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        stage(repo);
        let second = commit(repo, &request("second", false)).unwrap();
        assert_eq!(second.parents, vec![result.oid]);
        assert_eq!(second.parent_count, 1);
    }

    #[test]
    fn commit_on_a_detached_head_advances_head_without_a_branch() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let base = commit_all(repo, "base");
        repo.set_head_detached(base).unwrap();

        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        stage(repo);
        let result = commit(repo, &request("detached", false)).unwrap();

        assert!(result.detached);
        assert!(result.branch.is_none());
        assert_eq!(result.parents, vec![base.to_string()]);

        let head = repo.head().unwrap();
        assert!(!head.is_branch());
        assert_eq!(head.target().unwrap().to_string(), result.oid);
    }

    #[test]
    fn commit_during_a_merge_records_both_parents() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("base.txt"), "base\n").unwrap();
        stage(repo);
        let base = commit_all(repo, "base");
        let main = branch_of(repo);
        repo.branch("feature", &repo.find_commit(base).unwrap(), false)
            .unwrap();

        std::fs::write(dir.join("main.txt"), "main\n").unwrap();
        stage(repo);
        let main_tip = commit(repo, &request("main work", false)).unwrap();

        checkout(repo, "refs/heads/feature");
        std::fs::write(dir.join("feature.txt"), "feature\n").unwrap();
        stage(repo);
        let feature_tip = commit(repo, &request("feature work", false)).unwrap();

        checkout(repo, &format!("refs/heads/{main}"));
        let their = repo.refname_to_id("refs/heads/feature").unwrap();
        let annotated = repo.find_annotated_commit(their).unwrap();
        repo.merge(&[&annotated], None, None).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Merge);

        let result = commit(repo, &request("merge feature", false)).unwrap();
        assert_eq!(result.parent_count, 2);
        assert_eq!(result.parents, vec![main_tip.oid, feature_tip.oid]);
        // Completing the merge leaves merge state behind.
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
    }

    #[test]
    fn refuses_to_commit_a_conflicted_index() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        conflicting_merge(repo);

        let err = commit(repo, &request("merge", false)).unwrap_err();
        assert_eq!(err.code, ErrorCode::CheckoutConflicts);
        // Nothing moved: the merge is still in progress.
        assert_eq!(repo.state(), git2::RepositoryState::Merge);
    }

    #[test]
    fn refuses_when_user_email_is_unset() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;

        // Blank the local value so it shadows any global identity.
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.email", "").unwrap();
        }
        assert_eq!(missing_identity(repo).unwrap(), Some("user.email"));

        std::fs::write(repo.workdir().unwrap().join("a.txt"), "one\n").unwrap();
        stage(repo);
        let err = commit(repo, &request("initial", false)).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn refuses_when_signing_is_requested() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        std::fs::write(repo.workdir().unwrap().join("a.txt"), "one\n").unwrap();
        stage(repo);

        let err = commit(
            repo,
            &CommitRequest {
                message: Some("signed".to_string()),
                amend: false,
                opts: CommitOptions {
                    sign: Some(CommitSign::Gpg { key: None }),
                    ..CommitOptions::default()
                },
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        // Nothing was committed.
        assert!(repo.head().is_err());
    }

    #[test]
    fn sign_off_appends_the_committer_trailer_once() {
        let scratch = scratch_repo();
        seed_identity(&scratch);
        let repo = &scratch.repo;
        let dir = repo.workdir().unwrap().to_path_buf();

        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage(repo);
        let result = commit(
            repo,
            &CommitRequest {
                message: Some("signed".to_string()),
                amend: false,
                opts: CommitOptions {
                    sign_off: true,
                    ..CommitOptions::default()
                },
            },
        )
        .unwrap();

        assert_eq!(
            result.message.lines().last(),
            Some("Signed-off-by: Committer <committer@meridian.invalid>")
        );
        assert_eq!(result.summary, "signed");
    }
}
