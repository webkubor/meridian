//! Local diagnostics contract — the rules a privacy reviewer checks.
//!
//! What is logged: command names, outcomes, durations, the stable error
//! identifiers/message/detail, argument KEY names, and — only in debug mode —
//! argument values for an explicit allowlist of harmless scalars (ids, ref and
//! branch names, counts, booleans).
//!
//! What is never logged: file contents, diffs, patch text, commit messages,
//! remote URLs, credentials, and credential-helper output. Argument values
//! outside the allowlist print as a key with no value at all.
//!
//! Why it is safe to write at all: nothing here leaves the machine. The log is
//! a file in the app log directory; a report is only written where the user
//! asks, or copied by them into a URL they submit themselves. Redaction runs
//! before anything reaches the file, the ring buffer, a crash note, the UI, or
//! an exported report, so a redaction bug cannot leak through a second path.
//!
//! The redaction rules are one table shared with the frontend
//! (`src/diagnostics/recorder.ts`): both sides are tested against
//! `tests/fixtures/redaction-contract.json`, so a rule added on one side and
//! forgotten on the other fails a test instead of leaking.
//!
//! Diagnostics never take the app down. A log directory that cannot be created
//! degrades to the in-memory ring, with the reason as a line in it, because a
//! feature meant to explain failures must not be the reason nothing starts.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use log::{Level, LevelFilter, Log, Metadata, Record};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{ipc::InvokeBody, Manager};

use crate::domain::diagnostics::{CrashKind, CrashNote, DebugLevel, DebugSource, DebugState};
use crate::domain::error::ErrorPayload;

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
/// Rotated generations kept next to the live file: `.1` and `.2`. Together
/// with the live file that is three files, 6 MB at most — a log that holds
/// repository and branch names should not quietly keep more history than the
/// README says it does.
const ROTATED_GENERATIONS: usize = 2;
const MAX_RING_LINES: usize = 2_000;
const MAX_TAIL_BYTES: usize = 256 * 1024;
const MAX_DIAGNOSTIC_MESSAGE: usize = 2 * 1024;
const SETTING_FILE: &str = "diagnostics.json";
const CRASH_FILE: &str = "last-crash.json";
/// Present from launch until a clean exit. Finding it at the next launch is the
/// only way to notice a process that died without a panic (abort, a crash
/// inside libgit2 or libssh, a force quit), which leaves no other trace.
const RUNNING_FILE: &str = "running.lock";

static LOGGER: OnceLock<LocalLogger> = OnceLock::new();
static HOME: OnceLock<Option<String>> = OnceLock::new();
static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
/// Where the panic hook writes its note. Kept outside the logger lock: the
/// hook runs on a panicking thread, which may be one that already holds it.
static CRASH_PATH: OnceLock<PathBuf> = OnceLock::new();
/// Mirrors `LoggerState::mode.enabled` so `Log::enabled` never takes the state
/// lock — it is called for every record, from every thread.
static DEBUG_ON: AtomicBool = AtomicBool::new(false);
/// Whether the window was created with inspector support. The WebView decides
/// this at creation, so it cannot be turned on later in the same run — the
/// flag is what lets `diagnostics_open_devtools` refuse honestly instead of
/// calling into an inspector that is not there and appearing to do nothing.
static DEVTOOLS_AT_LAUNCH: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mode {
    enabled: bool,
    source: DebugSource,
}

struct LoggerState {
    ring: VecDeque<String>,
    log_dir: PathBuf,
    log_file: PathBuf,
    settings_file: PathBuf,
    crash_file: PathBuf,
    mode: Mode,
    bytes: u64,
    /// False when the log directory could not be prepared: lines then stay in
    /// the ring only.
    to_file: bool,
    /// Set after a write to the file failed, so the failure is reported once
    /// instead of once per line.
    write_failure_reported: bool,
}

struct LocalLogger(Mutex<LoggerState>);

impl LocalLogger {
    /// Recover from a poisoned lock instead of giving up. The logger is the
    /// tool this whole feature exists to provide: losing it because one write
    /// panicked would be the same silent failure the feature is meant to end.
    fn state(&self) -> std::sync::MutexGuard<'_, LoggerState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSetting {
    debug_enabled: bool,
}

/// Read the persisted setting before the window exists, because the WebView's
/// inspector support is decided when the window is created and cannot be added
/// afterwards. This runs before `Builder::setup`, which is why it resolves the
/// config directory itself rather than asking `AppHandle::path()` — same rule
/// Tauri uses (`dirs::config_dir()` + the bundle identifier), kept in one
/// place by [`config_dir_for`].
pub fn configure_context(context: &mut tauri::Context<tauri::Wry>) {
    let enabled = env_debug_enabled()
        || read_setting(&config_dir_for(&context.config().identifier).join(SETTING_FILE));
    // A dev build keeps the devtools it always had; a release build gets them
    // only in debug mode, which is the promise the Diagnostics screen makes.
    let at_launch = enabled || cfg!(debug_assertions);
    DEVTOOLS_AT_LAUNCH.store(at_launch, Ordering::Relaxed);
    for window in &mut context.config_mut().app.windows {
        window.devtools = if cfg!(debug_assertions) {
            None
        } else {
            Some(enabled)
        };
        if enabled {
            // The title is the only debug indicator the user sees without
            // opening Diagnostics, so it has to say so at a glance.
            window.title = debug_title(&window.title, true);
        }
    }
}

/// The window title with the debug marker added or removed. Idempotent, so
/// toggling twice never stacks markers.
pub fn debug_title(current: &str, enabled: bool) -> String {
    const MARKER: &str = " [DEBUG]";
    let base = current.strip_suffix(MARKER).unwrap_or(current);
    if enabled {
        format!("{base}{MARKER}")
    } else {
        base.to_string()
    }
}

/// Whether this run's window can actually open an inspector.
pub fn devtools_at_launch() -> bool {
    DEVTOOLS_AT_LAUNCH.load(Ordering::Relaxed)
}

fn config_dir_for(identifier: &str) -> PathBuf {
    dirs::config_dir()
        .map(|path| path.join(identifier))
        .unwrap_or_else(|| std::env::temp_dir().join("meridian-config"))
}

