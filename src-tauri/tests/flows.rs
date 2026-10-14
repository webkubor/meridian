//! Headless command flows: JSON wire args, real temporary Git repositories,
//! and the same validation/execution path production runs through AppState.

#[path = "flows/history.rs"]
mod history;
#[path = "flows/lifecycle.rs"]
mod lifecycle;
#[path = "flows/refs_and_stash.rs"]
mod refs_and_stash;
#[path = "flows/support/mod.rs"]
mod support;
#[path = "flows/working_copy.rs"]
mod working_copy;
