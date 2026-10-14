//! Stash operations — the engine half of docs/03 §10 (`GTGitOperationGroupStash`).
//!
//! # The `&mut Repository` problem
//!
//! Every libgit2 stash entry point takes `&mut Repository`: the stash rewrites
//! the `refs/stash` reflog in place, and git2 models that as `&mut`. The
//! command layer's [`crate::commands::ExecCx`] only hands the engine a shared
//! `&Repository`, so this module opens a **second handle on the same gitdir**
//! ([`reopen`]) for the duration of each stash operation. That is safe — a
//! git2 handle owns no exclusive lock, the registry's `Mutex<Repository>`
//! already serializes commands per repository, and both handles address the
//! same files. The alternative (widening `ExecCx::repo` to `&mut`) lives in
//! `state.rs`, which this module does not own; it is recorded as a cross-module
//! note in this module's delivery.

use serde::{Deserialize, Serialize};

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::status::{EntryStatus, FileStatus};

// ---------------------------------------------------------------------------
// Vocabulary (destined for `domain/stash.rs` once it exists)
// ---------------------------------------------------------------------------

/// One stash entry as the stash list wants it (docs/03 §10 `stash_list`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashInfo {
    /// Position in the stash reflog; `0` is the most recent (git's numbering).
    pub index: usize,
    pub oid: String,
    /// The reflog subject: `On main: msg` / `WIP on main: …`.
    pub name: String,
    /// The branch the stash was created on, when the name carries it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The message without the `On <branch>: ` / `WIP on <branch>: ` prefix.
    pub message: String,
    /// Unix seconds of the stash commit.
    pub time: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StashSaveOptions {
    pub message: Option<String>,
    /// `git stash -u`.
    pub include_untracked: bool,
    /// `git stash -a`.
    pub include_ignored: bool,
    /// `git stash --keep-index`.
    pub keep_index: bool,
    /// Restrict the stash to these paths (`git stash push -- <pathspec>`).
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StashApplyOptions {
    /// Restore the staged/unstaged split (`--index`).
    pub reinstate_index: bool,
}

/// A stash plus the changes it holds (docs/03 §10 `stash_show`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashDetail {
    pub stash: StashInfo,
    pub entries: Vec<EntryStatus>,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// `stash_list` (docs/03 §10).
pub fn list(repo: &git2::Repository) -> Result<Vec<StashInfo>, ErrorPayload> {
    let mut handle = reopen(repo)?;
    let mut entries: Vec<(usize, String, git2::Oid)> = Vec::new();
    handle.stash_foreach(|index, name, oid| {
        entries.push((index, name.to_string(), *oid));
        true
    })?;

    let mut out = Vec::with_capacity(entries.len());
    for (index, name, oid) in entries {
        out.push(stash_info(repo, index, &name, oid)?);
    }
    Ok(out)
}

/// `stash_count`: the number of entries, without building the rows.
pub fn count(repo: &git2::Repository) -> Result<usize, ErrorPayload> {
    let mut handle = reopen(repo)?;
    let mut count = 0usize;
    handle.stash_foreach(|_, _, _| {
        count += 1;
        true
    })?;
    Ok(count)
}

/// `stash_save` (docs/03 §10).
///
/// A custom message *and* a path restriction cannot be combined: libgit2's
/// `StashSaveOptions` (behind `stash_save_ext`) has no message setter, so the
/// message would be silently dropped. Rather than ignore the argument, this
/// refuses with class `stash_message_with_paths`.
pub fn save(repo: &git2::Repository, opts: &StashSaveOptions) -> Result<StashInfo, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }

    let signature = repo.signature()?;
    let flags = stash_flags(opts);
    let mut handle = reopen(repo)?;

    let oid = if opts.paths.is_empty() {
        handle.stash_save2(&signature, opts.message.as_deref(), Some(flags))?
    } else {
        if opts.message.is_some() {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "A custom stash message cannot be combined with a path restriction",
            )
            .with_param("class", "stash_message_with_paths"));
        }
        let mut save_options = git2::StashSaveOptions::new(signature);
        save_options.flags(Some(flags));
        for path in &opts.paths {
            save_options.pathspec(path.as_str());
        }
        handle.stash_save_ext(Some(&mut save_options))?
    };

    // Entry 0 is the stash we just wrote; read its reflog subject back.
    let name = stash_name_at(repo, 0)?.ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::Unclassified,
            "Stash was written but did not appear in the stash list",
        )
    })?;
    stash_info(repo, 0, &name, oid)
}

/// `stash_apply` (docs/03 §10): apply without removing the entry.
pub fn apply(
    repo: &git2::Repository,
    index: usize,
    opts: &StashApplyOptions,
) -> Result<StashInfo, ErrorPayload> {
    let stash = entry_at(repo, index)?;
    let mut handle = reopen(repo)?;
    let mut apply_options = git2::StashApplyOptions::new();
    if opts.reinstate_index {
        apply_options.reinstantiate_index();
    }
    apply_options.checkout_options({
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.safe();
        checkout
    });
    handle.stash_apply(index, Some(&mut apply_options))?;
    Ok(stash)
}

