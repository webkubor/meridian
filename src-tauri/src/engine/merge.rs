//! Merge, merge-state continuation, conflict detection and conflict
//! resolution — the `GTGitMerge*Operation` / `GTGitResolveMergeConflicts*`
//! counterpart (docs/03 §13).
//!
//! Design notes that the rest of the integrate engine depends on:
//!
//! - **Git-native state first.** A merge in progress is a real `MERGE_HEAD`
//!   and the repository reports `RepositoryState::Merge`; abort and continue
//!   read that, so a merge started by the CLI behaves the same as one started
//!   here. `ORIG_HEAD` is written before anything moves, which is what abort
//!   rolls back to.
//! - **Conflicts are index stages, never files.** Stage 1/2/3 are read from
//!   the index; the conflict-marker text in the working tree is only a
//!   rendering. Per-hunk resolution rebuilds that rendering deterministically
//!   (diff3 style, our own labels) instead of trusting whatever markers the
//!   file happens to contain.

use std::path::Path;

use git2::{build::CheckoutBuilder, IndexEntry, IndexTime, MergeOptions, Oid, ResetType};

use crate::domain::conflict::{
    ConflictEntry, ConflictHunk, ConflictKind, ConflictSides, ConflictSummary, ConflictVersion,
    HunkChoice, HunkResolution, IntegrationOutcome, IntegrationResult, MergeState, MergeStatus,
    MergeStrategy, ResolutionStrategy,
};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::repository::IntegrationState;
use crate::domain::revision::Revision;
use crate::engine::history;

use super::rebase;

/// Options for `merge_start`. The booleans are the dialogue options the
/// evidence records (`GTUserDefaultsDialogueOptionMerge*`); `strategy` is the
/// "custom strategy" axis (`-X ours` / `-X theirs` / `-X union`).
#[derive(Clone, Debug, Default)]
pub struct MergeRequest {
    /// Always create a merge commit, even when a fast-forward is possible
    /// (`--no-ff`).
    pub no_ff: bool,
    /// Merge into the index/working tree and stop (`--squash`): the changes
    /// are staged, no commit is made and no merge state is left behind.
    pub squash: bool,
    /// Create the merge commit when the merge is clean. With `false` the
    /// staged result waits for `integration_continue` — the "review before
    /// committing" flow.
    pub commit: bool,
    /// Message for the merge commit; `None` reads `MERGE_MSG` (which the
    /// dialogue may have edited), falling back to a default.
    pub message: Option<String>,
    pub strategy: MergeStrategy,
    /// Refuse when the incoming tip carries no signature
    /// (`GTUserDefaultsDialogueOptionMergeVerifySignatures`).
    pub verify_signatures: bool,
    /// Stash a dirty working tree before merging and re-apply it afterwards.
    pub autostash: bool,
    /// Rename detection during the merge (git's own default; libgit2's is off).
    pub find_renames: bool,
}

impl MergeRequest {
    /// The defaults the merge dialogue opens with: rename detection on, a
    /// commit when the merge is clean, auto-stash offered but off.
    pub fn dialogue_defaults() -> Self {
        Self {
            commit: true,
            find_renames: true,
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Shared plumbing (also used by rebase.rs / cherry.rs)
// ---------------------------------------------------------------------------

/// Open a second handle on the same repository.
///
/// A handful of libgit2 calls (stash save/pop) are only exposed behind
/// `&mut Repository`, and our engine functions receive a shared handle from
/// the command orchestrator. A second handle is exactly what a second `git`
/// process would do; all command side effects are serialized by the repo
/// queue, so nothing races.
pub(crate) fn own_handle(repo: &git2::Repository) -> Result<git2::Repository, ErrorPayload> {
    git2::Repository::open(repo.path()).map_err(ErrorPayload::from)
}

/// The signature new commits are made with: the repository's configured
/// identity, falling back to a fixed one so a misconfigured repo can still
/// finish an operation it already started (the CLI refuses instead — we
/// prefer to finish what validation let through).
pub(crate) fn committer(repo: &git2::Repository) -> Result<git2::Signature<'static>, ErrorPayload> {
    repo.signature().or_else(|_| {
        git2::Signature::now("Meridian", "meridian@localhost").map_err(ErrorPayload::from)
    })
}

/// Does the working tree or index hold anything a merge would have to
/// overwrite? Untracked files do not count (git merges around them).
pub(crate) fn workdir_dirty(repo: &git2::Repository) -> Result<bool, ErrorPayload> {
    if repo.index()?.has_conflicts() {
        return Ok(true);
    }
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false)
        .include_ignored(false)
        .renames_head_to_index(true);
    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(!statuses.is_empty())
}

/// Read a one-value pseudoref (`ORIG_HEAD`, `MERGE_HEAD`, …) straight from the
/// gitdir. The references API only knows proper refs, and these files are
/// written by git itself in exactly this shape.
pub(crate) fn read_pseudoref(repo: &git2::Repository, name: &str) -> Option<Oid> {
    let raw = std::fs::read_to_string(repo.path().join(name)).ok()?;
    let first = raw.lines().next()?.trim();
    Oid::from_str(first).ok()
}

/// Write a one-value pseudoref. Used for `ORIG_HEAD` — the anchor every abort
/// path rolls back to.
pub(crate) fn write_pseudoref(
    repo: &git2::Repository,
    name: &str,
    oid: Oid,
) -> Result<(), ErrorPayload> {
    std::fs::write(repo.path().join(name), format!("{oid}\n"))
        .map_err(|err| ErrorPayload::new(ErrorCode::Io, format!("Could not write {name}: {err}")))
}

/// `<gitdir>/meridian/` — Meridian's own metadata directory. It never holds
/// repository data (docs/02 §9: git-native data only through git mechanisms);
/// it holds the sequence state a multi-step operation needs to survive the
/// gap between two commands.
pub(crate) fn meridian_dir(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("meridian")
}

fn autostash_path(repo: &git2::Repository) -> std::path::PathBuf {
    meridian_dir(repo).join("autostash")
}

/// Stash the working tree when it is dirty and auto-stash was asked for.
/// Returns `true` when a stash was created; the stash id is recorded next to
/// the sequence state so the finishing command can re-apply it.
pub(crate) fn autostash_save(
    repo: &git2::Repository,
    enabled: bool,
    label: &str,
    notes: &mut Vec<String>,
) -> Result<bool, ErrorPayload> {
    if !enabled || !workdir_dirty(repo)? {
        return Ok(false);
    }
    let mut handle = own_handle(repo)?;
    let sig = committer(repo)?;
    let oid = handle.stash_save(&sig, &format!("meridian: {label}"), None)?;
    let dir = meridian_dir(repo);
    std::fs::create_dir_all(&dir).map_err(|err| {
        ErrorPayload::new(
            ErrorCode::Io,
            format!("Could not create {}: {err}", dir.display()),
        )
    })?;
    std::fs::write(autostash_path(repo), format!("{oid}\n")).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not record autostash: {err}"))
    })?;
    notes.push(
        "Local changes were auto-stashed and will be re-applied when the operation finishes."
            .to_string(),
    );
    Ok(true)
}

