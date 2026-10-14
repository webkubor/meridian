//! The refs command surface — docs/03 §6 branches, §7 remote branches, §8
//! remotes, §9 tags, §10 stashes, §11 submodules, §12 worktrees.
//!
//! Every verb is one [`Command`]: a serde args struct, a pure `validate`, and
//! an `execute` that calls into `engine::{branches,remotes,stash,submodules,-
//! worktrees}` and emits the domain events the stores reload on.
//!
//! # Validators live here, not in `validation/`
//!
//! `validation/` is owned by another module; the preconditions this surface
//! needs (detached HEAD, uncommitted changes, upstream divergence, missing
//! parent, unmerged delete, worktree locked/changed) are therefore written as
//! pure free functions over [`ValidateCx`] at the bottom of this file, with the
//! same `Violation` contract the `validation/` module uses. Promoting them into
//! `validation/refs.rs` later is a move, not a rewrite.
//!
//! # Error classes
//!
//! `ErrorCode` is a closed enum in `domain/error.rs` (not owned here), so the
//! observed classes that have no dedicated code — `missing_parent`,
//! `delete_unmerged_branch`, `worktree_locked`, `worktree_has_changes`,
//! `force_push_lease_mismatch`, `upstream_diverged`, … — are carried in
//! `ErrorPayload::params["class"]` (the engine sets them) alongside the closest
//! existing code. A cross-module note records the codes to add.

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx, Risk};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::refs::{BranchInfo, CheckoutResult};
use crate::domain::revision::Revision;
use crate::domain::sender::SenderTag;
use crate::engine::branches::{
    self, DeleteReport, StackedBranchInfo, TagCreateOptions, TagInfo, TagSort,
};
use crate::engine::remotes::{
    self, FetchReport, ForceWithLease, PublishReport, PullOptions, PullResult, PushReport,
    RemoteBranchInfo, RemoteFetchMode, RemoteInfo, RemoteRenameReport,
};
use crate::engine::stash::{self, StashApplyOptions, StashDetail, StashInfo, StashSaveOptions};
use crate::engine::submodules::{
    self, SubmoduleIgnoreMode, SubmoduleInfo, SubmoduleReport, SubmoduleUpdateOptions,
    SubmoduleUpdateStrategy,
};
use crate::engine::worktrees::{self, WorktreeAddOptions, WorktreeInfo};
use crate::events::{self, AppEvent};
use crate::state::AppState;
use crate::validation::{ValidateCx, Violation};

// ===========================================================================
// Branches (docs/03 §6)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchLoad {
    pub repo: RepoId,
    pub name: String,
}