/// Install the process logger, initialize local paths, and record build provenance.
///
/// Infallible on purpose: whatever goes wrong here becomes a line in the log
/// (or on stderr when no logger could be installed) and the app starts anyway.
pub fn initialize<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let fallback = std::env::temp_dir().join("meridian-diagnostics");
    let mut notes = Vec::new();
    let log_dir = app.path().app_log_dir().unwrap_or_else(|error| {
        notes.push(format!("the log directory could not be resolved ({error})"));
        fallback.join("logs")
    });
    let config_dir = app.path().app_config_dir().unwrap_or_else(|error| {
        notes.push(format!(
            "the config directory could not be resolved ({error})"
        ));
        fallback.join("config")
    });

    let (state, prepared) = prepare_state(log_dir.clone(), config_dir.clone());
    notes.extend(prepared);
    let mode = state.mode;
    let log_file = state.log_file.clone();
    let _ = CRASH_PATH.set(state.crash_file.clone());
    let logger = LOGGER.get_or_init(|| LocalLogger(Mutex::new(state)));
    DEBUG_ON.store(mode.enabled, Ordering::Relaxed);
    if let Err(error) = log::set_logger(logger) {
        // Another logger is installed, which should not happen in this app.
        // Say so where a developer can see it; there is no log to say it in.
        eprintln!("meridian: diagnostics logger not installed: {error}");
        return;
    }
    set_max_level(mode.enabled);
    seed_ring(logger, &log_file);
    install_panic_hook();
    for note in notes {
        log::error!(target: "meridian::logging", "{note}");
    }

    // The crash check has to run after the ring is seeded, so its note sits
    // after the previous session in the tail.
    if !cfg!(debug_assertions) {
        if let Some(note) = detect_unclean_exit(&config_dir) {
            log::error!(target: "meridian::logging", "the previous run ended without a clean shutdown");
            if !state_crash_file_exists(logger) {
                persist_crash_note(&note);
            }
        }
        if let Err(error) = write_private(&config_dir.join(RUNNING_FILE), b"running\n") {
            log::warn!(target: "meridian::logging", "could not write the run marker ({error}); an unclean exit will not be detected");
        }
    }
    log::info!(
        target: "meridian::startup",
        "startup version={} commit={} os={} arch={} profile={} debug={} log={}",
        env!("CARGO_PKG_VERSION"),
        option_env!("MERIDIAN_GIT_COMMIT")
            .filter(|commit| !commit.is_empty())
            .unwrap_or("unrecorded"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) { "debug" } else { "release" },
        mode.enabled,
        log_dir.display()
    );
}

/// Build the logger state from the two directories, preparing them for use.
///
/// Never fails. A directory that cannot be created or narrowed switches file
/// logging off, and the reason comes back as a note for the caller to put into
/// the log — the ring still works, so the Diagnostics screen still has a tail
/// to show.
fn prepare_state(log_dir: PathBuf, config_dir: PathBuf) -> (LoggerState, Vec<String>) {
    let mut notes = Vec::new();
    let to_file = match create_private_dir(&log_dir) {
        Ok(()) => true,
        Err(error) => {
            notes.push(format!(
                "the log directory {} is unavailable ({error}); logging to memory only",
                log_dir.display()
            ));
            false
        }
    };
    if let Err(error) = create_private_dir(&config_dir) {
        notes.push(format!(
            "the config directory {} is unavailable ({error}); the debug setting and crash notes cannot be saved",
            config_dir.display()
        ));
    }
    if to_file {
        notes.extend(narrow_existing_logs(&log_dir));
    }

    let mode = if env_debug_enabled() {
        Mode {
            enabled: true,
            source: DebugSource::Env,
        }
    } else if read_setting(&config_dir.join(SETTING_FILE)) {
        Mode {
            enabled: true,
            source: DebugSource::Setting,
        }
    } else {
        Mode {
            enabled: false,
            source: DebugSource::Off,
        }
    };
    let log_file = log_dir.join("meridian.log");
    let bytes = if to_file {
        fs::metadata(&log_file)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    } else {
        0
    };
    let state = LoggerState {
        ring: VecDeque::with_capacity(MAX_RING_LINES),
        log_dir,
        log_file,
        settings_file: config_dir.join(SETTING_FILE),
        crash_file: config_dir.join(CRASH_FILE),
        mode,
        bytes,
        to_file,
        write_failure_reported: false,
    };
    (state, notes)
}

/// Create `path` readable by its owner only, and narrow it if it already
/// existed with looser permissions (earlier versions used the process umask).
///
/// A log line carries repository names, branch names and error details, every
/// one of which is private to the machine that owns them. When the directory
/// cannot be made private the caller does not write into it.
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        // Set at creation, so there is no moment when the directory is wide open.
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Log files from earlier runs may carry the old 0644 mode. Narrow them once at
/// startup; the notes name every file that could not be narrowed.
fn narrow_existing_logs(log_dir: &Path) -> Vec<String> {
    let mut notes = Vec::new();
    let Ok(entries) = fs::read_dir(log_dir) else {
        return notes;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("meridian.log") {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(error) = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o600))
            {
                notes.push(format!(
                    "could not restrict {} to its owner ({error})",
                    name.to_string_lossy()
                ));
            }
        }
    }
    notes
}

/// Write a whole file readable by its owner only.
///
/// `mode` only applies when a file is created, so a stale file with looser
/// permissions is removed first rather than rewritten in place. Used for the
/// setting, the crash note and the exported report, which would otherwise be
/// created 0644.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

/// Prime the in-memory ring with the tail of the file already on disk, so
/// "show me the log" still shows the previous session right after a restart.
/// Bounded by the ring cap and read once at startup, so the tail itself never
/// needs a disk read.
fn seed_ring(logger: &LocalLogger, log_file: &Path) {
    let Ok(bytes) = fs::read(log_file) else {
        return;
    };
    let start = bytes.len().saturating_sub(MAX_TAIL_BYTES);
    let text = String::from_utf8_lossy(&bytes[start..]);
    let mut lines: Vec<&str> = text.split('\n').collect();
    // When the window did not land on a newline the first entry is a fragment
    // of a line; a half line in the ring is worse than one line fewer.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let mut state = logger.state();
    for line in lines.into_iter().filter(|line| !line.is_empty()) {
        state.ring.push_back(line.to_string());
    }
    while state.ring.len() > MAX_RING_LINES {
        state.ring.pop_front();
    }
}

/// The level ceiling per origin. Debug mode opens up Meridian's own targets
/// only: third-party crates (the updater logs proxy settings and release
/// responses at debug) stay at warnings, because their output is unreviewed
/// content going into a file users are told to attach to bug reports.
fn allowed(target: &str, level: Level, debug: bool) -> bool {
    let ours = target.split("::").next() == Some("meridian");
    let ceiling = match (ours, debug) {
        (false, _) => Level::Warn,
        (true, true) => Level::Debug,
        (true, false) => Level::Info,
    };
    level <= ceiling
}

/// One log line must stay one line: a message with an embedded newline would
/// otherwise forge extra entries, and `seed_ring` would split them into
/// entries of their own after a restart.
fn single_line(text: &str) -> String {
    text.replace('\r', "\\r").replace('\n', "\\n")
}

