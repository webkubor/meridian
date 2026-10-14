//! Domain vocabulary — the `FNGit*` counterpart (docs/02 §4).
//!
//! Dependency rule (docs/02 §1): nothing in `domain` may depend on `git2`,
//! `tauri`, or `rusqlite`. These types are plain data: serde in, serde out.

pub mod app;
pub mod commit;
pub mod conflict;
pub mod diagnostics;
pub mod error;
pub mod history;
pub mod ids;
pub mod progress;
pub mod refs;
pub mod repository;
pub mod revision;
pub mod sender;
pub mod status;
pub mod working_copy;
