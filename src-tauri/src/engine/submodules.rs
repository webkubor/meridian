//! Submodule operations — the engine half of docs/03 §11
//! (`GTGitOperationGroupSubmodules`): list / init / update / sync / settings.
//!
//! # Honest limits
//!
//! libgit2's `git_submodule_update` only understands the **checkout** strategy;
//! `rebase` and `merge` (and `none`) are configuration values it stores but
//! cannot execute. This module refuses those strategies with class
//! `submodule_strategy_unsupported` rather than silently doing a checkout — the
//! "refuse loudly rather than silently ignore an argument" rule (CLAUDE.md).
//!
//! Ignore/update/recurse settings are written through `repo.config()` as
//! `submodule.<name>.{ignore,update,fetchRecurseSubmodules}`, which is exactly
//! where `git submodule` reads them — writing the config directly avoids
//! needing `&mut Repository` (which `submodule_set_ignore` would require).

use serde::{Deserialize, Serialize};

use crate::domain::error::{ErrorCode, ErrorPayload};

/// Cap on recursive descent, matching git's own `submodule.recurse` sanity
/// limit; deeper nesting is reported, not followed.
const MAX_RECURSE_DEPTH: u32 = 5;

// ---------------------------------------------------------------------------
// Vocabulary (destined for `domain/submodules.rs` once it exists)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleIgnoreMode {
    /// Use the submodule's configuration.
    #[default]
    Unspecified,
    /// Any change or untracked file is dirty.
    None,
    /// Only tracked-file changes make it dirty.
    Untracked,
    /// Only a HEAD move makes it dirty.
    Dirty,
    /// Never dirty.
    All,
}

impl SubmoduleIgnoreMode {
    fn to_git2(self) -> git2::SubmoduleIgnore {
        match self {
            SubmoduleIgnoreMode::Unspecified => git2::SubmoduleIgnore::Unspecified,
            SubmoduleIgnoreMode::None => git2::SubmoduleIgnore::None,
            SubmoduleIgnoreMode::Untracked => git2::SubmoduleIgnore::Untracked,
            SubmoduleIgnoreMode::Dirty => git2::SubmoduleIgnore::Dirty,
            SubmoduleIgnoreMode::All => git2::SubmoduleIgnore::All,
        }
    }

    fn from_git2(value: git2::SubmoduleIgnore) -> Self {
        match value {
            git2::SubmoduleIgnore::Unspecified => SubmoduleIgnoreMode::Unspecified,
            git2::SubmoduleIgnore::None => SubmoduleIgnoreMode::None,
            git2::SubmoduleIgnore::Untracked => SubmoduleIgnoreMode::Untracked,
            git2::SubmoduleIgnore::Dirty => SubmoduleIgnoreMode::Dirty,
            git2::SubmoduleIgnore::All => SubmoduleIgnoreMode::All,
        }
    }

    fn config_value(self) -> &'static str {
        match self {
            SubmoduleIgnoreMode::Unspecified => "unspecified",
            SubmoduleIgnoreMode::None => "none",
            SubmoduleIgnoreMode::Untracked => "untracked",
            SubmoduleIgnoreMode::Dirty => "dirty",
            SubmoduleIgnoreMode::All => "all",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleUpdateStrategy {
    #[default]
    Checkout,
    Rebase,
    Merge,
    None,
}

impl SubmoduleUpdateStrategy {
    fn from_git2(value: git2::SubmoduleUpdate) -> Self {
        match value {
            git2::SubmoduleUpdate::Checkout => SubmoduleUpdateStrategy::Checkout,
            git2::SubmoduleUpdate::Rebase => SubmoduleUpdateStrategy::Rebase,
            git2::SubmoduleUpdate::Merge => SubmoduleUpdateStrategy::Merge,
            git2::SubmoduleUpdate::None => SubmoduleUpdateStrategy::None,
            git2::SubmoduleUpdate::Default => SubmoduleUpdateStrategy::Checkout,
        }
    }

    fn config_value(self) -> &'static str {
        match self {
            SubmoduleUpdateStrategy::Checkout => "checkout",
            SubmoduleUpdateStrategy::Rebase => "rebase",
            SubmoduleUpdateStrategy::Merge => "merge",
            SubmoduleUpdateStrategy::None => "none",
        }
    }
}

/// The reconciled state of a submodule: its recorded commit (index) versus its
/// checked-out HEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleState {
    /// Not initialized / no working directory.
    Uninitialized,
    /// HEAD matches the recorded commit.
    Current,
    /// The submodule has commits the superproject does not record.
    Ahead,
    /// The recorded commit is not checked out (needs `update`).
    Behind,
    /// Diverged, or has uncommitted changes.
    Modified,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub ignore: SubmoduleIgnoreMode,
    pub update: SubmoduleUpdateStrategy,
    pub state: SubmoduleState,
    /// The submodule worktree's HEAD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_oid: Option<String>,
    /// The commit recorded in the superproject's index (the "pinned" oid).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_oid: Option<String>,
    pub initialized: bool,
    /// The submodule is present in `.git/config`.
    pub in_config: bool,
}