/// `stash_pop` (docs/03 §10): apply and drop on success.
pub fn pop(
    repo: &git2::Repository,
    index: usize,
    opts: &StashApplyOptions,
) -> Result<StashInfo, ErrorPayload> {
    let stash = entry_at(repo, index)?;
    let mut handle = reopen(repo)?;
    let mut apply_options = git2::StashApplyOptions::new();
    if opts.reinstate_index {
        apply_options.reinstantiate_index();
    }
    apply_options.checkout_options({
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.safe();
        checkout
    });
    handle.stash_pop(index, Some(&mut apply_options))?;
    Ok(stash)
}

/// `stash_drop` (docs/03 §10).
pub fn drop(repo: &git2::Repository, index: usize) -> Result<StashInfo, ErrorPayload> {
    let stash = entry_at(repo, index)?;
    let mut handle = reopen(repo)?;
    handle.stash_drop(index)?;
    Ok(stash)
}

/// `stash_show` (docs/03 §10): the stash's changes, as working-copy-style
/// entries (the stash commit diffed against its first parent).
pub fn show(repo: &git2::Repository, index: usize) -> Result<StashDetail, ErrorPayload> {
    let stash = entry_at(repo, index)?;
    let oid = git2::Oid::from_str(&stash.oid)?;
    let commit = repo.find_commit(oid)?;
    let parent = commit.parent(0)?;
    let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;

    let entries = diff
        .deltas()
        .filter_map(|delta| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())?;
            Some(EntryStatus::new(
                path.to_string_lossy().into_owned(),
                delta_status(delta.status()),
                false,
                false,
            ))
        })
        .collect();

    Ok(StashDetail { stash, entries })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A second handle on the same gitdir, needed for the `&mut`-only stash API.
fn reopen(repo: &git2::Repository) -> Result<git2::Repository, ErrorPayload> {
    git2::Repository::open(repo.path()).map_err(ErrorPayload::from)
}

fn stash_flags(opts: &StashSaveOptions) -> git2::StashFlags {
    let mut flags = git2::StashFlags::empty();
    if opts.include_untracked {
        flags |= git2::StashFlags::INCLUDE_UNTRACKED;
    }
    if opts.include_ignored {
        flags |= git2::StashFlags::INCLUDE_IGNORED;
    }
    if opts.keep_index {
        flags |= git2::StashFlags::KEEP_INDEX;
    }
    flags
}

/// The stash reflog subject for `index`, if that entry exists.
fn stash_name_at(repo: &git2::Repository, wanted: usize) -> Result<Option<String>, ErrorPayload> {
    let mut handle = reopen(repo)?;
    let mut name = None;
    handle.stash_foreach(|index, subject, _oid| {
        if index == wanted {
            name = Some(subject.to_string());
            return false;
        }
        true
    })?;
    Ok(name)
}

/// The full stash row for `index`, or a not-found error.
fn entry_at(repo: &git2::Repository, wanted: usize) -> Result<StashInfo, ErrorPayload> {
    let mut handle = reopen(repo)?;
    let mut found: Option<(String, git2::Oid)> = None;
    handle.stash_foreach(|index, subject, oid| {
        if index == wanted {
            found = Some((subject.to_string(), *oid));
            return false;
        }
        true
    })?;

    let (name, oid) = found.ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("No stash entry at index {wanted}"),
        )
        .with_param("index", wanted.to_string())
        .with_param("class", "stash_not_found")
    })?;
    stash_info(repo, wanted, &name, oid)
}

fn stash_info(
    repo: &git2::Repository,
    index: usize,
    name: &str,
    oid: git2::Oid,
) -> Result<StashInfo, ErrorPayload> {
    let (branch, message) = split_stash_subject(name);
    let time = repo
        .find_commit(oid)
        .map(|commit| commit.time().seconds())
        .unwrap_or(0);

    Ok(StashInfo {
        index,
        oid: oid.to_string(),
        name: name.to_string(),
        branch,
        message,
        time,
    })
}

/// Split `On main: msg` / `WIP on main: msg` into `(Some("main"), "msg")`.
/// Anything that does not match is left whole as the message.
fn split_stash_subject(name: &str) -> (Option<String>, String) {
    let rest = name
        .strip_prefix("WIP on ")
        .or_else(|| name.strip_prefix("On "));
    let Some(rest) = rest else {
        return (None, name.to_string());
    };
    match rest.split_once(": ") {
        Some((branch, message)) => (Some(branch.to_string()), message.to_string()),
        None => (None, rest.to_string()),
    }
}

fn delta_status(delta: git2::Delta) -> FileStatus {
    match delta {
        git2::Delta::Added => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Modified => FileStatus::Modified,
        git2::Delta::Renamed => FileStatus::Renamed,
        git2::Delta::Copied => FileStatus::Copied,
        git2::Delta::Untracked => FileStatus::Untracked,
        git2::Delta::Ignored => FileStatus::Ignored,
        git2::Delta::Typechange => FileStatus::Modified,
        git2::Delta::Conflicted => FileStatus::Unmerged,
        _ => FileStatus::Modified,
    }
}