/// Re-apply (and drop) the auto-stash recorded by [`autostash_save`]. A stash
/// that conflicts is left in place — that must never fail the operation that
/// just finished.
pub(crate) fn autostash_restore(
    repo: &git2::Repository,
    notes: &mut Vec<String>,
) -> Result<(), ErrorPayload> {
    let path = autostash_path(repo);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let _ = std::fs::remove_file(&path);
    let Ok(oid) = Oid::from_str(raw.trim()) else {
        return Ok(());
    };

    let mut handle = own_handle(repo)?;
    let mut found = None;
    handle.stash_foreach(|index, _message, stash_oid| {
        if *stash_oid == oid {
            found = Some(index);
            false
        } else {
            true
        }
    })?;

    match found {
        Some(index) => match handle.stash_pop(index, None) {
            Ok(()) => notes.push("Auto-stashed changes were re-applied.".to_string()),
            Err(err) => notes.push(format!(
                "Auto-stashed changes could not be re-applied cleanly ({err}); the stash entry was kept."
            )),
        },
        None => notes.push(
            "The auto-stash entry is gone; local changes were not re-applied.".to_string(),
        ),
    }
    Ok(())
}

/// The revision state, including the sequences Meridian itself runs (whose
/// state lives in `<gitdir>/meridian/sequence` because git has no on-disk
/// marker for a cherry-pick-driven rebase).
pub fn integration_state(repo: &git2::Repository) -> IntegrationState {
    if let Some(state) = rebase::sequence_state(repo) {
        return state.integration_state();
    }
    super::repository::integration_state(repo)
}

/// True when *any* integration is in progress — the precondition every
/// integrating command checks first.
pub fn integration_in_progress(repo: &git2::Repository) -> bool {
    repo.state() != git2::RepositoryState::Clean || rebase::sequence_present(repo)
}

/// Is the tip of `revision` signed? The presence check behind
/// `verify_signatures`; proving trust is the security module's job
/// (`signature_verify`), and duplicating gpg here would be a second source of
/// truth.
fn require_signature(repo: &git2::Repository, oid: Oid) -> Result<(), ErrorPayload> {
    repo.extract_signature(&oid, None).map(|_| ()).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The incoming commit is not signed; signature verification was requested",
        )
        .with_param("oid", oid.to_string())
    })
}

fn annotated<'r>(
    repo: &'r git2::Repository,
    revision: &Revision,
) -> Result<git2::AnnotatedCommit<'r>, ErrorPayload> {
    let commit = history::resolve(repo, revision)?;
    repo.find_annotated_commit(commit.id())
        .map_err(ErrorPayload::from)
}

pub(crate) fn head_commit(repo: &git2::Repository) -> Result<git2::Commit<'_>, ErrorPayload> {
    repo.head()?.peel_to_commit().map_err(ErrorPayload::from)
}

// ---------------------------------------------------------------------------
// Merge status (the read-only side)
// ---------------------------------------------------------------------------