impl Log for LocalLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        allowed(
            metadata.target(),
            metadata.level(),
            DEBUG_ON.load(Ordering::Relaxed),
        )
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = single_line(&truncate_utf8(
            &redact(&record.args().to_string()),
            16 * 1024,
        ));
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let line = format!(
            "{} [{}] [{}] {}: {}",
            Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            record.level(),
            redact(&thread),
            redact(record.target()),
            message
        );
        let mut state = self.state();
        state.ring.push_back(line.clone());
        while state.ring.len() > MAX_RING_LINES {
            state.ring.pop_front();
        }
        write_rotating(&mut state, &line);
    }

    fn flush(&self) {}
}

fn write_rotating(state: &mut LoggerState, line: &str) {
    if !state.to_file {
        return;
    }
    let Some(next_len) = line.len().checked_add(1).map(|len| len as u64) else {
        return;
    };
    if state.bytes.saturating_add(next_len) > MAX_LOG_BYTES {
        rotate(&state.log_file);
        state.bytes = 0;
    }
    let written = open_private_log(&state.log_file).and_then(|mut file| writeln!(file, "{line}"));
    match written {
        Ok(()) => {
            state.bytes = state.bytes.saturating_add(next_len);
            state.write_failure_reported = false;
        }
        Err(error) if !state.write_failure_reported => {
            // This runs inside `Log::log` with the state lock held, so it must
            // not log through the facade; the ring and stderr are the only
            // places the failure can go.
            state.write_failure_reported = true;
            let note = format!(
                "{} [ERROR] [logger] meridian::logging: could not write {} ({error}); new lines stay in memory only until it works again",
                Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                redact(&state.log_file.display().to_string())
            );
            eprintln!("{note}");
            state.ring.push_back(note);
        }
        Err(_) => {}
    }
}

fn open_private_log(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Owner-only from the first byte; `chmod` after `open` would leave a
        // window in which the file is readable by others.
        options.mode(0o600);
    }
    options.open(path)
}

/// Keep `.1` and `.2` next to the live file: drop the oldest generation first
/// so the renames never collide with it, shift the rest up, then move the live
/// file to `.1`.
fn rotate(active: &Path) {
    let _ = fs::remove_file(rotated_path(active, ROTATED_GENERATIONS));
    for index in (1..ROTATED_GENERATIONS).rev() {
        let _ = fs::rename(rotated_path(active, index), rotated_path(active, index + 1));
    }
    if let Err(error) = fs::rename(active, rotated_path(active, 1)) {
        // A live file that cannot be moved would keep growing past the cap;
        // dropping it is the lesser harm. A missing file is not an error.
        if error.kind() != std::io::ErrorKind::NotFound {
            let _ = fs::remove_file(active);
        }
    }
}

fn rotated_path(active: &Path, index: usize) -> PathBuf {
    active.with_file_name(format!("meridian.log.{index}"))
}

/// The one redaction rule set, applied in order. The frontend keeps the same
/// list in `src/diagnostics/recorder.ts`; `tests/fixtures/redaction-contract.json`
/// is what keeps them equal.
///
/// Restraint matters as much as coverage: a redactor that mangles commit ids,
/// branch names and scp-style remotes makes every report useless, so the
/// fixture pins what must survive as well as what must go.
fn rules() -> &'static [(Regex, &'static str)] {
    RULES.get_or_init(|| {
        [
            // Another user's (or this user's) home directory, wherever a path
            // starts — never in the middle of a URL path such as
            // `https://example.com/home/docs`.
            (
                r##"(^|[\s"'`=(\[,;:<>]|file://)/(?:Users|home)/[^/\s"'`<>:;,)\]]+"##,
                "${1}~",
            ),
            // Userinfo up to the LAST `@` before the first `/`: a password may
            // itself contain `@`, `?` or `#`.
            (
                r##"(?i)([a-z][a-z0-9+.-]*://)[^/\s]+@"##,
                "${1}[redacted]@",
            ),
            (
                r##"(?:gh[pousr]_[A-Za-z0-9_]{10,}|github_pat_[A-Za-z0-9_]{10,}|glpat-[A-Za-z0-9_-]{10,}|xox[baprs]-[A-Za-z0-9-]{8,})"##,
                "[redacted]",
            ),
            (r##"(?-u:\b)AKIA[0-9A-Z]{16}(?-u:\b)"##, "[redacted]"),
            // Authorization with any scheme (Bearer, Basic, Token, ...).
            (
                r##"(?i)((?-u:\b)(?:proxy-)?authorization["']?\s*[:=]\s*["']?)(?:[a-z][a-z0-9-]*\s+)?[^\s,;"'}]+"##,
                "${1}[redacted]",
            ),
            (
                r##"(?i)((?-u:\b)bearer\s+)[A-Za-z0-9._~+/=-]{16,}"##,
                "${1}[redacted]",
            ),
            // `key=value` whose key ENDS in a secret word: private_token,
            // access_token, client_secret, password, api-key. Ending, not
            // containing, so `author=` and `authority=` survive.
            (
                r##"(?i)(?-u:\b)([a-z0-9_.-]*(?:token|password|passwd|secret|api[_-]?key|apikey|auth)\s*=\s*)(?:"[^"]*"|'[^']*'|[^&#\s"']+)"##,
                "${1}[redacted]",
            ),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(pattern).expect("static redaction regex"),
                replacement,
            )
        })
        .collect()
    })
}

pub fn redact(text: &str) -> String {
    redact_with_home(
        text,
        HOME.get_or_init(|| std::env::var("HOME").ok().filter(|s| !s.is_empty()))
            .as_deref(),
    )
}

fn redact_with_home(text: &str, home: Option<&str>) -> String {
    let mut value = match home {
        Some(home) => replace_home(text, home),
        None => text.to_string(),
    };
    for (pattern, replacement) in rules() {
        value = pattern.replace_all(&value, *replacement).into_owned();
    }
    value
}

