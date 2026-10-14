//! Branch and tag operations — the engine half of docs/03 §6 and §9
//! (`GTGitOperationGroupBranches` / `GTGitOperationGroupTags`).
//!
//! Tag operations live in this module instead of a separate `engine/tags.rs`
//! because branches and tags share one ref vocabulary: the report types
//! (`DeleteReport`), the refname validators and the push refspec helpers are
//! identical, and duplicating them would let the two surfaces drift.
//!
//! The value structs here (`DeleteReport`, `TagInfo`, …) are plain serde data
//! with no git2 inside. They are declared next to their operations because this
//! module owns the vocabulary until the `domain` module grows the matching
//! `domain/refs.rs` rows; moving them later is a re-export, not a rewrite.

use serde::{Deserialize, Serialize};

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::history::Signature;
use crate::domain::refs::{BranchFilter, BranchInfo, CheckoutOptions, CheckoutResult};
use crate::domain::revision::Revision;

use super::history;

// ---------------------------------------------------------------------------
// Shared report types
// ---------------------------------------------------------------------------

/// Result of a bulk ref deletion (`branch_delete`, `tag_delete`, …). Branches
/// that vanished between the UI's snapshot and the command land in `skipped`
/// with their class instead of failing the whole batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub deleted: Vec<String>,
    pub skipped: Vec<DeleteSkip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSkip {
    pub name: String,
    pub reason: String,
    /// The observed engine error class behind the skip (docs/02 §14):
    /// `reference_not_found`, `delete_unmerged_branch`, `branch_is_head`.
    pub class: String,
}

// ---------------------------------------------------------------------------
// Branch listing / creation / checkout (P0 surface, unchanged)
// ---------------------------------------------------------------------------

pub fn list(
    repo: &git2::Repository,
    filter: BranchFilter,
) -> Result<Vec<BranchInfo>, ErrorPayload> {
    let mut branches = Vec::new();
    let types: &[git2::BranchType] = match filter {
        BranchFilter::Local => &[git2::BranchType::Local],
        BranchFilter::Remote => &[git2::BranchType::Remote],
        BranchFilter::All => &[git2::BranchType::Local, git2::BranchType::Remote],
    };

    for branch_type in types {
        for entry in repo.branches(Some(*branch_type))? {
            let (branch, kind) = entry?;
            branches.push(branch_info(
                repo,
                &branch,
                kind == git2::BranchType::Remote,
            )?);
        }
    }

    // Most recent first — the ordering every branch list in the UI uses.
    branches.sort_by(|a, b| {
        b.tip_time
            .cmp(&a.tip_time)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(branches)
}

/// `branch_load` (docs/03 §6): one branch, same shape as a `branch_list` row.
pub fn load(repo: &git2::Repository, name: &str) -> Result<BranchInfo, ErrorPayload> {
    let branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| branch_not_found(name))?;
    branch_info(repo, &branch, false)
}

/// Move the working copy and index to `commit`'s tree, refusing — before
/// anything is written — when that would overwrite a local change.
///
/// **This must run before HEAD moves.** libgit2 compares the working copy with
/// the tree HEAD points at when the checkout starts (the "baseline") and treats
/// every difference as the user's own edit. Moving HEAD first makes the
/// baseline equal the target, so nothing differs from it, no conflict is ever
/// found, and files are left as they were: that is how a checkout used to
/// report success while keeping the old branch's content, leave files the
/// target lacks behind, and clobber edits. With the right order libgit2 does
/// what `git checkout` does — carries unrelated edits (staged or not) across,
/// removes what the target lacks (directories included), and refuses with
/// `CheckoutConflicts` when an edit or an untracked file is in the way.
///
/// `name` is only used for the message. The conflicting paths are collected
/// through libgit2's notification hook, so the refusal can name them.
fn checkout_tree_safely(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    name: &str,
) -> Result<(), ErrorPayload> {
    let conflicts = std::cell::RefCell::new(Vec::<String>::new());
    let mut builder = git2::build::CheckoutBuilder::new();
    builder
        .safe()
        .notify_on(git2::CheckoutNotificationType::CONFLICT)
        .notify(|_why, path, _baseline, _target, _workdir| {
            if let Some(path) = path {
                conflicts.borrow_mut().push(path.display().to_string());
            }
            // Keep going: the refusal should list every blocker, not the first.
            true
        });

    match repo.checkout_tree(commit.as_object(), Some(&mut builder)) {
        Ok(()) => {
            log::debug!(target: "meridian::engine", "checkout of '{name}' updated the working copy");
            Ok(())
        }
        Err(error) if error.code() == git2::ErrorCode::Conflict => {
            let mut paths = conflicts.borrow().clone();
            paths.sort();
            paths.dedup();
            log::debug!(
                target: "meridian::engine",
                "checkout of '{name}' refused: {} conflicting path(s)",
                paths.len()
            );
            let message = if paths.is_empty() {
                // libgit2 refused without naming paths; its own count is the
                // best statement of what it found.
                format!("Local changes would be overwritten by checking out '{name}'")
            } else {
                format!(
                    "{} local change(s) would be overwritten by checking out '{name}'",
                    paths.len()
                )
            };
            let payload = ErrorPayload::new(ErrorCode::CheckoutConflicts, message)
                .with_domain(crate::domain::error::ErrorDomain::Git)
                .with_param("name", name);
            Err(if paths.is_empty() {
                payload.with_detail(error.message().to_string())
            } else {
                payload.with_detail(paths.join(", "))
            })
        }
        Err(error) => Err(error.into()),
    }
}

/// `branch_create` (docs/03 §6): create at the resolved start point, optionally
/// checking out with libgit2's *safe* strategy — local changes are never
/// clobbered; a conflicting checkout surfaces as `CheckoutConflicts` and the new
/// branch is not left behind, as with `git checkout -b`.
pub fn create(
    repo: &git2::Repository,
    name: &str,
    start: &Revision,
    checkout: bool,
) -> Result<BranchInfo, ErrorPayload> {
    let commit = history::resolve(repo, start)?;
    let mut branch = repo.branch(name, &commit, false)?;

    if checkout {
        let refname = branch.get().name()?.to_string();
        if let Err(error) = checkout_tree_safely(repo, &commit, name) {
            let _ = branch.delete();
            return Err(error);
        }
        repo.set_head(&refname)?;
    }

    branch_info(repo, &branch, false)
}

/// `branch_stacked_create` (docs/03 §6; `GTGitCreateStackedBranchActionValidator`).
///
/// The git-visible half is identical to `create` with the parent's tip as the
/// start point — what makes it *stacked* is the parent link on record. The
/// stack store (SQLite, docs/02 §9) is a later phase, so the relation is
/// returned in [`StackedBranchInfo`] for the caller to persist; it is never
/// silently dropped.
///
/// The parent-existence precondition lives here because failure is the
/// evidence's dedicated `missing_parent` symbol ("The selected parent branch
/// does not exist." / "Cannot create new branch with an orphaned parent
/// branch.", EV:strings).
pub fn stacked_create(
    repo: &git2::Repository,
    name: &str,
    parent: &str,
    checkout: bool,
) -> Result<StackedBranchInfo, ErrorPayload> {
    let parent_branch = repo
        .find_branch(parent, git2::BranchType::Local)
        .map_err(|_| missing_parent(parent))?;
    let parent_commit = parent_branch.get().peel_to_commit()?;
    let parent_oid = parent_commit.id();

    let mut branch = repo.branch(name, &parent_commit, false)?;

    if checkout {
        let refname = branch.get().name()?.to_string();
        if let Err(error) = checkout_tree_safely(repo, &parent_commit, name) {
            let _ = branch.delete();
            return Err(error);
        }
        repo.set_head(&refname)?;
    }

    Ok(StackedBranchInfo {
        branch: branch_info(repo, &branch, false)?,
        parent: parent.to_string(),
        parent_oid: parent_oid.to_string(),
    })
}

/// A stacked branch plus the parent link the stack store must record.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackedBranchInfo {
    pub branch: BranchInfo,
    pub parent: String,
    pub parent_oid: String,
}

