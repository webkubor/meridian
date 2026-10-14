//! Repository-model engine (docs/02 §4): open, init, refresh, HEAD, config
//! (`core.bare` / `core.worktree` / `extensions.*`), remotes with upstream
//! tracking, worktrees, stats, description, and a health read.
//!
//! Everything here is git2-backed and returns **domain** values — no `git2`
//! type crosses upward except the two lifecycle handles (`open`, `init`,
//! `reopen`) that the command layer stores in the registry, and this module
//! never depends on `tauri` (docs/02 §1).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::repository::{
    ConfigEntry, ConfigScope, ConfigSnapshot, HeadState, HealthCheck, HealthSeverity,
    IntegrationState, ObjectFormat, RemoteFetchMode, RepoInfo, RepoStats, RepositoryConfigFacts,
    RepositoryHealth, RepositoryRemote, RepositoryRemotes, UpstreamTracking, WorktreeEntry,
};

// ---------------------------------------------------------------- open ------

/// Open a repository, falling back to discovery of a parent (docs/03 §2).
/// Returns the handle plus whether discovery was needed, which the caller
/// surfaces to the user.
///
/// `Repository::open` maps to libgit2's exact open (no upward search — the
/// `NO_SEARCH` flag), so the boolean is a real answer, not a guess.
pub fn open(path: &Path) -> Result<(git2::Repository, bool), ErrorPayload> {
    match git2::Repository::open(path) {
        Ok(repo) => Ok((repo, false)),
        Err(open_err) => match git2::Repository::discover(path) {
            Ok(repo) => Ok((repo, true)),
            Err(_) => Err(ErrorPayload::new(
                ErrorCode::InvalidRepository,
                format!("Not a Git repository: {}", path.display()),
            )
            .with_detail(open_err.message())),
        },
    }
}

#[derive(Clone, Debug)]
pub struct InitOptions {
    pub bare: bool,
    pub initial_branch: String,
    pub create_gitignore: bool,
}

/// Our own minimal default — deliberately small, and ours (docs/02 §16).
const DEFAULT_GITIGNORE: &str = "/target\nnode_modules/\n.DS_Store\n";

pub fn init(path: &Path, opts: &InitOptions) -> Result<git2::Repository, ErrorPayload> {
    let mut init = git2::RepositoryInitOptions::new();
    init.bare(opts.bare)
        .mkdir(true)
        .initial_head(&opts.initial_branch);
    let repo = git2::Repository::init_opts(path, &init)?;

    if opts.create_gitignore && !opts.bare {
        if let Some(workdir) = repo.workdir() {
            let gitignore = workdir.join(".gitignore");
            if !gitignore.exists() {
                std::fs::write(&gitignore, DEFAULT_GITIGNORE).map_err(|err| {
                    ErrorPayload::new(ErrorCode::Io, format!("Could not write .gitignore: {err}"))
                        .with_domain(ErrorDomain::Fs)
                })?;
            }
        }
    }

    Ok(repo)
}

/// Re-open the repository from disk so edits made by another process (refs,
/// index, config) become visible to the next read — `repository_refresh`'s
/// "re-read HEAD, refs, config" (docs/03 §2). The caller keeps its own handle;
/// this one is only for reading fresh state.
pub fn reopen(repo: &git2::Repository) -> Result<git2::Repository, ErrorPayload> {
    git2::Repository::open(repo.path()).map_err(ErrorPayload::from)
}

// ---------------------------------------------------------------- info ------

/// The HEAD reading (docs/03 §2): which ref HEAD names, where it points, and
/// whether either answer is still pending. A broken symbolic HEAD (its target
/// ref is missing on a repository that has commits) is reported rather than
/// raised — `repository_health` is the place that calls it a problem.
pub fn head_state(repo: &git2::Repository) -> Result<HeadState, ErrorPayload> {
    match repo.head() {
        Ok(head) => Ok(HeadState {
            detached: !head.is_branch(),
            branch: if head.is_branch() {
                head.shorthand().ok().map(str::to_string)
            } else {
                None
            },
            oid: head.target().map(|oid| oid.to_string()),
            unborn: false,
        }),
        Err(err) if err.code() == git2::ErrorCode::UnbornBranch => Ok(HeadState {
            oid: None,
            branch: head_symbolic_branch(repo),
            detached: false,
            unborn: true,
        }),
        Err(err) if err.code() == git2::ErrorCode::NotFound => Ok(HeadState {
            oid: None,
            branch: head_symbolic_branch(repo),
            detached: false,
            unborn: repo.is_empty().unwrap_or(false),
        }),
        Err(err) => Err(ErrorPayload::from(err)),
    }
}

fn head_symbolic_branch(repo: &git2::Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    let target = head.symbolic_target().ok().flatten()?;
    target.strip_prefix("refs/heads/").map(str::to_string)
}

