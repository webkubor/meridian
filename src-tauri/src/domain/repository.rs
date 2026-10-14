//! Repository-model vocabulary (docs/03 §2): identity, HEAD, config, remotes,
//! worktrees, stats and health.
//!
//! Everything here is plain data — serde in, serde out; no `git2`, `tauri` or
//! `rusqlite` (docs/02 §1). The engine fills these values in, the web layer
//! reads them, and nothing in between mutates them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ids::RepoId;

/// The unified merge-process state machine (docs/02 §5; evidence F5): one enum
/// with a payload per state, produced by one loader, feeding the shared
/// Continue / Skip / Abort surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationState {
    Clean,
    Merge,
    Revert,
    RevertSequence,
    CherryPick,
    CherryPickSequence,
    Bisect,
    Rebase,
    RebaseInteractive,
    RebaseMerge,
    ApplyMailbox,
    ApplyMailboxOrRebase,
    Other,
}

/// The HEAD reading (docs/03 §2 `repository_refresh` / `repository_state`).
///
/// [`RepoInfo::head_branch`] / [`RepoInfo::head_oid`] are convenience mirrors
/// of this struct; the flags here are the part a branch name alone cannot
/// carry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadState {
    /// The commit HEAD resolves to; `None` while the branch is unborn.
    pub oid: Option<String>,
    /// The branch HEAD is attached to — set for an unborn HEAD too, naming the
    /// branch the next commit will create.
    pub branch: Option<String>,
    /// HEAD points at a commit directly instead of at a branch.
    pub detached: bool,
    /// HEAD names a branch that has no commits yet.
    pub unborn: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub id: RepoId,
    pub gitdir: String,
    pub workdir: Option<String>,
    pub is_bare: bool,
    /// Unborn HEAD — a repository with no commits yet.
    pub is_empty: bool,
    pub head_branch: Option<String>,
    pub head_oid: Option<String>,
    pub state: IntegrationState,
    /// True when the requested path was not itself a repository and a parent
    /// repository was found instead (the `repository_open` contract, docs/03 §2).
    pub discovered: bool,
    /// The full HEAD reading: mirrors `head_branch` / `head_oid` and adds the
    /// detached / unborn flags.
    pub head: HeadState,
}

/// One repository root found by `repository_discover` (docs/03 §2).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRoot {
    /// Directory name — what a "Found nearby" row shows.
    pub name: String,
    /// The directory the walk found (the working tree of a normal repository).
    pub path: String,
    /// The resolved git directory.
    pub gitdir: String,
    pub workdir: Option<String>,
    pub is_bare: bool,
    /// A linked worktree or submodule checkout (its `.git` is a file).
    pub is_worktree: bool,
}

/// The classification `repository_validate_path` returns (docs/03 §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathKind {
    Missing,
    NotDirectory,
    Unreadable,
    /// A working tree with a `.git` directory.
    Repository,
    /// A git directory with no working tree.
    BareRepository,
    /// A linked worktree or submodule checkout (its `.git` is a file).
    Worktree,
    /// The path is inside the working tree of a repository rooted elsewhere.
    ParentRepository,
    NoRepository,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathCheck {
    pub path: String,
    pub kind: PathKind,
    pub detail: Option<String>,
    pub gitdir: Option<String>,
    pub workdir: Option<String>,
}

// ---------------------------------------------------------------- config ---

/// The five git config levels (docs/02 §9; docs/03 §2 `repository_config_load`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigScope {
    System,
    Global,
    Xdg,
    #[default]
    Local,
    Worktree,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEntry {
    pub key: String,
    /// `None` for a valueless key (a bare `key` line means boolean true).
    pub value: Option<String>,
}

/// The object hash of the repository's object database, read from
/// `extensions.objectFormat` (docs/03 §2: "core.bare, core.worktree,
/// extensions").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectFormat {
    #[default]
    Sha1,
    Sha256,
}