/// A branch named by any of the forms the UI and quick-open use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBranch {
    /// The full refname, e.g. `refs/heads/feature` or `refs/remotes/origin/feature`.
    pub full_name: String,
    /// The shorthand, e.g. `feature` or `origin/feature`.
    pub name: String,
    pub is_remote: bool,
}

/// Resolve a branch the way every list row names it. Accepts
/// `feature`, `refs/heads/feature`, `origin/feature` and
/// `refs/remotes/origin/feature`, and **prefers the local branch when both a
/// local and a remote branch match** (the ambiguous `origin/feature` case).
/// The error names both refnames it tried.
pub fn resolve_branch(repo: &git2::Repository, name: &str) -> Result<ResolvedBranch, ErrorPayload> {
    let name = name.trim();

    // An explicit refname is taken at face value.
    if name.starts_with("refs/heads/") {
        if let Ok(reference) = repo.find_reference(name) {
            return Ok(ResolvedBranch {
                full_name: reference.name().unwrap_or(name).to_string(),
                name: name.trim_start_matches("refs/heads/").to_string(),
                is_remote: false,
            });
        }
    }
    if name.starts_with("refs/remotes/") {
        if let Ok(reference) = repo.find_reference(name) {
            return Ok(ResolvedBranch {
                full_name: reference.name().unwrap_or(name).to_string(),
                name: name.trim_start_matches("refs/remotes/").to_string(),
                is_remote: true,
            });
        }
    }

    // Shorthands: a local branch first (so a local `origin/feature` shadows the
    // remote `origin/feature`), then a remote-tracking branch.
    if let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) {
        return Ok(ResolvedBranch {
            full_name: branch.get().name().unwrap_or_default().to_string(),
            name: name.to_string(),
            is_remote: false,
        });
    }
    if let Ok(branch) = repo.find_branch(name, git2::BranchType::Remote) {
        return Ok(ResolvedBranch {
            full_name: branch.get().name().unwrap_or_default().to_string(),
            name: name.to_string(),
            is_remote: true,
        });
    }

    Err(ErrorPayload::new(
        ErrorCode::ReferenceNotFound,
        format!(
            "Branch '{name}' does not exist (tried 'refs/heads/{name}' and 'refs/remotes/{name}')"
        ),
    )
    .with_param("name", name))
}

