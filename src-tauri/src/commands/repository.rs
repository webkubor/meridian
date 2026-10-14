//! `repository` — lifecycle and discovery (docs/03 §2).
//!
//! Lifecycle commands (`repository_open`, `repository_init`) run *before* a
//! `RepoId` exists, so they validate and call the engine directly and then
//! populate the registry. Every command that acts on an open repository is a
//! [`Command`] and takes the `AppState::run` path — resolve, enqueue, validate,
//! execute, emit (docs/02 §3).
//!
//! The model reads (`repository_remotes_load`, `repository_worktrees_load`,
//! `repository_health_load`) are named with the `repository_` prefix even
//! though docs/03 §8/§12 cover the same data from the remotes and worktrees
//! command groups: those groups own the *mutating* verbs (`remote_add`,
//! `worktree_add`, …) and their own `remote_list` / `worktree_list` shapes.
//! Here the data belongs to the repository model, so the command names stay in
//! this module's namespace and cannot collide with theirs.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx};
use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::repository::{
    ConfigScope, ConfigSnapshot, IntegrationState, PathCheck, RepoInfo, RepoRoot, RepoStats,
    RepositoryHealth, RepositoryRemotes, WorktreeEntry,
};
use crate::domain::sender::SenderTag;
use crate::engine;
use crate::events::{self, AppEvent};
use crate::logging;
use crate::state::AppState;
use crate::validation::{self, ValidateCx, Violation};

// ------------------------------------------------------------- lifecycle ---

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOpenArgs {
    pub path: String,
    #[serde(default)]
    pub sender: SenderTag,
}

impl RepositoryOpenArgs {
    /// Opens and registers a repository through the lifecycle command path.
    ///
    /// The lifecycle commands run before a `RepoId` exists, so they bypass
    /// `AppState::run` and its outcome line; each logs its own here, because
    /// opening a repository is the first thing anyone does and used to leave no
    /// trace when it failed.
    pub fn execute(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let started = Instant::now();
        let result = self.open(state, sink);
        logging::instrument("repository_open", started, &result);
        result
    }

    fn open(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let path = resolve_open_path(&self)?;
        validation::repository::validate_open_path(&path).map_err(ErrorPayload::from)?;

        let (repo, discovered) = engine::repository::open(&path)?;
        let mut info = state.insert_repo(repo)?;
        info.discovered = discovered;

        events::emit(sink, AppEvent::ReposChanged);
        Ok(info)
    }
}

#[tauri::command]
pub async fn repository_open(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryOpenArgs,
) -> Result<RepoInfo, ErrorPayload> {
    args.execute(&state, &app)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInitArgs {
    pub path: String,
    #[serde(default)]
    pub bare: bool,
    #[serde(default = "default_initial_branch")]
    pub initial_branch: String,
    #[serde(default)]
    pub create_gitignore: bool,
}

fn default_initial_branch() -> String {
    "main".to_string()
}

impl RepositoryInitArgs {
    /// Validates, initializes, and registers a repository.
    pub fn execute(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let started = Instant::now();
        let result = self.init(state, sink);
        logging::instrument("repository_init", started, &result);
        result
    }

    fn init(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let path = PathBuf::from(&self.path);
        validation::repository::validate_init_path(&path, &self.initial_branch)
            .map_err(ErrorPayload::from)?;

        let opts = engine::repository::InitOptions {
            bare: self.bare,
            initial_branch: self.initial_branch,
            create_gitignore: self.create_gitignore,
        };
        let repo = engine::repository::init(&path, &opts)?;
        let info = state.insert_repo(repo)?;

        events::emit(sink, AppEvent::ReposChanged);
        Ok(info)
    }
}

#[tauri::command]
pub async fn repository_init(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryInitArgs,
) -> Result<RepoInfo, ErrorPayload> {
    args.execute(&state, &app)
}

fn resolve_open_path(args: &RepositoryOpenArgs) -> Result<PathBuf, ErrorPayload> {
    if !args.path.trim().is_empty() {
        return Ok(PathBuf::from(&args.path));
    }
    // Sender-scoped default: a CLI invocation without a path means "here".
    if args.sender == SenderTag::Cli {
        return std::env::current_dir().map_err(|err| {
            ErrorPayload::new(
                ErrorCode::Io,
                format!("Cannot read the current directory: {err}"),
            )
        });
    }
    Err(ErrorPayload::new(
        ErrorCode::InvalidRequest,
        "No repository path given",
    ))
}

// ------------------------------------------------------------- discovery ---

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCloneArgs {
    pub url: String,
    /// Where to put the working tree. Empty means "next to the caller's idea of
    /// a default", which the frontend fills in — see `suggest_clone_destination`.
    pub destination: String,
    #[serde(default)]
    pub bare: bool,
    #[serde(default)]
    pub initial_branch: String,
}

impl RepositoryCloneArgs {
    /// Clones and registers a repository through the lifecycle command path.
    pub async fn execute(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let started = Instant::now();
        let result = self.clone_into(state, sink).await;
        logging::instrument("repository_clone", started, &result);
        result
    }

    async fn clone_into(
        self,
        state: &AppState,
        sink: &dyn crate::events::EventSink,
    ) -> Result<RepoInfo, ErrorPayload> {
        let url = self.url.trim().to_string();
        if url.is_empty() {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "No clone URL given",
            ));
        }

        let destination = if self.destination.trim().is_empty() {
            repository_clone_suggest_destination(url.clone())
        } else {
            self.destination.trim().to_string()
        };
        let destination = PathBuf::from(destination);
        validation::repository::validate_init_path(&destination, "main")
            .map_err(ErrorPayload::from)?;

        let initial_branch = self.initial_branch.trim().to_string();
        let bare = self.bare;
        let target = destination.clone();
        let repo = tauri::async_runtime::spawn_blocking(move || {
            engine::repository::clone(&url, &target, bare, &initial_branch)
        })
        .await
        .map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("The clone did not finish: {err}"))
        })??;

        let mut info = state.insert_repo(repo)?;
        info.discovered = false;
        events::emit(sink, AppEvent::ReposChanged);
        Ok(info)
    }
}