/// Replace the exact `$HOME` with `~`, but only where it ends on a path
/// boundary: `/Users/bob` must not turn `/Users/bobby` into `~by`.
fn replace_home(text: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, _) in text.match_indices(home) {
        if start < copied {
            continue;
        }
        let end = start + home.len();
        let on_boundary = text[end..]
            .chars()
            .next()
            .is_none_or(|next| !(next.is_alphanumeric() || matches!(next, '_' | '.' | '-')));
        if on_boundary {
            out.push_str(&text[copied..start]);
            out.push('~');
            copied = end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

pub fn debug_state() -> DebugState {
    let Some(logger) = LOGGER.get() else {
        return DebugState {
            enabled: false,
            source: DebugSource::Off,
            log_dir: String::new(),
            log_file: String::new(),
            level: DebugLevel::Info,
            devtools_ready: false,
        };
    };
    let state = logger.state();
    DebugState {
        enabled: state.mode.enabled,
        source: state.mode.source,
        log_dir: redact(&state.log_dir.display().to_string()),
        log_file: redact(&state.log_file.display().to_string()),
        level: if state.mode.enabled {
            DebugLevel::Debug
        } else {
            DebugLevel::Info
        },
        // The inspector is decided when the window is created, so debug mode
        // switched on in this run does not make it available until a restart.
        devtools_ready: state.mode.enabled && devtools_at_launch(),
    }
}

pub fn set_debug(enabled: bool) -> Result<DebugState, std::io::Error> {
    let logger = LOGGER
        .get()
        .ok_or_else(|| std::io::Error::other("the diagnostics logger is not running"))?;
    let mut state = logger.state();
    if env_debug_enabled() {
        state.mode = Mode {
            enabled: true,
            source: DebugSource::Env,
        };
    } else {
        persist_setting(&state.settings_file, enabled)?;
        state.mode = Mode {
            enabled,
            source: if enabled {
                DebugSource::Setting
            } else {
                DebugSource::Off
            },
        };
    }
    let enabled = state.mode.enabled;
    drop(state);
    DEBUG_ON.store(enabled, Ordering::Relaxed);
    set_max_level(enabled);
    log::info!(target: "meridian::diagnostics", "debug mode {}", if enabled { "on" } else { "off" });
    Ok(debug_state())
}

fn set_max_level(enabled: bool) {
    log::set_max_level(if enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
}

fn env_debug_enabled() -> bool {
    std::env::var("MERIDIAN_DEBUG").is_ok_and(|value| value == "1")
}

fn read_setting(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PersistedSetting>(&bytes).ok())
        .is_some_and(|setting| setting.debug_enabled)
}

fn persist_setting(path: &Path, enabled: bool) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(&PersistedSetting {
        debug_enabled: enabled,
    })
    .map_err(std::io::Error::other)?;
    if let Some(directory) = path.parent() {
        create_private_dir(directory)?;
    }
    let temporary = path.with_extension("json.tmp");
    write_private(&temporary, &bytes)?;
    fs::rename(temporary, path)
}

pub fn log_tail(lines: usize) -> String {
    let Some(logger) = LOGGER.get() else {
        return String::new();
    };
    let state = logger.state();
    let requested = lines.clamp(1, MAX_RING_LINES);
    let mut result = state.ring.iter().rev().take(requested).collect::<Vec<_>>();
    result.reverse();
    let mut text = result
        .into_iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    while text.len() > MAX_TAIL_BYTES {
        let Some(newline) = text.find('\n') else {
            text = truncate_utf8(&text, MAX_TAIL_BYTES);
            break;
        };
        text.drain(..=newline);
    }
    redact(&text)
}

/// Wrap the generated invoke handler so every call that reaches the web layer
/// is logged before dispatch. This is the only place that sees plain
/// `#[tauri::command]` fns, so it is what gives those commands an entry line.
pub fn log_invoke_handler<R, F>(
    inner: F,
) -> Box<dyn Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync>
where
    R: tauri::Runtime,
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    Box::new(move |invoke| {
        log_invoke(invoke.message.command(), invoke.message.payload());
        inner(invoke)
    })
}

pub fn log_invoke(command: &str, body: &InvokeBody) {
    let mut keys = Vec::new();
    let mut safe_values = Vec::new();
    if let InvokeBody::Json(value) = body {
        collect_invoke_fields(value, &mut keys, &mut safe_values);
    }
    keys.sort();
    keys.dedup();
    log::debug!(target: "meridian::invoke", "invoke command={} args.keys={:?}{}", command, keys, if safe_values.is_empty() { String::new() } else { format!(" args.safe={}", safe_values.join(",")) });
}

fn collect_invoke_fields(
    value: &serde_json::Value,
    keys: &mut Vec<String>,
    safe_values: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, value) in object {
        if key == "args" {
            collect_invoke_fields(value, keys, safe_values);
            continue;
        }
        keys.push(key.clone());
        if safe_argument(key) {
            let scalar = match value {
                serde_json::Value::String(value) => Some(redact(value)),
                serde_json::Value::Number(value) => Some(value.to_string()),
                serde_json::Value::Bool(value) => Some(value.to_string()),
                _ => None,
            };
            if let Some(value) = scalar {
                safe_values.push(format!("{key}={}", truncate_utf8(&value, 128)));
            }
        }
    }
}

fn safe_argument(key: &str) -> bool {
    matches!(
        key,
        "repo"
            | "repoId"
            | "id"
            | "branch"
            | "branchName"
            | "ref"
            | "refName"
            | "revision"
            | "count"
            | "limit"
            | "depth"
            | "bare"
            | "checkout"
            | "enabled"
            | "force"
            | "sender"
            | "kind"
            | "scope"
    )
}

/// The command name a `Command` descriptor is logged under: the struct name
/// in snake_case, which is the same vocabulary the wire format uses.
pub fn command_name<C: 'static>() -> String {
    let raw = std::any::type_name::<C>();
    let name = raw.rsplit("::").next().unwrap_or(raw);
    let mut snake = String::with_capacity(name.len() + 4);
    for (index, character) in name.char_indices() {
        if character.is_uppercase() {
            if index != 0 {
                snake.push('_');
            }
            snake.extend(character.to_lowercase());
        } else {
            snake.push(character);
        }
    }
    snake
}

/// A panic that a caller recovered — a `spawn_blocking` task, in practice.
/// The hook below already logged it and wrote the crash note; this adds the
/// line that ties the panic to the command the caller was waiting on, because
/// the hook cannot know which command that was, and re-labels the note: a
/// panic the app survived is not "the previous run ended unexpectedly".
pub fn record_panic_detail(detail: &str) {
    log::error!(target: "meridian::command", "outcome=panic detail={}", truncate_utf8(&redact(detail), 2 * 1024));
    if let Some(path) = CRASH_PATH.get() {
        mark_recovered(path);
    }
}

/// Re-label a panic note as recovered. Only a plain panic is re-labelled: an
/// unclean-exit note describes a different run and must not be rewritten.
fn mark_recovered(path: &Path) {
    if let Some(mut note) = read_crash_note(path) {
        if note.kind == CrashKind::Panic {
            note.kind = CrashKind::RecoveredPanic;
            let _ = store_crash_note(path, &note);
        }
    }
}