/// `branch_checkout` (docs/03 §6): switch branches (or detach) using the safe
/// checkout strategy. Preconditions — including "no integration in progress" —
/// are enforced by the validator.
///
/// The name may be any form [`resolve_branch`] accepts, so the quick-open ids
/// (`branch:` + the row's full refname) work. A local branch attaches HEAD; a
/// remote-tracking ref cannot, so it detaches at that tip.
///
/// The working copy moves first and HEAD second (see [`checkout_tree_safely`]),
/// so a refusal is a no-op the user can retry after staging, committing or
/// discarding.
pub fn checkout(
    repo: &git2::Repository,
    name: &str,
    opts: &CheckoutOptions,
) -> Result<CheckoutResult, ErrorPayload> {
    let resolved = resolve_branch(repo, name)?;
    let reference = repo.find_reference(&resolved.full_name)?;
    let oid = reference.target().ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::UnknownRevision,
            format!("Branch '{}' has no target", resolved.name),
        )
        .with_param("name", resolved.name.clone())
    })?;

    let commit = repo.find_commit(oid)?;
    checkout_tree_safely(repo, &commit, &resolved.name)?;

    if opts.detach || resolved.is_remote {
        // A remote-tracking ref cannot anchor HEAD — detach at its tip.
        repo.set_head_detached(oid)?;
        return Ok(CheckoutResult {
            branch: None,
            oid: oid.to_string(),
            detached: true,
        });
    }

    repo.set_head(&resolved.full_name)?;
    Ok(CheckoutResult {
        branch: Some(resolved.name),
        oid: oid.to_string(),
        detached: false,
    })
}

// ---------------------------------------------------------------------------
// Branch rename / delete / tracking
// ---------------------------------------------------------------------------

/// `branch_checkout_revision` (docs/03 §6; `GTCheckoutRevisionViewController`):
/// check out a revision. A branch revision attaches HEAD; anything else (tag,
/// oid, HEAD) detaches — matching git's own "detached HEAD" warning surface.
pub fn checkout_revision(
    repo: &git2::Repository,
    revision: &Revision,
    detach: bool,
) -> Result<CheckoutResult, ErrorPayload> {
    let commit = history::resolve(repo, revision)?;
    let oid = commit.id();
    let label = oid.to_string();

    if detach {
        checkout_tree_safely(repo, &commit, &label)?;
        repo.set_head_detached(oid)?;
        return Ok(CheckoutResult {
            branch: None,
            oid: oid.to_string(),
            detached: true,
        });
    }

    match revision {
        Revision::Branch(name) => {
            checkout_tree_safely(repo, &commit, name)?;
            repo.set_head(&format!("refs/heads/{name}"))?;
            Ok(CheckoutResult {
                branch: Some(name.clone()),
                oid: oid.to_string(),
                detached: false,
            })
        }
        _ => Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Only a branch can be checked out with an attached HEAD — pass detach for a tag or commit",
        )
        .with_param("class", "revision_not_a_branch")),
    }
}

/// `branch_rename` (docs/03 §6; `GTGitRenameBranchOperation`).
pub fn rename(
    repo: &git2::Repository,
    old: &str,
    new: &str,
    force: bool,
) -> Result<BranchInfo, ErrorPayload> {
    let mut branch = repo
        .find_branch(old, git2::BranchType::Local)
        .map_err(|_| branch_not_found(old))?;
    let renamed = branch.rename(new, force)?;
    branch_info(repo, &renamed, false)
}

/// `branch_delete` (docs/03 §6; `GTGitDeleteBranchesOperation`).
///
/// Without `force` a branch is deletable when its tip is reachable from HEAD
/// (git's "fully merged" rule) or when its upstream is gone — the same three
/// arms the docs table names: "merged or upstream gone or `force`".
/// Unmerged branches are the validator's hard error
/// (`GTGitDeleteUnmergedBranchError`); the engine skips them with the class so
/// a direct caller still gets the full report.
pub fn delete(
    repo: &git2::Repository,
    names: &[String],
    force: bool,
) -> Result<DeleteReport, ErrorPayload> {
    let head_branch = head_branch_name(repo);

    let mut report = DeleteReport {
        deleted: Vec::new(),
        skipped: Vec::new(),
    };

    for name in names {
        let Ok(mut branch) = repo.find_branch(name, git2::BranchType::Local) else {
            report.skipped.push(DeleteSkip {
                name: name.clone(),
                reason: format!("Branch '{name}' does not exist"),
                class: "reference_not_found".to_string(),
            });
            continue;
        };

        if head_branch.as_deref() == Some(name.as_str()) {
            report.skipped.push(DeleteSkip {
                name: name.clone(),
                reason: "The branch is checked out — switch away first".to_string(),
                class: "branch_is_head".to_string(),
            });
            continue;
        }

        if !force && !is_merged(repo, &branch)? && !upstream_gone(repo, name)? {
            report.skipped.push(DeleteSkip {
                name: name.clone(),
                reason: format!("Branch '{name}' is not fully merged"),
                class: "delete_unmerged_branch".to_string(),
            });
            continue;
        }

        branch.delete()?;
        report.deleted.push(name.clone());
    }

    Ok(report)
}