/// `merge_status_load` (docs/03 §3): `merge_analysis` plus a trial merge into
/// an in-memory index. Nothing on disk changes; a conflict here is a warning,
/// not a state.
///
/// With `revision: None` the analysed candidate is HEAD's upstream when it has
/// one; otherwise the call reports [`MergeState::Merged`] for HEAD itself.
pub fn status(
    repo: &git2::Repository,
    revision: Option<&Revision>,
) -> Result<MergeStatus, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }

    let Some(revision) = revision_or_upstream(repo, revision)? else {
        return Ok(MergeStatus {
            state: MergeState::Merged,
            incoming: None,
            base: None,
            fast_forward: false,
            conflict_preview: Vec::new(),
        });
    };

    let their = history::resolve(repo, &revision)?;
    let annotated = repo.find_annotated_commit(their.id())?;
    let (analysis, _) = repo.merge_analysis(&[&annotated])?;

    if analysis.is_up_to_date() {
        return Ok(MergeStatus {
            state: MergeState::Merged,
            incoming: Some(their.id().to_string()),
            base: None,
            fast_forward: false,
            conflict_preview: Vec::new(),
        });
    }
    if analysis.is_unborn() {
        return Ok(MergeStatus {
            state: MergeState::Mergeable,
            incoming: Some(their.id().to_string()),
            base: None,
            fast_forward: true,
            conflict_preview: Vec::new(),
        });
    }

    // The trial merge: ours = HEAD, theirs = candidate, base = merge base.
    let base_oid = repo.merge_base(head_commit(repo)?.id(), their.id())?;
    let our_tree = head_commit(repo)?.tree()?;
    let their_tree = their.tree()?;
    let base_tree = repo.find_commit(base_oid)?.tree()?;

    let index = repo.merge_trees(&base_tree, &our_tree, &their_tree, None)?;
    if !index.has_conflicts() {
        return Ok(MergeStatus {
            state: MergeState::Mergeable,
            incoming: Some(their.id().to_string()),
            base: Some(base_oid.to_string()),
            fast_forward: analysis.is_fast_forward(),
            conflict_preview: Vec::new(),
        });
    }

    let mut preview = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        if let Some(path) = conflict_path(&conflict) {
            preview.push(path);
            if preview.len() >= 200 {
                break;
            }
        }
    }
    Ok(MergeStatus {
        state: MergeState::Conflict,
        incoming: Some(their.id().to_string()),
        base: Some(base_oid.to_string()),
        fast_forward: false,
        conflict_preview: preview,
    })
}

fn revision_or_upstream(
    repo: &git2::Repository,
    revision: Option<&Revision>,
) -> Result<Option<Revision>, ErrorPayload> {
    if let Some(revision) = revision {
        return Ok(Some(revision.clone()));
    }
    let Ok(head) = repo.head() else {
        return Ok(None);
    };
    if !head.is_branch() {
        return Ok(None);
    }
    let branch = git2::Branch::wrap(head);
    let Ok(upstream) = branch.upstream() else {
        return Ok(None);
    };
    Ok(upstream
        .name()
        .ok()
        .flatten()
        .map(|name| Revision::Branch(name.to_string())))
}

// ---------------------------------------------------------------------------
// Merge start / continue / abort
// ---------------------------------------------------------------------------

/// `merge_start` (docs/03 §13). The full matrix — fast-forward, `--no-ff`,
/// `--squash`, staged-without-commit, custom strategy — is driven by
/// [`MergeRequest`].
pub fn start(
    repo: &git2::Repository,
    revision: &Revision,
    req: &MergeRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }

    let annotated = annotated(repo, revision)?;
    let their = repo.find_commit(annotated.id())?;
    if req.verify_signatures {
        require_signature(repo, their.id())?;
    }

    let (analysis, _) = repo.merge_analysis(&[&annotated])?;
    log::debug!(
        target: "meridian::engine",
        "merge analysis: up_to_date={} fast_forward={} normal={} unborn={}",
        analysis.is_up_to_date(),
        analysis.is_fast_forward(),
        analysis.is_normal(),
        analysis.is_unborn()
    );
    if analysis.is_up_to_date() {
        return Ok(IntegrationResult::new(IntegrationOutcome::UpToDate {
            head: head_commit(repo)?.id().to_string(),
        }));
    }

    let mut notes = Vec::new();
    autostash_save(repo, req.autostash, "merge", &mut notes)?;

    // Unborn HEAD: there is nothing to merge *into*; the only correct answer is
    // to adopt the incoming history, which is what git does too.
    if analysis.is_unborn() {
        let head = fast_forward_unborn(repo, their.id())?;
        autostash_restore(repo, &mut notes)?;
        return Ok(IntegrationResult::new(IntegrationOutcome::FastForward {
            head,
            from: "unborn HEAD".to_string(),
        })
        .also_notes(notes));
    }

    let head = head_commit(repo)?;
    write_pseudoref(repo, "ORIG_HEAD", head.id())?;

    if analysis.is_fast_forward() && !req.no_ff {
        fast_forward(repo, their.id())?;
        autostash_restore(repo, &mut notes)?;
        return Ok(IntegrationResult::new(IntegrationOutcome::FastForward {
            head: their.id().to_string(),
            from: head.id().to_string(),
        })
        .also_notes(notes));
    }

    // A real merge. `Repository::merge` stages the result, writes `MERGE_HEAD`
    // and checks the working tree out with conflict markers.
    let mut merge_opts = MergeOptions::new();
    merge_opts.find_renames(req.find_renames);
    match req.strategy {
        MergeStrategy::Default => {}
        MergeStrategy::Ours => {
            merge_opts.file_favor(git2::FileFavor::Ours);
        }
        MergeStrategy::Theirs => {
            merge_opts.file_favor(git2::FileFavor::Theirs);
        }
        MergeStrategy::Union => {
            merge_opts.file_favor(git2::FileFavor::Union);
        }
    }
    let mut checkout = CheckoutBuilder::new();
    checkout
        .safe()
        .allow_conflicts(true)
        .conflict_style_merge(true);

    repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))?;

    let conflicts = conflict_summaries(repo)?;
    if !conflicts.is_empty() {
        return Ok(IntegrationResult::new(IntegrationOutcome::Conflicted)
            .with_conflicts(conflicts)
            .also_notes(notes));
    }

    if req.squash {
        // `--squash`: staged result, no merge state — `integration_continue`
        // must not offer to "continue the merge"; the user commits.
        repo.cleanup_state()?;
        autostash_restore(repo, &mut notes)?;
        return Ok(IntegrationResult::new(IntegrationOutcome::Staged).also_notes(notes));
    }

    if !req.commit {
        // Keep `MERGE_HEAD`: `integration_continue` produces the commit, and
        // abort stays available.
        return Ok(IntegrationResult::new(IntegrationOutcome::Staged).also_notes(notes));
    }

    let commit = create_merge_commit(repo, &their, req.message.as_deref())?;
    repo.cleanup_state()?;
    autostash_restore(repo, &mut notes)?;
    Ok(IntegrationResult::new(IntegrationOutcome::Merged {
        commit: commit.to_string(),
        parents: vec![head.id().to_string(), their.id().to_string()],
    })
    .also_notes(notes))
}