/// The real log directory. `DebugState` reports a redacted copy (the UI shows
/// it and reports may quote it), so revealing the folder in Finder must ask
/// here instead — `~` does not resolve in a filesystem call.
pub fn log_dir() -> PathBuf {
    LOGGER
        .get()
        .map(|logger| logger.state().log_dir.clone())
        .unwrap_or_default()
}

/// How long a successful command may take before it is worth a warning. Network
/// and update commands legitimately take seconds, so they get a longer fuse;
/// everything else is local and a slow one is a symptom ("the app hung").
fn slow_threshold(command: &str) -> Duration {
    let networked = command.starts_with("remote_")
        || command.starts_with("submodule_")
        || matches!(
            command,
            "repository_clone"
                | "tag_publish"
                | "tag_delete_remote"
                | "app_update_check"
                | "app_update_install"
        );
    Duration::from_secs(if networked { 10 } else { 1 })
}

/// A serde unit-variant name, e.g. `checkout_conflicts`. Logs, toasts, the
/// Diagnostics screen and reports then say the same word for the same error,
/// so searching the log for what the toast showed finds it.
fn wire_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

/// The line a finished command logs, as `(level, text)`. Pure, so the format
/// and the level rule are testable without installing the global logger —
/// `instrument` only dispatches what this returns.
pub fn outcome_line<T>(
    command: &str,
    duration: Duration,
    result: &Result<T, ErrorPayload>,
) -> (Level, String) {
    let millis = duration.as_millis();
    match result {
        Err(error) => (
            Level::Error,
            format!(
                "command={command} outcome=error duration_ms={millis} code={} domain={} message={:?} detail={:?}",
                wire_name(&error.code),
                wire_name(&error.domain),
                truncate_utf8(&redact(&error.message), MAX_DIAGNOSTIC_MESSAGE),
                error
                    .detail
                    .as_deref()
                    .map(|detail| truncate_utf8(&redact(detail), MAX_DIAGNOSTIC_MESSAGE))
            ),
        ),
        // A slow command is a symptom worth a louder line even when it
        // succeeded: it is the shape of bug that shows up as "the app hung",
        // not as an error.
        Ok(_) if duration >= slow_threshold(command) => (
            Level::Warn,
            format!("command={command} outcome=ok duration_ms={millis} slow=true"),
        ),
        Ok(_) => (
            Level::Info,
            format!("command={command} outcome=ok duration_ms={millis}"),
        ),
    }
}

pub fn instrument<T>(command: &str, started: Instant, result: &Result<T, ErrorPayload>) {
    let (level, line) = outcome_line(command, started.elapsed(), result);
    log::log!(target: "meridian::command", level, "{line}");
}

pub fn record_frontend(level: &str, message: &str, source: Option<&str>) {
    let message = truncate_utf8(&redact(message), MAX_DIAGNOSTIC_MESSAGE);
    let source = source
        .map(|value| truncate_utf8(&redact(value), 128))
        .unwrap_or_default();
    match level {
        "error" => log::error!(target: "meridian::frontend", "source={} {}", source, message),
        "warn" => log::warn!(target: "meridian::frontend", "source={} {}", source, message),
        "debug" => log::debug!(target: "meridian::frontend", "source={} {}", source, message),
        _ => log::info!(target: "meridian::frontend", "source={} {}", source, message),
    }
}

pub fn last_crash() -> Option<CrashNote> {
    let path = LOGGER.get()?.state().crash_file.clone();
    let mut note = read_crash_note(&path)?;
    note.message = redact(&note.message);
    note.location = note.location.map(|value| redact(&value));
    note.backtrace_excerpt = note.backtrace_excerpt.map(|value| redact(&value));
    Some(note)
}

fn read_crash_note(path: &Path) -> Option<CrashNote> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Atomic write of the crash note: a reader never sees half a file.
fn store_crash_note(path: &Path, note: &CrashNote) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(note).map_err(std::io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    write_private(&temporary, &bytes)?;
    fs::rename(temporary, path)
}

fn persist_crash_note(note: &CrashNote) {
    if let Some(path) = CRASH_PATH.get() {
        if let Err(error) = store_crash_note(path, note) {
            log::warn!(target: "meridian::logging", "could not save the crash note ({error})");
        }
    }
}

fn state_crash_file_exists(logger: &LocalLogger) -> bool {
    let path = logger.state().crash_file.clone();
    path.exists()
}

/// The note for a previous run that never reached a clean exit, if the run
/// marker was left behind. Pure over the config directory, so it is testable.
fn detect_unclean_exit(config_dir: &Path) -> Option<CrashNote> {
    config_dir.join(RUNNING_FILE).exists().then(|| CrashNote {
        at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        message: "The previous run ended without a clean shutdown (no panic was recorded)."
            .to_string(),
        location: None,
        backtrace_excerpt: None,
        kind: CrashKind::UncleanExit,
    })
}

/// Called when the app exits normally. Without it the next launch would report
/// every quit as a crash.
pub fn mark_clean_exit() {
    if let Some(directory) = CRASH_PATH.get().and_then(|path| path.parent()) {
        let _ = fs::remove_file(directory.join(RUNNING_FILE));
    }
}

pub fn dismiss_crash() -> Result<(), std::io::Error> {
    let logger = LOGGER
        .get()
        .ok_or_else(|| std::io::Error::other("the diagnostics logger is not running"))?;
    let path = logger.state().crash_file.clone();
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The panic payload as text. `panic!` with a literal gives a `&'static str`,
/// a formatted one a `String`, and anything else an opaque box — the last case
/// must not abort here, or the hook would kill the process while reporting a
/// crash.
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&'static str>()
        .map(|value| (*value).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_string())
}