/// The `branch_delete` precondition, shared with the command validator: the
/// subset of `names` that is unmerged and not protected by `force`.
pub fn unmerged(repo: &git2::Repository, names: &[String]) -> Result<Vec<String>, ErrorPayload> {
    let mut out = Vec::new();
    for name in names {
        let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) else {
            continue; // not-found is reported by `delete`/the validator separately
        };
        if !is_merged(repo, &branch)? && !upstream_gone(repo, name)? {
            out.push(name.clone());
        }
    }
    Ok(out)
}

/// git's "fully merged" rule: the branch tip is reachable from HEAD.
pub fn is_merged(repo: &git2::Repository, branch: &git2::Branch<'_>) -> Result<bool, ErrorPayload> {
    let tip = branch.get().peel_to_commit()?.id();
    let Ok(head) = repo.head().and_then(|head| head.peel_to_commit()) else {
        return Ok(false); // unborn HEAD: nothing is merged yet
    };
    if head.id() == tip {
        return Ok(true);
    }
    Ok(repo.graph_descendant_of(head.id(), tip)?)
}

/// A branch whose `branch.<name>.{remote,merge}` config exists but whose
/// upstream ref no longer does — "upstream gone", the second delete arm.
pub fn upstream_gone(repo: &git2::Repository, name: &str) -> Result<bool, ErrorPayload> {
    let config = repo.config()?;
    let configured = config.get_string(&format!("branch.{name}.remote")).is_ok()
        && config.get_string(&format!("branch.{name}.merge")).is_ok();
    if !configured {
        return Ok(false);
    }
    let has_ref = repo
        .find_branch(name, git2::BranchType::Local)
        .ok()
        .and_then(|branch| branch.upstream().ok())
        .is_some();
    Ok(!has_ref)
}

/// `branch_fast_forward` (docs/03 §6; `GTGitFastForwardBranchAction`).
///
/// `to` defaults to the branch's upstream tip — the "fast-forward my branch to
/// its upstream" action the branch list's sync affordance performs. The
/// descendant precondition is enforced by the validator; the engine re-checks
/// because a direct caller must not be able to rewind a ref.
pub fn fast_forward(
    repo: &git2::Repository,
    name: &str,
    to: Option<&Revision>,
) -> Result<BranchInfo, ErrorPayload> {
    let branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| branch_not_found(name))?;
    let tip = branch.get().peel_to_commit()?.id();

    let target = match to {
        Some(revision) => history::resolve(repo, revision)?.id(),
        None => branch
            .upstream()
            .and_then(|up| up.get().peel_to_commit())
            .map_err(|_| {
                ErrorPayload::new(
                    ErrorCode::ReferenceNotFound,
                    format!("Branch '{name}' has no upstream to fast-forward to"),
                )
                .with_param("name", name)
                .with_param("class", "no_upstream")
            })?
            .id(),
    };

    if target != tip && !repo.graph_descendant_of(target, tip)? {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("'{name}' cannot fast-forward: the target is not a descendant"),
        )
        .with_param("name", name)
        .with_param("class", "not_fast_forwardable"));
    }

    let is_head = branch.is_head();
    // The checked-out branch moves its working copy first: once the ref moves,
    // HEAD already points at the target and nothing is left to compare against
    // (see `checkout_tree_safely`). A refusal leaves the branch where it was.
    if is_head {
        let commit = repo.find_commit(target)?;
        checkout_tree_safely(repo, &commit, name)?;
    }
    let mut reference = branch.into_reference();
    reference.set_target(target, &format!("meridian: fast-forward {name}"))?;

    load(repo, name)
}

/// `branch_set_upstream` (docs/03 §6; `GTGitSetUpstreamBranchAction`).
///
/// `upstream` is `local`, `remote/branch`, or a full refname; it must resolve
/// locally (git's own rule for `--set-upstream-to`: fetch first, then track).
pub fn set_upstream(
    repo: &git2::Repository,
    name: &str,
    upstream: &str,
) -> Result<BranchInfo, ErrorPayload> {
    let shorthand = resolve_upstream(repo, upstream)?;
    let mut branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| branch_not_found(name))?;
    branch.set_upstream(Some(&shorthand))?;
    branch_info(repo, &branch, false)
}

