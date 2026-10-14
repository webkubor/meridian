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
        // Held for the whole call, queue wait included: a command stuck behind
        // another one is exactly the case no completion line can ever report.
        let _watchdog = Watchdog::start(&name);
        let result = self.run_inner(&name, sink, cmd).await;
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
    async fn run_inner<C, S>(&self, name: &str, sink: &S, cmd: C) -> Result<C::Output, ErrorPayload>
    where
        C: Command,
        S: EventSink + ?Sized,
    {
        // 1. Resolve
        let repo_id = cmd.repo_id();
        let handle = self.handle(&repo_id)?;

        // 2. Enqueue. The wait is logged at debug: "the app hung" is most often
        // a command waiting for its turn, and nothing else shows that.
        let queued = std::time::Instant::now();
        let _turn = self.queue.enqueue(&repo_id).await;
        log::debug!(
            target: "meridian::command",
            "command={name} queue_wait_ms={}",
            queued.elapsed().as_millis()
        );

        let sink = sink.shared();
        let sender = cmd.sender();
        let task_name = name.to_string();

        let task = tokio::task::spawn_blocking(move || -> Result<C::Output, ErrorPayload> {
            // A panic in an earlier command poisons this lock. The repository
            // handle is only a view onto state that lives on disk, so recover
            // it instead of failing every later command on this repository
            // with "lock poisoned" until the user reopens it.
            let repo_guard = handle.lock().unwrap_or_else(|poisoned| {
                log::warn!(
                    target: "meridian::command",
                    "command={task_name} recovered a repository lock poisoned by an earlier panic"
                );
                poisoned.into_inner()
            });
            let repo: &git2::Repository = &repo_guard;

            // 3. Validate
            let validate_cx = ValidateCx { repo, sender };
            let validated = cmd.validate(&validate_cx).map_err(ErrorPayload::from);
            log::debug!(
                target: "meridian::command",
                "command={task_name} validation={}",
                if validated.is_ok() { "ok" } else { "refused" }
            );
            validated?;

            // 4./5. Policy hooks and snapshots attach here in later phases.

            // 6. Execute (events are emitted from inside `execute`).
            let exec_cx = ExecCx {
                repo_id,
                repo,
                app: sink.as_ref(),
            };
            let executing = std::time::Instant::now();
            let result = cmd.execute(&exec_cx);
            log::debug!(
                target: "meridian::command",
                "command={task_name} execute_ms={}",
                executing.elapsed().as_millis()
            );
            result
        });

        // 7. Propagate the result, translating a panicked task. The panic
        // message is carried in `detail` because that is the field the log
        // shows and the copy-report action sends; the plain `JoinError` text
        // ("task N panicked") says nothing about what broke. It is a bug in
        // Meridian, not something the user cancelled, so it is reported as an
        // internal error — a `Cancelled` code would render as "Cancelled."
        task.await.map_err(|join_err| {
            let detail = match join_err.try_into_panic() {
                Ok(payload) => {
                    let message = logging::panic_message(payload.as_ref());
                    format!("The command task panicked: {message}")
                }
                Err(join_err) => join_err.to_string(),
            };
            logging::record_panic_detail(&detail);
            ErrorPayload::new(
                ErrorCode::Unclassified,
                "An internal error stopped this command",
            )
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

/// Logs a warning while a command is still running, so a hang leaves a trace
/// before it ends — a completion line is useless for a command that never
/// completes. Dropping it (the command finished) cancels the timer.
struct Watchdog(tokio::task::JoinHandle<()>);

/// First warning, then one per interval. Network commands legitimately take
/// seconds, so the first fuse is long and the repeats are sparse.
const WATCHDOG_FIRST: std::time::Duration = std::time::Duration::from_secs(15);
const WATCHDOG_REPEAT: std::time::Duration = std::time::Duration::from_secs(30);

impl Watchdog {
    fn start(name: &str) -> Option<Self> {
        Self::with_delays(name, WATCHDOG_FIRST, WATCHDOG_REPEAT)
    }

    /// `None` outside a tokio runtime: diagnostics never fail the command.
    fn with_delays(
        name: &str,
        first: std::time::Duration,
        repeat: std::time::Duration,
    ) -> Option<Self> {
        let runtime = tokio::runtime::Handle::try_current().ok()?;
        let name = name.to_string();
        Some(Self(runtime.spawn(async move {
            let started = std::time::Instant::now();
            tokio::time::sleep(first).await;
            loop {
                log::warn!(
                    target: "meridian::command",
                    "command={name} still running after {} s",
                    started.elapsed().as_secs()
                );
                tokio::time::sleep(repeat).await;
            }
        })))
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;

    /// Collects every record so a test can assert on what was logged. Installed
    /// once for this test binary; only this module uses the global logger.
    struct Capture(Mutex<Vec<String>>);

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if let Ok(mut lines) = self.0.lock() {
                lines.push(format!("{}", record.args()));
            }
        }
        fn flush(&self) {}
    }

    static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

    fn captured() -> Vec<String> {
        CAPTURE.0.lock().expect("capture lock").clone()
    }

    #[tokio::test]
    async fn a_command_that_outlives_the_fuse_is_reported_while_it_runs() {
        let _ = log::set_logger(&CAPTURE);
        log::set_max_level(log::LevelFilter::Debug);

        let watchdog = Watchdog::with_delays(
            "slow_probe_command",
            Duration::from_millis(20),
            Duration::from_millis(20),
        )
        .expect("inside a runtime");
        tokio::time::sleep(Duration::from_millis(150)).await;
        let during = captured()
            .into_iter()
            .filter(|line| line.contains("slow_probe_command") && line.contains("still running"))
            .count();
        assert!(during >= 2, "expected repeated warnings, got {during}");

        // Finishing the command cancels the timer: no more lines afterwards.
        drop(watchdog);
        let frozen = captured().len();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(captured().len(), frozen);
    }

    #[test]
    fn the_watchdog_is_inert_outside_a_runtime() {
        assert!(Watchdog::with_delays("probe", Duration::ZERO, Duration::ZERO).is_none());
    }
}