/// Read everything `RepoInfo` carries. `discovered` is filled in by the
/// caller, which is the only party that knows the request path.
pub fn info(repo: &git2::Repository) -> Result<RepoInfo, ErrorPayload> {
    let id = RepoId::new(repo.path());
    let head = head_state(repo)?;

    Ok(RepoInfo {
        gitdir: id.as_str().to_string(),
        id,
        workdir: repo.workdir().map(|p| p.to_string_lossy().into_owned()),
        is_bare: repo.is_bare(),
        is_empty: repo.is_empty().unwrap_or(false),
        head_branch: head.branch.clone(),
        head_oid: head.oid.clone(),
        state: integration_state(repo),
        discovered: false,
        head,
    })
}

/// The one loader for the merge-process state machine (docs/02 §5).
pub fn integration_state(repo: &git2::Repository) -> IntegrationState {
    match repo.state() {
        git2::RepositoryState::Clean => IntegrationState::Clean,
        git2::RepositoryState::Merge => IntegrationState::Merge,
        git2::RepositoryState::Revert => IntegrationState::Revert,
        git2::RepositoryState::RevertSequence => IntegrationState::RevertSequence,
        git2::RepositoryState::CherryPick => IntegrationState::CherryPick,
        git2::RepositoryState::CherryPickSequence => IntegrationState::CherryPickSequence,
        git2::RepositoryState::Bisect => IntegrationState::Bisect,
        git2::RepositoryState::Rebase => IntegrationState::Rebase,
        git2::RepositoryState::RebaseInteractive => IntegrationState::RebaseInteractive,
        git2::RepositoryState::RebaseMerge => IntegrationState::RebaseMerge,
        git2::RepositoryState::ApplyMailbox => IntegrationState::ApplyMailbox,
        git2::RepositoryState::ApplyMailboxOrRebase => IntegrationState::ApplyMailboxOrRebase,
    }
}

fn integration_label(state: IntegrationState) -> &'static str {
    match state {
        IntegrationState::Clean => "No operation",
        IntegrationState::Merge => "A merge",
        IntegrationState::Revert => "A revert",
        IntegrationState::RevertSequence => "A revert sequence",
        IntegrationState::CherryPick => "A cherry-pick",
        IntegrationState::CherryPickSequence => "A cherry-pick sequence",
        IntegrationState::Bisect => "A bisect",
        IntegrationState::Rebase => "A rebase",
        IntegrationState::RebaseInteractive => "An interactive rebase",
        IntegrationState::RebaseMerge => "A rebase",
        IntegrationState::ApplyMailbox => "An apply-mailbox run",
        IntegrationState::ApplyMailboxOrRebase => "An apply/rebase run",
        IntegrationState::Other => "An operation",
    }
}

// -------------------------------------------------------------- config ------

/// Open one config level as its own object, so reads show *that file* and a
/// write targets exactly it (docs/03 §2 `repository_config_load` / `_set`).
///
/// A level whose file does not exist is `Ok(None)`; only a broken file is an
/// error. `Local` is the repository's own config (the common dir, so all
/// worktrees share it); `Worktree` is `config.worktree`, available only when
/// `extensions.worktreeConfig` is on.
fn open_scope(
    repo: &git2::Repository,
    scope: ConfigScope,
) -> Result<Option<git2::Config>, ErrorPayload> {
    let level = match scope {
        ConfigScope::System => git2::ConfigLevel::System,
        ConfigScope::Global => git2::ConfigLevel::Global,
        ConfigScope::Xdg => git2::ConfigLevel::XDG,
        ConfigScope::Local => git2::ConfigLevel::Local,
        ConfigScope::Worktree => git2::ConfigLevel::Worktree,
    };

    let base = match scope {
        ConfigScope::Local | ConfigScope::Worktree => repo.config()?,
        _ => match git2::Config::open_default() {
            Ok(config) => config,
            // No default config file exists at all — the level is simply absent.
            Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(err) => return Err(ErrorPayload::from(err)),
        },
    };

    match base.open_level(level) {
        Ok(config) => Ok(Some(config)),
        Err(err) if err.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(err) => Err(ErrorPayload::from(err)),
    }
}

/// The file backing a scope, when it exists — what the config editor shows as
/// its title. (`Config` itself exposes no path getter, so this resolves the
/// same locations git does.)
fn scope_file(repo: &git2::Repository, scope: ConfigScope) -> Option<String> {
    let candidate = match scope {
        ConfigScope::System => git2::Config::find_system().ok()?,
        ConfigScope::Global => git2::Config::find_global().ok()?,
        ConfigScope::Xdg => git2::Config::find_xdg().ok()?,
        ConfigScope::Local => repo.commondir().join("config"),
        ConfigScope::Worktree => repo.commondir().join("config.worktree"),
    };
    candidate
        .exists()
        .then(|| candidate.to_string_lossy().into_owned())
}

fn scope_label(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::System => "system",
        ConfigScope::Global => "global",
        ConfigScope::Xdg => "xdg",
        ConfigScope::Local => "local",
        ConfigScope::Worktree => "worktree",
    }
}