/// `branch_unset_upstream` (docs/03 §6; `GTGitUnsetUpstreamBranchAction`).
pub fn unset_upstream(repo: &git2::Repository, name: &str) -> Result<BranchInfo, ErrorPayload> {
    let mut branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| branch_not_found(name))?;
    branch.set_upstream(None)?;
    branch_info(repo, &branch, false)
}

/// `branch_track_remote` (docs/03 §6; `GTTrackRemoteBranchViewController`) —
/// link a local branch to a remote branch, renaming it first when asked.
pub fn track_remote(
    repo: &git2::Repository,
    local: &str,
    remote_branch: &str,
    rename_to: Option<&str>,
) -> Result<BranchInfo, ErrorPayload> {
    let remote_ref = repo
        .find_branch(remote_branch, git2::BranchType::Remote)
        .map_err(|_| {
            ErrorPayload::new(
                ErrorCode::ReferenceNotFound,
                format!("Remote branch '{remote_branch}' does not exist — fetch first"),
            )
            .with_param("name", remote_branch)
            .with_param("class", "remote_branch_missing")
        })?;
    let shorthand = remote_ref.name()?.unwrap_or(remote_branch).to_string();

    let mut branch = repo
        .find_branch(local, git2::BranchType::Local)
        .map_err(|_| branch_not_found(local))?;
    if let Some(new_name) = rename_to {
        branch = branch.rename(new_name, false)?;
    }
    branch.set_upstream(Some(&shorthand))?;
    branch_info(repo, &branch, false)
}

/// `branch_untrack` (docs/03 §6; `GTGitRemoveBranchTrackingOperation`).
///
/// Removes `branch.<name>.remote` / `branch.<name>.merge` from the local
/// config — the same end state as `git branch --unset-upstream`.
pub fn untrack(repo: &git2::Repository, name: &str) -> Result<BranchInfo, ErrorPayload> {
    let mut config = repo.config()?;
    for key in [
        format!("branch.{name}.remote"),
        format!("branch.{name}.merge"),
    ] {
        remove_config_key(&mut config, &key)?;
    }
    load(repo, name)
}

/// `branch_checkout_remote` (docs/03 §6; `GTGitCheckoutRemoteBranchAction`):
/// create a local branch at the remote tip, optionally track it, check it out.
pub fn checkout_remote(
    repo: &git2::Repository,
    remote_branch: &str,
    local_name: Option<&str>,
    track: bool,
) -> Result<BranchInfo, ErrorPayload> {
    let remote_ref = repo
        .find_branch(remote_branch, git2::BranchType::Remote)
        .map_err(|_| {
            ErrorPayload::new(
                ErrorCode::ReferenceNotFound,
                format!("Remote branch '{remote_branch}' does not exist — fetch first"),
            )
            .with_param("name", remote_branch)
            .with_param("class", "remote_branch_missing")
        })?;
    let shorthand = remote_ref.name()?.unwrap_or(remote_branch).to_string();
    let short = shorthand
        .split_once('/')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| shorthand.clone());
    let name = local_name.unwrap_or(&short).to_string();

    if repo.find_branch(&name, git2::BranchType::Local).is_ok() {
        return Err(ErrorPayload::new(
            ErrorCode::ReferenceExists,
            format!("Branch '{name}' already exists"),
        )
        .with_param("name", name));
    }

    let commit = remote_ref.get().peel_to_commit()?;
    let mut branch = repo.branch(&name, &commit, false)?;

    // Check out first, and take the branch back if that is refused: nothing
    // else (tracking configuration included) may be left behind by a no-op.
    let refname = branch.get().name()?.to_string();
    if let Err(error) = checkout_tree_safely(repo, &commit, &name) {
        let _ = branch.delete();
        return Err(error);
    }
    repo.set_head(&refname)?;
    if track {
        branch.set_upstream(Some(&shorthand))?;
    }

    branch_info(repo, &branch, false)
}

// ---------------------------------------------------------------------------
// Stacked-branch support (the `missing_parent` precondition)
// ---------------------------------------------------------------------------

/// The stacked-branch parent's health. The `Missing` arm is the condition the
/// observed UI renders with its dedicated `missing_parent` symbol (EV:assets):
/// a branch is on record as *stacked on* a parent that no longer exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParentStatus {
    Present,
    Missing,
}

/// Classify a recorded parent refname. Pure read; the stack store calls this
/// when it rebuilds its rows, and `stacked_create`'s validator calls it before
/// writing anything.
pub fn parent_status(repo: &git2::Repository, parent_refname: &str) -> ParentStatus {
    let name = parent_refname
        .strip_prefix("refs/heads/")
        .unwrap_or(parent_refname);
    if repo.find_branch(name, git2::BranchType::Local).is_ok() {
        ParentStatus::Present
    } else {
        ParentStatus::Missing
    }
}