/// Continue a merge that is staged: commit what the index holds.
pub fn continue_merge(
    repo: &git2::Repository,
    message: Option<&str>,
    notes: &mut Vec<String>,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.state() != git2::RepositoryState::Merge {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "No merge is in progress",
        ));
    }
    if repo.index()?.has_conflicts() {
        return Ok(IntegrationResult::new(IntegrationOutcome::Conflicted)
            .with_conflicts(conflict_summaries(repo)?));
    }
    let their_oid = read_pseudoref(repo, "MERGE_HEAD").ok_or_else(|| {
        ErrorPayload::new(ErrorCode::OperationInProgress, "MERGE_HEAD is missing")
    })?;
    let their = repo.find_commit(their_oid)?;
    let commit = create_merge_commit(repo, &their, message)?;
    repo.cleanup_state()?;
    autostash_restore(repo, notes)?;
    Ok(IntegrationResult::new(IntegrationOutcome::Merged {
        commit: commit.to_string(),
        parents: vec![head_commit(repo)?.id().to_string(), their_oid.to_string()],
    })
    .also_notes(std::mem::take(notes)))
}

/// `merge_abort` (docs/03 §13): mode-aware rollback. Merge state here; the
/// sequence kinds live with their engines.
pub fn abort_merge(
    repo: &git2::Repository,
    notes: &mut Vec<String>,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.state() != git2::RepositoryState::Merge {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "No merge is in progress",
        ));
    }
    let target = read_pseudoref(repo, "ORIG_HEAD")
        .or_else(|| head_commit(repo).ok().map(|commit| commit.id()))
        .ok_or_else(|| ErrorPayload::new(ErrorCode::Unclassified, "HEAD does not resolve"))?;

    // Drop the state files first, then roll the tree back: after this the
    // repository is exactly as it was before `merge_start` (the precondition
    // made sure the tree was clean or stashed then).
    repo.cleanup_state()?;
    let object = repo.find_object(target, None)?;
    repo.reset(&object, ResetType::Hard, None)?;
    autostash_restore(repo, notes)?;

    Ok(IntegrationResult::new(IntegrationOutcome::Aborted {
        head: target.to_string(),
    })
    .also_notes(std::mem::take(notes)))
}

/// Move HEAD (and the checked-out branch) to `target`, updating index and
/// working tree. The caller has already established the move is safe.
pub(crate) fn fast_forward(repo: &git2::Repository, target: Oid) -> Result<(), ErrorPayload> {
    let target_commit = repo.find_commit(target)?;
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(target_commit.as_object(), Some(&mut checkout))?;

    let head = repo.head().ok();
    match head {
        Some(reference) if reference.is_branch() => {
            let name = reference.name().ok();
            if let Some(name) = name {
                repo.reference(name, target, true, "merge: fast-forward")?;
            }
        }
        Some(_) => repo.set_head_detached(target)?,
        None => {
            // Unborn: HEAD is symbolic but its target does not exist yet.
            let name = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|head| head.symbolic_target().ok().flatten().map(|s| s.to_string()));
            if let Some(name) = name {
                repo.reference(&name, target, true, "merge: initial history")?;
                repo.set_head(&name)?;
            } else {
                repo.set_head_detached(target)?;
            }
        }
    }
    Ok(())
}

fn fast_forward_unborn(repo: &git2::Repository, target: Oid) -> Result<String, ErrorPayload> {
    fast_forward(repo, target)?;
    Ok(target.to_string())
}

/// Build and write the merge commit from the current index. Author and
/// committer are the local identity (git's behaviour for merges); the message
/// is the caller's, else `MERGE_MSG` (editable in the dialogue), else a
/// generated default.
fn create_merge_commit(
    repo: &git2::Repository,
    their: &git2::Commit<'_>,
    message: Option<&str>,
) -> Result<Oid, ErrorPayload> {
    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Err(ErrorPayload::new(
            ErrorCode::CheckoutConflicts,
            "Unresolved conflicts remain in the index",
        ));
    }
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head = head_commit(repo)?;

    let message = match message {
        Some(message) => message.to_string(),
        None => merge_message(repo, their),
    };

    let sig = committer(repo)?;
    let oid = repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&head, their])?;
    Ok(oid)
}

/// The default merge message: `MERGE_MSG` first (git and our dialogue write
/// it), else the conventional one-liner.
fn merge_message(repo: &git2::Repository, their: &git2::Commit<'_>) -> String {
    if let Ok(raw) = std::fs::read_to_string(repo.path().join("MERGE_MSG")) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return raw;
        }
    }
    let summary = their.summary().ok().flatten().unwrap_or("merge");
    format!("Merge commit '{}' — {summary}", short(their.id()))
}

