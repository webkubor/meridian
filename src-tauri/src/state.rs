use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use crate::commands::{Command, ExecCx};
use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::repository::RepoInfo;
use crate::engine;
use crate::events::{CollectingEventSink, EventSink};
use crate::logging;
use crate::queue::RepoQueueManager;
use crate::validation::ValidateCx;

/// A shared, individually locked repository handle as stored in the registry.
type RepoHandle = Arc<StdMutex<git2::Repository>>;
/// The open-repository registry, keyed by canonical gitdir.
type RepoRegistry = HashMap<RepoId, RepoHandle>;

/// Application state: the open-repository registry plus the per-repository
/// queues.
///
/// Commands never touch these directly — they go through [`AppState::run`],
/// which performs the fixed sequence from docs/02 §3.
pub struct AppState {
    repos: StdMutex<RepoRegistry>,
    queue: RepoQueueManager,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            repos: StdMutex::new(HashMap::new()),
            queue: RepoQueueManager::new(),
        }
    }

    /// Register a repository under its canonical gitdir — the identity the
    /// registry, the queue, and later the SQLite store all key on.
    pub fn insert_repo(&self, repo: git2::Repository) -> Result<RepoInfo, ErrorPayload> {
        let info = engine::repository::info(&repo)?;
        self.lock_registry()?
            .insert(info.id.clone(), Arc::new(StdMutex::new(repo)));
        Ok(info)
    }

    /// The fixed sequence from docs/02 §3:
    ///
    /// 1. **Resolve** `RepoId` → repo handle.
    /// 2. **Enqueue** on the repository's `RepoQueue` (serialized).
    /// 3. **Validate** — a `Violation` becomes an `ErrorPayload`.
    /// 4. **Policy hooks** — layered on later; the trait already carries
    ///    `risk()` and `sender()` so both attachment points exist.
    /// 5. **Snapshot** — later, same reason.
    /// 6. **Execute** on a blocking thread.
    /// 7. **Emit** domain events from inside `execute`.
    pub async fn run<C, S>(&self, sink: &S, cmd: C) -> Result<C::Output, ErrorPayload>
    where
        C: Command,
        S: EventSink + ?Sized,
    {
        let name = logging::command_name::<C>();
        let started = std::time::Instant::now();
        let result = self.run_inner(sink, cmd).await;
        logging::instrument(&name, started, &result);
        result
    }

    /// Run a real command descriptor without constructing a Tauri window.
    ///
    /// This uses the same registry, queue, validator, blocking executor,
    /// diagnostics instrumentation, and command execution as [`AppState::run`];
    /// only app-event delivery is replaced with a collecting sink.
    pub async fn run_headless<C>(
        &self,
        cmd: C,
    ) -> (
        Result<C::Output, ErrorPayload>,
        Vec<crate::events::AppEvent>,
    )
    where
        C: Command,
    {
        let sink = CollectingEventSink::new();
        let result = self.run(&sink, cmd).await;
        (result, sink.take())
    }

    /// The sequence documented on [`AppState::run`]. Split out so the timing
    /// and the outcome line wrap every exit — including the early `?`s, which
    /// are the ones that used to fail silently.
    async fn run_inner<C, S>(&self, sink: &S, cmd: C) -> Result<C::Output, ErrorPayload>
    where
        C: Command,
        S: EventSink + ?Sized,
    {
        // 1. Resolve
        let repo_id = cmd.repo_id();
        let handle = self.handle(&repo_id)?;

        // 2. Enqueue
        let _turn = self.queue.enqueue(&repo_id).await;

        let sink = sink.shared();
        let sender = cmd.sender();

        let task = tokio::task::spawn_blocking(move || -> Result<C::Output, ErrorPayload> {
            let repo_guard = handle.lock().map_err(|_| {
                ErrorPayload::new(ErrorCode::Unclassified, "Repository lock poisoned")
                    .with_domain(ErrorDomain::Internal)
            })?;
            let repo: &git2::Repository = &repo_guard;

            // 3. Validate
            let validate_cx = ValidateCx { repo, sender };
            cmd.validate(&validate_cx).map_err(ErrorPayload::from)?;

            // 4./5. Policy hooks and snapshots attach here in later phases.

            // 6. Execute (events are emitted from inside `execute`).
            let exec_cx = ExecCx {
                repo_id,
                repo,
                app: sink.as_ref(),
            };
            cmd.execute(&exec_cx)
        });

        // 7. Propagate the result, translating a panicked task. The panic
        // message is carried in `detail` because that is the field the log
        // shows and the copy-report action sends; the plain `JoinError` text
        // ("task N panicked") says nothing about what broke.
        task.await.map_err(|join_err| {
            let detail = match join_err.try_into_panic() {
                Ok(payload) => {
                    let message = logging::panic_message(payload.as_ref());
                    format!("The command task panicked: {message}")
                }
                Err(join_err) => join_err.to_string(),
            };
            logging::record_panic_detail(&detail);
            ErrorPayload::new(ErrorCode::Cancelled, "The command task did not complete")
                .with_detail(detail)
                .with_domain(ErrorDomain::Internal)
        })?
    }

    fn handle(&self, repo: &RepoId) -> Result<RepoHandle, ErrorPayload> {
        self.lock_registry()?.get(repo).cloned().ok_or_else(|| {
            ErrorPayload::new(
                ErrorCode::InvalidRepository,
                format!("Repository is not open: {repo}"),
            )
            .with_param("repo", repo.as_str())
        })
    }

    fn lock_registry(&self) -> Result<std::sync::MutexGuard<'_, RepoRegistry>, ErrorPayload> {
        self.repos.lock().map_err(|_| {
            ErrorPayload::new(ErrorCode::Unclassified, "Repository registry poisoned")
                .with_domain(ErrorDomain::Internal)
        })
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