/// `repo.config()` writes through the local level; a missing key is not an
/// error when the caller asked for removal.
pub(crate) fn remove_config_key(config: &mut git2::Config, key: &str) -> Result<(), ErrorPayload> {
    match config.remove_multivar(key, ".*") {
        Ok(()) => Ok(()),
        Err(err) if err.code() == git2::ErrorCode::NotFound => Ok(()),
        Err(err) => Err(ErrorPayload::from(err)),
    }
}

/// Write `branch.<name>.remote` / `branch.<name>.merge` directly. Used after a
/// push, where the remote-tracking ref may not exist locally yet and
/// `Branch::set_upstream` (a DWIM lookup) would refuse.
pub(crate) fn set_tracking_config(
    repo: &git2::Repository,
    branch: &str,
    remote: &str,
    merge_ref: &str,
) -> Result<(), ErrorPayload> {
    let mut config = repo.config()?;
    config.set_str(&format!("branch.{branch}.remote"), remote)?;
    config.set_str(&format!("branch.{branch}.merge"), merge_ref)?;
    Ok(())
}

/// Is `refname` checked out in the main working directory or in any linked
/// worktree? This is libgit2's `git_branch_is_checked_out` rule, re-built from
/// the worktree list because git2 0.21 does not bind it.
pub fn branch_is_checked_out(repo: &git2::Repository, refname: &str) -> Result<bool, ErrorPayload> {
    if let Ok(head) = repo.head() {
        if head.name().ok() == Some(refname) {
            return Ok(true);
        }
    }
    for name in repo.worktrees()?.iter() {
        let Ok(Some(name)) = name else { continue };
        let worktree = repo.find_worktree(name)?;
        if let Ok(wt_repo) = git2::Repository::open_from_worktree(&worktree) {
            if let Ok(head) = wt_repo.head() {
                if head.name().ok() == Some(refname) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// The checked-out local branch's name, or `None` on a detached/unborn HEAD.
pub fn head_branch_name(repo: &git2::Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if head.is_branch() {
        head.shorthand().ok().map(str::to_string)
    } else {
        None
    }
}

fn resolve_upstream(repo: &git2::Repository, upstream: &str) -> Result<String, ErrorPayload> {
    let missing = || {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("Upstream '{upstream}' does not exist — fetch it first"),
        )
        .with_param("name", upstream)
        .with_param("class", "upstream_missing")
    };

    if upstream.starts_with("refs/") {
        let reference = repo.find_reference(upstream).map_err(|_| missing())?;
        return Ok(reference.name().unwrap_or(upstream).to_string());
    }
    if let Some((remote, rest)) = upstream.split_once('/') {
        let full = format!("refs/remotes/{remote}/{rest}");
        if repo.find_reference(&full).is_ok() {
            return Ok(upstream.to_string());
        }
    }
    if repo.find_branch(upstream, git2::BranchType::Local).is_ok() {
        return Ok(upstream.to_string());
    }
    Err(missing())
}

fn branch_not_found(name: &str) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::ReferenceNotFound,
        format!("Branch '{name}' does not exist"),
    )
    .with_param("name", name)
}

fn missing_parent(parent: &str) -> ErrorPayload {
    // The class behind the evidence's dedicated `missing_parent` symbol.
    ErrorPayload::new(
        ErrorCode::UnknownRevision,
        format!("The selected parent branch '{parent}' does not exist"),
    )
    .with_param("name", parent)
    .with_param("class", "missing_parent")
}

fn branch_info(
    repo: &git2::Repository,
    branch: &git2::Branch<'_>,
    is_remote: bool,
) -> Result<BranchInfo, ErrorPayload> {
    let name = branch.name()?.unwrap_or_default().to_string();
    let full_name = branch.get().name().unwrap_or_default().to_string();
    let tip = branch.get().peel_to_commit()?;
    let oid = tip.id();

    let upstream_branch = if is_remote {
        None
    } else {
        branch.upstream().ok()
    };
    let upstream = upstream_branch
        .as_ref()
        .and_then(|up| up.name().ok().flatten())
        .map(str::to_string);

    let (ahead, behind) = match upstream_branch
        .as_ref()
        .and_then(|up| up.get().peel_to_commit().ok())
    {
        Some(upstream_tip) => repo
            .graph_ahead_behind(oid, upstream_tip.id())
            .map(|(ahead, behind)| (ahead as u32, behind as u32))
            .unwrap_or((0, 0)),
        None => (0, 0),
    };

    Ok(BranchInfo {
        name,
        full_name,
        oid: oid.to_string(),
        is_head: branch.is_head(),
        is_remote,
        upstream,
        ahead,
        behind,
        tip_summary: tip.summary()?.unwrap_or_default().to_string(),
        tip_time: tip.time().seconds(),
    })
}

// ---------------------------------------------------------------------------
// Tags (docs/03 §9 — `GTGitOperationGroupTags`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagSort {
    #[default]
    Name,
    CreationDate,
    CommitDate,
    Author,
}

