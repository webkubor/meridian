//! Reset (`GTGitResetAction`, `performResetActionWithRevision:headRevision:
//! mode:snapshot:`, `GTResetEntriesToRevisionDialogueViewController`).
//!
//! Two commands, one rule: **`ORIG_HEAD` is written before anything moves.**
//! That is what the snapshot/undo surface (docs/02 §7) reads to offer "undo the
//! reset", and it is a git-native pseudoref, not Meridian state. Hard reset
//! additionally demands explicit confirmation, because it is the one mode that
//! discards working-tree changes unrecoverably.

use std::path::Path;

use git2::{Oid, ResetType};

use crate::domain::conflict::{ResetMode, ResetResult};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::revision::Revision;
use crate::domain::status::{StatusDelta, StatusSummary};
use crate::engine::working_tree;

use super::{history, merge};

/// `reset_to_revision` (docs/03 §13). `confirm` is the "yes, discard my
/// changes" tick the Hard dialogue requires; it is a validator-level rule, so
/// we enforce it here too (the engine is a public seam).
pub fn reset_to_revision(
    repo: &git2::Repository,
    revision: &Revision,
    mode: ResetMode,
    confirm: bool,
) -> Result<ResetResult, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "Finish or abort the current operation before resetting",
        ));
    }
    if mode == ResetMode::Hard && !confirm {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "A hard reset discards working-tree changes and must be confirmed",
        ));
    }

    let target = history::resolve(repo, revision)?;
    let previous = repo.head().ok().and_then(|head| head.target());
    let anchor = previous.unwrap_or(target.id());

    // The undo anchor, before any ref or tree moves.
    merge::write_pseudoref(repo, "ORIG_HEAD", anchor)?;

    let object = repo.find_object(target.id(), None)?;
    let kind = match mode {
        ResetMode::Soft => ResetType::Soft,
        ResetMode::Mixed => ResetType::Mixed,
        ResetMode::Hard => ResetType::Hard,
    };
    repo.reset(&object, kind, None)?;

    Ok(ResetResult {
        mode,
        head: target.id().to_string(),
        previous_head: previous.map(|oid| oid.to_string()),
        undo_anchor: anchor.to_string(),
        delta: status_delta(repo)?,
    })
}

/// `reset_entries_to_revision` (docs/03 §13): reset only these paths' index
/// entries to their state at `revision` — HEAD does not move.
pub fn reset_entries_to_revision(
    repo: &git2::Repository,
    paths: &[String],
    revision: &Revision,
) -> Result<StatusDelta, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if paths.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No paths were given",
        ));
    }

    let target = history::resolve(repo, revision)?;
    let object = repo.find_object(target.id(), None)?;

    // A missing path at the revision means "remove it from the index"; an
    // all-or-nothing failure would be wrong, so resolve each path against the
    // tree and reset the resolvable ones.
    let tree = target.tree()?;
    let mut pathspecs: Vec<String> = Vec::with_capacity(paths.len());
    for path in paths {
        if Path::new(path).components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) || Path::new(path).is_absolute()
        {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("Path leaves the repository: {path}"),
            )
            .with_param("path", path));
        }
        // Present in the target tree, or being removed here — both are a
        // legitimate reset entry.
        let _present = tree.get_path(Path::new(path)).is_ok();
        pathspecs.push(path.clone());
    }

    repo.reset_default(Some(&object), pathspecs.iter().map(Path::new))?;

    status_delta(repo)
}

/// The fresh status delta every reset answers with, so the UI does not need a
/// second round-trip.
pub(crate) fn status_delta(repo: &git2::Repository) -> Result<StatusDelta, ErrorPayload> {
    let entries = working_tree::status(
        repo,
        &working_tree::StatusOptions {
            include_ignored: false,
            include_dirs: false,
        },
    )?;
    Ok(StatusDelta {
        repo: RepoId::new(repo.path()),
        summary: StatusSummary::of(&entries),
        entries,
    })
}

/// Read `ORIG_HEAD` — the anchor the undo surface resets back to. Exposed for
/// the "Undo" affordance the evidence records next to a completed reset.
pub fn undo_anchor(repo: &git2::Repository) -> Option<Oid> {
    merge::read_pseudoref(repo, "ORIG_HEAD")
}
