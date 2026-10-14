//! `history` — the commit-history command group (docs/03 §5).
//!
//! Every command here is a read: it validates, calls one engine function and
//! returns a domain value. Nothing is queued for mutation, so `Risk::None`
//! throughout — the repository queue exists for commands that change it.

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx};
use crate::domain::commit::{
    Changeset, CommitDetail, Contributor, FileRevision, GraphLayout, HistoryPage, HistoryQuery,
    SearchMode, MAX_HISTORY_LIMIT,
};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::history::HistorySort;
use crate::domain::ids::RepoId;
use crate::domain::revision::Revision;
use crate::engine;
use crate::state::AppState;
use crate::validation::{ValidateCx, Violation};

/// The `history_commits_load` argument struct (docs/03 §5), matching the
/// `HistoryCommitsOptions` the web layer already sends.
///
/// `from` is the commit to walk from (inclusive; default `HEAD`) — the "load
/// more" affordance resumes from the tail's oid. `to`, when given, hides that
/// commit and its ancestors.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCommitsLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub from: Option<Revision>,
    #[serde(default)]
    pub to: Option<Revision>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub skip: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort: HistorySort,
    #[serde(default)]
    pub first_parent: bool,
    #[serde(default)]
    pub follow_renames: bool,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub search_mode: SearchMode,
}

fn default_limit() -> usize {
    crate::domain::commit::DEFAULT_HISTORY_LIMIT
}

impl HistoryCommitsLoad {
    fn query(&self) -> HistoryQuery {
        HistoryQuery {
            from: self.from.clone(),
            to: self.to.clone(),
            skip: self.skip,
            limit: self.limit,
            sort: self.sort,
            first_parent: self.first_parent,
            path: self.path.clone(),
            follow_renames: self.follow_renames,
            search: self.search.clone(),
            search_mode: self.search_mode,
        }
    }
}

impl Command for HistoryCommitsLoad {
    type Output = HistoryPage;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.limit == 0 {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "A history page needs a limit of at least one commit",
            ));
        }
        // The documented precondition: every named revision resolves.
        for revision in [self.from.as_ref(), self.to.as_ref()].into_iter().flatten() {
            engine::history::resolve(cx.repo, revision).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(revision))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::load_page(cx.repo, &self.query())
    }
}

/// `history_graph_compute` (docs/03 §5): lane layout for an explicit oid list.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryGraphCompute {
    pub repo: RepoId,
    pub oids: Vec<String>,
}

impl Command for HistoryGraphCompute {
    type Output = GraphLayout;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.oids.len() > MAX_HISTORY_LIMIT {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!(
                    "A graph page is limited to {MAX_HISTORY_LIMIT} commits, got {}",
                    self.oids.len()
                ),
            ));
        }
        for hex in &self.oids {
            let oid = git2::Oid::from_str(hex).map_err(|_| {
                Violation::new(
                    ErrorCode::UnknownRevision,
                    format!("'{hex}' is not a commit hash"),
                )
                .with_param("revision", hex.clone())
            })?;
            cx.repo.find_commit(oid).map_err(|_| {
                Violation::new(ErrorCode::UnknownRevision, format!("No commit {hex}"))
                    .with_param("revision", hex.clone())
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::graph_for_oids(cx.repo, &self.oids)
    }
}

/// `history_commit_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCommitLoad {
    pub repo: RepoId,
    pub oid: String,
}

impl Command for HistoryCommitLoad {
    type Output = CommitDetail;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        let oid = parse_commit_oid(cx, &self.oid)?;
        cx.repo.find_commit(oid).map_err(|_| {
            Violation::new(
                ErrorCode::UnknownRevision,
                format!("No commit {}", self.oid),
            )
            .with_param("revision", self.oid.clone())
        })?;
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::load_commit(cx.repo, git2::Oid::from_str(&self.oid)?)
    }
}

/// Options for the two changeset commands.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetLoadOptions {
    /// Rename/copy detection. On by default (the list shows renames); off is
    /// the cheaper mode for very wide changesets.
    #[serde(default = "default_true")]
    pub detect_renames: bool,
}

fn default_true() -> bool {
    true
}

/// `history_changeset_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryChangesetLoad {
    pub repo: RepoId,
    pub oid: String,
    #[serde(default)]
    pub opts: ChangesetLoadOptions,
}

impl Command for HistoryChangesetLoad {
    type Output = Changeset;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        let oid = parse_commit_oid(cx, &self.oid)?;
        cx.repo.find_commit(oid).map_err(|_| {
            Violation::new(
                ErrorCode::UnknownRevision,
                format!("No commit {}", self.oid),
            )
            .with_param("revision", self.oid.clone())
        })?;
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let opts = engine::history::ChangesetOptions {
            detect_renames: self.opts.detect_renames,
        };
        engine::history::load_changeset(cx.repo, git2::Oid::from_str(&self.oid)?, &opts)
    }
}

/// `history_revision_range_changeset_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRevisionRangeChangesetLoad {
    pub repo: RepoId,
    pub from: Revision,
    pub to: Revision,
    #[serde(default)]
    pub opts: ChangesetLoadOptions,
}

impl Command for HistoryRevisionRangeChangesetLoad {
    type Output = Changeset;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        for revision in [&self.from, &self.to] {
            engine::history::resolve(cx.repo, revision).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(revision))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let opts = engine::history::ChangesetOptions {
            detect_renames: self.opts.detect_renames,
        };
        engine::history::load_range_changeset(cx.repo, &self.from, &self.to, &opts)
    }
}

