//! Engine layer — the `GTGit*Operation` counterpart (docs/02 §4).
//!
//! Every function here is git2-backed and returns **domain** values; no
//! `git2` type ever crosses upward, and this module must never depend on
//! `tauri` (docs/02 §1).

pub mod branches;
pub mod cherry;
pub mod credentials;
pub mod diff;
pub mod discovery;
pub mod error;
pub mod graph;
pub mod history;
pub mod index;
pub mod merge;
pub mod rebase;
pub mod remotes;
pub mod repository;
pub mod reset;
pub mod stash;
pub mod status;
pub mod submodules;
pub mod working_tree;
pub mod worktrees;