fn collect_entries(config: &git2::Config) -> Result<Vec<ConfigEntry>, ErrorPayload> {
    let mut out = Vec::new();
    let mut entries = config.entries(None)?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let key = entry.name()?.to_string();
        let value = if entry.has_value() {
            Some(entry.value()?.to_string())
        } else {
            None
        };
        out.push(ConfigEntry { key, value });
    }
    Ok(out)
}

/// The repository-identity facts (docs/03 §2: "core.bare, core.worktree,
/// extensions"): the effective answers, read from the repository's own config
/// (local plus, when enabled, the worktree level).
pub fn repository_config_facts(
    repo: &git2::Repository,
) -> Result<RepositoryConfigFacts, ErrorPayload> {
    let config = repo.config()?;

    let core_bare = config.get_bool("core.bare").ok();
    let worktree = config
        .get_string("core.worktree")
        .ok()
        .filter(|value| !value.is_empty());
    let format_version = config
        .get_i64("core.repositoryformatversion")
        .unwrap_or(0)
        .max(0) as u32;

    let mut extensions = BTreeMap::new();
    let mut entries = config.entries(None)?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let name = entry.name()?;
        if let Some(stripped) = name.strip_prefix("extensions.") {
            let value = if entry.has_value() {
                entry.value()?.to_string()
            } else {
                "true".to_string()
            };
            extensions.insert(stripped.to_string(), value);
        }
    }

    let object_format = match extensions
        .get("objectformat")
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("sha256") => ObjectFormat::Sha256,
        _ => ObjectFormat::Sha1,
    };
    let worktree_config = extensions
        .get("worktreeconfig")
        .map(|value| !value.eq_ignore_ascii_case("false"))
        .unwrap_or(false);

    Ok(RepositoryConfigFacts {
        bare: repo.is_bare(),
        core_bare,
        worktree,
        format_version,
        extensions,
        object_format,
        worktree_config,
    })
}

/// `repository_config_load` (docs/03 §2): every entry at one scope plus the
/// repository-identity facts.
pub fn config_snapshot(
    repo: &git2::Repository,
    scope: ConfigScope,
) -> Result<ConfigSnapshot, ErrorPayload> {
    let entries = match open_scope(repo, scope)? {
        Some(config) => collect_entries(&config)?,
        None => Vec::new(),
    };
    Ok(ConfigSnapshot {
        scope,
        entries,
        file: scope_file(repo, scope),
        facts: repository_config_facts(repo)?,
    })
}

/// `repository_config_set` (docs/03 §2): write one key at one scope. The
/// protected managed set is enforced by the validator, not here — this
/// function writes what it is told.
pub fn config_set(
    repo: &git2::Repository,
    scope: ConfigScope,
    key: &str,
    value: &str,
) -> Result<(), ErrorPayload> {
    let mut config = match open_scope(repo, scope)? {
        Some(config) => config,
        None => open_scope_for_write(repo, scope)?,
    };
    config
        .set_str(key, value)
        .map_err(|err| ErrorPayload::from(err).with_param("key", key.to_string()))
}

/// Create the file for a scope that has none yet, so a first write works on a
/// fresh machine (`~/.gitconfig` before it exists, `config.worktree` before
/// its first key). `Worktree` is refused unless the extension is enabled —
/// writing a file git would ignore is worse than an error.
fn open_scope_for_write(
    repo: &git2::Repository,
    scope: ConfigScope,
) -> Result<git2::Config, ErrorPayload> {
    let path = match scope {
        ConfigScope::System => git2::Config::find_system().ok(),
        ConfigScope::Global => git2::Config::find_global().ok(),
        ConfigScope::Xdg => git2::Config::find_xdg().ok(),
        ConfigScope::Local => Some(repo.commondir().join("config")),
        ConfigScope::Worktree => {
            if repository_config_facts(repo)?.worktree_config {
                Some(repo.commondir().join("config.worktree"))
            } else {
                None
            }
        }
    };

    let Some(path) = path else {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "The {} config is not available: worktree config needs extensions.worktreeConfig",
                scope_label(scope)
            ),
        )
        .with_domain(ErrorDomain::Git)
        .with_param("scope", scope_label(scope)));
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            ErrorPayload::new(
                ErrorCode::Io,
                format!("Cannot create {}: {err}", parent.display()),
            )
            .with_domain(ErrorDomain::Fs)
            .with_param("path", parent.display().to_string())
        })?;
    }
    if !path.exists() {
        std::fs::write(&path, b"").map_err(|err| {
            ErrorPayload::new(
                ErrorCode::Io,
                format!("Cannot create {}: {err}", path.display()),
            )
            .with_domain(ErrorDomain::Fs)
            .with_param("path", path.display().to_string())
        })?;
    }

    git2::Config::open(&path).map_err(ErrorPayload::from)
}

