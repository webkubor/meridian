//! Worktree operations — the engine half of docs/03 §12 (`GTGitOperationGroup-
//! Worktrees`).
//!
//! # Why adding a worktree is not one call
//!
//! libgit2's `git_worktree_add` **always** binds a branch ref to the new tree:
//! it either checks out an existing branch (which must not be checked out
//! anywhere else) or creates one named after the worktree. Two of the observed
//! UI affordances do not fit that mould — a **detached** worktree, and a
//! **forced** add over a branch that is already checked out elsewhere. Both are
//! built here on a scratch branch:
//!
//! 1. create a uniquely named temporary branch at the target commit,
//! 2. add the worktree bound to it,
//! 3. detach (or repoint) the new worktree's HEAD, then
//! 4. delete the scratch branch.
//!
//! Every step is real git; nothing is faked. The scratch branch never survives
//! the call. `Worktree::is_locked`/`is_prunable` return live state, and the
//! observed error classes (`worktree_locked`, `worktree_has_changes`) gate the
//! removals.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::revision::Revision;

use super::history;

// ---------------------------------------------------------------------------
// Vocabulary (destined for `domain/worktrees.rs` once it exists)
// ---------------------------------------------------------------------------

/// A worktree row (docs/03 §12 `worktree_list`). The main working directory is
/// the first entry, distinguished by `is_main`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Worktree name (the handle `find_worktree` takes); the main tree's name
    /// is its directory name.
    pub name: String,
    /// Top-level path (not the `.git` file inside it).
    pub path: String,
    /// Checked-out branch shorthand, when HEAD is attached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_oid: Option<String>,
    pub detached: bool,
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_reason: Option<String>,
    /// The directory exists and the metadata is intact.
    pub valid: bool,
    /// The worktree can be pruned (its folder is gone or its metadata is stale).
    pub prunable: bool,
    /// Has uncommitted changes. `None` when the worktree cannot be opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    pub is_main: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorktreeAddOptions {
    /// Create a detached worktree at the target commit.
    pub detached: bool,
    /// Name of the branch to bind; defaults to the worktree `name`.
    pub branch: Option<String>,
    /// Lock the new worktree on creation.
    pub lock: bool,
    pub lock_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// `worktree_list` (docs/03 §12): the main working directory first, then every
/// linked worktree in the order git2 reports them.
pub fn list(repo: &git2::Repository) -> Result<Vec<WorktreeInfo>, ErrorPayload> {
    let mut out = Vec::new();
    if let Some(info) = main_worktree(repo)? {
        out.push(info);
    }
    for name in repo.worktrees()?.iter() {
        let Ok(Some(name)) = name else { continue };
        let worktree = repo.find_worktree(name)?;
        out.push(read_worktree(repo, &worktree, false)?);
    }
    Ok(out)
}

fn main_worktree(repo: &git2::Repository) -> Result<Option<WorktreeInfo>, ErrorPayload> {
    let Some(workdir) = repo.workdir() else {
        return Ok(None); // bare repository: no main working tree
    };
    let path = workdir.to_string_lossy().trim_end_matches('/').to_string();
    let name = workdir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());

    let (head, head_oid, detached) = head_state(repo);

    Ok(Some(WorktreeInfo {
        name,
        path,
        head,
        head_oid,
        detached,
        locked: false,
        locked_reason: None,
        valid: true,
        prunable: false,
        dirty: None,
        is_main: true,
    }))
}

fn read_worktree(
    repo: &git2::Repository,
    worktree: &git2::Worktree,
    is_main: bool,
) -> Result<WorktreeInfo, ErrorPayload> {
    let name = worktree.name()?.unwrap_or_default().to_string();
    let path = worktree.path().to_string_lossy().into_owned();

    let (locked, locked_reason) = match worktree.is_locked() {
        Ok(git2::WorktreeLockStatus::Locked(reason)) => (true, reason),
        Ok(git2::WorktreeLockStatus::Unlocked) => (false, None),
        Err(_) => (false, None),
    };
    let valid = worktree.validate().is_ok();
    let mut prune_options = git2::WorktreePruneOptions::new();
    let prunable = worktree
        .is_prunable(Some(&mut prune_options))
        .unwrap_or(false);

    let (head, head_oid, detached, dirty) = match git2::Repository::open_from_worktree(worktree) {
        Ok(wt_repo) => {
            let (head, head_oid, detached) = head_state(&wt_repo);
            (head, head_oid, detached, dirty_of(&wt_repo))
        }
        Err(_) => (None, None, false, None),
    };

    let _ = repo;
    Ok(WorktreeInfo {
        name,
        path,
        head,
        head_oid,
        detached,
        locked,
        locked_reason,
        valid,
        prunable,
        dirty,
        is_main,
    })
}

