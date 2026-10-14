//! The `integrate` command surface (docs/03 §13) plus the merge-status and
//! conflict rows from §3 and the branch-update/restack rows from §6.
//!
//! Every command is the same triad: an argument struct, a pure `validate()`
//! (working copy present, no integration already in flight), and an
//! `execute()` that delegates to the engine and emits the events the UI
//! listens for. Conflict resolution returns the fresh `StatusDelta` so the
//! conflict list and the file list stay in step without a second round-trip.

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx, Risk};
use crate::domain::conflict::{
    ConflictEntry, ConflictHunk, HunkResolution, IntegrationResult, MergeStatus, MergeStrategy,
    ResetMode, ResetResult, ResolutionStrategy, RestackReport, RewriteImpact, SequencePlan,
    ToolReport,
};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::revision::Revision;
use crate::domain::status::StatusDelta;
use crate::engine;
use crate::engine::merge::MergeRequest;
use crate::engine::rebase::RebaseRequest;
use crate::events::{self, AppEvent};
use crate::state::AppState;
use crate::validation::{self, ValidateCx, Violation};

// ---------------------------------------------------------------------------
// Shared validation
// ---------------------------------------------------------------------------

fn require_no_integration(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    validation::working_copy::validate_working_copy(cx)?;
    if engine::merge::integration_in_progress(cx.repo) {
        return Err(Violation::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }
    Ok(())
}

fn require_integration(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    if !engine::merge::integration_in_progress(cx.repo) {
        return Err(Violation::new(
            ErrorCode::OperationInProgress,
            "No integration is in progress",
        ));
    }
    Ok(())
}

fn emit_integration(cx: &ExecCx<'_>) {
    events::emit(
        cx.app,
        AppEvent::WorkingCopyChanged {
            repo: cx.repo_id.clone(),
        },
    );
    events::emit(
        cx.app,
        AppEvent::RefsChanged {
            repo: cx.repo_id.clone(),
        },
    );
}

fn status_delta(cx: &ExecCx<'_>) -> Result<StatusDelta, ErrorPayload> {
    engine::reset::status_delta(cx.repo)
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MergeOptions {
    /// `--no-ff`: always create a merge commit.
    pub always_merge_commit: bool,
    /// Commit when the merge is clean; with `false` the staged result waits
    /// for `integration_continue`.
    pub auto_commit: bool,
    pub verify_signatures: bool,
    pub autostash: bool,
    /// `--squash`: stage the merged result and stop.
    pub squash: bool,
    pub strategy: MergeStrategy,
    pub message: Option<String>,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            always_merge_commit: false,
            auto_commit: true,
            verify_signatures: false,
            autostash: false,
            squash: false,
            strategy: MergeStrategy::default(),
            message: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStart {
    pub repo: RepoId,
    pub revision: Revision,
    #[serde(default)]
    pub opts: MergeOptions,
}

impl Command for MergeStart {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let req = MergeRequest {
            no_ff: self.opts.always_merge_commit,
            squash: self.opts.squash,
            commit: self.opts.auto_commit,
            message: self.opts.message.clone(),
            strategy: self.opts.strategy,
            verify_signatures: self.opts.verify_signatures,
            autostash: self.opts.autostash,
            find_renames: true,
        };
        let result = engine::merge::start(cx.repo, &self.revision, &req)?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStatusLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub revision: Option<Revision>,
}

impl Command for MergeStatusLoad {
    type Output = MergeStatus;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::merge::status(cx.repo, self.revision.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeAbort {
    pub repo: RepoId,
}

impl Command for MergeAbort {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Destructive
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let mut notes = Vec::new();
        // Mode-aware: a rebase/cherry-pick sequence is aborted by its own
        // engine; only a plain merge goes through the merge path.
        if engine::rebase::sequence_state(cx.repo).is_some() {
            engine::rebase::abort(cx.repo, &mut notes)?;
        } else if cx.repo.state() == git2::RepositoryState::Merge {
            engine::merge::abort_merge(cx.repo, &mut notes)?;
        } else {
            // A git-native cherry-pick/revert left CHERRY_PICK_HEAD or
            // REVERT_HEAD: drop the state files and roll back to ORIG_HEAD.
            let target = engine::merge::read_pseudoref(cx.repo, "ORIG_HEAD")
                .or_else(|| cx.repo.head().ok().and_then(|head| head.target()))
                .ok_or_else(|| {
                    ErrorPayload::new(
                        ErrorCode::OperationInProgress,
                        "No integration is in progress",
                    )
                })?;
            cx.repo.cleanup_state()?;
            let object = cx.repo.find_object(target, None)?;
            cx.repo.reset(&object, git2::ResetType::Hard, None)?;
        }
        emit_integration(cx);
        status_delta(cx)
    }
}

// ---------------------------------------------------------------------------
// Rebase
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RebaseOptions {
    pub autostash: bool,
    pub verify_signatures: bool,
}

impl From<RebaseOptions> for RebaseRequest {
    fn from(opts: RebaseOptions) -> Self {
        Self {
            autostash: opts.autostash,
            verify_signatures: opts.verify_signatures,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStart {
    pub repo: RepoId,
    pub upstream: Revision,
    #[serde(default)]
    pub onto: Option<Revision>,
    #[serde(default)]
    pub opts: RebaseOptions,
}

impl Command for RebaseStart {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::start(
            cx.repo,
            &self.upstream,
            self.onto.as_ref(),
            &self.opts.into(),
        )?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseInteractivePlan {
    pub repo: RepoId,
    pub upstream: Revision,
    #[serde(default)]
    pub onto: Option<Revision>,
}

impl Command for RebaseInteractivePlan {
    type Output = SequencePlan;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::rebase::plan(cx.repo, &self.upstream, self.onto.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseInteractiveStart {
    pub repo: RepoId,
    pub plan: SequencePlan,
    #[serde(default)]
    pub opts: RebaseOptions,
}

impl Command for RebaseInteractiveStart {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::start_interactive(cx.repo, &self.plan, &self.opts.into())?;
        emit_integration(cx);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Cherry-pick / revert
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CherryPickOptions {
    pub ff: bool,
    pub always_merge_commit: bool,
    pub no_commit: bool,
    pub record_origin: bool,
    pub mainline: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CherryPick {
    pub repo: RepoId,
    pub revisions: Vec<Revision>,
    #[serde(default)]
    pub opts: CherryPickOptions,
}

impl Command for CherryPick {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let req = engine::cherry::CherryPickRequest {
            ff: self.opts.ff,
            always_merge_commit: self.opts.always_merge_commit,
            no_commit: self.opts.no_commit,
            record_origin: self.opts.record_origin,
            mainline: self.opts.mainline,
        };
        let result = engine::cherry::cherry_pick(cx.repo, &self.revisions, &req)?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RevertOptions {
    pub commit_after: bool,
    pub mainline: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revert {
    pub repo: RepoId,
    pub revisions: Vec<Revision>,
    #[serde(default)]
    pub opts: RevertOptions,
}

impl Command for Revert {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let req = engine::cherry::RevertRequest {
            commit_after: self.opts.commit_after,
            mainline: self.opts.mainline,
        };
        let result = engine::cherry::revert(cx.repo, &self.revisions, &req)?;
        emit_integration(cx);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResetOptions {
    pub confirm: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetToRevision {
    pub repo: RepoId,
    pub revision: Revision,
    #[serde(default)]
    pub mode: ResetMode,
    #[serde(default)]
    pub opts: ResetOptions,
}

impl Command for ResetToRevision {
    type Output = ResetResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        if self.mode == ResetMode::Hard {
            Risk::Destructive
        } else {
            Risk::Rewrite
        }
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)?;
        if self.mode == ResetMode::Hard && !self.opts.confirm {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "A hard reset discards working-tree changes and must be confirmed",
            ));
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::reset::reset_to_revision(
            cx.repo,
            &self.revision,
            self.mode,
            self.opts.confirm,
        )?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetEntriesToRevision {
    pub repo: RepoId,
    pub paths: Vec<String>,
    pub revision: Revision,
}

impl Command for ResetEntriesToRevision {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let delta = engine::reset::reset_entries_to_revision(cx.repo, &self.paths, &self.revision)?;
        emit_integration(cx);
        Ok(delta)
    }
}

// ---------------------------------------------------------------------------
// Rewrite commands
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewritePlanArgs {
    pub branch: String,
    pub revisions: Vec<Revision>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteImpactLoad {
    pub repo: RepoId,
    pub plan: RewritePlanArgs,
}

impl Command for RewriteImpactLoad {
    type Output = RewriteImpact;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::rebase::rewrite_impact(cx.repo, &self.plan.branch, &self.plan.revisions)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquashRevisions {
    pub repo: RepoId,
    pub base: Revision,
    pub tips: Vec<Revision>,
    #[serde(default)]
    pub message: Option<String>,
}

impl Command for SquashRevisions {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::squash_revisions(
            cx.repo,
            &self.base,
            &self.tips,
            self.message.as_deref(),
        )?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixupRevisions {
    pub repo: RepoId,
    pub target: Revision,
    pub fixups: Vec<Revision>,
}

impl Command for FixupRevisions {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::fixup_revisions(cx.repo, &self.target, &self.fixups)?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveMeta {
    pub onto: String,
    pub direction: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRevisions {
    pub repo: RepoId,
    pub revisions: Vec<Revision>,
    pub meta: MoveMeta,
}

impl Command for MoveRevisions {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::move_revisions(
            cx.repo,
            &self.revisions,
            &self.meta.onto,
            &self.meta.direction,
        )?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRevisions {
    pub repo: RepoId,
    pub revisions: Vec<Revision>,
}

impl Command for DeleteRevisions {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::delete_revisions(cx.repo, &self.revisions)?;
        emit_integration(cx);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Continue / skip
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationContinue {
    pub repo: RepoId,
    #[serde(default)]
    pub message: Option<String>,
}

impl Command for IntegrationContinue {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = if engine::rebase::sequence_state(cx.repo).is_some() {
            engine::rebase::continue_(cx.repo)?
        } else if cx.repo.state() == git2::RepositoryState::Merge {
            let mut notes = Vec::new();
            engine::merge::continue_merge(cx.repo, self.message.as_deref(), &mut notes)?
        } else {
            return Err(ErrorPayload::new(
                ErrorCode::OperationInProgress,
                "No integration is in progress",
            ));
        };
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSkip {
    pub repo: RepoId,
}

impl Command for IntegrationSkip {
    type Output = IntegrationResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::rebase::skip(cx.repo)?;
        emit_integration(cx);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcConflictsLoad {
    pub repo: RepoId,
}

impl Command for WcConflictsLoad {
    type Output = Vec<ConflictEntry>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::merge::conflicts(cx.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcConflictResolve {
    pub repo: RepoId,
    pub path: String,
    pub strategy: ResolutionStrategy,
    #[serde(default)]
    pub save_to: Option<String>,
}

impl Command for WcConflictResolve {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::merge::resolve_file(cx.repo, &self.path, &self.strategy, self.save_to.as_deref())?;
        emit_integration(cx);
        status_delta(cx)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictHunksLoad {
    pub repo: RepoId,
    pub path: String,
}

impl Command for ConflictHunksLoad {
    type Output = Vec<ConflictHunk>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::merge::hunks(cx.repo, &self.path)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolveHunks {
    pub repo: RepoId,
    pub path: String,
    pub resolutions: Vec<HunkResolution>,
}

impl Command for ConflictResolveHunks {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::merge::resolve_hunks(cx.repo, &self.path, &self.resolutions)?;
        emit_integration(cx);
        status_delta(cx)
    }
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeToolInvoke {
    pub repo: RepoId,
    pub paths: Vec<String>,
}

impl Command for MergeToolInvoke {
    type Output = ToolReport;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = run_merge_tool(cx.repo, &self.paths)?;
        emit_integration(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiffToolOptions {
    pub directory_mode: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffToolInvoke {
    pub repo: RepoId,
    pub left: String,
    #[serde(default)]
    pub right: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub opts: DiffToolOptions,
}

impl Command for DiffToolInvoke {
    type Output = ToolReport;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.left.is_empty() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "No left revision was given",
            ));
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        run_diff_tool(
            cx.repo,
            &self.left,
            self.right.as_deref(),
            &self.paths,
            self.opts.directory_mode,
        )
    }
}

/// The configured merge tool, or `mergetool`-style `opendiff` as a fallback.
fn tool_command(repo: &git2::Repository, kind: &str) -> (String, Option<String>) {
    let config = repo.config().ok();
    let key = if kind == "merge" {
        "merge.tool"
    } else {
        "diff.tool"
    };
    let tool = config
        .as_ref()
        .and_then(|config| config.get_string(key).ok())
        .unwrap_or_else(|| {
            if kind == "merge" {
                "mergetool".to_string()
            } else {
                "opendiff".to_string()
            }
        });
    let cmd = config
        .as_ref()
        .and_then(|config| config.get_string(&format!("{kind}tool.{tool}.cmd")).ok())
        .or_else(|| {
            config
                .as_ref()
                .and_then(|config| config.get_string(&format!("{kind}.toolcmd")).ok())
        });
    (tool, cmd)
}

fn run_merge_tool(repo: &git2::Repository, paths: &[String]) -> Result<ToolReport, ErrorPayload> {
    let index = repo.index()?;
    if !index.has_conflicts() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "There are no unmerged entries for a merge tool",
        ));
    }
    let (tool, cmd_template) = tool_command(repo, "merge");

    let mut staged = Vec::new();
    let mut last_args = Vec::new();
    let mut last_exit = None;

    for path in paths {
        let conflict = index
            .conflict_get(std::path::Path::new(path))
            .map_err(|_| {
                ErrorPayload::new(ErrorCode::InvalidRequest, "Path is not an unmerged entry")
                    .with_param("path", path)
            })?;
        let dir = std::env::temp_dir().join(format!("meridian-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("Could not create temp dir: {err}"))
        })?;
        let base = write_side(repo, &dir, "BASE", "base", conflict.ancestor.as_ref(), "")?;
        let local = write_side(repo, &dir, "LOCAL", "ours", conflict.our.as_ref(), path)?;
        let remote = write_side(repo, &dir, "REMOTE", "theirs", conflict.their.as_ref(), "")?;
        let merged = repo.workdir().map(|workdir| workdir.join(path));

        let args = match &cmd_template {
            Some(template) => expand_template(
                template,
                &base,
                &local,
                &remote,
                merged.as_deref().and_then(|p| p.to_str()).unwrap_or(path),
            ),
            None => vec![local.clone(), remote.clone()],
        };
        last_args = args.clone();
        let (program, rest) = split_program(&args);
        let status = std::process::Command::new(&program)
            .args(&rest)
            .status()
            .map_err(|err| {
                ErrorPayload::new(
                    ErrorCode::Io,
                    format!("Could not launch the merge tool '{tool}': {err}"),
                )
            })?;
        last_exit = status.code();

        if status.success() {
            let mut guard = repo.index()?;
            guard.add_path(std::path::Path::new(path))?;
            guard.write()?;
            staged.push(path.clone());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    Ok(ToolReport {
        tool,
        args: last_args,
        exit_code: last_exit,
        staged,
    })
}

fn run_diff_tool(
    repo: &git2::Repository,
    left: &str,
    right: Option<&str>,
    paths: &[String],
    directory_mode: bool,
) -> Result<ToolReport, ErrorPayload> {
    let (tool, cmd_template) = tool_command(repo, "diff");
    let workdir = repo.workdir().map(|path| path.to_path_buf());
    let left_path = workdir
        .as_ref()
        .map(|workdir| workdir.join(left))
        .unwrap_or_else(|| std::path::PathBuf::from(left));

    let args = match &cmd_template {
        Some(template) => expand_template(
            template,
            right.unwrap_or(left),
            &left_path.to_string_lossy(),
            right.unwrap_or(""),
            "",
        ),
        None => {
            let mut args = vec![left_path.to_string_lossy().into_owned()];
            if let Some(right) = right {
                let right_path = workdir
                    .as_ref()
                    .map(|workdir| workdir.join(right))
                    .unwrap_or_else(|| std::path::PathBuf::from(right));
                args.push(right_path.to_string_lossy().into_owned());
            }
            if directory_mode {
                args.insert(0, "-d".to_string());
            }
            if !paths.is_empty() {
                args.extend(paths.iter().cloned());
            }
            args
        }
    };

    let (program, rest) = split_program(&args);
    let status = std::process::Command::new(&program)
        .args(&rest)
        .status()
        .map_err(|err| {
            ErrorPayload::new(
                ErrorCode::Io,
                format!("Could not launch the diff tool '{tool}': {err}"),
            )
        })?;

    Ok(ToolReport {
        tool,
        args,
        exit_code: status.code(),
        staged: Vec::new(),
    })
}

/// Write one conflict side to a temp file; returns its path as a string.
fn write_side(
    repo: &git2::Repository,
    dir: &std::path::Path,
    env_name: &str,
    side: &str,
    version: Option<&git2::IndexEntry>,
    fallback: &str,
) -> Result<String, ErrorPayload> {
    let path = dir.join(format!("{env_name}.{side}"));
    let content = match version {
        Some(entry) => repo.find_blob(entry.id)?.content().to_vec(),
        None => Vec::new(),
    };
    let _ = fallback;
    std::fs::write(&path, content).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write side file: {err}"))
    })?;
    Ok(path.to_string_lossy().into_owned())
}

/// Substitute the `$BASE/$LOCAL/$REMOTE/$MERGED` placeholders in a tool
/// command template and return the argv it becomes.
fn expand_template(
    template: &str,
    base: &str,
    local: &str,
    remote: &str,
    merged: &str,
) -> Vec<String> {
    template
        .split_whitespace()
        .map(|token| {
            token
                .replace("$BASE", base)
                .replace("$LOCAL", local)
                .replace("$REMOTE", remote)
                .replace("$MERGED", merged)
        })
        .collect()
}

fn split_program(args: &[String]) -> (String, Vec<String>) {
    let mut iter = args.iter();
    let program = iter.next().cloned().unwrap_or_default();
    (program, iter.cloned().collect())
}

// ---------------------------------------------------------------------------
// Branch update / restack
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchUpdate {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub opts: RebaseOptions,
}

impl Command for BranchUpdate {
    type Output = engine::rebase::UpdateResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let mode = if self.mode.is_empty() {
            "Merge".to_string()
        } else {
            self.mode
        };
        let result =
            engine::rebase::update_branch(cx.repo, &self.name, &mode, self.opts.autostash)?;
        emit_integration(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchStackRestack {
    pub repo: RepoId,
    #[serde(default)]
    pub names: Vec<String>,
}

impl Command for BranchStackRestack {
    type Output = RestackReport;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_no_integration(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = engine::rebase::restack(cx.repo, &self.names)?;
        emit_integration(cx);
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn merge_start(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MergeStart,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn merge_status_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MergeStatusLoad,
) -> Result<MergeStatus, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_merge_status_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MergeStatusLoad,
) -> Result<MergeStatus, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn merge_abort(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MergeAbort,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn rebase_start(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RebaseStart,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn rebase_interactive_plan(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RebaseInteractivePlan,
) -> Result<SequencePlan, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn rebase_interactive_start(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RebaseInteractiveStart,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn cherry_pick(
    state: State<'_, AppState>,
    app: AppHandle,
    args: CherryPick,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn revert(
    state: State<'_, AppState>,
    app: AppHandle,
    args: Revert,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn reset_to_revision(
    state: State<'_, AppState>,
    app: AppHandle,
    args: ResetToRevision,
) -> Result<ResetResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn reset_entries_to_revision(
    state: State<'_, AppState>,
    app: AppHandle,
    args: ResetEntriesToRevision,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn rewrite_impact_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RewriteImpactLoad,
) -> Result<RewriteImpact, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn squash_revisions(
    state: State<'_, AppState>,
    app: AppHandle,
    args: SquashRevisions,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn fixup_revisions(
    state: State<'_, AppState>,
    app: AppHandle,
    args: FixupRevisions,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn move_revisions(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MoveRevisions,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn delete_revisions(
    state: State<'_, AppState>,
    app: AppHandle,
    args: DeleteRevisions,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn integration_continue(
    state: State<'_, AppState>,
    app: AppHandle,
    args: IntegrationContinue,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn integration_skip(
    state: State<'_, AppState>,
    app: AppHandle,
    args: IntegrationSkip,
) -> Result<IntegrationResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_conflicts_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcConflictsLoad,
) -> Result<Vec<ConflictEntry>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_conflict_resolve(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcConflictResolve,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn conflict_hunks_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: ConflictHunksLoad,
) -> Result<Vec<ConflictHunk>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn conflict_resolve_hunks(
    state: State<'_, AppState>,
    app: AppHandle,
    args: ConflictResolveHunks,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn merge_tool_invoke(
    state: State<'_, AppState>,
    app: AppHandle,
    args: MergeToolInvoke,
) -> Result<ToolReport, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn diff_tool_invoke(
    state: State<'_, AppState>,
    app: AppHandle,
    args: DiffToolInvoke,
) -> Result<ToolReport, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn branch_update(
    state: State<'_, AppState>,
    app: AppHandle,
    args: BranchUpdate,
) -> Result<engine::rebase::UpdateResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn branch_stack_restack(
    state: State<'_, AppState>,
    app: AppHandle,
    args: BranchStackRestack,
) -> Result<RestackReport, ErrorPayload> {
    state.run(&app, args).await
}