// ------------------------------------------------------------- remotes ------

/// `repository_remotes_load` (docs/03 §2/§8 read path): every remote with its
/// fetch/push URLs and refspecs, plus the upstream binding of every local
/// branch — config first, then the remote-tracking ref when it still exists.
pub fn remotes_model(repo: &git2::Repository) -> Result<RepositoryRemotes, ErrorPayload> {
    let upstreams = branch_upstreams(repo)?;
    let mut remotes = Vec::new();

    for name in remote_names(repo)? {
        let remote = repo.find_remote(&name)?;
        let fetch_refspecs = string_array_to_vec(remote.fetch_refspecs()?);
        let push_refspecs = string_array_to_vec(remote.push_refspecs()?);
        let fetch_mode = if fetch_refspecs.iter().any(|spec| spec.contains('*')) {
            RemoteFetchMode::All
        } else {
            RemoteFetchMode::Specified
        };
        let tracked_branches = upstreams
            .iter()
            .filter(|up| up.remote.as_deref() == Some(name.as_str()))
            .map(|up| up.branch.clone())
            .collect();

        remotes.push(RepositoryRemote {
            name: name.clone(),
            fetch_url: remote.url().ok().map(str::to_string),
            push_url: remote.pushurl().ok().flatten().map(str::to_string),
            fetch_refspecs,
            push_refspecs,
            fetch_mode,
            tracked_branches,
        });
    }

    Ok(RepositoryRemotes { remotes, upstreams })
}

/// The upstream bindings of every local branch (docs/03 §2 "upstream
/// tracking"). The config keys are authoritative — a branch whose upstream ref
/// has been deleted still shows its binding, flagged `gone`.
pub fn branch_upstreams(repo: &git2::Repository) -> Result<Vec<UpstreamTracking>, ErrorPayload> {
    let config = repo.config()?;
    let mut out = Vec::new();

    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = entry?;
        let Some(name) = branch.name().ok().flatten().map(str::to_string) else {
            continue;
        };

        let config_remote = config.get_string(&format!("branch.{name}.remote")).ok();
        let config_merge = config.get_string(&format!("branch.{name}.merge")).ok();

        let (remote_branch, ahead, behind) = match branch.upstream() {
            Ok(upstream) => {
                let upstream_name = upstream.name().ok().flatten().map(str::to_string);
                let (ahead, behind) = match (branch.get().target(), upstream.get().target()) {
                    (Some(local), Some(up)) => repo
                        .graph_ahead_behind(local, up)
                        .map(|(ahead, behind)| (ahead as u32, behind as u32))
                        .unwrap_or((0, 0)),
                    _ => (0, 0),
                };
                (upstream_name, ahead, behind)
            }
            // No upstream ref: either nothing is configured, or the upstream is
            // gone — `gone` below tells the two apart.
            Err(_) => (None, 0, 0),
        };

        let remote = config_remote.clone().or_else(|| {
            remote_branch
                .as_deref()
                .and_then(|name| name.split('/').next().map(str::to_string))
        });
        let gone = remote_branch.is_none() && config_remote.is_some() && config_merge.is_some();

        out.push(UpstreamTracking {
            branch: name,
            remote,
            remote_branch,
            merge_ref: config_merge,
            ahead,
            behind,
            gone,
        });
    }

    out.sort_by(|a, b| a.branch.cmp(&b.branch));
    Ok(out)
}

fn string_array_to_vec(array: git2::string_array::StringArray) -> Vec<String> {
    array
        .iter()
        .filter_map(|entry| entry.ok().flatten())
        .map(str::to_string)
        .collect()
}

/// The names in a `StringArray`, skipping entries that are not valid UTF-8.
/// (`StringArray::iter` yields `Result<Option<&str>>` — both layers are
/// dropped here; a non-UTF-8 name cannot be displayed or addressed anyway.)
fn string_array_names(array: &git2::string_array::StringArray) -> Vec<String> {
    array
        .iter()
        .filter_map(|entry| entry.ok().flatten())
        .map(str::to_string)
        .collect()
}

fn remote_names(repo: &git2::Repository) -> Result<Vec<String>, ErrorPayload> {
    Ok(string_array_names(&repo.remotes()?))
}

// ----------------------------------------------------------- worktrees ------