/// One tag row. `oid` is the tag object for annotated tags and the commit
/// itself for lightweight ones (the distinction the UI renders).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: String,
    pub full_name: String,
    pub oid: String,
    pub target_oid: String,
    pub annotated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagger: Option<Signature>,
    /// Tagger date (annotated) or target commit date (lightweight), unix seconds.
    pub tag_time: i64,
    /// The target commit's committer date, unix seconds.
    pub target_time: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TagCreateOptions {
    pub annotated: bool,
    pub message: Option<String>,
    /// Signing needs the `security` module (GPG/SSH signing); the command
    /// validator refuses `sign: true` loudly until it lands.
    pub sign: bool,
    /// `true` stamps the tagger with the system clock (default); `false`
    /// re-uses the target commit's committer timestamp with the configured
    /// identity.
    pub date_from_system: bool,
}

impl Default for TagCreateOptions {
    fn default() -> Self {
        Self {
            annotated: false,
            message: None,
            sign: false,
            date_from_system: true,
        }
    }
}

pub fn tag_list(repo: &git2::Repository, sort: TagSort) -> Result<Vec<TagInfo>, ErrorPayload> {
    let mut tags = Vec::new();
    for reference in repo.references_glob("refs/tags/*")? {
        let reference = reference?;
        let Ok(full_name) = reference.name() else {
            continue;
        };
        let name = full_name.trim_start_matches("refs/tags/").to_string();
        if let Ok(info) = tag_info(repo, &name) {
            tags.push(info);
        }
    }

    match sort {
        TagSort::Name => tags.sort_by(|a, b| a.name.cmp(&b.name)),
        TagSort::CreationDate => tags.sort_by_key(|tag| std::cmp::Reverse(tag.tag_time)),
        TagSort::CommitDate => tags.sort_by_key(|tag| std::cmp::Reverse(tag.target_time)),
        TagSort::Author => tags.sort_by(|a, b| {
            let key = |tag: &TagInfo| {
                tag.tagger
                    .as_ref()
                    .map(|sig| sig.name.clone())
                    .unwrap_or_default()
            };
            key(a).cmp(&key(b)).then_with(|| a.name.cmp(&b.name))
        }),
    }
    Ok(tags)
}

pub fn tag_load(repo: &git2::Repository, name: &str) -> Result<TagInfo, ErrorPayload> {
    tag_info(repo, name)
}

/// `tag_create` (docs/03 §9; `GTGitCreateTagAction`). Lightweight tags point
/// straight at the target; annotated tags write a tag object with a tagger and
/// message.
pub fn tag_create(
    repo: &git2::Repository,
    name: &str,
    target: &Revision,
    opts: &TagCreateOptions,
) -> Result<TagInfo, ErrorPayload> {
    let commit = history::resolve(repo, target)?;

    if opts.annotated {
        let message = opts.message.clone().unwrap_or_default();
        let tagger = tagger_signature(repo, &commit, opts.date_from_system)?;
        repo.tag(name, commit.as_object(), &tagger, &message, false)?;
    } else {
        repo.tag_lightweight(name, commit.as_object(), false)?;
    }

    tag_info(repo, name)
}

/// `tag_delete` (docs/03 §9; `GTGitDeleteTagsOperation`).
pub fn tag_delete(repo: &git2::Repository, names: &[String]) -> Result<DeleteReport, ErrorPayload> {
    let mut report = DeleteReport {
        deleted: Vec::new(),
        skipped: Vec::new(),
    };
    for name in names {
        match repo.tag_delete(name) {
            Ok(()) => report.deleted.push(name.clone()),
            Err(err) if err.code() == git2::ErrorCode::NotFound => {
                report.skipped.push(DeleteSkip {
                    name: name.clone(),
                    reason: format!("Tag '{name}' does not exist"),
                    class: "reference_not_found".to_string(),
                });
            }
            Err(err) => return Err(ErrorPayload::from(err)),
        }
    }
    Ok(report)
}

/// Refspecs for `tag_publish` / `tag_delete_remote`; the push itself goes
/// through `engine::remotes` so tags and branches share one transport path.
pub fn tag_publish_refspecs(names: &[String], all: bool) -> Vec<String> {
    if all {
        return vec!["refs/tags/*:refs/tags/*".to_string()];
    }
    names
        .iter()
        .map(|name| format!("refs/tags/{name}:refs/tags/{name}"))
        .collect()
}

pub fn tag_delete_refspecs(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| format!(":refs/tags/{name}"))
        .collect()
}

