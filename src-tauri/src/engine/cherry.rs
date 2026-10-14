//! Cherry-pick and revert (`GTGitCherryPickRevisionsAction`,
//! `GTRevertRevisionsDialogueViewController`).
//!
//! Both are the same machine: replay a set of commits as *diffs* onto HEAD.
//! Cherry-pick applies each commit's patch forward; revert applies it backward.
//! The replay, conflict parking and continue/skip/abort are the sequence engine
//! in [`super::rebase`] — this module only resolves the arguments, decides
//! whether a fast-forward shortcut applies, and builds the todo.

use git2::Oid;

use crate::domain::conflict::{IntegrationOutcome, IntegrationResult, PlanStep, SequenceAction};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::revision::Revision;

use super::merge;
use super::rebase::{self, SequenceKind};

/// `GTUserDefaultsDialogueOptionCherryPick*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CherryPickRequest {
    /// Allow a fast-forward when the picked commit is already a descendant of
    /// HEAD (the "reapply ahead" shortcut).
    pub ff: bool,
    /// Never take that shortcut: always create a commit.
    pub always_merge_commit: bool,
    /// Stage the result and stop (`--no-commit`).
    pub no_commit: bool,
    /// Append the `(cherry picked from commit …)` trailer.
    pub record_origin: bool,
    /// For a merge commit: which parent is the mainline (1-based).
    pub mainline: Option<u32>,
}

/// `cherry_pick` (docs/03 §13).
pub fn cherry_pick(
    repo: &git2::Repository,
    revisions: &[Revision],
    req: &CherryPickRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    if revisions.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No revisions were given to cherry-pick",
        ));
    }
    let oids = rebase::resolve_revisions(repo, revisions)?;

    // Fast-forward shortcut only makes sense for a single commit that already
    // sits ahead of HEAD.
    if oids.len() == 1 && req.ff && !req.always_merge_commit && !req.no_commit {
        let head = merge::head_commit(repo)?.id();
        if repo.graph_descendant_of(oids[0], head)? {
            if merge::integration_in_progress(repo) {
                return Err(ErrorPayload::new(
                    ErrorCode::OperationInProgress,
                    "An integration is already in progress",
                ));
            }
            merge::fast_forward(repo, oids[0])?;
            return Ok(IntegrationResult::new(IntegrationOutcome::FastForward {
                head: oids[0].to_string(),
                from: head.to_string(),
            }));
        }
    }

    let steps = oids
        .into_iter()
        .map(|oid| PlanStep {
            action: SequenceAction::Pick,
            oid: oid.to_string(),
            ..PlanStep::default()
        })
        .collect();

    rebase::start_sequence(
        repo,
        SequenceKind::CherryPick,
        steps,
        req.no_commit,
        req.record_origin,
        req.mainline.unwrap_or(0),
    )
}

/// `revert` (docs/03 §13).
#[derive(Clone, Copy, Debug)]
pub struct RevertRequest {
    /// Commit the revert immediately; when false the change is staged only.
    pub commit_after: bool,
    /// For a merge commit: which parent line to revert (1-based).
    pub mainline: Option<u32>,
}

impl Default for RevertRequest {
    fn default() -> Self {
        Self {
            commit_after: true,
            mainline: None,
        }
    }
}

pub fn revert(
    repo: &git2::Repository,
    revisions: &[Revision],
    req: &RevertRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    if revisions.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No revisions were given to revert",
        ));
    }

    // A merge commit has no single diff to reverse; refuse early with a clear
    // message rather than let libgit2 fail opaquely.
    for revision in revisions {
        let commit = super::history::resolve(repo, revision)?;
        if commit.parent_count() > 1 && req.mainline.is_none() {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "Reverting a merge commit needs a mainline parent",
            )
            .with_param("oid", commit.id().to_string()));
        }
    }

    let oids = rebase::resolve_revisions(repo, revisions)?;
    let steps: Vec<PlanStep> = oids
        .into_iter()
        .map(|oid| PlanStep {
            action: SequenceAction::Pick,
            oid: oid.to_string(),
            ..PlanStep::default()
        })
        .collect();

    rebase::start_sequence(
        repo,
        SequenceKind::Revert,
        steps,
        !req.commit_after,
        false,
        req.mainline.unwrap_or(0),
    )
}

/// Convenience for the command layer: the first revision of a set, resolved.
pub fn first_oid(repo: &git2::Repository, revisions: &[Revision]) -> Result<Oid, ErrorPayload> {
    let Some(first) = revisions.first() else {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No revisions were given",
        ));
    };
    Ok(super::history::resolve(repo, first)?.id())
}