/// `repository_worktrees_load` (docs/03 §12 read path, modelled here because a
/// repository's worktrees are part of its model): the main entry plus every
/// linked worktree, each with its HEAD, lock state and validity.
pub fn worktrees(repo: &git2::Repository) -> Result<Vec<WorktreeEntry>, ErrorPayload> {
    let mut out = Vec::new();

    // The main entry: the repository's own working tree, or the bare gitdir.
    let head = head_state(repo)?;
    match repo.workdir() {
        Some(workdir) => out.push(WorktreeEntry {
            name: dir_name(workdir),
            path: workdir.to_string_lossy().into_owned(),
            head_branch: head.branch.clone(),
            head_oid: head.oid.clone(),
            detached: head.detached,
            locked: false,
            lock_reason: None,
            valid: true,
            prunable: false,
            is_main: true,
            is_bare: false,
            detail: None,
        }),
        None => out.push(WorktreeEntry {
            name: dir_name(repo.path()),
            path: repo.path().to_string_lossy().into_owned(),
            head_branch: head.branch.clone(),
            head_oid: head.oid.clone(),
            detached: head.detached,
            locked: false,
            lock_reason: None,
            valid: true,
            prunable: false,
            is_main: true,
            is_bare: true,
            detail: None,
        }),
    }

    for name in string_array_names(&repo.worktrees()?) {
        match repo.find_worktree(&name) {
            Ok(worktree) => {
                let (locked, lock_reason) = match worktree.is_locked() {
                    Ok(git2::WorktreeLockStatus::Locked(reason)) => (true, reason),
                    _ => (false, None),
                };
                let (valid, detail) = match worktree.validate() {
                    Ok(()) => (true, None),
                    Err(err) => (false, Some(err.message().to_string())),
                };
                let wt_head = if valid {
                    git2::Repository::open_from_worktree(&worktree)
                        .ok()
                        .and_then(|repo| head_state(&repo).ok())
                } else {
                    None
                };
                out.push(WorktreeEntry {
                    name: name.clone(),
                    path: worktree.path().to_string_lossy().into_owned(),
                    head_branch: wt_head.as_ref().and_then(|head| head.branch.clone()),
                    head_oid: wt_head.as_ref().and_then(|head| head.oid.clone()),
                    detached: wt_head.as_ref().map(|head| head.detached).unwrap_or(false),
                    locked,
                    lock_reason,
                    valid,
                    prunable: worktree.is_prunable(None).unwrap_or(false),
                    is_main: false,
                    is_bare: false,
                    detail,
                });
            }
            // The name is known but its metadata is unreadable — report the
            // row as invalid instead of dropping it.
            Err(err) => out.push(WorktreeEntry {
                name: name.clone(),
                path: String::new(),
                head_branch: None,
                head_oid: None,
                detached: false,
                locked: false,
                lock_reason: None,
                valid: false,
                prunable: false,
                is_main: false,
                is_bare: false,
                detail: Some(err.message().to_string()),
            }),
        }
    }

    // Main first, then by name.
    out.sort_by(|a, b| b.is_main.cmp(&a.is_main).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// --------------------------------------------------------------- stats ------

/// `repository_stats_load` (docs/03 §2; `GTGitUpdateRepositoryStatsOperation`):
/// count commits and distinct authors over the reachable history, and — only
/// when asked — the git directory's size on disk.
pub fn stats(repo: &git2::Repository, include_size: bool) -> Result<RepoStats, ErrorPayload> {
    let mut commits: u64 = 0;
    let mut contributors: BTreeSet<String> = BTreeSet::new();

    if let Ok(mut walk) = repo.revwalk() {
        // An unborn HEAD has nothing to walk — zero is the right answer.
        if walk.push_head().is_ok() {
            for oid in walk {
                let oid = oid?;
                commits += 1;
                if let Ok(commit) = repo.find_commit(oid) {
                    let author = commit.author();
                    let key = author
                        .email()
                        .map(|email| email.to_ascii_lowercase())
                        .unwrap_or_else(|_| author.name().unwrap_or("unknown").to_string());
                    if !key.is_empty() {
                        contributors.insert(key);
                    }
                }
            }
        }
    }

    Ok(RepoStats {
        commits,
        contributors: contributors.len() as u64,
        size_bytes: if include_size {
            Some(directory_size(repo))
        } else {
            None
        },
    })
}

/// Best-effort disk usage of the git directory; for a linked worktree the
/// common dir holds the objects, so it is measured instead of the (far
/// smaller, nested) worktree gitdir.
fn directory_size(repo: &git2::Repository) -> u64 {
    let path = repo.path();
    let common = repo.commondir();
    if path.starts_with(common) {
        dir_size(common)
    } else {
        dir_size(path) + dir_size(common)
    }
}

fn dir_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
        }
    }
    total
}

// --------------------------------------------------------- description ------

/// `repository_description_set` (docs/03 §2; `GTGitSetRepositoryDescriptionOperation`):
/// direct read/write of `.git/description`. The file is not a ref and not a
/// config key, so this is plain IO by design — one trailing newline, always.
pub fn set_description(repo: &git2::Repository, text: &str) -> Result<(), ErrorPayload> {
    let path = repo.path().join("description");
    let mut content = text.trim_end_matches(['\n', '\r']).to_string();
    content.push('\n');
    std::fs::write(&path, content).map_err(|err| {
        ErrorPayload::new(
            ErrorCode::Io,
            format!("Cannot write {}: {err}", path.display()),
        )
        .with_domain(ErrorDomain::Fs)
        .with_param("path", path.display().to_string())
    })
}