/// One submodule that could not be acted on, with its class.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSkip {
    pub name: String,
    pub reason: String,
    pub class: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleReport {
    pub updated: Vec<String>,
    pub skipped: Vec<SubmoduleSkip>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubmoduleUpdateOptions {
    /// Run `init` on uninitialized submodules first (`git submodule update --init`).
    pub init: bool,
    /// Recurse into nested submodules.
    pub recursive: bool,
    pub strategy: SubmoduleUpdateStrategy,
    /// Overrides [`MAX_RECURSE_DEPTH`]; clamped to it.
    pub max_depth: Option<u32>,
}

impl Default for SubmoduleUpdateOptions {
    fn default() -> Self {
        Self {
            init: false,
            recursive: false,
            strategy: SubmoduleUpdateStrategy::Checkout,
            max_depth: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// `submodule_list` (docs/03 §11).
pub fn list(repo: &git2::Repository) -> Result<Vec<SubmoduleInfo>, ErrorPayload> {
    let mut out = Vec::new();
    for submodule in repo.submodules()? {
        out.push(describe(repo, &submodule)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// `submodule_load` for a single submodule.
pub fn info(repo: &git2::Repository, name: &str) -> Result<SubmoduleInfo, ErrorPayload> {
    let submodule = repo.find_submodule(name).map_err(|_| not_found(name))?;
    describe(repo, &submodule)
}

fn describe(
    repo: &git2::Repository,
    submodule: &git2::Submodule<'_>,
) -> Result<SubmoduleInfo, ErrorPayload> {
    let name = submodule.name()?.to_string();
    let path = submodule.path().to_string_lossy().into_owned();
    let url = submodule.url().ok().flatten().map(str::to_string);
    let branch = submodule.branch().ok().flatten().map(str::to_string);
    let ignore = SubmoduleIgnoreMode::from_git2(submodule.ignore_rule());
    let update = SubmoduleUpdateStrategy::from_git2(submodule.update_strategy());
    let head_oid = submodule.head_id();
    let index_oid = submodule.index_id();
    let initialized = submodule.open().is_ok();

    let status = repo.submodule_status(&name, ignore.to_git2()).ok();
    let in_config = status.map(|status| status.is_in_config()).unwrap_or(false);

    let state = classify_state(repo, submodule, head_oid, index_oid, status);

    Ok(SubmoduleInfo {
        name,
        path,
        url,
        branch,
        ignore,
        update,
        state,
        head_oid: head_oid.map(|oid| oid.to_string()),
        index_oid: index_oid.map(|oid| oid.to_string()),
        initialized,
        in_config,
    })
}

fn classify_state(
    _repo: &git2::Repository,
    submodule: &git2::Submodule<'_>,
    head: Option<git2::Oid>,
    index: Option<git2::Oid>,
    status: Option<git2::SubmoduleStatus>,
) -> SubmoduleState {
    if let Some(status) = status {
        if status.is_wd_uninitialized() || status.is_index_deleted() || status.is_wd_deleted() {
            return SubmoduleState::Uninitialized;
        }
    }

    let dirty = status
        .map(|status| {
            status.is_wd_modified() || status.is_wd_wd_modified() || status.is_index_modified()
        })
        .unwrap_or(false);

    match (head, index) {
        (Some(head), Some(index)) if head == index && !dirty => SubmoduleState::Current,
        (Some(head), Some(index)) if head == index && dirty => SubmoduleState::Modified,
        (Some(head), Some(index)) => {
            if let Ok(sub_repo) = submodule.open() {
                match sub_repo.graph_ahead_behind(head, index) {
                    Ok((ahead, 0)) if ahead > 0 => SubmoduleState::Ahead,
                    Ok((0, behind)) if behind > 0 => SubmoduleState::Behind,
                    _ => SubmoduleState::Modified,
                }
            } else {
                SubmoduleState::Behind
            }
        }
        _ if dirty => SubmoduleState::Modified,
        _ => SubmoduleState::Current,
    }
}

// ---------------------------------------------------------------------------
// Init / update / sync
// ---------------------------------------------------------------------------

/// `submodule_init` (docs/03 §11).
pub fn init(repo: &git2::Repository, names: &[String]) -> Result<SubmoduleReport, ErrorPayload> {
    let mut report = SubmoduleReport {
        updated: Vec::new(),
        skipped: Vec::new(),
    };
    for name in select(repo, names)? {
        match repo.find_submodule(&name) {
            Ok(mut submodule) => match submodule.init(false) {
                Ok(()) => report.updated.push(name),
                Err(err) => report.skipped.push(SubmoduleSkip {
                    name,
                    reason: err.message().to_string(),
                    class: "submodule_init_failed".to_string(),
                }),
            },
            Err(err) => report.skipped.push(SubmoduleSkip {
                name,
                reason: err.message().to_string(),
                class: "submodule_not_found".to_string(),
            }),
        }
    }
    Ok(report)
}

/// `submodule_update` (docs/03 §11).
pub fn update(
    repo: &git2::Repository,
    names: &[String],
    opts: &SubmoduleUpdateOptions,
) -> Result<SubmoduleReport, ErrorPayload> {
    if opts.strategy != SubmoduleUpdateStrategy::Checkout {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "The '{}' update strategy cannot be executed — only 'checkout' is supported",
                opts.strategy.config_value()
            ),
        )
        .with_param("class", "submodule_strategy_unsupported")
        .with_param("strategy", opts.strategy.config_value()));
    }

    let depth_cap = opts
        .max_depth
        .unwrap_or(MAX_RECURSE_DEPTH)
        .min(MAX_RECURSE_DEPTH);
    update_at(repo, names, opts, 0, depth_cap)
}

fn update_at(
    repo: &git2::Repository,
    names: &[String],
    opts: &SubmoduleUpdateOptions,
    depth: u32,
    depth_cap: u32,
) -> Result<SubmoduleReport, ErrorPayload> {
    let mut report = SubmoduleReport {
        updated: Vec::new(),
        skipped: Vec::new(),
    };

    for name in select(repo, names)? {
        let mut submodule = match repo.find_submodule(&name) {
            Ok(submodule) => submodule,
            Err(err) => {
                report.skipped.push(SubmoduleSkip {
                    name,
                    reason: err.message().to_string(),
                    class: "submodule_not_found".to_string(),
                });
                continue;
            }
        };

        let initialized = submodule.open().is_ok();
        let mut options = git2::SubmoduleUpdateOptions::new();
        match submodule.update(opts.init && !initialized, Some(&mut options)) {
            Ok(()) => report.updated.push(name.clone()),
            Err(err) => {
                report.skipped.push(SubmoduleSkip {
                    name: name.clone(),
                    reason: err.message().to_string(),
                    class: "submodule_update_failed".to_string(),
                });
                continue;
            }
        }

        if opts.recursive && depth + 1 < depth_cap {
            if let Ok(sub_repo) = submodule.open() {
                let nested = update_at(&sub_repo, &[], opts, depth + 1, depth_cap)?;
                report.updated.extend(nested.updated);
                report.skipped.extend(nested.skipped);
            }
        }
    }

    Ok(report)
}

/// `submodule_sync` (docs/03 §11): copy the (possibly updated) URL into the
/// submodule's own `.git/config`.
pub fn sync(repo: &git2::Repository, names: &[String]) -> Result<SubmoduleReport, ErrorPayload> {
    let mut report = SubmoduleReport {
        updated: Vec::new(),
        skipped: Vec::new(),
    };
    for name in select(repo, names)? {
        match repo.find_submodule(&name) {
            Ok(mut submodule) => match submodule.sync() {
                Ok(()) => report.updated.push(name),
                Err(err) => report.skipped.push(SubmoduleSkip {
                    name,
                    reason: err.message().to_string(),
                    class: "submodule_sync_failed".to_string(),
                }),
            },
            Err(err) => report.skipped.push(SubmoduleSkip {
                name,
                reason: err.message().to_string(),
                class: "submodule_not_found".to_string(),
            }),
        }
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// `submodule_settings_save` (docs/03 §11). Each `Option` field writes the
/// matching config key when `Some`, and removes it when `None` (falling back to
/// the global default).
pub fn settings_save(
    repo: &git2::Repository,
    name: &str,
    ignore: Option<SubmoduleIgnoreMode>,
    update: Option<SubmoduleUpdateStrategy>,
    fetch_recurse: Option<bool>,
) -> Result<SubmoduleInfo, ErrorPayload> {
    if repo.find_submodule(name).is_err() {
        return Err(not_found(name));
    }
    let mut config = repo.config()?;

    set_or_remove(
        &mut config,
        &format!("submodule.{name}.ignore"),
        ignore.map(|value| value.config_value()),
    )?;
    set_or_remove(
        &mut config,
        &format!("submodule.{name}.update"),
        update.map(|value| value.config_value()),
    )?;
    match fetch_recurse {
        Some(value) => {
            config.set_bool(&format!("submodule.{name}.fetchRecurseSubmodules"), value)?
        }
        None => crate::engine::branches::remove_config_key(
            &mut config,
            &format!("submodule.{name}.fetchRecurseSubmodules"),
        )?,
    }

    info(repo, name)
}

fn set_or_remove(
    config: &mut git2::Config,
    key: &str,
    value: Option<&str>,
) -> Result<(), ErrorPayload> {
    match value {
        Some(value) => {
            config.set_str(key, value)?;
            Ok(())
        }
        None => crate::engine::branches::remove_config_key(config, key),
    }
}

/// Resolve the requested names against the repository: an empty list means
/// "every submodule".
fn select(repo: &git2::Repository, names: &[String]) -> Result<Vec<String>, ErrorPayload> {
    if !names.is_empty() {
        return Ok(names.to_vec());
    }
    Ok(repo
        .submodules()?
        .iter()
        .filter_map(|submodule| submodule.name().ok().map(str::to_string))
        .collect())
}

fn not_found(name: &str) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::ReferenceNotFound,
        format!("Submodule '{name}' does not exist"),
    )
    .with_param("name", name)
    .with_param("class", "submodule_not_found")
}