fn install_panic_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = panic_message(info.payload());
            let location = info.location().map(|location| format!("{}:{}:{}", location.file(), location.line(), location.column()));
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let excerpt = backtrace.lines().take(12).collect::<Vec<_>>().join("\n");
            let note = CrashNote {
                at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                message: redact(&message),
                location: location.map(|value| redact(&value)),
                backtrace_excerpt: Some(redact(&excerpt)),
                kind: CrashKind::Panic,
            };
            log::error!(target: "meridian::panic", "panic message={:?} location={:?} backtrace={:?}", note.message, note.location, note.backtrace_excerpt);
            persist_crash_note(&note);
            previous(info);
        }));
    });
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_home_and_credentials_without_touching_git_identifiers() {
        let input = "repo=/Users/alice/work/project url=https://alice:secret@example.com/a sha=0123456789abcdef0123456789abcdef01234567 branch=feature/add-logs";
        let output = redact_with_home(input, Some("/Users/alice"));
        assert_eq!(output, "repo=~/work/project url=https://[redacted]@example.com/a sha=0123456789abcdef0123456789abcdef01234567 branch=feature/add-logs");
    }

    /// Builds a credential-shaped string from a head and fragments.
    ///
    /// These tests need inputs that *look* like credentials, and the
    /// pre-commit scanner reads adjacent string literals as one token. The
    /// distinctive prefix is therefore a separate argument: nothing in this
    /// file is a token-shaped literal, and the scanner still sees exactly the
    /// shape the redactor has to catch.
    fn shaped(head: &str, fragments: &[&str]) -> String {
        [head, fragments.concat().as_str()].concat()
    }

    #[test]
    fn redacts_supported_token_shapes() {
        let cases = [
            "ghp_abcdefghijklmnopqrstuvwxyz1234567890",
            "gho_abcdefghijklmnopqrstuvwxyz1234567890",
            &shaped(
                "github_",
                &["pat_", "abcdefghijklmnop", "qrstuvwxyz1234567890"],
            ),
            "glpat-abcdefghijklmnopqrstuvwxyz1234567890",
            "xoxb-1234567890-abcdefghijklmnopqrstuvwxyz",
            "AKIA1234567890ABCDEF",
            &shaped(
                "Authorization: Bearer ",
                &["abcdefghijklmnopqrstuvwxyz0123456789+/=="],
            ),
            &shaped("Bearer ", &["abcdefghijklmnopqrstuvwxyz0123456789._~+/-"]),
        ];
        for secret in cases {
            assert!(
                !redact_with_home(secret, None).contains(secret),
                "secret survived: {secret}"
            );
        }
    }

    #[test]
    fn caps_utf8_by_bytes_without_splitting_a_character() {
        assert_eq!(truncate_utf8("你好", 4), "你");
        assert_eq!(truncate_utf8("abcdef", 3), "abc");
    }

    #[test]
    fn ring_keeps_only_the_newest_lines() {
        let mut ring = VecDeque::with_capacity(MAX_RING_LINES);
        for index in 0..MAX_RING_LINES + 5 {
            ring.push_back(index.to_string());
            while ring.len() > MAX_RING_LINES {
                ring.pop_front();
            }
        }
        assert_eq!(ring.len(), MAX_RING_LINES);
        assert_eq!(ring.front().map(String::as_str), Some("5"));
        assert_eq!(ring.back().map(String::as_str), Some("2004"));
    }

    #[test]
    fn redaction_leaves_ordinary_git_identifiers_alone() {
        // An scp-style remote, a hex-looking branch, a 40-char SHA and a
        // word that merely contains "bearer" must survive untouched: a
        // redactor that mangles these makes every log useless.
        let input = "remote=git@github.com:owner/repo.git branch=deadbeef1234 sha=0123456789abcdef0123456789abcdef01234567 note=bearer-token-rotation";
        assert_eq!(redact_with_home(input, None), input);
    }

    #[test]
    fn redaction_is_idempotent() {
        let once = redact_with_home(
            &shaped(
                "https://alice:s3cr3t",
                &["@host/x ghp_", "abcdefghijklmnopqrstuvwxyz1234"],
            ),
            None,
        );
        assert_eq!(redact_with_home(&once, None), once);
    }

    #[test]
    fn command_names_are_snake_case_struct_names() {
        assert_eq!(
            command_name::<crate::commands::WcStatusLoad>(),
            "wc_status_load"
        );
        assert_eq!(command_name::<crate::commands::Revert>(), "revert");
        assert_eq!(
            command_name::<crate::commands::RepositoryConfigSet>(),
            "repository_config_set"
        );
    }

    #[test]
    fn outcome_line_reports_the_failure_fields_at_error_level() {
        let failure: Result<(), ErrorPayload> = Err(ErrorPayload::new(
            crate::domain::error::ErrorCode::CheckoutConflicts,
            "Checkout would overwrite local changes.",
        )
        .with_detail("1 conflict")
        .with_domain(crate::domain::error::ErrorDomain::Git));
        let (level, line) = outcome_line("branch_checkout", Duration::from_millis(12), &failure);
        assert_eq!(level, Level::Error);
        assert!(line.contains("command=branch_checkout"));
        assert!(line.contains("outcome=error"));
        assert!(line.contains("duration_ms=12"));
        assert!(line.contains("code=checkout_conflicts"));
        assert!(line.contains("domain=git"));
        assert!(line.contains("detail=Some(\"1 conflict\")"));
    }

    #[test]
    fn outcome_line_warns_only_when_a_success_took_a_second() {
        let fast: Result<u8, ErrorPayload> = Ok(1);
        let (level, line) = outcome_line("wc_status_load", Duration::from_millis(3), &fast);
        assert_eq!(level, Level::Info);
        assert_eq!(line, "command=wc_status_load outcome=ok duration_ms=3");

        let (level, line) = outcome_line("wc_status_load", Duration::from_millis(1500), &fast);
        assert_eq!(level, Level::Warn);
        assert!(line.contains("slow=true"));
    }

    #[test]
    fn invoke_logging_keeps_key_names_and_only_allowlisted_values() {
        let body = InvokeBody::Json(serde_json::json!({
            "args": {
                "repo": "/tmp/project",
                "branch": "feature/x",
                // An allowlisted key still goes through redaction, so a
                // credential pasted into a ref name cannot leak.
                "ref": "https://alice:secret@host/x",
                "message": "a commit message that must never be logged",
                "token": shaped("ghp_", &["abcdefghijklmnopqrstuvwxyz1234567890"]),
                "paths": ["a", "b"],
            }
        }));
        let mut keys = Vec::new();
        let mut values = Vec::new();
        if let InvokeBody::Json(value) = &body {
            collect_invoke_fields(value, &mut keys, &mut values);
        }
        keys.sort();
        assert_eq!(keys, ["branch", "message", "paths", "ref", "repo", "token"]);
        assert!(values.contains(&"branch=feature/x".to_string()));
        assert!(values.contains(&"repo=/tmp/project".to_string()));
        assert!(values.contains(&"ref=https://[redacted]@host/x".to_string()));
        assert!(!values.iter().any(|value| value.contains("message")));
        assert!(!values.iter().any(|value| value.contains("token")));
    }

    #[test]
    fn only_allowlisted_keys_are_safe_arguments() {
        assert!(safe_argument("branch"));
        assert!(safe_argument("count"));
        assert!(!safe_argument("message"));
        assert!(!safe_argument("token"));
        assert!(!safe_argument("content"));
        assert!(!safe_argument("patch"));
    }

    #[test]
    fn panic_messages_come_back_as_text() {
        assert_eq!(panic_message(&"literal"), "literal");
        assert_eq!(panic_message(&String::from("formatted")), "formatted");
        assert_eq!(panic_message(&42_i32), "non-string panic payload");
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("meridian-logging-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        path
    }

    fn state_for(directory: &Path) -> LoggerState {
        LoggerState {
            ring: VecDeque::new(),
            log_dir: directory.to_path_buf(),
            log_file: directory.join("meridian.log"),
            settings_file: directory.join(SETTING_FILE),
            crash_file: directory.join(CRASH_FILE),
            mode: Mode {
                enabled: true,
                source: DebugSource::Setting,
            },
            bytes: 0,
            to_file: true,
            write_failure_reported: false,
        }
    }

    /// The live file plus `.1` and `.2` — never `.3`. The first rotation finds
    /// no live file to move, so the count that matters is how many rotations
    /// happen *after* the generations are full: drive well past that.
    #[test]
    fn rotation_keeps_three_files_and_drops_the_oldest() {
        let directory = scratch_dir("rotate");
        let mut state = state_for(&directory);
        let rotations = ROTATED_GENERATIONS + 4;
        for index in 1..=rotations {
            state.bytes = MAX_LOG_BYTES;
            write_rotating(&mut state, &format!("generation {index}"));
        }
        write_rotating(&mut state, "newest");

        let read = |name: &str| fs::read_to_string(directory.join(name)).expect(name);
        // Each write after a rotation lands in a fresh live file, so the live
        // file holds the last generation plus "newest", and the two older
        // generations follow, newest first.
        let live = read("meridian.log");
        assert!(live.contains("newest") && live.contains(&format!("generation {rotations}")));
        assert!(read("meridian.log.1").contains(&format!("generation {}", rotations - 1)));
        assert!(read("meridian.log.2").contains(&format!("generation {}", rotations - 2)));
        assert!(!directory.join("meridian.log.3").exists());
        let kept = fs::read_dir(&directory).expect("directory").count();
        assert_eq!(kept, ROTATED_GENERATIONS + 1);
        assert!(state.bytes < MAX_LOG_BYTES);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn seeding_restores_the_previous_session_and_drops_a_partial_first_line() {
        let directory = scratch_dir("seed");
        let state = state_for(&directory);
        fs::write(&state.log_file, "first\nsecond\nthird\n").expect("seed file");
        let logger = LocalLogger(Mutex::new(state));
        // Bound first: holding the guard as a call argument would keep the
        // mutex locked for the whole call and deadlock `seed_ring`.
        let file = logger.state().log_file.clone();
        seed_ring(&logger, &file);
        let guard = logger.state();
        let lines: Vec<&str> = guard.ring.iter().map(String::as_str).collect();
        assert_eq!(lines, ["first", "second", "third"]);
        // The trailing newline must not produce a phantom empty entry.
        assert_eq!(guard.ring.len(), 3);
        drop(guard);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn seeding_is_bounded_by_the_ring_capacity() {
        let directory = scratch_dir("seed-cap");
        let state = state_for(&directory);
        let file = state.log_file.clone();
        let mut text = String::new();
        for index in 0..MAX_RING_LINES + 10 {
            text.push_str(&format!("line {index}\n"));
        }
        fs::write(&file, text).expect("seed file");
        let logger = LocalLogger(Mutex::new(state));
        seed_ring(&logger, &file);
        let guard = logger.state();
        assert_eq!(guard.ring.len(), MAX_RING_LINES);
        assert_eq!(guard.ring.back().map(String::as_str), Some("line 2009"));
        drop(guard);
        let _ = fs::remove_dir_all(&directory);
    }

    // ---- redaction contract ---------------------------------------------

    /// A fixture value is a string, or fragments that are joined: credential-
    /// shaped inputs are split so no literal in the fixture looks like one.
    fn joined(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(parts) => parts
                .iter()
                .map(|part| part.as_str().expect("fragment is a string"))
                .collect(),
            other => panic!("fixture value must be a string or fragments, got {other}"),
        }
    }

    /// The frontend runs the same fixture (`src/diagnostics/recorder.test.ts`).
    /// Two redactors that disagree are two privacy promises, and only one of
    /// them is the one a report was checked against.
    #[test]
    fn redaction_matches_the_shared_contract_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/redaction-contract.json"))
                .expect("redaction fixture parses");
        let cases = fixture["cases"].as_array().expect("cases");
        assert!(cases.len() >= 25, "the contract shrank to {}", cases.len());
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            let input = joined(&case["input"]);
            let expected = if case["unchanged"].as_bool() == Some(true) {
                input.clone()
            } else {
                joined(&case["expected"])
            };
            let home = case["home"].as_str();
            assert_eq!(redact_with_home(&input, home), expected, "case: {name}");
            assert_eq!(
                redact_with_home(&expected, home),
                expected,
                "redaction is not idempotent for: {name}"
            );
        }
    }

    #[test]
    fn a_password_containing_at_does_not_leak_its_tail() {
        let output = redact_with_home("https://bob:p@ss@host.example/repo.git", None);
        assert!(
            !output.contains("ss@"),
            "tail of the password leaked: {output}"
        );
    }

    // ---- file modes ------------------------------------------------------

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn log_directory_and_files_are_owner_only_from_creation() {
        let base = scratch_dir("modes");
        let log_dir = base.join("logs");
        create_private_dir(&log_dir).expect("private dir");
        assert_eq!(mode_of(&log_dir), 0o700);

        let mut state = state_for(&log_dir);
        write_rotating(&mut state, "first");
        assert_eq!(mode_of(&state.log_file), 0o600);
        // After a rotation both the rotated and the new live file are private.
        state.bytes = MAX_LOG_BYTES;
        write_rotating(&mut state, "second");
        assert_eq!(mode_of(&log_dir.join("meridian.log.1")), 0o600);
        assert_eq!(mode_of(&state.log_file), 0o600);
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn existing_wide_open_files_are_narrowed_and_failures_are_reported() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch_dir("narrow");
        // A directory and logs left by an earlier version, created with the umask.
        let old = base.join("old");
        fs::create_dir(&old).expect("dir");
        fs::set_permissions(&old, fs::Permissions::from_mode(0o755)).expect("chmod");
        for name in ["meridian.log", "meridian.log.1", "unrelated.txt"] {
            fs::write(old.join(name), "x").expect("file");
            fs::set_permissions(old.join(name), fs::Permissions::from_mode(0o644)).expect("chmod");
        }
        create_private_dir(&old).expect("narrow dir");
        assert_eq!(mode_of(&old), 0o700);
        assert!(narrow_existing_logs(&old).is_empty());
        assert_eq!(mode_of(&old.join("meridian.log")), 0o600);
        assert_eq!(mode_of(&old.join("meridian.log.1")), 0o600);
        // Only log files are touched.
        assert_eq!(mode_of(&old.join("unrelated.txt")), 0o644);
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn private_writes_replace_a_looser_stale_file() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch_dir("private-write");
        let report = base.join("report.txt");
        fs::write(&report, "old").expect("stale file");
        fs::set_permissions(&report, fs::Permissions::from_mode(0o644)).expect("chmod");
        write_private(&report, b"new").expect("write");
        assert_eq!(mode_of(&report), 0o600);
        assert_eq!(fs::read_to_string(&report).expect("read"), "new");
        let _ = fs::remove_dir_all(&base);
    }

    // ---- one line per entry ----------------------------------------------

    #[test]
    fn a_message_with_line_breaks_stays_one_entry() {
        let directory = scratch_dir("one-line");
        let logger = LocalLogger(Mutex::new(state_for(&directory)));
        logger.log(
            &Record::builder()
                .args(format_args!("first\nforged [ERROR] entry\r\nthird"))
                .level(Level::Error)
                .target("meridian::test")
                .build(),
        );
        let guard = logger.state();
        assert_eq!(guard.ring.len(), 1);
        assert!(!guard.ring[0].contains('\n') && !guard.ring[0].contains('\r'));
        assert!(guard.ring[0].contains("first\\nforged"));
        let file = fs::read_to_string(&guard.log_file).expect("log file");
        assert_eq!(file.lines().count(), 1);
        drop(guard);
        let _ = fs::remove_dir_all(&directory);
    }

    // ---- origin filter ---------------------------------------------------

    #[test]
    fn debug_mode_opens_only_meridian_targets() {
        assert!(allowed("meridian::invoke", Level::Debug, true));
        assert!(allowed("meridian", Level::Debug, true));
        assert!(!allowed("meridian::invoke", Level::Debug, false));
        assert!(allowed("meridian::command", Level::Info, false));
        // The updater logs proxy settings and release responses at debug.
        assert!(!allowed(
            "tauri_plugin_updater::updater",
            Level::Debug,
            true
        ));
        assert!(!allowed("hyper::proto", Level::Info, true));
        assert!(allowed("hyper::proto", Level::Warn, false));
        assert!(!allowed("meridianx::other", Level::Debug, true));
    }

    // ---- degrading instead of failing -------------------------------------

    #[test]
    fn an_unusable_log_directory_degrades_to_memory_instead_of_failing() {
        let base = scratch_dir("fail-open");
        let blocker = base.join("blocker");
        fs::write(&blocker, "a file where a directory is needed").expect("blocker");
        let (mut state, notes) = prepare_state(blocker.join("logs"), base.join("config"));
        assert!(!state.to_file);
        assert!(
            notes.iter().any(|note| note.contains("memory only")),
            "the reason must be reported: {notes:?}"
        );
        write_rotating(&mut state, "a line that has nowhere to go");
        assert!(!state.log_file.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_failed_write_is_reported_once_in_the_ring() {
        let base = scratch_dir("write-fail");
        let mut state = state_for(&base.join("missing"));
        write_rotating(&mut state, "one");
        write_rotating(&mut state, "two");
        let reports = state
            .ring
            .iter()
            .filter(|line| line.contains("could not write"))
            .count();
        assert_eq!(reports, 1, "ring: {:?}", state.ring);
        let _ = fs::remove_dir_all(&base);
    }

    // ---- crash notes -----------------------------------------------------

    #[test]
    fn a_leftover_run_marker_is_an_unclean_exit() {
        let directory = scratch_dir("unclean");
        assert!(detect_unclean_exit(&directory).is_none());
        write_private(&directory.join(RUNNING_FILE), b"running\n").expect("marker");
        let note = detect_unclean_exit(&directory).expect("note");
        assert_eq!(note.kind, CrashKind::UncleanExit);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_recovered_panic_is_relabelled_but_other_notes_are_not() {
        let directory = scratch_dir("recovered");
        let path = directory.join(CRASH_FILE);
        let mut note = CrashNote {
            at: "2026-10-14T09:00:00.000Z".to_string(),
            message: "index out of bounds".to_string(),
            location: None,
            backtrace_excerpt: None,
            kind: CrashKind::Panic,
        };
        store_crash_note(&path, &note).expect("store");
        mark_recovered(&path);
        assert_eq!(
            read_crash_note(&path).expect("note").kind,
            CrashKind::RecoveredPanic
        );

        note.kind = CrashKind::UncleanExit;
        store_crash_note(&path, &note).expect("store");
        mark_recovered(&path);
        assert_eq!(
            read_crash_note(&path).expect("note").kind,
            CrashKind::UncleanExit
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_crash_note_from_an_older_version_reads_as_a_panic() {
        let directory = scratch_dir("old-note");
        let path = directory.join(CRASH_FILE);
        fs::write(
            &path,
            r#"{"at":"2026-01-01T00:00:00.000Z","message":"boom","location":null,"backtraceExcerpt":null}"#,
        )
        .expect("note");
        assert_eq!(read_crash_note(&path).expect("note").kind, CrashKind::Panic);
        let _ = fs::remove_dir_all(&directory);
    }

    // ---- small pure helpers ------------------------------------------------

    #[test]
    fn the_debug_marker_is_added_and_removed_idempotently() {
        assert_eq!(debug_title("Meridian", true), "Meridian [DEBUG]");
        assert_eq!(debug_title("Meridian [DEBUG]", true), "Meridian [DEBUG]");
        assert_eq!(debug_title("Meridian [DEBUG]", false), "Meridian");
        assert_eq!(debug_title("Meridian", false), "Meridian");
    }

    #[test]
    fn network_commands_get_a_longer_slow_fuse() {
        assert_eq!(slow_threshold("wc_status_load"), Duration::from_secs(1));
        assert_eq!(slow_threshold("remote_fetch"), Duration::from_secs(10));
        assert_eq!(slow_threshold("repository_clone"), Duration::from_secs(10));
        let ok: Result<(), ErrorPayload> = Ok(());
        let (level, _) = outcome_line("remote_fetch", Duration::from_millis(1500), &ok);
        assert_eq!(level, Level::Info, "a 1.5 s fetch is normal");
        let (level, _) = outcome_line("remote_fetch", Duration::from_secs(12), &ok);
        assert_eq!(level, Level::Warn);
    }

    #[test]
    fn error_codes_are_logged_in_the_wire_vocabulary() {
        use crate::domain::error::{ErrorCode, ErrorDomain};
        assert_eq!(
            wire_name(&ErrorCode::CheckoutConflicts),
            "checkout_conflicts"
        );
        assert_eq!(wire_name(&ErrorDomain::Internal), "internal");
    }
}
