use serde::{Deserialize, Serialize};

use super::error::ErrorPayload;

/// Progress is a protocol (docs/02 §6; evidence F17): the engine reports
/// phases and ticks; the UI renders them and never polls. These types are
/// declared now — the activity registry and the `Channel<ProgressEvent>`
/// plumbing land with the remote operations in a later phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Connecting,
    Receiving,
    Resolving,
    Writing,
    CheckingOut,
    Scanning,
    WritingObjects,
    Done,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub done: Option<u64>,
    pub total: Option<u64>,
    pub item: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "payload")]
pub enum ProgressEvent {
    Started { kind: String, total: Option<u64> },
    PhaseUpdate { phase: Phase },
    Tick { progress: Progress },
    Finished { summary: String },
    Failed { error: ErrorPayload },
}
