use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::domain::ids::RepoId;

/// App-wide domain events (docs/02 §6). The web layer subscribes with
/// `@tauri-apps/api/event` and reloads the affected store slice; stores never
/// poll.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "payload")]
pub enum AppEvent {
    ReposChanged,
    RefsChanged { repo: RepoId },
    WorkingCopyChanged { repo: RepoId },
}

/// The single window event name every `AppEvent` travels on.
pub const EVENT_NAME: &str = "meridian://app-event";

/// Events are advisory: a closed window or dropped subscriber must never fail
/// a command.
pub fn emit(app: &AppHandle, event: AppEvent) {
    if let Err(err) = app.emit(EVENT_NAME, event) {
        eprintln!("meridian: failed to emit {EVENT_NAME}: {err}");
    }
}