pub(crate) fn short(oid: Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

/// `conflicts_load` (docs/03 §3 / §13): every unmerged index entry, classified
/// and (for text conflicts) decomposed into conflict regions.
pub fn conflicts(repo: &git2::Repository) -> Result<Vec<ConflictEntry>, ErrorPayload> {
    let index = repo.index()?;
    let mut entries = Vec::new();

    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let Some(path) = conflict_path(&conflict) else {
            continue;
        };
        let kind = classify(
            conflict.ancestor.is_some(),
            conflict.our.is_some(),
            conflict.their.is_some(),
        );
        let sides = ConflictSides {
            base: version(conflict.ancestor.as_ref()),
            ours: version(conflict.our.as_ref()),
            theirs: version(conflict.their.as_ref()),
        };

        let (binary, mergeable, hunks) = if kind.is_content_conflict() {
            match text_merge(repo, &sides)? {
                Some((_content, hunks)) => (false, true, hunks),
                None => (true, false, Vec::new()),
            }
        } else {
            (false, false, Vec::new())
        };

        entries.push(ConflictEntry {
            path,
            kind,
            sides,
            binary,
            mergeable,
            hunks,
        });
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

/// The compact conflict list carried by outcomes. Cheap: classification only.
pub fn conflict_summaries(repo: &git2::Repository) -> Result<Vec<ConflictSummary>, ErrorPayload> {
    let index = repo.index()?;
    let mut summaries = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        if let Some(path) = conflict_path(&conflict) {
            summaries.push(ConflictSummary {
                path,
                kind: classify(
                    conflict.ancestor.is_some(),
                    conflict.our.is_some(),
                    conflict.their.is_some(),
                ),
            });
        }
    }
    summaries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(summaries)
}

/// The conflict regions of one path, in merge-output order. `Ok(None)`-style
/// absence is reported as an error: asking for the hunks of a path that is
/// not conflicted is a caller bug, and the evidence has a dedicated class for
/// it (`GTGitMergeConflictNotAnUnmergedEntry`).
pub fn hunks(repo: &git2::Repository, path: &str) -> Result<Vec<ConflictHunk>, ErrorPayload> {
    let (_, hunks) = text_merge_for_path(repo, path)?;
    Ok(hunks)
}

/// Resolve one path with a whole-file strategy (`wc_conflict_resolve` /
/// `conflict_resolve`). `save_to` overrides where a `SaveVersions` request
/// writes its backups (`<save_to>.ours`, …); `None` writes beside the path.
pub fn resolve_file(
    repo: &git2::Repository,
    path: &str,
    strategy: &ResolutionStrategy,
    save_to: Option<&str>,
) -> Result<(), ErrorPayload> {
    let mut index = repo.index()?;
    let conflict = index.conflict_get(Path::new(path)).map_err(|_| {
        ErrorPayload::new(ErrorCode::InvalidRequest, "Path is not an unmerged entry")
            .with_param("path", path)
    })?;

    match strategy {
        ResolutionStrategy::SaveVersions => {
            let sides = ConflictSides {
                base: version(conflict.ancestor.as_ref()),
                ours: version(conflict.our.as_ref()),
                theirs: version(conflict.their.as_ref()),
            };
            save_versions(repo, path, &sides, save_to)
        }
        ResolutionStrategy::MarkResolved => {
            let workdir_file = workdir_path(repo, path)?;
            if workdir_file.map(|file| file.exists()).unwrap_or(false) {
                index.add_path(Path::new(path))?;
            } else {
                // The user deleted the file: that *is* the resolution.
                index.conflict_remove(Path::new(path))?;
                index.remove_path(Path::new(path))?;
            }
            index.write()?;
            Ok(())
        }
        ResolutionStrategy::Ours
        | ResolutionStrategy::Theirs
        | ResolutionStrategy::Base
        | ResolutionStrategy::Both
        | ResolutionStrategy::Manual { .. } => {
            // `IndexEntry` is not `Clone`; only the mode is needed, and the
            // content comes from the blobs by way of the sides below.
            let mode = match strategy {
                ResolutionStrategy::Ours => conflict.our.as_ref().map(|entry| entry.mode),
                ResolutionStrategy::Theirs => conflict.their.as_ref().map(|entry| entry.mode),
                ResolutionStrategy::Base => conflict.ancestor.as_ref().map(|entry| entry.mode),
                ResolutionStrategy::Both | ResolutionStrategy::Manual { .. } => conflict
                    .our
                    .as_ref()
                    .or(conflict.their.as_ref())
                    .map(|entry| entry.mode),
                _ => None,
            }
            .unwrap_or(0o100644);

            let content: Option<Vec<u8>> = match strategy {
                ResolutionStrategy::Manual { content } => Some(content.as_bytes().to_vec()),
                ResolutionStrategy::Both => {
                    let sides = ConflictSides {
                        base: version(conflict.ancestor.as_ref()),
                        ours: version(conflict.our.as_ref()),
                        theirs: version(conflict.their.as_ref()),
                    };
                    Some(union_merge(repo, &sides)?)
                }
                ResolutionStrategy::Ours => stage_content(repo, conflict.our.as_ref())?,
                ResolutionStrategy::Theirs => stage_content(repo, conflict.their.as_ref())?,
                ResolutionStrategy::Base => {
                    if conflict.ancestor.is_none() {
                        return Err(ErrorPayload::new(
                            ErrorCode::InvalidRequest,
                            "This conflict has no base version to take",
                        )
                        .with_param("path", path));
                    }
                    stage_content(repo, conflict.ancestor.as_ref())?
                }
                _ => unreachable!(),
            };

            stage_resolution(repo, &mut index, path, content, mode)
        }
    }
}

/// Resolve a text conflict hunk by hunk. Every conflict region of the file
/// must be covered by exactly one [`HunkResolution`] — a partial answer is an
/// error, never a half-written file.
pub fn resolve_hunks(
    repo: &git2::Repository,
    path: &str,
    resolutions: &[HunkResolution],
) -> Result<(), ErrorPayload> {
    if resolutions.is_empty() {
        return Err(
            ErrorPayload::new(ErrorCode::InvalidRequest, "No hunk resolutions were given")
                .with_param("path", path),
        );
    }

    let mut index = repo.index()?;
    let conflict = index.conflict_get(Path::new(path)).map_err(|_| {
        ErrorPayload::new(ErrorCode::InvalidRequest, "Path is not an unmerged entry")
            .with_param("path", path)
    })?;
    let sides = ConflictSides {
        base: version(conflict.ancestor.as_ref()),
        ours: version(conflict.our.as_ref()),
        theirs: version(conflict.their.as_ref()),
    };

    let segments = merged_segments(repo, &sides)?.ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This conflict cannot be resolved hunk by hunk",
        )
        .with_param("path", path)
    })?;

    let content = assemble(&segments, resolutions)?;
    let mode = conflict
        .our
        .as_ref()
        .or(conflict.their.as_ref())
        .map(|entry| entry.mode)
        .unwrap_or(0o100644);
    stage_resolution(repo, &mut index, path, Some(content), mode)
}

