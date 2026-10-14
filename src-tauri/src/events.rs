use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::domain::ids::RepoId;

/// App-wide domain events (docs/02 §6). The web layer subscribes with
/// `@tauri-apps/api/event` and reloads the affected store slice; stores never
/// poll.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "payload")]
pub enum AppEvent {
    ReposChanged,
    RefsChanged { repo: RepoId },
    WorkingCopyChanged { repo: RepoId },
}

/// The single window event name every `AppEvent` travels on.
pub const EVENT_NAME: &str = "meridian://app-event";

/// Receives advisory app events emitted by a command execution.
pub trait EventSink: Send + Sync {
    /// Delivers one event; failure to notify a subscriber never fails a command.
    fn emit(&self, event: AppEvent);

    /// Produces an owned handle that can move into the blocking command worker.
    fn shared(&self) -> Arc<dyn EventSink>;
}

impl<R: Runtime> EventSink for AppHandle<R> {
    fn emit(&self, event: AppEvent) {
        if let Err(err) = Emitter::emit(self, EVENT_NAME, event) {
            eprintln!("meridian: failed to emit {EVENT_NAME}: {err}");
        }
    }

    fn shared(&self) -> Arc<dyn EventSink> {
        Arc::new(self.clone())
    }
}

/// A clonable collector for headless flow tests and other non-window callers.
#[derive(Clone, Default)]
pub struct CollectingEventSink {
    events: Arc<Mutex<Vec<AppEvent>>>,
}

impl CollectingEventSink {
    /// Creates an empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes all events collected so far, preserving their emission order.
    pub fn take(&self) -> Vec<AppEvent> {
        self.events
            .lock()
            .map(|mut events| std::mem::take(&mut *events))
            .unwrap_or_default()
    }
}

impl EventSink for CollectingEventSink {
    fn emit(&self, event: AppEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    fn shared(&self) -> Arc<dyn EventSink> {
        Arc::new(self.clone())
    }
}

/// Emits one event through a production or test event sink.
pub fn emit(sink: &dyn EventSink, event: AppEvent) {
    sink.emit(event);
}