fn tag_info(repo: &git2::Repository, name: &str) -> Result<TagInfo, ErrorPayload> {
    let full_name = format!("refs/tags/{name}");
    let reference = repo.find_reference(&full_name).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::ReferenceNotFound,
            format!("Tag '{name}' does not exist"),
        )
        .with_param("name", name)
    })?;
    let oid = reference.target().ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::UnknownRevision,
            format!("Tag '{name}' has no target"),
        )
        .with_param("name", name)
    })?;
    let commit = reference.peel_to_commit()?;
    let target_time = commit.time().seconds();

    let tag_object = repo.find_tag(oid).ok();
    let (annotated, message, tagger, tag_time) = match &tag_object {
        Some(tag) => (
            true,
            tag.message().ok().flatten().map(str::to_string),
            tag.tagger().map(|sig| Signature {
                name: sig.name().unwrap_or_default().to_string(),
                email: sig.email().unwrap_or_default().to_string(),
                time: sig.when().seconds(),
            }),
            tag.tagger()
                .map(|sig| sig.when().seconds())
                .unwrap_or(target_time),
        ),
        None => (false, None, None, target_time),
    };

    Ok(TagInfo {
        name: name.to_string(),
        full_name,
        oid: oid.to_string(),
        target_oid: commit.id().to_string(),
        annotated,
        message,
        tagger,
        tag_time,
        target_time,
    })
}

fn tagger_signature(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    date_from_system: bool,
) -> Result<git2::Signature<'static>, ErrorPayload> {
    let base = repo.signature()?;
    let name = base.name().unwrap_or_default().to_string();
    let email = base.email().unwrap_or_default().to_string();
    if date_from_system {
        Ok(git2::Signature::now(&name, &email)?)
    } else {
        Ok(git2::Signature::new(&name, &email, &commit.time())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::index::test_support::{commit_all, scratch_repo, ScratchRepo};

    /// A repo with HEAD on the default branch and `feature` pointing at the same
    /// commit. A remote-tracking `origin/feature` is added at that tip too, so the
    /// resolver has both a local and a remote branch to choose from.
    fn seed() -> ScratchRepo {
        let scratch = scratch_repo();
        let dir = scratch.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        let first = commit_all(&scratch, "first");
        scratch
            .branch("feature", &scratch.find_commit(first).unwrap(), false)
            .unwrap();
        scratch
            .reference(
                "refs/remotes/origin/feature",
                first,
                true,
                "test: seed remote",
            )
            .unwrap();
        scratch
    }

    // -----------------------------------------------------------------
    // resolve_branch
    // -----------------------------------------------------------------

    #[test]
    fn resolve_accepts_the_short_and_full_local_forms() {
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        for form in ["feature", "refs/heads/feature"] {
            let resolved = resolve_branch(repo, form).unwrap();
            assert_eq!(resolved.full_name, "refs/heads/feature", "form {form}");
            assert_eq!(resolved.name, "feature", "form {form}");
            assert!(!resolved.is_remote, "form {form}");
        }
    }

    #[test]
    fn resolve_accepts_the_short_and_full_remote_forms() {
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        for form in ["origin/feature", "refs/remotes/origin/feature"] {
            let resolved = resolve_branch(repo, form).unwrap();
            assert_eq!(
                resolved.full_name, "refs/remotes/origin/feature",
                "form {form}"
            );
            assert_eq!(resolved.name, "origin/feature", "form {form}");
            assert!(resolved.is_remote, "form {form}");
        }
    }

    #[test]
    fn resolve_prefers_the_local_branch_when_both_match() {
        // A local branch literally named `origin/feature` shadows the remote.
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        let tip = repo
            .find_branch("feature", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        repo.branch("origin/feature", &tip, false).unwrap();

        let resolved = resolve_branch(repo, "origin/feature").unwrap();
        assert_eq!(resolved.full_name, "refs/heads/origin/feature");
        assert!(!resolved.is_remote);
    }

    #[test]
    fn resolve_names_what_it_tried_when_the_branch_is_missing() {
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        let err = resolve_branch(repo, "nope").unwrap_err();
        assert_eq!(err.code, ErrorCode::ReferenceNotFound);
        assert!(
            err.message.contains("refs/heads/nope") && err.message.contains("refs/remotes/nope"),
            "the error should name both refnames tried, got: {}",
            err.message
        );
    }

    // -----------------------------------------------------------------
    // checkout — the quick-open `branch:refs/heads/<name>` id
    // -----------------------------------------------------------------

    #[test]
    fn checkout_takes_a_full_refname() {
        // The bug: quick-open passes `refs/heads/feature`, which
        // `find_branch(name, Local)` cannot locate.
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        let result = checkout(repo, "refs/heads/feature", &CheckoutOptions::default()).unwrap();
        assert_eq!(result.branch.as_deref(), Some("feature"));
        assert!(!result.detached);
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature");
    }

    #[test]
    fn checkout_of_a_remote_branch_detaches_at_its_tip() {
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        let result = checkout(
            repo,
            "refs/remotes/origin/feature",
            &CheckoutOptions::default(),
        )
        .unwrap();
        assert!(result.detached);
        assert_eq!(result.branch, None);
        assert!(!repo.head().unwrap().is_branch());
    }

    #[test]
    fn checkout_reports_a_missing_branch_clearly() {
        let scratch = seed();
        let repo: &git2::Repository = &scratch;
        let err = checkout(repo, "ghost", &CheckoutOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::ReferenceNotFound);
        assert!(err.message.contains("ghost"));
    }
}