// ---------------------------------------------------------------------------
// Conflict internals
// ---------------------------------------------------------------------------

fn conflict_path(conflict: &git2::IndexConflict) -> Option<String> {
    let entry = conflict
        .our
        .as_ref()
        .or(conflict.their.as_ref())
        .or(conflict.ancestor.as_ref())?;
    Some(String::from_utf8_lossy(&entry.path).into_owned())
}

fn classify(base: bool, ours: bool, theirs: bool) -> ConflictKind {
    match (base, ours, theirs) {
        (true, true, true) => ConflictKind::BothModified,
        (false, true, true) => ConflictKind::BothAdded,
        (true, false, false) => ConflictKind::BothDeleted,
        (true, true, false) => ConflictKind::DeletedByThem,
        (true, false, true) => ConflictKind::DeletedByUs,
        (false, true, false) => ConflictKind::AddedByUs,
        (false, false, true) => ConflictKind::AddedByThem,
        (false, false, false) => ConflictKind::BothDeleted,
    }
}

fn version(entry: Option<&IndexEntry>) -> Option<ConflictVersion> {
    entry.map(|entry| ConflictVersion {
        oid: entry.id.to_string(),
        mode: entry.mode,
        size: u64::from(entry.file_size),
    })
}

fn workdir_path(
    repo: &git2::Repository,
    path: &str,
) -> Result<Option<std::path::PathBuf>, ErrorPayload> {
    Ok(repo.workdir().map(|workdir| workdir.join(path)))
}

/// Content of one stage, when it is a blob. `None` means "no content": an
/// absent stage, or a gitlink (submodule) whose entry is a commit id.
fn stage_content(
    repo: &git2::Repository,
    entry: Option<&IndexEntry>,
) -> Result<Option<Vec<u8>>, ErrorPayload> {
    match entry {
        None => Ok(None),
        Some(entry) if is_gitlink(entry.mode) => Ok(None),
        Some(entry) => {
            let blob = repo.find_blob(entry.id)?;
            Ok(Some(blob.content().to_vec()))
        }
    }
}

fn is_gitlink(mode: u32) -> bool {
    mode & 0o170000 == 0o160000
}

fn is_symlink(mode: u32) -> bool {
    mode & 0o170000 == 0o120000
}

/// Binary per git's heuristic: a NUL byte in the first 8000 bytes.
fn looks_binary(content: &[u8]) -> bool {
    content[..content.len().min(8000)].contains(&0)
}

/// A three-way merge of the text sides, diff3-style with our own labels.
struct MergedText {
    content: Vec<u8>,
    automergeable: bool,
}

fn merge_text_sides(
    repo: &git2::Repository,
    sides: &ConflictSides,
    favor: Option<git2::FileFavor>,
) -> Result<Option<MergedText>, ErrorPayload> {
    let ours = stage_bytes(repo, sides.ours.as_ref())?;
    let theirs = stage_bytes(repo, sides.theirs.as_ref())?;
    let base = stage_bytes(repo, sides.base.as_ref())?.unwrap_or_default();
    let (Some(ours), Some(theirs)) = (ours, theirs) else {
        // A side without content is not a text merge: choose a side instead.
        return Ok(None);
    };
    if looks_binary(&ours) || looks_binary(&theirs) || looks_binary(&base) {
        return Ok(None);
    }

    let mut base_input = git2::MergeFileInput::new();
    base_input.content(&base).path("base");
    let mut ours_input = git2::MergeFileInput::new();
    ours_input.content(&ours).path("ours");
    let mut theirs_input = git2::MergeFileInput::new();
    theirs_input.content(&theirs).path("theirs");

    let mut opts = git2::MergeFileOptions::new();
    opts.style_diff3(true)
        .ancestor_label("base")
        .our_label("ours")
        .their_label("theirs");
    if let Some(favor) = favor {
        opts.favor(favor);
    }

    let result = git2::merge_file(&base_input, &ours_input, &theirs_input, Some(&mut opts))?;
    Ok(Some(MergedText {
        content: result.content().to_vec(),
        automergeable: result.is_automergeable(),
    }))
}