/// The repository-identity config the model layer always reads first: the
/// `core.*` keys that decide what the repository *is*.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigFacts {
    /// The effective answer (git2's `Repository::is_bare`) — the one the rest
    /// of the model follows.
    pub bare: bool,
    /// The raw `core.bare` value when the config file sets it.
    pub core_bare: Option<bool>,
    /// Raw `core.worktree` when set (relative paths are relative to the gitdir).
    pub worktree: Option<String>,
    /// `core.repositoryformatversion` (0 when unset).
    pub format_version: u32,
    /// Every effective `extensions.*` key with the prefix stripped.
    pub extensions: BTreeMap<String, String>,
    pub object_format: ObjectFormat,
    /// `extensions.worktreeConfig` — whether per-worktree config is on.
    pub worktree_config: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSnapshot {
    pub scope: ConfigScope,
    /// Every entry at that scope, in file order, one item per occurrence (a
    /// multi-valued key appears more than once).
    pub entries: Vec<ConfigEntry>,
    /// The file backing the scope, when one exists (`~/.gitconfig`, …).
    pub file: Option<String>,
    pub facts: RepositoryConfigFacts,
}

// --------------------------------------------------------------- remotes ---

/// How a remote's fetch refspecs are shaped (docs/03 §8 `remote_set_fetch_mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteFetchMode {
    /// A wildcard refspec (`+refs/heads/*:refs/remotes/<name>/*`).
    #[default]
    All,
    /// Explicit branch refspecs only.
    Specified,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemote {
    pub name: String,
    pub fetch_url: Option<String>,
    /// The push URL — present only when it is set separately.
    pub push_url: Option<String>,
    pub fetch_refspecs: Vec<String>,
    pub push_refspecs: Vec<String>,
    pub fetch_mode: RemoteFetchMode,
    /// Local branches that track this remote.
    pub tracked_branches: Vec<String>,
}

/// A local branch's upstream binding (docs/03 §2 "upstream tracking"), read
/// from `branch.<name>.remote` / `branch.<name>.merge` plus the
/// remote-tracking ref when it still exists.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamTracking {
    pub branch: String,
    /// The remote name from config; `.` means the branch tracks a local branch.
    pub remote: Option<String>,
    /// The resolved remote-tracking branch (`origin/main`); `None` when gone.
    pub remote_branch: Option<String>,
    /// The raw `branch.<name>.merge` value (`refs/heads/main`).
    pub merge_ref: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Config says the branch tracks something that no longer exists.
    pub gone: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemotes {
    pub remotes: Vec<RepositoryRemote>,
    pub upstreams: Vec<UpstreamTracking>,
}

// ------------------------------------------------------------- worktrees ---

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub name: String,
    /// The worktree's working directory; empty when its metadata is broken.
    pub path: String,
    pub head_branch: Option<String>,
    pub head_oid: Option<String>,
    pub detached: bool,
    pub locked: bool,
    pub lock_reason: Option<String>,
    /// `git2::Worktree::validate` — false means the metadata needs repair
    /// (docs/03 §12 rows carry a "validity warning with repair action").
    pub valid: bool,
    pub prunable: bool,
    pub is_main: bool,
    /// The main entry of a bare repository (no working tree at all).
    pub is_bare: bool,
    pub detail: Option<String>,
}

// ----------------------------------------------------------------- stats ---

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStats {
    pub commits: u64,
    /// Distinct author identities over the reachable history.
    pub contributors: u64,
    /// Disk usage of the git directory (the common dir for worktrees);
    /// computed only when the caller asks for it.
    pub size_bytes: Option<u64>,
}

// ---------------------------------------------------------------- health ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthSeverity {
    Ok,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    /// Stable machine id (`head`, `index_lock`, `objects`, …).
    pub id: String,
    pub severity: HealthSeverity,
    pub summary: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryHealth {
    /// No check reported `Error`.
    pub healthy: bool,
    pub checks: Vec<HealthCheck>,
}