/// `history_commit_hashes_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCommitHashesLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub start: Option<Revision>,
    #[serde(default)]
    pub skip: usize,
    pub count: usize,
}

impl Command for HistoryCommitHashesLoad {
    type Output = Vec<String>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.count == 0 || self.count > MAX_HISTORY_LIMIT {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("`count` must be between 1 and {MAX_HISTORY_LIMIT}"),
            ));
        }
        if let Some(start) = &self.start {
            engine::history::resolve(cx.repo, start).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(start))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::commit_hashes(cx.repo, self.start.as_ref(), self.skip, self.count)
    }
}

/// `history_count_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCountLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub from: Option<Revision>,
}

impl Command for HistoryCountLoad {
    type Output = u64;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if let Some(from) = &self.from {
            engine::history::resolve(cx.repo, from).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(from))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::count(cx.repo, self.from.as_ref())
    }
}

/// `history_contributors_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryContributorsLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub from: Option<Revision>,
}

impl Command for HistoryContributorsLoad {
    type Output = Vec<Contributor>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if let Some(from) = &self.from {
            engine::history::resolve(cx.repo, from).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(from))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::contributors(cx.repo, self.from.as_ref())
    }
}

/// `history_file_history_load` (docs/03 §5).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFileHistoryLoad {
    pub repo: RepoId,
    pub path: String,
    #[serde(default)]
    pub skip: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub follow_renames: bool,
}

impl Command for HistoryFileHistoryLoad {
    type Output = Vec<FileRevision>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        if self.path.trim().is_empty() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "File history needs a path",
            ));
        }
        if self.limit == 0 {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "A file-history page needs a limit of at least one revision",
            ));
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::history::file_history(
            cx.repo,
            self.path.trim(),
            self.skip,
            self.limit,
            self.follow_renames,
        )
    }
}

fn parse_commit_oid(cx: &ValidateCx<'_>, hex: &str) -> Result<git2::Oid, Violation> {
    git2::Oid::from_str(hex)
        .map_err(|_| {
            Violation::new(
                ErrorCode::UnknownRevision,
                format!("'{hex}' is not a commit hash"),
            )
            .with_param("revision", hex.to_string())
        })
        .and_then(|oid| {
            cx.repo.find_commit(oid).map(|_| oid).map_err(|_| {
                Violation::new(ErrorCode::UnknownRevision, format!("No commit {hex}"))
                    .with_param("revision", hex.to_string())
            })
        })
}

/// The user-facing spelling of a revision, for error params.
fn revision_spec(revision: &Revision) -> String {
    match revision {
        Revision::Head => "HEAD".to_string(),
        Revision::Branch(name) => name.clone(),
        Revision::Tag(name) => name.clone(),
        Revision::Oid(hex) => hex.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tauri entry points
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn history_commits_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryCommitsLoad,
) -> Result<HistoryPage, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_graph_compute(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryGraphCompute,
) -> Result<GraphLayout, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_commit_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryCommitLoad,
) -> Result<CommitDetail, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_changeset_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryChangesetLoad,
) -> Result<Changeset, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_revision_range_changeset_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryRevisionRangeChangesetLoad,
) -> Result<Changeset, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_commit_hashes_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryCommitHashesLoad,
) -> Result<Vec<String>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_count_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryCountLoad,
) -> Result<u64, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_contributors_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryContributorsLoad,
) -> Result<Vec<Contributor>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn history_file_history_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryFileHistoryLoad,
) -> Result<Vec<FileRevision>, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMergeBase {
    pub repo: RepoId,
    pub left: Revision,
    pub right: Revision,
}

/// The newest commit reachable from both revisions — `git merge-base`.
///
/// It exists because the compare screen has to be able to say the truth about
/// where two branches diverged. Without it the pane has two honest options,
/// both bad: show nothing, or show the newest commit of one side and imply it
/// is the fork point, and how far each side is from it. git2 answers all three
/// in one call, so there is no case for guessing any of them.
///
/// A `None` merge base is a real answer, not a failure: two unrelated histories
/// have no fork point, and that is exactly what the user needs to be told.
#[tauri::command]
pub async fn history_merge_base(
    state: State<'_, AppState>,
    app: AppHandle,
    args: HistoryMergeBase,
) -> Result<HistoryCompareSummary, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCompareSummary {
    pub merge_base: Option<String>,
    /// Commits reachable from `right` and not `left`.
    pub ahead: usize,
    /// Commits reachable from `left` and not `right`.
    pub behind: usize,
}

impl Command for HistoryMergeBase {
    type Output = HistoryCompareSummary;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        for revision in [&self.left, &self.right] {
            engine::history::resolve(cx.repo, revision).map_err(|err| {
                Violation::new(ErrorCode::UnknownRevision, err.message)
                    .with_param("revision", revision_spec(revision))
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let (merge_base, left_only, right_only) =
            engine::history::compare(cx.repo, &self.left, &self.right)?;
        Ok(HistoryCompareSummary {
            merge_base,
            // Ahead is what the *right* side has that the left lacks: the
            // screen reads "left is the base, right is what is being compared".
            ahead: right_only,
            behind: left_only,
        })
    }
}