fn stage_bytes(
    repo: &git2::Repository,
    entry: Option<&ConflictVersion>,
) -> Result<Option<Vec<u8>>, ErrorPayload> {
    match entry {
        None => Ok(None),
        Some(entry) if is_gitlink(entry.mode) => Ok(None),
        Some(entry) => {
            let oid = Oid::from_str(&entry.oid)?;
            Ok(Some(repo.find_blob(oid)?.content().to_vec()))
        }
    }
}

/// The merged output of a text conflict as segments: runs of file text and
/// conflict regions in output order.
enum Segment {
    Text(String),
    Region {
        ours: String,
        base: String,
        theirs: String,
    },
}

fn merged_segments(
    repo: &git2::Repository,
    sides: &ConflictSides,
) -> Result<Option<Vec<Segment>>, ErrorPayload> {
    let Some(merged) = merge_text_sides(repo, sides, None)? else {
        return Ok(None);
    };
    if merged.automergeable {
        // No conflict regions: the entry is unmerged for another reason
        // (mode/type), and hunk resolution does not apply.
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&merged.content).into_owned();
    Ok(Some(parse_regions(&text)?))
}

fn parse_regions(text: &str) -> Result<Vec<Segment>, ErrorPayload> {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        Text,
        Ours,
        Base,
        Theirs,
    }

    let mut segments = Vec::new();
    let mut phase = Phase::Text;
    let mut text_buf = String::new();
    let mut ours = String::new();
    let mut base = String::new();
    let mut theirs = String::new();

    for line in text.split_inclusive('\n') {
        match phase {
            Phase::Text => {
                if line.starts_with("<<<<<<< ours") {
                    if !text_buf.is_empty() {
                        segments.push(Segment::Text(std::mem::take(&mut text_buf)));
                    }
                    phase = Phase::Ours;
                } else {
                    text_buf.push_str(line);
                }
            }
            Phase::Ours => {
                if line.starts_with("||||||| base") {
                    phase = Phase::Base;
                } else if line == "=======\n" || line == "=======" {
                    phase = Phase::Theirs;
                } else {
                    ours.push_str(line);
                }
            }
            Phase::Base => {
                if line == "=======\n" || line == "=======" {
                    phase = Phase::Theirs;
                } else {
                    base.push_str(line);
                }
            }
            Phase::Theirs => {
                if line.starts_with(">>>>>>> theirs") {
                    segments.push(Segment::Region {
                        ours: std::mem::take(&mut ours),
                        base: std::mem::take(&mut base),
                        theirs: std::mem::take(&mut theirs),
                    });
                    phase = Phase::Text;
                } else {
                    theirs.push_str(line);
                }
            }
        }
    }

    if phase != Phase::Text {
        return Err(ErrorPayload::new(
            ErrorCode::Unclassified,
            "The merged output is malformed (unterminated conflict region)",
        ));
    }
    if !text_buf.is_empty() {
        segments.push(Segment::Text(text_buf));
    }
    Ok(segments)
}

fn segments_to_hunks(segments: &[Segment]) -> Vec<ConflictHunk> {
    let mut hunks = Vec::new();
    for segment in segments {
        if let Segment::Region { ours, base, theirs } = segment {
            hunks.push(ConflictHunk {
                index: hunks.len(),
                ours: ours.clone(),
                base: base.clone(),
                theirs: theirs.clone(),
            });
        }
    }
    hunks
}

fn assemble(segments: &[Segment], resolutions: &[HunkResolution]) -> Result<Vec<u8>, ErrorPayload> {
    let mut out = Vec::new();
    let mut index = 0usize;

    for segment in segments {
        match segment {
            Segment::Text(text) => out.extend_from_slice(text.as_bytes()),
            Segment::Region { ours, base, theirs } => {
                let resolution = resolutions.iter().find(|r| r.hunk == index);
                let Some(resolution) = resolution else {
                    return Err(ErrorPayload::new(
                        ErrorCode::InvalidRequest,
                        format!("Hunk {index} has no resolution"),
                    )
                    .with_param("hunk", index.to_string()));
                };
                let text = match &resolution.choice {
                    HunkChoice::Ours => ours,
                    HunkChoice::Theirs => theirs,
                    HunkChoice::Base => base,
                    HunkChoice::Manual { content } => content,
                };
                out.extend_from_slice(text.as_bytes());
                index += 1;
            }
        }
    }

    let unresolved: Vec<usize> = resolutions
        .iter()
        .filter(|r| r.hunk >= index)
        .map(|r| r.hunk)
        .collect();
    if !unresolved.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("Hunk {} does not exist in this file", unresolved[0]),
        )
        .with_param("hunk", unresolved[0].to_string()));
    }

    Ok(out)
}

/// The merged text of one conflicted path and the hunks it still contains, or
/// `None` when the path cannot be text-merged at all.
type TextMerge = Option<(Vec<u8>, Vec<ConflictHunk>)>;

