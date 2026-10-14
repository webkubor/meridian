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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashNote {
    pub at: String,
    pub message: String,
    pub location: Option<String>,
    pub backtrace_excerpt: Option<String>,
}
