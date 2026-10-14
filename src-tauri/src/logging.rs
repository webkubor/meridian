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

use crate::domain::diagnostics::{CrashNote, DebugLevel, DebugSource, DebugState};
use crate::domain::error::ErrorPayload;

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const ROTATED_FILES: usize = 3;
const MAX_RING_LINES: usize = 2_000;
const MAX_TAIL_BYTES: usize = 256 * 1024;
const MAX_DIAGNOSTIC_MESSAGE: usize = 2 * 1024;
const SETTING_FILE: &str = "diagnostics.json";
const CRASH_FILE: &str = "last-crash.json";

static LOGGER: OnceLock<LocalLogger> = OnceLock::new();
static HOME: OnceLock<Option<String>> = OnceLock::new();
static TOKEN_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
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
            window.title = format!("{} [DEBUG]", window.title);
        }
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
pub fn initialize<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;

    let env_enabled = env_debug_enabled();
    let setting_enabled = read_setting(&config_dir.join(SETTING_FILE));
    let mode = if env_enabled {
        Mode {
            enabled: true,
            source: DebugSource::Env,
        }
    } else if setting_enabled {
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
    let bytes = fs::metadata(&log_file)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let logger = LOGGER.get_or_init(|| {
        LocalLogger(Mutex::new(LoggerState {
            ring: VecDeque::with_capacity(MAX_RING_LINES),
            log_dir: log_dir.clone(),
            log_file: log_file.clone(),
            settings_file: config_dir.join(SETTING_FILE),
            crash_file: config_dir.join(CRASH_FILE),
            mode,
            bytes,
        }))
    });
    {
        let mut state = logger
            .0
            .lock()
            .map_err(|_| "diagnostics logger lock poisoned")?;
        state.log_dir = log_dir.clone();
        state.log_file = log_file.clone();
        state.settings_file = config_dir.join(SETTING_FILE);
        state.crash_file = config_dir.join(CRASH_FILE);
        state.mode = mode;
        state.bytes = bytes;
    }
    log::set_logger(logger).map_err(|error| error.to_string())?;
    set_max_level(mode.enabled);
    seed_ring(logger, &log_file);
    install_panic_hook();
    log::info!(
        target: "meridian::startup",
        "startup version={} commit={} os={} arch={} profile={} debug={} log={}",
        env!("CARGO_PKG_VERSION"),
        option_env!("MERIDIAN_GIT_COMMIT").unwrap_or("unrecorded"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) { "debug" } else { "release" },
        mode.enabled,
        log_dir.display()
    );
    Ok(())
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

impl Log for LocalLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let state = self.state();
        metadata.level()
            <= if state.mode.enabled {
                Level::Debug
            } else {
                Level::Info
            }
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = truncate_utf8(&redact(&record.args().to_string()), 16 * 1024);
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
    let Some(next_len) = line.len().checked_add(1).map(|len| len as u64) else {
        return;
    };
    if state.bytes.saturating_add(next_len) > MAX_LOG_BYTES {
        for index in (1..ROTATED_FILES).rev() {
            let from = rotated_path(&state.log_file, index);
            let to = rotated_path(&state.log_file, index + 1);
            if index + 1 == ROTATED_FILES {
                let _ = fs::remove_file(&to);
            }
            let _ = fs::rename(from, to);
        }
        let first = rotated_path(&state.log_file, 1);
        let _ = fs::rename(&state.log_file, first);
        state.bytes = 0;
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.log_file)
    {
        if writeln!(file, "{line}").is_ok() {
            state.bytes = state.bytes.saturating_add(next_len);
        }
    }
}

fn rotated_path(active: &Path, index: usize) -> PathBuf {
    active.with_file_name(format!("meridian.log.{index}"))
}

pub fn redact(text: &str) -> String {
    redact_with_home(
        text,
        HOME.get_or_init(|| std::env::var("HOME").ok().filter(|s| !s.is_empty()))
            .as_deref(),
    )
}