/// The merged text of one conflicted path plus its hunks.
fn text_merge(repo: &git2::Repository, sides: &ConflictSides) -> Result<TextMerge, ErrorPayload> {
    let Some(merged) = merge_text_sides(repo, sides, None)? else {
        return Ok(None);
    };
    if merged.automergeable {
        return Ok(Some((merged.content, Vec::new())));
    }
    let text = String::from_utf8_lossy(&merged.content).into_owned();
    let segments = parse_regions(&text)?;
    Ok(Some((merged.content, segments_to_hunks(&segments))))
}

fn text_merge_for_path(
    repo: &git2::Repository,
    path: &str,
) -> Result<(Vec<u8>, Vec<ConflictHunk>), ErrorPayload> {
    let index = repo.index()?;
    let conflict = index.conflict_get(Path::new(path)).map_err(|_| {
        ErrorPayload::new(ErrorCode::InvalidRequest, "Path is not an unmerged entry")
            .with_param("path", path)
    })?;
    let sides = ConflictSides {
        base: version(conflict.ancestor.as_ref()),
        ours: version(conflict.our.as_ref()),
        theirs: version(conflict.their.as_ref()),
    };
    text_merge(repo, &sides)?.ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This conflict has no text hunks (binary or type conflict)",
        )
        .with_param("path", path)
    })
}

fn union_merge(repo: &git2::Repository, sides: &ConflictSides) -> Result<Vec<u8>, ErrorPayload> {
    let Some(merged) = merge_text_sides(repo, sides, Some(git2::FileFavor::Union))? else {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The 'both' strategy needs text on both sides",
        ));
    };
    Ok(merged.content)
}

/// Collapse the conflict stages of `path` and stage the resolved content.
/// `None` means the resolution deletes the file.
fn stage_resolution(
    repo: &git2::Repository,
    index: &mut git2::Index,
    path: &str,
    content: Option<Vec<u8>>,
    mode: u32,
) -> Result<(), ErrorPayload> {
    index.conflict_remove(Path::new(path))?;

    match content {
        Some(bytes) => {
            let file_mode = if is_symlink(mode) {
                0o120000
            } else if mode & 0o111 != 0 {
                0o100755
            } else {
                0o100644
            };

            if let Some(workdir) = repo.workdir() {
                let target = workdir.join(path);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| {
                        ErrorPayload::new(
                            ErrorCode::Io,
                            format!("Could not create {}: {err}", parent.display()),
                        )
                    })?;
                }
                // Replace whatever the checkout left (possibly a symlink).
                if let Ok(meta) = std::fs::symlink_metadata(&target) {
                    if meta.file_type().is_symlink() || meta.is_file() {
                        let _ = std::fs::remove_file(&target);
                    }
                }
                if file_mode == 0o120000 {
                    let link = String::from_utf8_lossy(&bytes).into_owned();
                    let _ = std::fs::remove_file(&target);
                    std::os::unix::fs::symlink(link, &target).map_err(|err| {
                        ErrorPayload::new(
                            ErrorCode::Io,
                            format!("Could not write symlink {}: {err}", target.display()),
                        )
                    })?;
                } else {
                    std::fs::write(&target, &bytes).map_err(|err| {
                        ErrorPayload::new(
                            ErrorCode::Io,
                            format!("Could not write {}: {err}", target.display()),
                        )
                    })?;
                }
            }

            let entry = IndexEntry {
                ctime: IndexTime::new(0, 0),
                mtime: IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: file_mode,
                uid: 0,
                gid: 0,
                file_size: bytes.len() as u32,
                // The oid is computed by `add_frombuffer` from the content.
                id: Oid::from_bytes(&[0u8; 20]).expect("zero oid is valid"),
                flags: 0,
                flags_extended: 0,
                path: path.as_bytes().to_vec(),
            };
            index.add_frombuffer(&entry, &bytes)?;
        }
        None => {
            if let Some(workdir) = repo.workdir() {
                let target = workdir.join(path);
                let _ = std::fs::remove_file(&target);
            }
        }
    }

    index.write()?;
    Ok(())
}

fn save_versions(
    repo: &git2::Repository,
    path: &str,
    sides: &ConflictSides,
    save_to: Option<&str>,
) -> Result<(), ErrorPayload> {
    let Some(workdir) = repo.workdir() else {
        return Ok(());
    };
    let target = match save_to {
        Some(save_to) => {
            let candidate = std::path::PathBuf::from(save_to);
            if candidate.is_absolute() {
                candidate
            } else {
                workdir.join(candidate)
            }
        }
        None => workdir.join(path),
    };
    let mut wrote = false;

    for (side, version) in [
        ("ours", &sides.ours),
        ("theirs", &sides.theirs),
        ("base", &sides.base),
    ] {
        let Some(version) = version else { continue };
        let oid = Oid::from_str(&version.oid)?;
        let blob = repo.find_blob(oid)?;
        let backup = std::path::PathBuf::from(format!("{}.{side}", target.display()));
        std::fs::write(&backup, blob.content()).map_err(|err| {
            ErrorPayload::new(
                ErrorCode::Io,
                format!("Could not write {}: {err}", backup.display()),
            )
        })?;
        wrote = true;
    }

    if !wrote {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This conflict has no versions to save",
        )
        .with_param("path", path));
    }
    Ok(())
}

/// Small ergonomic shim so outcome construction stays a single expression.
impl IntegrationResult {
    pub(crate) fn also_notes(mut self, notes: Vec<String>) -> Self {
        self.notes = notes;
        self
    }
}