impl Command for BranchLoad {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        branches::load(cx.repo, self.name.trim())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchStackedCreate {
    pub repo: RepoId,
    pub name: String,
    pub parent: String,
    #[serde(default)]
    pub checkout: bool,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for BranchStackedCreate {
    type Output = StackedBranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn sender(&self) -> SenderTag {
        self.sender
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validate_missing_parent(cx, &self.parent)?;
        if self.parent == self.name {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "A branch cannot be stacked on itself",
            )
            .with_param("name", self.name.clone()));
        }
        require_new_branch(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info =
            branches::stacked_create(cx.repo, self.name.trim(), self.parent.trim(), self.checkout)?;
        emit_refs(cx);
        if self.checkout {
            emit_working_copy(cx);
        }
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCheckoutRemote {
    pub repo: RepoId,
    pub remote_branch: String,
    #[serde(default)]
    pub local_name: Option<String>,
    #[serde(default)]
    pub track: bool,
}

impl Command for BranchCheckoutRemote {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        require_no_integration(cx)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::checkout_remote(
            cx.repo,
            self.remote_branch.trim(),
            self.local_name.as_deref(),
            self.track,
        )?;
        emit_refs(cx);
        emit_working_copy(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCheckoutRevision {
    pub repo: RepoId,
    #[serde(default)]
    pub revision: Revision,
    #[serde(default)]
    pub detach: bool,
}

impl Command for BranchCheckoutRevision {
    type Output = CheckoutResult;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        require_no_integration(cx)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = branches::checkout_revision(cx.repo, &self.revision, self.detach)?;
        emit_refs(cx);
        emit_working_copy(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRename {
    pub repo: RepoId,
    pub old: String,
    pub new: String,
    #[serde(default)]
    pub force: bool,
}

impl Command for BranchRename {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.old)?;
        require_free_name(cx, &self.new)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::rename(cx.repo, self.old.trim(), self.new.trim(), self.force)?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDelete {
    pub repo: RepoId,
    pub names: Vec<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for BranchDelete {
    type Output = DeleteReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn sender(&self) -> SenderTag {
        self.sender
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validate_unmerged_delete(cx, &self.names, self.force)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = branches::delete(cx.repo, &self.names, self.force)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchFastForward {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub to: Option<Revision>,
}

impl Command for BranchFastForward {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::fast_forward(cx.repo, self.name.trim(), self.to.as_ref())?;
        emit_refs(cx);
        emit_working_copy(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSetUpstream {
    pub repo: RepoId,
    pub name: String,
    pub upstream: String,
}

impl Command for BranchSetUpstream {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::set_upstream(cx.repo, self.name.trim(), self.upstream.trim())?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchUnsetUpstream {
    pub repo: RepoId,
    pub name: String,
}

impl Command for BranchUnsetUpstream {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        let branch = cx
            .repo
            .find_branch(&self.name, git2::BranchType::Local)
            .map_err(|_| {
                Violation::new(
                    ErrorCode::ReferenceNotFound,
                    format!("Branch '{}' does not exist", self.name),
                )
            })?;
        if branch.upstream().is_err() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("Branch '{}' is not tracking anything", self.name),
            )
            .with_param("name", self.name.clone())
            .with_param("class", "no_upstream"));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::unset_upstream(cx.repo, self.name.trim())?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchTrackRemote {
    pub repo: RepoId,
    pub local: String,
    pub remote_branch: String,
    #[serde(default)]
    pub rename: Option<String>,
}

impl Command for BranchTrackRemote {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.local)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::track_remote(
            cx.repo,
            self.local.trim(),
            self.remote_branch.trim(),
            self.rename.as_deref(),
        )?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchUntrack {
    pub repo: RepoId,
    pub name: String,
}

impl Command for BranchUntrack {
    type Output = BranchInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_branch(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::untrack(cx.repo, self.name.trim())?;
        emit_refs(cx);
        Ok(info)
    }
}

// ===========================================================================
// Remote branches (docs/03 §7)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchList {
    pub repo: RepoId,
    #[serde(default)]
    pub remote: Option<String>,
}

impl Command for RemoteBranchList {
    type Output = Vec<RemoteBranchInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::remote_branch_list(cx.repo, self.remote.as_deref())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchDelete {
    pub repo: RepoId,
    pub remote: String,
    pub names: Vec<String>,
}

impl Command for RemoteBranchDelete {
    type Output = DeleteReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = remotes::remote_branch_delete(cx.repo, self.remote.trim(), &self.names)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchPrune {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    pub remote: String,
    pub stale: Vec<String>,
    pub dry_run: bool,
}

impl Command for RemoteBranchPrune {
    type Output = PruneReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let stale = remotes::prune(cx.repo, self.remote.trim())?;
        if !self.dry_run {
            emit_refs(cx);
        }
        Ok(PruneReport {
            remote: self.remote,
            stale,
            dry_run: self.dry_run,
        })
    }
}

// ===========================================================================
// Remotes (docs/03 §8)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteList {
    pub repo: RepoId,
}

impl Command for RemoteList {
    type Output = Vec<RemoteInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::list(cx.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAdd {
    pub repo: RepoId,
    pub name: String,
    pub url: String,
}

impl Command for RemoteAdd {
    type Output = RemoteInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if !git2::Remote::is_valid_name(&self.name) {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("'{}' is not a valid remote name", self.name),
            ));
        }
        if cx.repo.find_remote(&self.name).is_ok() {
            return Err(Violation::new(
                ErrorCode::ReferenceExists,
                format!("Remote '{}' already exists", self.name),
            )
            .with_param("name", self.name.clone()));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = remotes::add(cx.repo, self.name.trim(), self.url.trim())?;
        events::emit(cx.app, AppEvent::ReposChanged);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRename {
    pub repo: RepoId,
    pub name: String,
    pub new: String,
}

impl Command for RemoteRename {
    type Output = RemoteRenameReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.name)?;
        if !git2::Remote::is_valid_name(&self.new) {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("'{}' is not a valid remote name", self.new),
            ));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = remotes::rename(cx.repo, self.name.trim(), self.new.trim())?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDelete {
    pub repo: RepoId,
    pub name: String,
}

impl Command for RemoteDelete {
    type Output = ();
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::remove(cx.repo, self.name.trim())?;
        emit_refs(cx);
        events::emit(cx.app, AppEvent::ReposChanged);
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSetUrl {
    pub repo: RepoId,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub push: bool,
}

impl Command for RemoteSetUrl {
    type Output = RemoteInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::set_url(cx.repo, self.name.trim(), self.url.trim(), self.push)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefspecDirection {
    Fetch,
    Push,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRefspecAdd {
    pub repo: RepoId,
    pub name: String,
    pub direction: RefspecDirection,
    pub spec: String,
}

impl Command for RemoteRefspecAdd {
    type Output = RemoteInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::add_refspec(
            cx.repo,
            self.name.trim(),
            self.spec.trim(),
            matches!(self.direction, RefspecDirection::Push),
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRefspecDelete {
    pub repo: RepoId,
    pub name: String,
    pub direction: RefspecDirection,
    pub spec: String,
}

impl Command for RemoteRefspecDelete {
    type Output = RemoteInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.name)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::remove_refspec(
            cx.repo,
            self.name.trim(),
            self.spec.trim(),
            matches!(self.direction, RefspecDirection::Push),
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSetFetchMode {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub mode: RemoteFetchMode,
}

impl Command for RemoteSetFetchMode {
    type Output = RemoteInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        remotes::set_fetch_mode(cx.repo, self.remote.trim(), &self.mode)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFetch {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub refspecs: Vec<String>,
    #[serde(default)]
    pub mode: RemoteFetchMode,
}

impl Command for RemoteFetch {
    type Output = FetchReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = remotes::fetch(cx.repo, self.remote.trim(), &self.refspecs, &self.mode)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePull {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub opts: PullOptions,
}

impl Command for RemotePull {
    type Output = PullResult;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        if self.opts.mode == crate::engine::remotes::PullMode::Rebase {
            Risk::Rewrite
        } else {
            Risk::None
        }
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)?;
        require_no_integration(cx)?;
        require_attached_head(cx)?;
        // A pull integrates into the branch it is pulling *for*, and that has
        // to be the checked-out branch: merge/rebase rewrite the working tree
        // as they go. Naming a different one would advance an un-checked-out
        // ref while leaving the tree on another branch — the "half-switched
        // repository" state. Refuse, as `git pull` does.
        if let Some(branch) = self.branch.as_deref() {
            let head = branches::head_branch_name(cx.repo);
            if head.as_deref() != Some(branch) {
                return Err(Violation::new(
                    ErrorCode::InvalidRequest,
                    format!(
                        "Cannot pull '{branch}' — it is not the checked-out branch. \
                         Check it out first, or omit the branch to pull the current one."
                    ),
                )
                .with_param("name", branch)
                .with_param("checkedOut", head.unwrap_or_default())
                .with_param("class", "branch_not_checked_out"));
            }
        }
        require_clean(cx)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let branch = match self.branch {
            Some(branch) => branch,
            None => branches::head_branch_name(cx.repo).ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    "HEAD is detached — name the branch to pull into",
                )
                .with_param("class", "detached_head")
            })?,
        };
        let result = remotes::pull(cx.repo, self.remote.trim(), &branch, &self.opts)?;
        emit_refs(cx);
        emit_working_copy(cx);
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePush {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub refspecs: Vec<String>,
    #[serde(default)]
    pub set_upstream: Option<String>,
}

impl Command for RemotePush {
    type Output = PushReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = remotes::push(cx.repo, self.remote.trim(), &self.refspecs)?;
        if let Some(branch) = self.set_upstream.as_deref() {
            let merge_ref = format!("refs/heads/{branch}");
            branches::set_tracking_config(cx.repo, branch, self.remote.trim(), &merge_ref)?;
        }
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteForcePush {
    pub repo: RepoId,
    pub remote: String,
    pub branch: String,
    #[serde(default)]
    pub lease: ForceWithLease,
    #[serde(default)]
    pub allow_unleased: bool,
}

impl Command for RemoteForcePush {
    type Output = PushReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Rewrite
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)?;
        require_local_branch(cx, &self.branch)?;
        if self.lease.oid.is_none() && !self.allow_unleased {
            // Without a lease oid we fall back to the remote-tracking tip, which
            // is still a lease — but an explicit acknowledgement is required for
            // a force push when that ref is absent, matching the observed
            // `GTForcePushDialogueViewController` confirmation.
            let tracking = remotes::tracking_ref(cx.repo, &self.remote, &self.branch);
            let has_tracking = tracking
                .as_deref()
                .and_then(|refname| cx.repo.find_reference(refname).ok())
                .is_some();
            if !has_tracking {
                return Err(Violation::new(
                    ErrorCode::InvalidRequest,
                    "A forced push with no lease needs explicit confirmation",
                )
                .with_param("class", "force_push_requires_lease")
                .with_param("branch", self.branch.clone()));
            }
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report =
            remotes::force_push(cx.repo, self.remote.trim(), self.branch.trim(), &self.lease)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePrune {
    pub repo: RepoId,
    pub remote: String,
}

impl Command for RemotePrune {
    type Output = PruneReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let stale = remotes::prune(cx.repo, self.remote.trim())?;
        emit_refs(cx);
        Ok(PruneReport {
            remote: self.remote,
            stale,
            dry_run: false,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePublishBranch {
    pub repo: RepoId,
    pub remote: String,
    pub branch: String,
    #[serde(default = "default_true")]
    pub set_upstream: bool,
}

impl Command for RemotePublishBranch {
    type Output = PublishReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)?;
        require_local_branch(cx, &self.branch)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = remotes::publish_branch(
            cx.repo,
            self.remote.trim(),
            self.branch.trim(),
            self.set_upstream,
        )?;
        emit_refs(cx);
        Ok(report)
    }
}

// ===========================================================================
// Tags (docs/03 §9)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagList {
    pub repo: RepoId,
    #[serde(default)]
    pub sort: TagSort,
}

impl Command for TagList {
    type Output = Vec<TagInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        branches::tag_list(cx.repo, self.sort)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDetailLoad {
    pub repo: RepoId,
    pub name: String,
}

impl Command for TagDetailLoad {
    type Output = TagInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        branches::tag_load(cx.repo, self.name.trim())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCreate {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub target: Revision,
    #[serde(default)]
    pub opts: TagCreateOptions,
}

impl Command for TagCreate {
    type Output = TagInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if !git2::Reference::is_valid_name(&format!("refs/tags/{}", self.name)) {
            return Err(Violation::new(
                ErrorCode::InvalidReferenceName,
                format!("'{}' is not a valid tag name", self.name),
            ));
        }
        if cx
            .repo
            .find_reference(&format!("refs/tags/{}", self.name))
            .is_ok()
        {
            return Err(Violation::new(
                ErrorCode::ReferenceExists,
                format!("Tag '{}' already exists", self.name),
            )
            .with_param("name", self.name.clone()));
        }
        if self.opts.sign {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "Tag signing is not available yet",
            )
            .with_param("class", "tag_sign_unavailable"));
        }
        if self.opts.annotated
            && self
                .opts
                .message
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "An annotated tag needs a message",
            )
            .with_param("name", self.name.clone()));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = branches::tag_create(cx.repo, self.name.trim(), &self.target, &self.opts)?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDelete {
    pub repo: RepoId,
    pub names: Vec<String>,
}

impl Command for TagDelete {
    type Output = DeleteReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = branches::tag_delete(cx.repo, &self.names)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagPublish {
    pub repo: RepoId,
    pub remote: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub all: bool,
}

impl Command for TagPublish {
    type Output = PushReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)?;
        if !self.all && self.names.is_empty() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "Name at least one tag, or pass all",
            ));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let refspecs = branches::tag_publish_refspecs(&self.names, self.all);
        let report = remotes::push(cx.repo, self.remote.trim(), &refspecs)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDeleteRemote {
    pub repo: RepoId,
    pub remote: String,
    pub names: Vec<String>,
}

impl Command for TagDeleteRemote {
    type Output = PushReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_remote(cx, &self.remote)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let refspecs = branches::tag_delete_refspecs(&self.names);
        let report = remotes::push(cx.repo, self.remote.trim(), &refspecs)?;
        emit_refs(cx);
        Ok(report)
    }
}

// ===========================================================================
// Stashes (docs/03 §10)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashList {
    pub repo: RepoId,
}

impl Command for StashList {
    type Output = Vec<StashInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        stash::list(cx.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashCountLoad {
    pub repo: RepoId,
}

impl Command for StashCountLoad {
    type Output = u64;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        Ok(stash::count(cx.repo)? as u64)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashSave {
    pub repo: RepoId,
    #[serde(default)]
    pub opts: StashSaveOptions,
}

impl Command for StashSave {
    type Output = StashInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        require_dirty(cx)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = stash::save(cx.repo, &self.opts)?;
        emit_working_copy(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashApply {
    pub repo: RepoId,
    pub index: usize,
    #[serde(default)]
    pub opts: StashApplyOptions,
}

impl Command for StashApply {
    type Output = StashInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        require_stash(cx, self.index)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = stash::apply(cx.repo, self.index, &self.opts)?;
        emit_working_copy(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashPop {
    pub repo: RepoId,
    pub index: usize,
    #[serde(default)]
    pub opts: StashApplyOptions,
}

impl Command for StashPop {
    type Output = StashInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        require_stash(cx, self.index)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = stash::pop(cx.repo, self.index, &self.opts)?;
        emit_working_copy(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashDrop {
    pub repo: RepoId,
    /// Dropped highest-index-first is the caller's concern; the engine drops by
    /// index, and the validator checks each one exists.
    pub indexes: Vec<usize>,
}

impl Command for StashDrop {
    type Output = Vec<StashInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        for index in &self.indexes {
            require_stash(cx, *index)?;
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        // Drop descending so earlier indexes stay valid.
        let mut indexes = self.indexes.clone();
        indexes.sort_unstable_by(|a, b| b.cmp(a));
        let mut dropped = Vec::new();
        for index in indexes {
            dropped.push(stash::drop(cx.repo, index)?);
        }
        emit_working_copy(cx);
        Ok(dropped)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashShow {
    pub repo: RepoId,
    pub index: usize,
}

impl Command for StashShow {
    type Output = StashDetail;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_stash(cx, self.index)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        stash::show(cx.repo, self.index)
    }
}

// ===========================================================================
// Submodules (docs/03 §11)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleList {
    pub repo: RepoId,
}

impl Command for SubmoduleList {
    type Output = Vec<SubmoduleInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        submodules::list(cx.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInit {
    pub repo: RepoId,
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Command for SubmoduleInit {
    type Output = SubmoduleReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = submodules::init(cx.repo, &self.paths)?;
        emit_refs(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleUpdate {
    pub repo: RepoId,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub opts: SubmoduleUpdateOptions,
}

impl Command for SubmoduleUpdate {
    type Output = SubmoduleReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.opts.strategy != SubmoduleUpdateStrategy::Checkout {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!(
                    "The '{:?}' update strategy cannot be executed",
                    self.opts.strategy
                ),
            )
            .with_param("class", "submodule_strategy_unsupported"));
        }
        require_no_integration(cx)
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let report = submodules::update(cx.repo, &self.paths, &self.opts)?;
        emit_refs(cx);
        emit_working_copy(cx);
        Ok(report)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSync {
    pub repo: RepoId,
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Command for SubmoduleSync {
    type Output = SubmoduleReport;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        submodules::sync(cx.repo, &self.paths)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSettingsSave {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub ignore: Option<SubmoduleIgnoreMode>,
    #[serde(default)]
    pub strategy: Option<SubmoduleUpdateStrategy>,
    #[serde(default)]
    pub recurse: Option<bool>,
}

impl Command for SubmoduleSettingsSave {
    type Output = SubmoduleInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if cx.repo.find_submodule(&self.name).is_err() {
            return Err(Violation::new(
                ErrorCode::ReferenceNotFound,
                format!("Submodule '{}' does not exist", self.name),
            )
            .with_param("name", self.name.clone()));
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        submodules::settings_save(
            cx.repo,
            self.name.trim(),
            self.ignore,
            self.strategy,
            self.recurse,
        )
    }
}

// ===========================================================================
// Worktrees (docs/03 §12)
// ===========================================================================

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeList {
    pub repo: RepoId,
}

impl Command for WorktreeList {
    type Output = Vec<WorktreeInfo>;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        worktrees::list(cx.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeAdd {
    pub repo: RepoId,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub start: Option<Revision>,
    #[serde(default)]
    pub opts: WorktreeAddOptions,
}

impl Command for WorktreeAdd {
    type Output = WorktreeInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        require_not_bare(cx)?;
        if self.name.trim().is_empty() || self.path.trim().is_empty() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "A worktree needs a name and a path",
            ));
        }
        if !self.opts.detached {
            let branch = self
                .opts
                .branch
                .clone()
                .unwrap_or_else(|| self.name.clone());
            if cx
                .repo
                .find_branch(&branch, git2::BranchType::Local)
                .is_ok()
                && branches::branch_is_checked_out(cx.repo, &format!("refs/heads/{branch}"))
                    .unwrap_or(false)
            {
                return Err(Violation::new(
                    ErrorCode::InvalidRequest,
                    format!("Branch '{branch}' is already checked out in another worktree"),
                )
                .with_param("name", branch)
                .with_param("class", "worktree_branch_checked_out"));
            }
        }
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = worktrees::add(
            cx.repo,
            self.name.trim(),
            self.path.trim(),
            self.start.as_ref(),
            &self.opts,
        )?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemove {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for WorktreeRemove {
    type Output = WorktreeInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn sender(&self) -> SenderTag {
        self.sender
    }
    fn risk(&self) -> Risk {
        Risk::Destructive
    }
    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.force {
            return Ok(());
        }
        match worktrees::removal_blocker(cx.repo, &self.name) {
            Ok(Some(blocker)) => Err(Violation::new(
                if blocker.class == "worktree_locked" {
                    ErrorCode::InvalidRequest
                } else {
                    ErrorCode::CheckoutConflicts
                },
                blocker.message,
            )
            .with_param("name", self.name.clone())
            .with_param("class", blocker.class)),
            _ => Ok(()),
        }
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = worktrees::remove(cx.repo, self.name.trim(), self.force)?;
        emit_refs(cx);
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeLock {
    pub repo: RepoId,
    pub name: String,
    pub lock: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

impl Command for WorktreeLock {
    type Output = WorktreeInfo;
    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }
    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }
    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = if self.lock {
            worktrees::lock(cx.repo, self.name.trim(), self.reason.as_deref())?
        } else {
            worktrees::unlock(cx.repo, self.name.trim())?
        };
        emit_refs(cx);
        Ok(info)
    }
}

// ===========================================================================
// Tauri wrappers
// ===========================================================================

macro_rules! tauri_command {
    ($name:ident, $args:ty, $out:ty) => {
        #[tauri::command]
        pub async fn $name(
            state: State<'_, AppState>,
            app: AppHandle,
            args: $args,
        ) -> Result<$out, ErrorPayload> {
            state.run(&app, args).await
        }
    };
}

tauri_command!(branch_load, BranchLoad, BranchInfo);
tauri_command!(
    branch_stacked_create,
    BranchStackedCreate,
    StackedBranchInfo
);
tauri_command!(branch_checkout_remote, BranchCheckoutRemote, BranchInfo);
tauri_command!(
    branch_checkout_revision,
    BranchCheckoutRevision,
    CheckoutResult
);
tauri_command!(branch_rename, BranchRename, BranchInfo);
tauri_command!(branch_delete, BranchDelete, DeleteReport);
tauri_command!(branch_fast_forward, BranchFastForward, BranchInfo);
tauri_command!(branch_set_upstream, BranchSetUpstream, BranchInfo);
tauri_command!(branch_unset_upstream, BranchUnsetUpstream, BranchInfo);
tauri_command!(branch_track_remote, BranchTrackRemote, BranchInfo);
tauri_command!(branch_untrack, BranchUntrack, BranchInfo);

tauri_command!(remote_branch_list, RemoteBranchList, Vec<RemoteBranchInfo>);
tauri_command!(remote_branch_delete, RemoteBranchDelete, DeleteReport);
tauri_command!(remote_branch_prune, RemoteBranchPrune, PruneReport);

tauri_command!(remote_list, RemoteList, Vec<RemoteInfo>);
tauri_command!(remote_add, RemoteAdd, RemoteInfo);
tauri_command!(remote_rename, RemoteRename, RemoteRenameReport);
tauri_command!(remote_delete, RemoteDelete, ());
tauri_command!(remote_set_url, RemoteSetUrl, RemoteInfo);
tauri_command!(remote_refspec_add, RemoteRefspecAdd, RemoteInfo);
tauri_command!(remote_refspec_delete, RemoteRefspecDelete, RemoteInfo);
tauri_command!(remote_set_fetch_mode, RemoteSetFetchMode, RemoteInfo);
tauri_command!(remote_fetch, RemoteFetch, FetchReport);
tauri_command!(remote_pull, RemotePull, PullResult);
tauri_command!(remote_push, RemotePush, PushReport);
tauri_command!(remote_force_push, RemoteForcePush, PushReport);
tauri_command!(remote_prune, RemotePrune, PruneReport);
tauri_command!(remote_publish_branch, RemotePublishBranch, PublishReport);

tauri_command!(tag_list, TagList, Vec<TagInfo>);
tauri_command!(tag_detail_load, TagDetailLoad, TagInfo);
tauri_command!(tag_create, TagCreate, TagInfo);
tauri_command!(tag_delete, TagDelete, DeleteReport);
tauri_command!(tag_publish, TagPublish, PushReport);
tauri_command!(tag_delete_remote, TagDeleteRemote, PushReport);

tauri_command!(stash_list, StashList, Vec<StashInfo>);
tauri_command!(stash_count_load, StashCountLoad, u64);
tauri_command!(stash_save, StashSave, StashInfo);
tauri_command!(stash_apply, StashApply, StashInfo);
tauri_command!(stash_pop, StashPop, StashInfo);
tauri_command!(stash_drop, StashDrop, Vec<StashInfo>);
tauri_command!(stash_show, StashShow, StashDetail);

tauri_command!(submodule_list, SubmoduleList, Vec<SubmoduleInfo>);
tauri_command!(submodule_init, SubmoduleInit, SubmoduleReport);
tauri_command!(submodule_update, SubmoduleUpdate, SubmoduleReport);
tauri_command!(submodule_sync, SubmoduleSync, SubmoduleReport);
tauri_command!(
    submodule_settings_save,
    SubmoduleSettingsSave,
    SubmoduleInfo
);

tauri_command!(worktree_list, WorktreeList, Vec<WorktreeInfo>);
tauri_command!(worktree_add, WorktreeAdd, WorktreeInfo);
tauri_command!(worktree_remove, WorktreeRemove, WorktreeInfo);
tauri_command!(worktree_lock, WorktreeLock, WorktreeInfo);

// ===========================================================================
// Event helpers
// ===========================================================================

fn emit_refs(cx: &ExecCx<'_>) {
    events::emit(
        cx.app,
        AppEvent::RefsChanged {
            repo: cx.repo_id.clone(),
        },
    );
}

fn emit_working_copy(cx: &ExecCx<'_>) {
    events::emit(
        cx.app,
        AppEvent::WorkingCopyChanged {
            repo: cx.repo_id.clone(),
        },
    );
}

fn default_true() -> bool {
    true
}

// ===========================================================================
// Validators (pure; see the module header on why they live here)
// ===========================================================================

fn require_not_bare(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    if cx.repo.is_bare() {
        return Err(Violation::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    Ok(())
}

fn require_no_integration(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    if cx.repo.state() != git2::RepositoryState::Clean {
        return Err(Violation::new(
            ErrorCode::OperationInProgress,
            "Cannot run while an operation is in progress",
        )
        .with_param("class", "integration_in_progress"));
    }
    Ok(())
}

/// Detached HEAD is a hard error for operations that need a branch to act on.
fn require_attached_head(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    match cx.repo.head() {
        Ok(head) if head.is_branch() => Ok(()),
        Ok(_) => Err(Violation::new(
            ErrorCode::InvalidRequest,
            "HEAD is detached — check out a branch first",
        )
        .with_param("class", "detached_head")),
        Err(_) => Err(Violation::new(
            ErrorCode::UnknownRevision,
            "The repository has no commits yet",
        )),
    }
}

/// Uncommitted changes block merge/rebase-style operations.
fn require_clean(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    let mut options = git2::StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = cx
        .repo
        .statuses(Some(&mut options))
        .map_err(|err| Violation::new(ErrorCode::Unclassified, err.message().to_string()))?;
    if !statuses.is_empty() {
        return Err(Violation::new(
            ErrorCode::CheckoutConflicts,
            "The working copy has uncommitted changes",
        )
        .with_param("class", "working_copy_dirty"));
    }
    Ok(())
}

/// A stash must exist at `index`.
fn require_stash(cx: &ValidateCx<'_>, index: usize) -> Result<(), Violation> {
    let count = stash::count(cx.repo)
        .map_err(|err| Violation::new(ErrorCode::Unclassified, err.message.clone()))?;
    if index >= count {
        return Err(Violation::new(
            ErrorCode::ReferenceNotFound,
            format!("No stash entry at index {index}"),
        )
        .with_param("index", index.to_string())
        .with_param("class", "stash_not_found"));
    }
    Ok(())
}

/// At least one tracked change must exist to make a stash.
fn require_dirty(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    let mut options = git2::StatusOptions::new();
    options.include_untracked(true);
    let statuses = cx
        .repo
        .statuses(Some(&mut options))
        .map_err(|err| Violation::new(ErrorCode::Unclassified, err.message().to_string()))?;
    if statuses.is_empty() {
        return Err(
            Violation::new(ErrorCode::InvalidRequest, "There is nothing to stash")
                .with_param("class", "save_stash_no_changes"),
        );
    }
    Ok(())
}

fn require_branch(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    if cx.repo.find_branch(name, git2::BranchType::Local).is_err() {
        return Err(Violation::new(
            ErrorCode::ReferenceNotFound,
            format!("Branch '{name}' does not exist"),
        )
        .with_param("name", name));
    }
    Ok(())
}

fn require_local_branch(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    require_branch(cx, name)
}

fn require_new_branch(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    if !git2::Reference::is_valid_name(&format!("refs/heads/{name}")) {
        return Err(Violation::new(
            ErrorCode::InvalidReferenceName,
            format!("'{name}' is not a valid branch name"),
        )
        .with_param("name", name));
    }
    if cx.repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Err(Violation::new(
            ErrorCode::ReferenceExists,
            format!("Branch '{name}' already exists"),
        )
        .with_param("name", name));
    }
    Ok(())
}

fn require_free_name(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    require_new_branch(cx, name)
}

fn require_remote(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    if cx.repo.find_remote(name).is_err() {
        return Err(Violation::new(
            ErrorCode::ReferenceNotFound,
            format!("Remote '{name}' does not exist"),
        )
        .with_param("name", name)
        .with_param("class", "remote_not_found"));
    }
    Ok(())
}

/// The `missing_parent` precondition the evidence renders with its dedicated
/// symbol (docs/03 §6 `branch_stacked_create`).
fn validate_missing_parent(cx: &ValidateCx<'_>, parent: &str) -> Result<(), Violation> {
    if !git2::Reference::is_valid_name(&format!("refs/heads/{parent}")) {
        return Err(Violation::new(
            ErrorCode::InvalidReferenceName,
            format!("'{parent}' is not a valid branch name"),
        )
        .with_param("name", parent));
    }
    if branches::parent_status(cx.repo, parent) == branches::ParentStatus::Missing {
        return Err(Violation::new(
            ErrorCode::UnknownRevision,
            format!("The selected parent branch '{parent}' does not exist"),
        )
        .with_param("name", parent)
        .with_param("class", "missing_parent"));
    }
    Ok(())
}

/// The `delete_unmerged_branch` precondition (docs/03 §6 `branch_delete`):
/// unmerged and not protected by `force`.
fn validate_unmerged_delete(
    cx: &ValidateCx<'_>,
    names: &[String],
    force: bool,
) -> Result<(), Violation> {
    if force {
        return Ok(());
    }
    let unmerged = branches::unmerged(cx.repo, names)
        .map_err(|err| Violation::new(ErrorCode::Unclassified, err.message.clone()))?;
    if !unmerged.is_empty() {
        return Err(Violation::new(
            ErrorCode::InvalidRequest,
            format!("Not fully merged: {}", unmerged.join(", ")),
        )
        .with_param("names", unmerged.join(", "))
        .with_param("class", "delete_unmerged_branch"));
    }
    Ok(())
}