// ---------------------------------------------------------------- health ----

/// `repository_health_load`: a read-only diagnostic over the repository's
/// on-disk state — working tree, HEAD, index (and a stale lock), object
/// database, refs, config, worktrees and any in-progress operation. Every
/// check carries a stable id and a severity; `healthy` is "no `Error`".
pub fn health(repo: &git2::Repository) -> Result<RepositoryHealth, ErrorPayload> {
    let mut checks = Vec::new();

    // Working tree.
    match (repo.is_bare(), repo.workdir()) {
        (true, _) => checks.push(check(
            "worktree",
            HealthSeverity::Ok,
            "Bare repository — no working tree",
            None,
        )),
        (false, Some(workdir)) if workdir.is_dir() => checks.push(check(
            "worktree",
            HealthSeverity::Ok,
            "Working tree present",
            None,
        )),
        (false, Some(workdir)) => checks.push(check(
            "worktree",
            HealthSeverity::Error,
            "The working tree is missing",
            Some(workdir.display().to_string()),
        )),
        (false, None) => checks.push(check(
            "worktree",
            HealthSeverity::Error,
            "The repository has no working tree",
            None,
        )),
    }

    // HEAD.
    let head = head_state(repo);
    match &head {
        Ok(state) if state.unborn => checks.push(check(
            "head",
            HealthSeverity::Info,
            match &state.branch {
                Some(branch) => format!("No commits yet on '{branch}'"),
                None => "No commits yet".to_string(),
            },
            None,
        )),
        Ok(state) if state.detached => checks.push(check(
            "head",
            HealthSeverity::Info,
            format!(
                "Detached HEAD at {}",
                state.oid.as_deref().unwrap_or("an unknown commit")
            ),
            None,
        )),
        Ok(state) if state.oid.is_none() => checks.push(check(
            "head",
            HealthSeverity::Warning,
            format!(
                "HEAD points at '{}', which does not exist",
                state.branch.as_deref().unwrap_or("an unnamed ref")
            ),
            None,
        )),
        Ok(state) => checks.push(check(
            "head",
            HealthSeverity::Ok,
            format!(
                "On branch {}",
                state.branch.as_deref().unwrap_or("(unknown)")
            ),
            None,
        )),
        Err(err) => checks.push(check(
            "head",
            HealthSeverity::Error,
            "HEAD cannot be read",
            Some(err.message.clone()),
        )),
    }

    // Stale locks. A lock file is how git serializes writers; one left behind
    // by a crashed process is the classic "index is locked" report.
    let locks: Vec<String> = ["index.lock", "HEAD.lock", "config.lock"]
        .iter()
        .map(|name| repo.path().join(name))
        .filter(|lock| lock.exists())
        .map(|lock| lock.display().to_string())
        .collect();
    if locks.is_empty() {
        checks.push(check(
            "locks",
            HealthSeverity::Ok,
            "No lock files present",
            None,
        ));
    } else {
        checks.push(check(
            "locks",
            HealthSeverity::Warning,
            format!("{} lock file(s) present", locks.len()),
            Some(format!(
                "Another Git process may be running; if not, delete {}",
                locks.join(", ")
            )),
        ));
    }

    // Index.
    match repo.index() {
        Ok(index) => {
            if index.has_conflicts() {
                checks.push(check(
                    "index",
                    HealthSeverity::Info,
                    "The index has unmerged entries",
                    None,
                ));
            } else {
                checks.push(check("index", HealthSeverity::Ok, "Index readable", None));
            }
        }
        Err(err) => checks.push(check(
            "index",
            HealthSeverity::Error,
            "The index cannot be read",
            Some(err.message().to_string()),
        )),
    }

    // Object database: the commit HEAD names must actually be readable.
    if let Ok(state) = &head {
        if let Some(oid) = state
            .oid
            .as_deref()
            .and_then(|raw| raw.parse::<git2::Oid>().ok())
        {
            match repo.odb().and_then(|odb| odb.read(oid).map(|_| ())) {
                Ok(()) => checks.push(check(
                    "objects",
                    HealthSeverity::Ok,
                    "HEAD object is readable",
                    None,
                )),
                Err(err) => checks.push(check(
                    "objects",
                    HealthSeverity::Error,
                    format!("The object HEAD points at ({oid}) is not readable"),
                    Some(err.message().to_string()),
                )),
            }
        }
    }

    // Refs.
    match repo.references() {
        Ok(references) => {
            let broken = references.filter(|reference| reference.is_err()).count();
            if broken > 0 {
                checks.push(check(
                    "refs",
                    HealthSeverity::Warning,
                    format!("{broken} reference(s) cannot be read"),
                    None,
                ));
            } else {
                checks.push(check(
                    "refs",
                    HealthSeverity::Ok,
                    "References readable",
                    None,
                ));
            }
        }
        Err(err) => checks.push(check(
            "refs",
            HealthSeverity::Error,
            "References cannot be read",
            Some(err.message().to_string()),
        )),
    }

    // Config.
    match repo.config() {
        Ok(_) => checks.push(check(
            "config",
            HealthSeverity::Ok,
            "Configuration readable",
            None,
        )),
        Err(err) => checks.push(check(
            "config",
            HealthSeverity::Error,
            "Configuration cannot be read",
            Some(err.message().to_string()),
        )),
    }

    // Worktrees.
    match worktrees(repo) {
        Ok(list) => {
            let invalid: Vec<&str> = list
                .iter()
                .filter(|worktree| !worktree.valid)
                .map(|worktree| worktree.name.as_str())
                .collect();
            if invalid.is_empty() {
                checks.push(check(
                    "worktrees",
                    HealthSeverity::Ok,
                    format!("{} worktree(s) valid", list.len()),
                    None,
                ));
            } else {
                checks.push(check(
                    "worktrees",
                    HealthSeverity::Warning,
                    format!("{} worktree(s) need repair", invalid.len()),
                    Some(invalid.join(", ")),
                ));
            }
            let prunable: Vec<&str> = list
                .iter()
                .filter(|worktree| worktree.prunable)
                .map(|worktree| worktree.name.as_str())
                .collect();
            if !prunable.is_empty() {
                checks.push(check(
                    "worktrees_stale",
                    HealthSeverity::Info,
                    "Stale worktree metadata can be pruned",
                    Some(prunable.join(", ")),
                ));
            }
        }
        Err(err) => checks.push(check(
            "worktrees",
            HealthSeverity::Error,
            "Worktrees cannot be listed",
            Some(err.message),
        )),
    }

    // An operation in progress is not a fault — it is state, and the UI shows
    // the Continue / Skip / Abort surface for it (docs/02 §5).
    let state = integration_state(repo);
    if state == IntegrationState::Clean {
        checks.push(check(
            "state",
            HealthSeverity::Ok,
            "No operation in progress",
            None,
        ));
    } else {
        checks.push(check(
            "state",
            HealthSeverity::Info,
            format!("{} is in progress", integration_label(state)),
            None,
        ));
    }

    Ok(RepositoryHealth {
        healthy: checks
            .iter()
            .all(|check| check.severity != HealthSeverity::Error),
        checks,
    })
}