/// Derives the folder name a URL implies: the last path segment of a
/// `.git`-suffixed URL, with the `.git` removed.
///
/// Public because the frontend shows this as the destination the user is about
/// to be asked to confirm, and a suggestion the UI has to recompute separately
/// is a suggestion that will disagree with the one the command uses.
#[tauri::command]
pub fn repository_clone_suggest_destination(url: String) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed.rsplit('/').next().unwrap_or_default();
    let name = last.strip_suffix(".git").unwrap_or(last);
    // A bare `host:` / `host:path` scp-style URL has no slash to split on.
    let name = name.rsplit(':').next().unwrap_or(name);
    let name = name.split('\\').next().unwrap_or(name);
    if name.is_empty() || name == "." || name == ".." {
        "repository".to_string()
    } else {
        name.to_string()
    }
}

/// Clones `url` into `destination` and opens the result.
///
/// The clone runs on a blocking thread (`spawn_blocking`) rather than inline:
/// libgit2's transfer is synchronous, and on the command's own task a large
/// clone would stall every other command in the app for the duration. The
/// frontend shows a progress chip; this command returns when it is done.
#[tauri::command]
pub async fn repository_clone(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryCloneArgs,
) -> Result<RepoInfo, ErrorPayload> {
    args.execute(&state, &app).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDiscoverArgs {
    pub root: String,
    /// Depth below the root to inspect; `0` means no limit.
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Directory names never entered (exact matches).
    #[serde(default)]
    pub ignored: Vec<String>,
}

fn default_max_depth() -> u32 {
    engine::discovery::DEFAULT_MAX_DEPTH
}

/// `repository_discover` (docs/03 §2): bounded fs walk + exact open per
/// candidate. The streaming indexer is `repository_quick_open_index`; this
/// command answers one question, once.
#[tauri::command]
pub async fn repository_discover(
    _state: State<'_, AppState>,
    args: RepositoryDiscoverArgs,
) -> Result<Vec<RepoRoot>, ErrorPayload> {
    let started = Instant::now();
    let root = PathBuf::from(&args.root);
    let result = validate_discover_root(&root)
        .map_err(ErrorPayload::from)
        .and_then(|()| {
            engine::discovery::discover(
                &root,
                &engine::discovery::DiscoverOptions {
                    max_depth: args.max_depth,
                    ignored: args.ignored,
                    ..Default::default()
                },
            )
        });
    logging::instrument("repository_discover", started, &result);
    result
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryValidatePathArgs {
    pub path: String,
}

/// `repository_validate_path` (docs/03 §2): classify a path without opening
/// it. Never an error — "not a repository" is an answer the caller renders.
#[tauri::command]
pub async fn repository_validate_path(
    _state: State<'_, AppState>,
    args: RepositoryValidatePathArgs,
) -> Result<PathCheck, ErrorPayload> {
    Ok(engine::discovery::probe_path(Path::new(&args.path)))
}

// ------------------------------------------------------- repo-scoped reads --

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRefresh {
    pub repo: RepoId,
}

impl Command for RepositoryRefresh {
    type Output = RepoInfo;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        // Re-open from disk so edits made outside Meridian (a `git fetch` in a
        // terminal, a config change) are part of the answer.
        let fresh = engine::repository::reopen(cx.repo)?;
        let info = engine::repository::info(&fresh)?;
        events::emit(
            cx.app,
            AppEvent::RefsChanged {
                repo: cx.repo_id.clone(),
            },
        );
        events::emit(
            cx.app,
            AppEvent::WorkingCopyChanged {
                repo: cx.repo_id.clone(),
            },
        );
        Ok(info)
    }
}

#[tauri::command]
pub async fn repository_refresh(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryRefresh,
) -> Result<RepoInfo, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryState {
    pub repo: RepoId,
}

impl Command for RepositoryState {
    type Output = IntegrationState;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        Ok(engine::repository::integration_state(cx.repo))
    }
}