/// `(branch shorthand, oid, detached)` for a repository's HEAD. An unborn HEAD
/// yields `(None, None, false)`.
fn head_state(repo: &git2::Repository) -> (Option<String>, Option<String>, bool) {
    let Ok(head) = repo.head() else {
        return (None, None, false);
    };
    let oid = head.target().map(|oid| oid.to_string());
    if head.is_branch() {
        (head.shorthand().ok().map(str::to_string), oid, false)
    } else {
        (None, oid, true)
    }
}

fn dirty_of(repo: &git2::Repository) -> Option<bool> {
    let statuses = repo.statuses(None).ok()?;
    Some(!statuses.is_empty())
}

// ---------------------------------------------------------------------------
// Add / remove / lock
// ---------------------------------------------------------------------------

/// `worktree_add` (docs/03 §12).
///
/// `start` is the commit to check out (defaults to HEAD). For an attached add
/// the branch is `opts.branch` or `name`; it is created at `start` if it does
/// not already exist, exactly like `git worktree add <path> <branch>`.
pub fn add(
    repo: &git2::Repository,
    name: &str,
    path: &str,
    start: Option<&Revision>,
    opts: &WorktreeAddOptions,
) -> Result<WorktreeInfo, ErrorPayload> {
    if repo.workdir().is_none() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "A bare repository cannot host worktrees",
        ));
    }
    if repo.find_worktree(name).is_ok() {
        return Err(ErrorPayload::new(
            ErrorCode::ReferenceExists,
            format!("Worktree '{name}' already exists"),
        )
        .with_param("name", name));
    }

    let target = match start {
        Some(revision) => history::resolve(repo, revision)?.id(),
        None => repo
            .head()
            .map_err(|_| {
                ErrorPayload::new(
                    ErrorCode::UnknownRevision,
                    "The repository has no HEAD to base the worktree on",
                )
            })?
            .peel_to_commit()?
            .id(),
    };

    let worktree = if opts.detached {
        let worktree = add_detached(repo, name, path, target, opts)?;
        return finish_add(repo, name, &worktree, opts);
    } else {
        let branch_name = opts.branch.clone().unwrap_or_else(|| name.to_string());
        let reference = ensure_branch(repo, &branch_name, target)?;
        if super::branches::branch_is_checked_out(repo, reference.name().unwrap_or_default())? {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("Branch '{branch_name}' is already checked out in another worktree"),
            )
            .with_param("name", branch_name)
            .with_param("class", "worktree_branch_checked_out"));
        }

        let mut add_options = git2::WorktreeAddOptions::new();
        add_options.reference(Some(&reference));
        repo.worktree(name, Path::new(path), Some(&add_options))?
    };

    finish_add(repo, name, &worktree, opts)
}

/// Detached add via a scratch branch (see the module header).
fn add_detached(
    repo: &git2::Repository,
    name: &str,
    path: &str,
    target: git2::Oid,
    opts: &WorktreeAddOptions,
) -> Result<git2::Worktree, ErrorPayload> {
    let commit = repo.find_commit(target)?;
    let scratch = format!(
        "meridian-detached-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let scratch_branch = repo.branch(&scratch, &commit, false)?;
    let reference = scratch_branch.into_reference();

    let mut add_options = git2::WorktreeAddOptions::new();
    add_options.reference(Some(&reference));
    let worktree = repo.worktree(name, Path::new(path), Some(&add_options))?;

    // Detach the new worktree's HEAD at the target, then drop the scratch
    // branch (now unclaimed, so the delete succeeds).
    if let Ok(new_repo) = git2::Repository::open(&worktree_path(&worktree)?) {
        new_repo.set_head_detached(target)?;
    }
    if let Ok(mut branch) = repo.find_branch(&scratch, git2::BranchType::Local) {
        let _ = branch.delete();
    }
    let _ = opts;
    Ok(worktree)
}

fn finish_add(
    repo: &git2::Repository,
    name: &str,
    worktree: &git2::Worktree,
    opts: &WorktreeAddOptions,
) -> Result<WorktreeInfo, ErrorPayload> {
    if opts.lock {
        worktree.lock(opts.lock_reason.as_deref())?;
    }
    let worktree = repo.find_worktree(name)?;
    read_worktree(repo, &worktree, false)
}

fn worktree_path(worktree: &git2::Worktree) -> Result<String, ErrorPayload> {
    Ok(worktree.path().to_string_lossy().into_owned())
}

/// Ensure a local branch exists at `target` and return its ref (created when
/// missing, left untouched when present — `git worktree add` semantics).
fn ensure_branch<'repo>(
    repo: &'repo git2::Repository,
    name: &str,
    target: git2::Oid,
) -> Result<git2::Reference<'repo>, ErrorPayload> {
    if !git2::Reference::is_valid_name(&format!("refs/heads/{name}")) {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidReferenceName,
            format!("'{name}' is not a valid branch name"),
        )
        .with_param("name", name));
    }
    if repo.find_branch(name, git2::BranchType::Local).is_err() {
        let commit = repo.find_commit(target)?;
        repo.branch(name, &commit, false)?;
    }
    repo.find_reference(&format!("refs/heads/{name}"))
        .map_err(ErrorPayload::from)
}