fn check(
    id: &str,
    severity: HealthSeverity,
    summary: impl Into<String>,
    detail: Option<String>,
) -> HealthCheck {
    HealthCheck {
        id: id.to_string(),
        severity,
        summary: summary.into(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A unique scratch directory (no `tempfile` dependency in this crate yet).
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "meridian-engine-test-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn init_repo(path: &std::path::Path) -> git2::Repository {
        init(
            path,
            &InitOptions {
                bare: false,
                initial_branch: "main".to_string(),
                create_gitignore: false,
            },
        )
        .unwrap()
    }

    fn commit_file(
        repo: &git2::Repository,
        file: &str,
        content: &str,
        author: (&str, &str),
    ) -> git2::Oid {
        let workdir = repo.workdir().expect("test repository has a workdir");
        std::fs::write(workdir.join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(file)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let signature = git2::Signature::now(author.0, author.1).unwrap();
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "test commit",
            &tree,
            &parents,
        )
        .unwrap()
    }

    #[test]
    fn head_state_tracks_unborn_born_and_detached() {
        let dir = temp_dir("head");
        let repo = init_repo(&dir);

        let head = head_state(&repo).unwrap();
        assert!(head.unborn, "a fresh repository is unborn");
        assert_eq!(head.branch.as_deref(), Some("main"));
        assert!(head.oid.is_none());

        let oid = commit_file(&repo, "a.txt", "a", ("Ada", "ada@example.invalid"));
        let head = head_state(&repo).unwrap();
        assert!(!head.unborn && !head.detached);
        assert_eq!(head.oid.as_deref(), Some(oid.to_string().as_str()));

        let info = info(&repo).unwrap();
        assert_eq!(info.head_branch.as_deref(), Some("main"));
        assert_eq!(info.head_oid.as_deref(), Some(oid.to_string().as_str()));
        assert!(!info.is_empty);
        assert_eq!(info.state, IntegrationState::Clean);

        repo.set_head_detached(oid).unwrap();
        let head = head_state(&repo).unwrap();
        assert!(head.detached && !head.unborn);
        assert!(head.branch.is_none());
        assert_eq!(head.oid.as_deref(), Some(oid.to_string().as_str()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_facts_and_local_snapshot() {
        let dir = temp_dir("config");
        let repo = init_repo(&dir);

        let facts = repository_config_facts(&repo).unwrap();
        assert!(!facts.bare);
        assert_eq!(facts.format_version, 0);
        assert_eq!(facts.object_format, ObjectFormat::Sha1);
        assert!(!facts.worktree_config);
        assert!(facts.extensions.is_empty());

        config_set(&repo, ConfigScope::Local, "meridian.model.selftest", "yes").unwrap();
        let snapshot = config_snapshot(&repo, ConfigScope::Local).unwrap();
        assert_eq!(snapshot.scope, ConfigScope::Local);
        assert!(snapshot.file.is_some());
        assert!(snapshot
            .entries
            .iter()
            .any(|entry| entry.key == "meridian.model.selftest"
                && entry.value.as_deref() == Some("yes")));

        // A level with no file behind it is an empty snapshot, not an error.
        let xdg = config_snapshot(&repo, ConfigScope::Xdg).unwrap();
        assert_eq!(xdg.scope, ConfigScope::Xdg);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upstream_tracking_reads_config_then_the_ref() {
        let dir = temp_dir("upstream");
        let repo = init_repo(&dir);
        let oid = commit_file(&repo, "a.txt", "a", ("Ada", "ada@example.invalid"));

        repo.remote("origin", "https://example.invalid/repo.git")
            .unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("branch.main.remote", "origin").unwrap();
        config
            .set_str("branch.main.merge", "refs/heads/main")
            .unwrap();

        let model = remotes_model(&repo).unwrap();
        assert_eq!(model.remotes.len(), 1);
        let remote = &model.remotes[0];
        assert_eq!(remote.name, "origin");
        assert_eq!(
            remote.fetch_url.as_deref(),
            Some("https://example.invalid/repo.git")
        );
        assert_eq!(remote.fetch_mode, RemoteFetchMode::All);
        assert_eq!(remote.tracked_branches, vec!["main".to_string()]);

        // The binding is configured but the remote-tracking ref is absent.
        let upstream = &model.upstreams[0];
        assert!(upstream.gone);
        assert_eq!(upstream.remote.as_deref(), Some("origin"));
        assert!(upstream.remote_branch.is_none());

        // Once the ref exists the binding resolves and ahead/behind is exact.
        repo.reference("refs/remotes/origin/main", oid, true, "test")
            .unwrap();
        let model = remotes_model(&repo).unwrap();
        let upstream = &model.upstreams[0];
        assert!(!upstream.gone);
        assert_eq!(upstream.remote_branch.as_deref(), Some("origin/main"));
        assert_eq!((upstream.ahead, upstream.behind), (0, 0));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn worktree_listing_covers_main_and_linked() {
        let dir = temp_dir("worktrees");
        let repo_dir = dir.join("repo");
        let repo = init_repo(&repo_dir);
        commit_file(&repo, "a.txt", "a", ("Ada", "ada@example.invalid"));

        let list = worktrees(&repo).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].is_main && list[0].valid && !list[0].is_bare);
        assert_eq!(list[0].head_branch.as_deref(), Some("main"));

        let linked_path = dir.join("linked");
        repo.worktree("linked", &linked_path, None).unwrap();

        let list = worktrees(&repo).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_main, "main sorts first");
        let linked = list.iter().find(|entry| !entry.is_main).unwrap();
        assert_eq!(linked.name, "linked");
        assert!(linked.valid, "{:?}", linked.detail);
        assert_eq!(linked.head_branch.as_deref(), Some("linked"));
        assert!(!linked.locked);
        assert!(linked.path.ends_with("linked"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stats_counts_commits_and_contributors() {
        let dir = temp_dir("stats");
        let repo = init_repo(&dir);
        commit_file(&repo, "a.txt", "a", ("Ada", "ada@example.invalid"));
        commit_file(&repo, "b.txt", "b", ("Grace", "grace@example.invalid"));

        let counted = stats(&repo, false).unwrap();
        assert_eq!(counted.commits, 2);
        assert_eq!(counted.contributors, 2);
        assert!(counted.size_bytes.is_none());

        let sized = stats(&repo, true).unwrap();
        assert!(sized.size_bytes.unwrap() > 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn health_reports_clean_then_flags_a_lock() {
        let dir = temp_dir("health");
        let repo = init_repo(&dir);
        commit_file(&repo, "a.txt", "a", ("Ada", "ada@example.invalid"));

        let report = health(&repo).unwrap();
        assert!(
            report.healthy,
            "unhealthy checks: {:?}",
            report
                .checks
                .iter()
                .filter(|check| check.severity == HealthSeverity::Error)
                .collect::<Vec<_>>()
        );
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "head" && check.severity == HealthSeverity::Ok));
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "objects" && check.severity == HealthSeverity::Ok));

        std::fs::write(repo.path().join("index.lock"), b"").unwrap();
        let report = health(&repo).unwrap();
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "locks" && check.severity == HealthSeverity::Warning));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn description_set_writes_the_file() {
        let dir = temp_dir("description");
        let repo = init_repo(&dir);
        set_description(&repo, "Hello\n\n").unwrap();
        let written = std::fs::read_to_string(repo.path().join("description")).unwrap();
        assert_eq!(written, "Hello\n");
        std::fs::remove_dir_all(&dir).ok();
    }
}