#[tauri::command]
pub async fn repository_state(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryState,
) -> Result<IntegrationState, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub scope: ConfigScope,
}

impl Command for RepositoryConfigLoad {
    type Output = ConfigSnapshot;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        // "Scope valid" is guaranteed by the `ConfigScope` enum at the command
        // boundary; a level whose file is absent is an empty snapshot, not a
        // violation (the caller renders it as an empty editor).
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::config_snapshot(cx.repo, self.scope)
    }
}

#[tauri::command]
pub async fn repository_config_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryConfigLoad,
) -> Result<ConfigSnapshot, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigSet {
    pub repo: RepoId,
    #[serde(default)]
    pub scope: ConfigScope,
    pub key: String,
    pub value: String,
}

impl Command for RepositoryConfigSet {
    type Output = ();

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validate_config_key(&self.key)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::config_set(cx.repo, self.scope, &self.key, &self.value)
    }
}

#[tauri::command]
pub async fn repository_config_set(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryConfigSet,
) -> Result<(), ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryStatsLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub include_size: bool,
}

impl Command for RepositoryStatsLoad {
    type Output = RepoStats;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::stats(cx.repo, self.include_size)
    }
}

#[tauri::command]
pub async fn repository_stats_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryStatsLoad,
) -> Result<RepoStats, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDescriptionSet {
    pub repo: RepoId,
    pub text: String,
}

impl Command for RepositoryDescriptionSet {
    type Output = ();

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::set_description(cx.repo, &self.text)
    }
}

#[tauri::command]
pub async fn repository_description_set(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryDescriptionSet,
) -> Result<(), ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemotesLoad {
    pub repo: RepoId,
}

impl Command for RepositoryRemotesLoad {
    type Output = RepositoryRemotes;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::remotes_model(cx.repo)
    }
}

#[tauri::command]
pub async fn repository_remotes_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryRemotesLoad,
) -> Result<RepositoryRemotes, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryWorktreesLoad {
    pub repo: RepoId,
}

impl Command for RepositoryWorktreesLoad {
    type Output = Vec<WorktreeEntry>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::worktrees(cx.repo)
    }
}

#[tauri::command]
pub async fn repository_worktrees_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryWorktreesLoad,
) -> Result<Vec<WorktreeEntry>, ErrorPayload> {
    state.run(&app, args).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryHealthLoad {
    pub repo: RepoId,
}

impl Command for RepositoryHealthLoad {
    type Output = RepositoryHealth;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::repository::health(cx.repo)
    }
}

#[tauri::command]
pub async fn repository_health_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: RepositoryHealthLoad,
) -> Result<RepositoryHealth, ErrorPayload> {
    state.run(&app, args).await
}

// ------------------------------------------------------------ validators ----
//
// These are pure precondition functions over the request (docs/02 §3, step 3).
// They live beside their commands because `validation/repository.rs` predates
// this module's second half; moving them there later is a pure file move.

/// `repository_discover` precondition (docs/03 §2): root readable.
fn validate_discover_root(root: &Path) -> Result<(), Violation> {
    let path_param = root.display().to_string();
    if !root.exists() {
        return Err(
            Violation::new(ErrorCode::Io, format!("Path does not exist: {path_param}"))
                .with_domain(ErrorDomain::Fs)
                .with_param("path", path_param),
        );
    }
    if !root.is_dir() {
        return Err(
            Violation::new(ErrorCode::Io, format!("Not a directory: {path_param}"))
                .with_domain(ErrorDomain::Fs)
                .with_param("path", path_param),
        );
    }
    if std::fs::read_dir(root).is_err() {
        return Err(
            Violation::new(ErrorCode::Io, format!("Cannot read {path_param}"))
                .with_domain(ErrorDomain::Fs)
                .with_param("path", path_param),
        );
    }
    Ok(())
}