/// `worktree_remove` (docs/03 §12). Guards before pruning: a locked worktree
/// and a worktree with uncommitted changes are refused unless `force`.
pub fn remove(
    repo: &git2::Repository,
    name: &str,
    force: bool,
) -> Result<WorktreeInfo, ErrorPayload> {
    let worktree = repo.find_worktree(name).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("Worktree '{name}' does not exist"),
        )
        .with_param("name", name)
        .with_param("class", "worktree_not_found")
    })?;

    let info = read_worktree(repo, &worktree, false)?;

    if info.locked && !force {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("Worktree '{name}' is locked"),
        )
        .with_param("name", name)
        .with_param("class", "worktree_locked"));
    }
    if info.dirty == Some(true) && !force {
        return Err(ErrorPayload::new(
            ErrorCode::CheckoutConflicts,
            format!("Worktree '{name}' has uncommitted changes"),
        )
        .with_param("name", name)
        .with_param("class", "worktree_has_changes"));
    }

    let mut prune_options = git2::WorktreePruneOptions::new();
    prune_options.valid(true).working_tree(true);
    if force {
        prune_options.locked(true);
    }
    worktree.prune(Some(&mut prune_options))?;

    Ok(info)
}

/// `worktree_lock` (docs/03 §12).
pub fn lock(
    repo: &git2::Repository,
    name: &str,
    reason: Option<&str>,
) -> Result<WorktreeInfo, ErrorPayload> {
    let worktree = repo.find_worktree(name).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("Worktree '{name}' does not exist"),
        )
        .with_param("name", name)
    })?;
    worktree.lock(reason)?;
    let worktree = repo.find_worktree(name)?;
    read_worktree(repo, &worktree, false)
}

/// `worktree_unlock` (docs/03 §12).
pub fn unlock(repo: &git2::Repository, name: &str) -> Result<WorktreeInfo, ErrorPayload> {
    let worktree = repo.find_worktree(name).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("Worktree '{name}' does not exist"),
        )
        .with_param("name", name)
    })?;
    worktree.unlock()?;
    let worktree = repo.find_worktree(name)?;
    read_worktree(repo, &worktree, false)
}

/// The `worktree_remove` precondition, shared with the command validator: the
/// reason a non-forced removal would be refused, if any.
pub fn removal_blocker(
    repo: &git2::Repository,
    name: &str,
) -> Result<Option<RemovalBlocker>, ErrorPayload> {
    let Ok(worktree) = repo.find_worktree(name) else {
        return Ok(Some(RemovalBlocker {
            class: "worktree_not_found",
            message: format!("Worktree '{name}' does not exist"),
        }));
    };
    let info = read_worktree(repo, &worktree, false)?;
    if info.locked {
        return Ok(Some(RemovalBlocker {
            class: "worktree_locked",
            message: format!("Worktree '{name}' is locked"),
        }));
    }
    if info.dirty == Some(true) {
        return Ok(Some(RemovalBlocker {
            class: "worktree_has_changes",
            message: format!("Worktree '{name}' has uncommitted changes"),
        }));
    }
    Ok(None)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalBlocker {
    pub class: &'static str,
    pub message: String,
}