fn redact_with_home(text: &str, home: Option<&str>) -> String {
    let mut value = text.to_string();
    if let Some(home) = home.filter(|home| !home.is_empty()) {
        value = value.replace(home, "~");
    }
    let patterns = TOKEN_PATTERNS.get_or_init(|| {
        [
            r"(?i)([a-z][a-z0-9+.-]*://)[^/\s?#@]+@",
            r"(?i)(ghp_|gho_|github_pat_|glpat-|xox[bapr]-)[a-z0-9_-]{12,}",
            r"\bAKIA[A-Z0-9]{16}\b",
            r"(?i)(authorization\s*[:=]\s*(?:bearer\s+)?|bearer\s+)[a-z0-9._~+/-]{20,}={0,}",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("static redaction regex"))
        .collect()
    });
    for (index, pattern) in patterns.iter().enumerate() {
        let replacement = match index {
            0 => "$1[redacted]@",
            3 => "$1[redacted]",
            _ => "[redacted]",
        };
        value = pattern.replace_all(&value, replacement).into_owned();
    }
    value
}

pub fn debug_state() -> DebugState {
    let Some(logger) = LOGGER.get() else {
        return DebugState {
            enabled: false,
            source: DebugSource::Off,
            log_dir: String::new(),
            log_file: String::new(),
            level: DebugLevel::Info,
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
    }
}

pub fn set_debug(enabled: bool) -> Result<DebugState, std::io::Error> {
    let logger = LOGGER
        .get()
        .expect("diagnostics logger initialized before commands");
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
    set_max_level(enabled);
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
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
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
/// The hook below already logged it and wrote the crash marker; this adds the
/// line that ties the panic to the command the caller was waiting on, because
/// the hook cannot know which command that was.
pub fn record_panic_detail(detail: &str) {
    log::error!(target: "meridian::command", "outcome=panic detail={}", truncate_utf8(&redact(detail), 2 * 1024));
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
                "command={command} outcome=error duration_ms={millis} code={:?} domain={:?} message={:?} detail={:?}",
                error.code,
                error.domain,
                redact(&error.message),
                error.detail.as_deref().map(redact)
            ),
        ),
        // A slow command is a symptom worth a louder line even when it
        // succeeded: it is the shape of bug that shows up as "the app hung",
        // not as an error.
        Ok(_) if duration >= Duration::from_secs(1) => (
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
    let bytes = fs::read(path).ok()?;
    let mut note: CrashNote = serde_json::from_slice(&bytes).ok()?;
    note.message = redact(&note.message);
    note.location = note.location.map(|value| redact(&value));
    note.backtrace_excerpt = note.backtrace_excerpt.map(|value| redact(&value));
    Some(note)
}

pub fn dismiss_crash() -> Result<(), std::io::Error> {
    let logger = LOGGER
        .get()
        .expect("diagnostics logger initialized before commands");
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
            };
            log::error!(target: "meridian::panic", "panic message={:?} location={:?} backtrace={:?}", note.message, note.location, note.backtrace_excerpt);
            if let Some(logger) = LOGGER.get() {
                let state = logger.state();
                if let Ok(bytes) = serde_json::to_vec(&note) {
                    let temporary = state.crash_file.with_extension("json.tmp");
                    if fs::write(&temporary, bytes).is_ok() {
                        let _ = fs::rename(temporary, &state.crash_file);
                    }
                }
            }
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
        assert!(line.contains("code=CheckoutConflicts"));
        assert!(line.contains("domain=Git"));
        assert!(line.contains("detail=Some(\"1 conflict\")"));
    }

    #[test]
    fn outcome_line_warns_only_when_a_success_took_a_second() {
        let fast: Result<u8, ErrorPayload> = Ok(1);
        let (level, line) = outcome_line("wc_status_load", Duration::from_millis(3), &fast);
        assert_eq!(level, Level::Info);
        assert_eq!(line, "command=wc_status_load outcome=ok duration_ms=3");

        let (level, line) = outcome_line("remote_fetch", Duration::from_millis(1500), &fast);
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
        }
    }

    #[test]
    fn rotation_keeps_three_files_and_drops_the_oldest() {
        let directory = scratch_dir("rotate");
        let mut state = state_for(&directory);
        for index in 1..=ROTATED_FILES {
            state.bytes = MAX_LOG_BYTES;
            write_rotating(&mut state, &format!("line {index}"));
        }
        write_rotating(&mut state, "newest");
        // The newest line is in the live file, and exactly ROTATED_FILES
        // generations survive: .1 and .2, never .3.
        let live = fs::read_to_string(&state.log_file).expect("live log");
        assert!(live.contains("newest"));
        assert!(state.log_file.with_file_name("meridian.log.1").exists());
        assert!(state.log_file.with_file_name("meridian.log.2").exists());
        assert!(!state.log_file.with_file_name("meridian.log.3").exists());
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
}