/// The keys Meridian writes through dedicated commands (docs/03 §2: "key not
/// in the protected managed set"). Two mechanisms must never fight over one
/// value: the profile commands own `user.*` (docs/03 §17) and repository
/// identity is owned by open/init.
const PROTECTED_CONFIG_KEYS: &[&str] = &[
    "user.name",
    "user.email",
    "user.signingkey",
    "core.bare",
    "core.worktree",
    "core.repositoryformatversion",
];

const PROTECTED_CONFIG_PREFIXES: &[&str] = &["extensions."];

fn is_protected_config_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    PROTECTED_CONFIG_KEYS
        .iter()
        .any(|protected| key == *protected)
        || PROTECTED_CONFIG_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// `repository_config_set` precondition (docs/03 §2): a well-formed key that is
/// not in the protected managed set.
fn validate_config_key(key: &str) -> Result<(), Violation> {
    let reject = |message: String| {
        Err(Violation::new(ErrorCode::InvalidRequest, message).with_param("key", key.to_string()))
    };

    if key.trim().is_empty() {
        return reject("A config key is required".to_string());
    }
    if key.trim() != key {
        return reject(format!("Config key has surrounding whitespace: '{key}'"));
    }
    if key.contains(|c: char| c.is_whitespace() || c == '=' || c == '\n' || c == '\r') {
        return reject(format!("'{key}' is not a valid config key"));
    }
    if !key.contains('.') || key.starts_with('.') || key.ends_with('.') {
        return reject(format!("'{key}' is not a section.key config name"));
    }
    if is_protected_config_key(key) {
        return reject(format!(
            "'{key}' is managed by Meridian; change it through its own settings"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_destination_is_the_last_path_segment_without_dot_git() {
        let cases = [
            ("https://github.com/owner/meridian.git", "meridian"),
            ("https://github.com/owner/meridian", "meridian"),
            ("https://github.com/owner/meridian/", "meridian"),
            ("ssh://git@host:2222/team/app.git", "app"),
            // scp-style has no slash to split on before the path.
            ("git@github.com:owner/meridian.git", "meridian"),
            ("git@host:repo.git", "repo"),
            ("  https://example.com/x/y.git  ", "y"),
        ];
        for (url, expected) in cases {
            assert_eq!(
                repository_clone_suggest_destination(url.to_string()),
                expected,
                "{url}"
            );
        }
    }

    #[test]
    fn clone_destination_never_returns_a_path_traversal_or_empty_name() {
        for url in [
            "",
            "   ",
            "/",
            "https://example.com/..",
            "https://example.com/.",
        ] {
            assert_eq!(
                repository_clone_suggest_destination(url.to_string()),
                "repository",
                "{url:?}"
            );
        }
    }

    #[test]
    fn protected_and_malformed_config_keys_are_refused() {
        // Owned by other commands (docs/03 §2 protected managed set).
        assert!(validate_config_key("user.name").is_err());
        assert!(
            validate_config_key("USER.name").is_err(),
            "keys are case-insensitive"
        );
        assert!(validate_config_key("core.bare").is_err());
        assert!(validate_config_key("extensions.worktreeConfig").is_err());

        // Malformed keys are refused before git sees them.
        assert!(validate_config_key("").is_err());
        assert!(validate_config_key("  ").is_err());
        assert!(validate_config_key(" nodot ").is_err());
        assert!(validate_config_key("nodot").is_err());
        assert!(validate_config_key(".value").is_err());
        assert!(validate_config_key("section.").is_err());
        assert!(validate_config_key("bad key.name").is_err());
        assert!(validate_config_key("bad=key.name").is_err());

        // Ordinary keys pass.
        assert!(validate_config_key("fetch.prune").is_ok());
        assert!(validate_config_key("remote.origin.fetch").is_ok());
        assert!(validate_config_key("pull.rebase").is_ok());
    }

    #[test]
    fn discover_root_precondition_reports_missing_and_file_paths() {
        let dir =
            std::env::temp_dir().join(format!("meridian-commands-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("file.txt");
        std::fs::write(&file, "x").unwrap();

        assert!(validate_discover_root(&dir).is_ok());
        assert!(validate_discover_root(&dir.join("nope")).is_err());
        assert!(validate_discover_root(&file).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
