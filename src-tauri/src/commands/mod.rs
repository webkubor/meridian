//! The Action surface (docs/03) — the `GTGit*Operation` third leg of the
//! Action → Validator → Operation triad (docs/02 §3).
//!
//! Every user-invocable verb is declared once as a [`Command`]: a descriptor
//! (argument struct), a pure `validate()`, and an `execute()`. `AppState::run`
//! sequences them — resolve → enqueue → validate → policy/snapshot → execute →
//! emit. Commands may depend on every layer below; nothing below depends on
//! `commands`.

pub mod app;
pub mod branches;
pub mod commit;
pub mod diagnostics;
pub mod history;
pub mod integrate;
pub mod refs;
pub mod repository;
pub mod working_copy;

pub use app::*;
pub use branches::*;
pub use commit::*;
pub use diagnostics::*;
pub use history::*;
pub use integrate::*;
pub use refs::*;
pub use repository::*;
pub use working_copy::*;

use crate::domain::error::ErrorPayload;
use crate::domain::ids::RepoId;
use crate::domain::sender::SenderTag;
use crate::events::EventSink;
use crate::validation::{ValidateCx, Violation};

/// How dangerous a command is. `Destructive` and `Rewrite` commands will
/// declare a snapshot policy when the snapshot store lands (docs/02 §7); the
/// trait carries the classification from day one so nothing has to be
/// re-annotated later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    None,
    Destructive,
    Rewrite,
}

/// Execution context handed to [`Command::execute`]: the resolved repository,
/// its id, and an event sink for emitting domain events.
pub struct ExecCx<'a> {
    pub repo_id: RepoId,
    pub repo: &'a git2::Repository,
    pub app: &'a dyn EventSink,
}

pub trait Command: Send + Sized + 'static {
    type Output: serde::Serialize + Send + 'static;

    /// The repository this command acts on; the orchestrator resolves it
    /// through the open-repository registry.
    fn repo_id(&self) -> RepoId;

    /// The invocation source (sender-scoped validation, docs/02 §3).
    fn sender(&self) -> SenderTag {
        SenderTag::Api
    }

    /// The risk classification used by snapshot policy (docs/02 §7).
    fn risk(&self) -> Risk {
        Risk::None
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation>;

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload>;
}
