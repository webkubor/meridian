//! Serializable diagnostics vocabulary shared with the web layer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSource {
    Env,
    Setting,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugLevel {
    Info,
    Debug,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugState {
    pub enabled: bool,
    pub source: DebugSource,
    pub log_dir: String,
    pub log_file: String,
    pub level: DebugLevel,
    /// Whether the developer-tools window can be opened right now. The WebView
    /// decides inspector support when the window is created, so debug mode
    /// switched on during this run is not enough: the UI needs this flag to
    /// offer a restart instead of a button that can only refuse.
    pub devtools_ready: bool,
}

/// What the crash note records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrashKind {
    /// A panic on a thread nothing recovered: the run ended with it.
    #[default]
    Panic,
    /// A panic inside a command that the app caught and carried on from. Worth
    /// a bug report, but not "the previous run ended unexpectedly".
    RecoveredPanic,
    /// No panic, but the run never reached a clean exit (abort, a crash inside
    /// a native library, a force quit).
    UncleanExit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashNote {
    pub at: String,
    pub message: String,
    pub location: Option<String>,
    pub backtrace_excerpt: Option<String>,
    /// Absent in notes written by earlier versions, which were all panics.
    #[serde(default)]
    pub kind: CrashKind,
}
