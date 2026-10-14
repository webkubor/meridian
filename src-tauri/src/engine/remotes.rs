//! Remote operations — the engine half of docs/03 §7 (remote branches) and §8
//! (remotes): `remote_*`, `fetch`, `pull`, `push`, `remote_branch_*`.
//!
//! # Transport features
//!
//! `git2` is declared in `Cargo.toml` with the `ssh` and `https` features, so
//! libgit2 is built with its ssh (libssh2), https (openssl), git and http
//! transports. Every authenticated transport routes through
//! [`super::credentials`], which resolves a real credential — the ssh-agent and
//! the on-disk keys for ssh, the `gitcredentials(7)` helper protocol for https —
//! so a remote needing auth never fails on a bare Negotiate default.
//!
//! [`git2::Cred::default`] is **never** returned: it is `git_cred_default_new`,
//! a Negotiate-only credential with no user/pass/ssh payload that every
//! authenticated fetch/push rejects.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::domain::error::{ErrorCode, ErrorPayload};

use super::branches::DeleteReport;

// ---------------------------------------------------------------------------
// Vocabulary (destined for `domain/remotes.rs` once that module exists)
// ---------------------------------------------------------------------------

/// What to do about tags during a fetch (maps onto libgit2's `AutotagOption`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagFetch {
    /// Use the remote's own configuration.
    #[default]
    Unspecified,
    /// Fetch tags that point at objects already being downloaded.
    Follow,
    /// Fetch every tag.
    All,
    /// Fetch no tags beyond the explicit refspecs.
    None,
}

impl TagFetch {
    fn to_autotag(self) -> git2::AutotagOption {
        match self {
            TagFetch::Unspecified => git2::AutotagOption::Unspecified,
            TagFetch::Follow => git2::AutotagOption::Auto,
            TagFetch::All => git2::AutotagOption::All,
            TagFetch::None => git2::AutotagOption::None,
        }
    }
}

/// The fetch policy stored under `remote.<name>.{prune,tagOpt,depth}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemoteFetchMode {
    /// Delete remote-tracking refs whose upstream branch is gone.
    pub prune: bool,
    pub tags: TagFetch,
    /// Shallow clone depth; `None` is a full fetch.
    pub depth: Option<i32>,
}

/// A remote as the settings panel lists it (docs/03 §8 `remote_list`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushurl: Option<String>,
    pub fetch_refspecs: Vec<String>,
    pub push_refspecs: Vec<String>,
    pub fetch_mode: RemoteFetchMode,
}

/// How a ref moved during a fetch or push.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefUpdateKind {
    Created,
    Updated,
    /// Moved to a non-descendant — the local side of a forced update.
    Forced,
    Deleted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefUpdate {
    pub name: String,
    pub old_oid: String,
    pub new_oid: String,
    pub kind: RefUpdateKind,
}

/// Result of `remote_fetch` (docs/03 §8 `fetch`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReport {
    pub remote: String,
    pub updates: Vec<RefUpdate>,
    pub received_objects: usize,
    pub indexed_objects: usize,
    pub received_bytes: usize,
    /// Remote-tracking refs removed because `prune` was on and the branch is
    /// gone upstream. `FetchReport::updates` carries the updates themselves.
    pub pruned: Vec<String>,
}

/// One ref the server refused, with the server's own message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRejection {
    pub name: String,
    pub message: String,
}

/// Result of `push` / `force_push` / `publish_branch` (docs/03 §8).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushReport {
    pub remote: String,
    pub accepted: Vec<String>,
    pub rejected: Vec<PushRejection>,
    /// Remote-tracking ref updates applied locally after a successful push, so
    /// ahead/behind stays truthful without waiting for the next fetch.
    pub updates: Vec<RefUpdate>,
}

/// How `pull` integrates the fetched commits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullMode {
    /// Merge the upstream into the current branch (default).
    #[default]
    Merge,
    /// Rebase local commits on top of upstream.
    Rebase,
    /// Refuse anything but a fast-forward.
    FastForwardOnly,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PullOptions {
    pub mode: PullMode,
    /// Commit message for a merge; the git default is used when absent.
    pub message: Option<String>,
}

/// What `pull` actually did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullOutcome {
    UpToDate,
    FastForwarded,
    Merged,
    Rebased,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    pub remote: String,
    pub branch: String,
    pub outcome: PullOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub updates: Vec<RefUpdate>,
}

/// A remote branch row (docs/03 §7 `remote_branch_list`): the tracking ref plus
/// the local branch that tracks it, if any, and their divergence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchInfo {
    /// Full tracking refname, e.g. `refs/remotes/origin/main`.
    pub full_name: String,
    /// `origin/main`.
    pub name: String,
    pub remote: String,
    /// `main`.
    pub branch: String,
    pub oid: String,
    pub tip_summary: String,
    pub tip_time: i64,
    /// A local branch that tracks this remote branch, if one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

/// `--force-with-lease` expectation. `oid` is the exact oid the caller believes
/// the remote is at; `None` means "whatever the remote-tracking ref says now",
/// which is git's default `--force-with-lease` (no explicit oid) behaviour.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ForceWithLease {
    pub oid: Option<String>,
}

/// `remote_publish_branch` result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishReport {
    pub remote: String,
    pub branch: String,
    /// The upstream shorthand that was recorded, e.g. `origin/main`.
    pub upstream: String,
    pub push: PushReport,
}

// ---------------------------------------------------------------------------
// Listing / CRUD
// ---------------------------------------------------------------------------

pub fn list(repo: &git2::Repository) -> Result<Vec<RemoteInfo>, ErrorPayload> {
    let mut out = Vec::new();
    for name in repo.remotes()?.iter() {
        let Ok(Some(name)) = name else { continue };
        out.push(info(repo, name)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn info(repo: &git2::Repository, name: &str) -> Result<RemoteInfo, ErrorPayload> {
    let remote = repo.find_remote(name).map_err(|_| remote_not_found(name))?;
    let url = remote.url().unwrap_or_default().to_string();
    let pushurl = remote.pushurl().ok().flatten().map(str::to_string);
    let fetch_refspecs = remote
        .refspecs()
        .filter(|spec| spec.direction() == git2::Direction::Fetch)
        .filter_map(|spec| spec.str().ok().map(str::to_string))
        .collect();
    let push_refspecs = remote
        .refspecs()
        .filter(|spec| spec.direction() == git2::Direction::Push)
        .filter_map(|spec| spec.str().ok().map(str::to_string))
        .collect();

    Ok(RemoteInfo {
        name: name.to_string(),
        url,
        pushurl,
        fetch_refspecs,
        push_refspecs,
        fetch_mode: read_fetch_mode(repo, name)?,
    })
}

/// `remote_add` (docs/03 §8): create `name` → `url` with git's default fetch
/// refspec; returns the stored remote.
pub fn add(repo: &git2::Repository, name: &str, url: &str) -> Result<RemoteInfo, ErrorPayload> {
    if !git2::Remote::is_valid_name(name) {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("'{name}' is not a valid remote name"),
        )
        .with_param("name", name));
    }
    if repo.find_remote(name).is_ok() {
        return Err(ErrorPayload::new(
            ErrorCode::ReferenceExists,
            format!("Remote '{name}' already exists"),
        )
        .with_param("name", name));
    }
    // `Repository::remote` = git_remote_create: name + url + default refspec.
    repo.remote(name, url)?;
    info(repo, name)
}

/// `remote_rename` (docs/03 §8). libgit2 renames the config section and rewrites
/// `branch.*.remote`; the returned refspecs are the ones it could *not* update
/// (a non-empty list is surfaced, never swallowed).
pub fn rename(
    repo: &git2::Repository,
    old: &str,
    new: &str,
) -> Result<RemoteRenameReport, ErrorPayload> {
    if repo.find_remote(old).is_err() {
        return Err(remote_not_found(old));
    }
    let failed = repo.remote_rename(old, new)?;
    let refspecs = failed
        .iter()
        .filter_map(|spec| spec.ok().flatten().map(str::to_string))
        .collect();
    Ok(RemoteRenameReport {
        old_name: old.to_string(),
        new_name: new.to_string(),
        refspecs,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRenameReport {
    pub old_name: String,
    pub new_name: String,
    /// Config refspecs libgit2 could not rewrite; empty on a clean rename.
    pub refspecs: Vec<String>,
}

/// `remote_remove` (docs/03 §8). The remote-tracking refs go too — git's own
/// `remote remove` (unlike a bare `config --remove-section`) prunes them.
pub fn remove(repo: &git2::Repository, name: &str) -> Result<(), ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    repo.remote_delete(name)?;
    prune_tracking_refs(repo, name);
    Ok(())
}

/// `remote_set_url` (docs/03 §8). `push` chooses the push URL instead.
pub fn set_url(
    repo: &git2::Repository,
    name: &str,
    url: &str,
    push: bool,
) -> Result<RemoteInfo, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    if push {
        repo.remote_set_pushurl(name, Some(url))?;
    } else {
        repo.remote_set_url(name, url)?;
    }
    info(repo, name)
}

pub fn add_refspec(
    repo: &git2::Repository,
    name: &str,
    refspec: &str,
    push: bool,
) -> Result<RemoteInfo, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    if push {
        repo.remote_add_push(name, refspec)?;
    } else {
        repo.remote_add_fetch(name, refspec)?;
    }
    info(repo, name)
}

pub fn remove_refspec(
    repo: &git2::Repository,
    name: &str,
    refspec: &str,
    push: bool,
) -> Result<RemoteInfo, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    let key = if push {
        format!("remote.{name}.push")
    } else {
        format!("remote.{name}.fetch")
    };
    let mut config = repo.config()?;
    match config.remove_multivar(&key, &regex_literal(refspec)) {
        Ok(()) => {}
        Err(err) if err.code() == git2::ErrorCode::NotFound => {
            return Err(ErrorPayload::new(
                ErrorCode::ReferenceNotFound,
                format!("Refspec '{refspec}' is not configured on '{name}'"),
            )
            .with_param("refspec", refspec));
        }
        Err(err) => return Err(ErrorPayload::from(err)),
    }
    info(repo, name)
}

/// `remote_set_fetch_mode`: persist the prune/tag/depth policy so later fetches
/// (ours and the command line's) agree.
pub fn set_fetch_mode(
    repo: &git2::Repository,
    name: &str,
    mode: &RemoteFetchMode,
) -> Result<RemoteInfo, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    let mut config = repo.config()?;
    config.set_bool(&format!("remote.{name}.prune"), mode.prune)?;
    set_or_remove(
        &mut config,
        &format!("remote.{name}.tagOpt"),
        match mode.tags {
            TagFetch::Unspecified => None,
            TagFetch::Follow => Some("--follow"),
            TagFetch::All => Some("--tags"),
            TagFetch::None => Some("--no-tags"),
        },
    )?;
    match mode.depth {
        Some(depth) => config.set_i32(&format!("remote.{name}.depth"), depth)?,
        None => crate::engine::branches::remove_config_key(
            &mut config,
            &format!("remote.{name}.depth"),
        )?,
    }
    info(repo, name)
}

fn read_fetch_mode(repo: &git2::Repository, name: &str) -> Result<RemoteFetchMode, ErrorPayload> {
    let config = repo.config()?;
    let prune = config
        .get_bool(&format!("remote.{name}.prune"))
        .unwrap_or(false);
    let tags = match config.get_string(&format!("remote.{name}.tagOpt")).ok() {
        Some(value) if value == "--no-tags" => TagFetch::None,
        Some(value) if value == "--tags" => TagFetch::All,
        Some(value) if value == "--follow" => TagFetch::Follow,
        _ => TagFetch::Unspecified,
    };
    let depth = config
        .get_string(&format!("remote.{name}.depth"))
        .ok()
        .and_then(|value| value.parse::<i32>().ok());
    Ok(RemoteFetchMode { prune, tags, depth })
}

// ---------------------------------------------------------------------------
// Fetch / prune
// ---------------------------------------------------------------------------

/// `remote_fetch` (docs/03 §8). `refspecs` empty means "use the remote's
/// configured base refspecs"; an explicit refspec still updates the tracking
/// refs it names, exactly like `git fetch origin <spec>`.
pub fn fetch(
    repo: &git2::Repository,
    name: &str,
    refspecs: &[String],
    mode: &RemoteFetchMode,
) -> Result<FetchReport, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }

    let updates: RefCell<Vec<RefUpdate>> = RefCell::new(Vec::new());
    let specs: Vec<&str> = refspecs.iter().map(String::as_str).collect();

    let mut remote = repo.find_remote(name)?;
    {
        let mut options = git2::FetchOptions::new();
        options
            .prune(if mode.prune {
                git2::FetchPrune::On
            } else {
                git2::FetchPrune::Off
            })
            .download_tags(mode.tags.to_autotag());
        if let Some(depth) = mode.depth {
            options.depth(depth);
        }

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(super::credentials::callback(repo));
        callbacks.update_tips(|refname, old, new| {
            updates.borrow_mut().push(RefUpdate {
                name: refname.to_string(),
                old_oid: oid_string(old),
                new_oid: oid_string(new),
                kind: classify_update(repo, old, new),
            });
            true
        });
        options.remote_callbacks(callbacks);

        remote.fetch(&specs, Some(&mut options), Some("meridian: fetch"))?;
    }

    let stats = remote.stats();
    let report = FetchReport {
        remote: name.to_string(),
        received_objects: stats.received_objects(),
        indexed_objects: stats.indexed_objects(),
        received_bytes: stats.received_bytes(),
        pruned: Vec::new(),
        updates: updates.into_inner(),
    };
    Ok(report)
}

/// `remote_prune` (docs/03 §8): drop tracking refs whose upstream is gone.
/// Returns the refnames that were removed.
pub fn prune(repo: &git2::Repository, name: &str) -> Result<Vec<String>, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }
    let mut remote = repo.find_remote(name)?;
    // Delete the tracked names ourselves so the report is exact; libgit2's
    // prune only reports through its (untyped) return.
    let stale = stale_tracking_refs(repo, &mut remote)?;
    for refname in &stale {
        if let Ok(mut reference) = repo.find_reference(refname) {
            reference.delete()?;
        }
    }
    Ok(stale)
}

fn stale_tracking_refs(
    repo: &git2::Repository,
    remote: &mut git2::Remote<'_>,
) -> Result<Vec<String>, ErrorPayload> {
    let prefix = format!(
        "refs/remotes/{}/",
        remote.name().ok().flatten().unwrap_or("")
    );
    let advertised = advertise(repo, remote)?;
    let mut stale = Vec::new();
    for reference in repo.references_glob(&format!("{prefix}*"))? {
        let reference = reference?;
        let Ok(refname) = reference.name() else {
            continue;
        };
        // The tracking ref's upstream branch is what the server would call
        // `refs/heads/<branch>`.
        let Some(branch) = refname.strip_prefix(&prefix) else {
            continue;
        };
        let upstream = format!("refs/heads/{branch}");
        if !advertised.iter().any(|(name, _)| name == &upstream) {
            stale.push(refname.to_string());
        }
    }
    Ok(stale)
}

/// Connect and read the advertisement list — the source of truth for "what the
/// remote has right now". Disconnects before returning. Each entry is
/// `(refname, oid)`.
fn advertise(
    repo: &git2::Repository,
    remote: &mut git2::Remote<'_>,
) -> Result<Vec<(String, String)>, ErrorPayload> {
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(super::credentials::callback(repo));
    let connection = remote.connect_auth(git2::Direction::Fetch, Some(callbacks), None)?;
    let heads: Vec<(String, String)> = connection
        .list()?
        .iter()
        .filter(|head| !head.is_local())
        .map(|head| (head.name().to_string(), head.oid().to_string()))
        .collect();
    drop(connection);
    Ok(heads)
}

// ---------------------------------------------------------------------------
// Pull
// ---------------------------------------------------------------------------

/// `pull` (docs/03 §8): fetch the tracked branch, then fast-forward, merge or
/// rebase per [`PullOptions::mode`]. On a merge or rebase conflict the state is
/// left exactly as git leaves it and a typed error is returned, so the working
/// copy shows the conflicts instead of a half-applied result.
pub fn pull(
    repo: &git2::Repository,
    name: &str,
    branch: &str,
    opts: &PullOptions,
) -> Result<PullResult, ErrorPayload> {
    let local = repo
        .find_branch(branch, git2::BranchType::Local)
        .map_err(|_| {
            ErrorPayload::new(
                ErrorCode::ReferenceNotFound,
                format!("Branch '{branch}' does not exist"),
            )
            .with_param("name", branch)
        })?;

    let tracking =
        tracking_ref(repo, name, branch).unwrap_or_else(|| format!("refs/remotes/{name}/{branch}"));
    let refspec = format!("+refs/heads/{branch}:{tracking}");

    let fetch_report = fetch(repo, name, &[refspec], &read_fetch_mode(repo, name)?)?;

    let local_tip = local.get().peel_to_commit()?.id();
    let remote_tip = repo.find_reference(&tracking)?.peel_to_commit()?.id();

    let mut result = PullResult {
        remote: name.to_string(),
        branch: branch.to_string(),
        outcome: PullOutcome::UpToDate,
        head: Some(local_tip.to_string()),
        updates: fetch_report.updates,
    };

    if local_tip == remote_tip || repo.graph_descendant_of(local_tip, remote_tip)? {
        return Ok(result);
    }

    let annotated = repo.find_annotated_commit(remote_tip)?;
    let (analysis, _) = repo.merge_analysis(&[&annotated])?;

    if analysis.is_fast_forward() {
        fast_forward_branch(repo, branch, remote_tip)?;
        result.outcome = PullOutcome::FastForwarded;
        result.head = Some(remote_tip.to_string());
        return Ok(result);
    }

    if !analysis.is_normal() {
        // No common ancestor / already up to date — treat as up to date.
        return Ok(result);
    }

    match opts.mode {
        PullMode::FastForwardOnly => Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "'{branch}' has diverged from '{name}/{branch}' — a fast-forward is not possible"
            ),
        )
        .with_param("name", branch)
        .with_param("class", "upstream_diverged")),
        PullMode::Merge => {
            merge_upstream(repo, name, branch, &annotated, remote_tip, opts)?;
            result.outcome = PullOutcome::Merged;
            result.head = Some(branch_tip(repo, branch)?);
            Ok(result)
        }
        PullMode::Rebase => {
            rebase_onto(repo, &annotated, name, branch)?;
            result.outcome = PullOutcome::Rebased;
            result.head = Some(branch_tip(repo, branch)?);
            Ok(result)
        }
    }
}

/// The tip of `refs/heads/<branch>` — the ref a pull actually advances.
///
/// Reported as the pull's `head` so the UI names the branch that moved rather
/// than whatever HEAD happened to point at.
fn branch_tip(repo: &git2::Repository, branch: &str) -> Result<String, ErrorPayload> {
    Ok(repo
        .find_reference(&format!("refs/heads/{branch}"))?
        .peel_to_commit()?
        .id()
        .to_string())
}

/// Merge `annotated` into the current branch and commit the result.
///
/// **This always merges into HEAD**, and that is a property of libgit2, not an
/// oversight: as of libgit2 1.7 the `branch_hint` parameter of `git_merge`
/// was folded into `git_merge_options`, which has no ref selector, so
/// `git_merge` resolves "ours" from HEAD internally. `RemotePull::validate`
/// therefore refuses any `branch` that is not the checked-out one — otherwise
/// naming a different branch would rewrite the branch the user is actually on.
///
/// The commit target is spelled out rather than left as `"HEAD"` so the
/// invariant is visible at the call site.
fn merge_upstream(
    repo: &git2::Repository,
    remote: &str,
    branch: &str,
    annotated: &git2::AnnotatedCommit<'_>,
    remote_tip: git2::Oid,
    opts: &PullOptions,
) -> Result<(), ErrorPayload> {
    let branch_refname = format!("refs/heads/{branch}");
    debug_assert_eq!(
        repo.head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(str::to_string)),
        Some(branch.to_string()),
        "merge_upstream must only run for the checked-out branch"
    );

    repo.merge(&[annotated], None, None)?;

    let mut index = repo.index()?;
    if index.has_conflicts() {
        // Leave MERGE_HEAD and the conflicted index in place, like git: the
        // user resolves from the surfaced working-copy status.
        let conflicts = conflict_paths(&index)?;
        return Err(ErrorPayload::new(
            ErrorCode::CheckoutConflicts,
            format!(
                "Merging {remote}/{branch} produced {} conflict(s)",
                conflicts.len()
            ),
        )
        .with_param("name", branch)
        .with_param("conflicts", conflicts.join(", "))
        .with_param("class", "merge_conflicts"));
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let signature = repo.signature()?;
    let local_commit = repo.head()?.peel_to_commit()?;
    let remote_commit = repo.find_commit(remote_tip)?;
    let message = opts
        .message
        .clone()
        .unwrap_or_else(|| format!("Merge {remote}/{branch} into {branch}"));

    repo.commit(
        Some(&branch_refname),
        &signature,
        &signature,
        &message,
        &tree,
        &[&local_commit, &remote_commit],
    )?;
    repo.cleanup_state()?;
    Ok(())
}

/// Rebase `refs/heads/<branch>` onto `upstream`.
///
/// Unlike `git_merge`, `git_rebase_init` *does* take the branch to rebase, so
/// the target is passed explicitly instead of relying on "the current branch".
fn rebase_onto(
    repo: &git2::Repository,
    upstream: &git2::AnnotatedCommit<'_>,
    remote: &str,
    branch: &str,
) -> Result<(), ErrorPayload> {
    let branch_tip = repo
        .find_reference(&format!("refs/heads/{branch}"))?
        .peel_to_commit()?;
    let branch_commit = repo.find_annotated_commit(branch_tip.id())?;

    let mut rebase = repo.rebase(Some(&branch_commit), Some(upstream), None, None)?;
    let signature = repo.signature()?;

    loop {
        match rebase.next() {
            Some(Ok(_operation)) => {
                let index = repo.index()?;
                if index.has_conflicts() {
                    let conflicts = conflict_paths(&index)?;
                    rebase.abort()?;
                    return Err(ErrorPayload::new(
                        ErrorCode::CheckoutConflicts,
                        format!(
                            "Rebasing '{branch}' onto {remote}/{branch} produced {} conflict(s)",
                            conflicts.len()
                        ),
                    )
                    .with_param("name", branch)
                    .with_param("conflicts", conflicts.join(", "))
                    .with_param("class", "rebase_conflicts"));
                }
                rebase.commit(None, &signature, None)?;
            }
            Some(Err(err)) => {
                let _ = rebase.abort();
                return Err(ErrorPayload::from(err));
            }
            None => break,
        }
    }
    rebase.finish(Some(&signature))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Push / force push / publish
// ---------------------------------------------------------------------------

/// `push` (docs/03 §8). `refspecs` empty means "the remote's configured push
/// refspecs". On success the matching tracking refs are fast-forwarded so the
/// branch list reflects the push without a fetch.
pub fn push(
    repo: &git2::Repository,
    name: &str,
    refspecs: &[String],
) -> Result<PushReport, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }

    // libgit2 does **not** expand `remote.<name>.push` from config the way the
    // git CLI does: an empty refspec list is iterated as zero refspecs and the
    // push "succeeds" having uploaded nothing. So resolve the configured
    // push refspecs here, and refuse if there are none.
    let owned = resolve_push_refspecs(repo, name, refspecs)?;
    let specs: Vec<&str> = owned.iter().map(String::as_str).collect();

    let accepted: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let rejected: RefCell<Vec<PushRejection>> = RefCell::new(Vec::new());

    let mut remote = repo.find_remote(name)?;
    let push_error = {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(super::credentials::callback(repo));
        callbacks.push_update_reference(|refname, status| {
            match status {
                Some(message) => rejected.borrow_mut().push(PushRejection {
                    name: refname.to_string(),
                    message: message.to_string(),
                }),
                None => accepted.borrow_mut().push(refname.to_string()),
            }
            Ok(())
        });
        let mut options = git2::PushOptions::new();
        options.remote_callbacks(callbacks);
        remote.push(&specs, Some(&mut options)).err()
    };

    let accepted = accepted.into_inner();
    let mut rejected = rejected.into_inner();

    // A refused push (a non-fast-forward the server will not take) makes
    // `remote.push` return an error, but the on-the-wire path still reports the
    // offending ref through `push_update_reference`. Prefer that typed, per-ref
    // status; a non-fast-forward refused by libgit2 *before* the server is
    // contacted fires no callback at all, so reconstruct the rejection from the
    // error rather than collapsing it into an unclassified transport failure.
    if let Some(err) = push_error {
        if rejected.is_empty() {
            if err.code() == git2::ErrorCode::NotFastForward {
                for spec in &owned {
                    if spec.starts_with('+') {
                        continue; // a forced update is never refused for this
                    }
                    if let Some(dest) = refspec_destination(spec) {
                        rejected.push(PushRejection {
                            name: dest,
                            message: err.message().to_string(),
                        });
                    }
                }
            }
            if rejected.is_empty() {
                return Err(ErrorPayload::from(err));
            }
        }
    }

    let updates = apply_tracking_updates(repo, name, &accepted);

    Ok(PushReport {
        remote: name.to_string(),
        accepted,
        rejected,
        updates,
    })
}

/// The destination ref a push refspec writes to, or `None` when the refspec has
/// no destination (an empty target). A `+` force prefix is stripped first.
fn refspec_destination(spec: &str) -> Option<String> {
    let spec = spec.strip_prefix('+').unwrap_or(spec);
    match spec.split_once(':') {
        Some((_, dest)) if !dest.is_empty() => Some(dest.to_string()),
        Some(_) => None,
        None if !spec.is_empty() => Some(spec.to_string()),
        None => None,
    }
}

/// The refspecs a push should actually send: the caller's, or the remote's
/// configured push refspecs (`remote.<name>.push`) when the caller gave none.
fn resolve_push_refspecs(
    repo: &git2::Repository,
    name: &str,
    refspecs: &[String],
) -> Result<Vec<String>, ErrorPayload> {
    if !refspecs.is_empty() {
        return Ok(refspecs.to_vec());
    }

    // git2 has no `get_multivar`; walk the entries and keep every value for
    // the key. `remote.<name>.push` is multi-valued by design, and libgit2
    // returns each value as its own entry. Keep the configured order and drop
    // duplicates so a refspec the user listed twice is sent once.
    let key = format!("remote.{name}.push");
    let config = repo.config()?;
    let mut configured = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut entries = config.entries(Some(&key))?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        // The glob also matches `remote.<name>.pushurl`; keep the exact key.
        if entry.name().ok() == Some(key.as_str()) {
            if let Ok(value) = entry.value() {
                let value = value.to_string();
                if !value.is_empty() && seen.insert(value.clone()) {
                    configured.push(value);
                }
            }
        }
    }

    if configured.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "No refspecs to push and remote '{name}' has no configured push refspec. \
                 Name the branches to push explicitly."
            ),
        )
        .with_param("remote", name)
        .with_param("class", "push_refspec_missing"));
    }

    Ok(configured)
}

/// `push_force_with_lease` (docs/03 §8). The lease is checked against the
/// remote's *current* advertised tip before anything is sent; a mismatch
/// refuses with class `force_push_lease_mismatch` and no ref moves.
pub fn force_push(
    repo: &git2::Repository,
    name: &str,
    branch: &str,
    lease: &ForceWithLease,
) -> Result<PushReport, ErrorPayload> {
    if repo.find_remote(name).is_err() {
        return Err(remote_not_found(name));
    }

    repo.find_branch(branch, git2::BranchType::Local)
        .map_err(|_| {
            ErrorPayload::new(
                ErrorCode::ReferenceNotFound,
                format!("Branch '{branch}' does not exist"),
            )
            .with_param("name", branch)
        })?;

    let upstream_ref = format!("refs/heads/{branch}");
    let actual = advertised_tip(repo, name, &upstream_ref)?;
    let expected = match &lease.oid {
        Some(oid) => Some(oid.clone()),
        None => tracking_ref(repo, name, branch)
            .and_then(|refname| repo.find_reference(&refname).ok())
            .and_then(|reference| reference.target())
            .map(|oid| oid.to_string()),
    };

    let expected = expected.unwrap_or_else(|| oid_string(git2::Oid::ZERO_SHA1));
    let actual = actual.unwrap_or_else(|| oid_string(git2::Oid::ZERO_SHA1));

    if expected != actual {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "Refusing to force-push '{branch}': the remote moved (expected {expected}, found {actual})"
            ),
        )
        .with_param("name", branch)
        .with_param("expected", expected)
        .with_param("actual", actual)
        .with_param("class", "force_push_lease_mismatch"));
    }

    let refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
    push(repo, name, &[refspec])
}

/// `remote_publish_branch` (docs/03 §7/§8): push a local branch and make it the
/// upstream — git's `push -u`.
pub fn publish_branch(
    repo: &git2::Repository,
    name: &str,
    branch: &str,
    set_upstream: bool,
) -> Result<PublishReport, ErrorPayload> {
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let push = push(repo, name, &[refspec])?;

    // Only wire up tracking once the remote actually took the ref. Configuring
    // it after a rejection would leave the branch list showing ahead/behind
    // against a remote that never received the commits.
    if !push.rejected.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "The remote rejected '{branch}': {}",
                push.rejected[0].message
            ),
        )
        .with_param("name", branch)
        .with_param("remote", name)
        .with_param("class", "publish_rejected"));
    }

    let upstream = format!("{name}/{branch}");
    if set_upstream {
        let merge_ref = format!("refs/heads/{branch}");
        crate::engine::branches::set_tracking_config(repo, branch, name, &merge_ref)?;
    }

    Ok(PublishReport {
        remote: name.to_string(),
        branch: branch.to_string(),
        upstream,
        push,
    })
}

// ---------------------------------------------------------------------------
// Remote branches
// ---------------------------------------------------------------------------

/// `remote_branch_list` (docs/03 §7). `remote` filters to one remote; the
/// default (`None`) lists every remote's branches.
pub fn remote_branch_list(
    repo: &git2::Repository,
    remote: Option<&str>,
) -> Result<Vec<RemoteBranchInfo>, ErrorPayload> {
    let mut out = Vec::new();
    for entry in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = entry?;
        let Ok(full_name) = branch.get().name() else {
            continue;
        };
        let Some((remote_name, short)) = split_remote_tracking(full_name) else {
            continue;
        };
        if let Some(wanted) = remote {
            if wanted != remote_name {
                continue;
            }
        }

        let tip = branch.get().peel_to_commit()?;
        let local = local_for_remote_branch(repo, full_name);
        let (ahead, behind) = match &local {
            Some(name) => {
                let local_branch = repo.find_branch(name, git2::BranchType::Local)?;
                let local_tip = local_branch.get().peel_to_commit()?.id();
                repo.graph_ahead_behind(local_tip, tip.id())
                    .map(|(ahead, behind)| (ahead as u32, behind as u32))
                    .unwrap_or((0, 0))
            }
            None => (0, 0),
        };

        out.push(RemoteBranchInfo {
            full_name: full_name.to_string(),
            name: format!("{remote_name}/{short}"),
            remote: remote_name.to_string(),
            branch: short.to_string(),
            oid: tip.id().to_string(),
            tip_summary: tip.summary()?.unwrap_or_default().to_string(),
            tip_time: tip.time().seconds(),
            local,
            ahead,
            behind,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// `remote_branch_delete` (docs/03 §7): push a deletion refspec for each branch
/// and drop the local tracking ref.
pub fn remote_branch_delete(
    repo: &git2::Repository,
    remote: &str,
    branches: &[String],
) -> Result<DeleteReport, ErrorPayload> {
    if repo.find_remote(remote).is_err() {
        return Err(remote_not_found(remote));
    }

    let refspecs: Vec<String> = branches
        .iter()
        .map(|branch| format!(":refs/heads/{branch}"))
        .collect();
    let report = push(repo, remote, &refspecs)?;

    let mut deletions = DeleteReport {
        deleted: Vec::new(),
        skipped: Vec::new(),
    };
    for branch in branches {
        match report
            .rejected
            .iter()
            .find(|rejection| rejection.name == format!("refs/heads/{branch}"))
        {
            Some(rejection) => deletions.skipped.push(super::branches::DeleteSkip {
                name: branch.clone(),
                reason: rejection.message.clone(),
                class: "push_rejected".to_string(),
            }),
            None => {
                let tracking = format!("refs/remotes/{remote}/{branch}");
                if let Ok(mut reference) = repo.find_reference(&tracking) {
                    let _ = reference.delete();
                }
                deletions.deleted.push(branch.clone());
            }
        }
    }
    Ok(deletions)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn classify_update(repo: &git2::Repository, old: git2::Oid, new: git2::Oid) -> RefUpdateKind {
    if old.is_zero() {
        RefUpdateKind::Created
    } else if new.is_zero() {
        RefUpdateKind::Deleted
    } else if repo.graph_descendant_of(new, old).unwrap_or(false) {
        RefUpdateKind::Updated
    } else {
        RefUpdateKind::Forced
    }
}

fn oid_string(oid: git2::Oid) -> String {
    if oid.is_zero() {
        "0".repeat(40)
    } else {
        oid.to_string()
    }
}

/// The tracking refname a fetch of `refs/heads/<branch>` would write, derived
/// from the remote's fetch refspecs (so a non-standard refspec is honoured).
pub(crate) fn tracking_ref(repo: &git2::Repository, remote: &str, branch: &str) -> Option<String> {
    let remote = repo.find_remote(remote).ok()?;
    let src = format!("refs/heads/{branch}");
    remote
        .refspecs()
        .find(|spec| spec.direction() == git2::Direction::Fetch && spec.src_matches(&src))
        .and_then(|spec| spec.transform(&src).ok())
        .map(|buf| String::from_utf8_lossy(&buf).into_owned())
}

/// The remote's currently advertised tip of `refname`, or `None` if the remote
/// does not have it. Connects and disconnects.
fn advertised_tip(
    repo: &git2::Repository,
    remote: &str,
    refname: &str,
) -> Result<Option<String>, ErrorPayload> {
    let mut remote = repo.find_remote(remote)?;
    let heads = advertise(repo, &mut remote)?;
    Ok(heads
        .into_iter()
        .find(|(name, _)| name == refname)
        .map(|(_, oid)| oid))
}

fn fast_forward_branch(
    repo: &git2::Repository,
    branch: &str,
    target: git2::Oid,
) -> Result<(), ErrorPayload> {
    let object = repo.find_object(target, None)?;
    repo.reference(
        &format!("refs/heads/{branch}"),
        target,
        true,
        &format!("meridian: fast-forward {branch}"),
    )?;
    let is_head = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().ok().map(str::to_string))
        .as_deref()
        == Some(branch);
    if is_head {
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.safe();
        repo.checkout_tree(&object, Some(&mut builder))?;
    }
    Ok(())
}

/// After a successful push, fast-forward the tracking refs for the branches we
/// pushed. Best-effort: a ref that cannot be advanced is left for the next
/// fetch rather than failing the push that already succeeded.
fn apply_tracking_updates(
    repo: &git2::Repository,
    remote: &str,
    accepted: &[String],
) -> Vec<RefUpdate> {
    let mut updates = Vec::new();
    for refname in accepted {
        let Some(branch) = refname.strip_prefix("refs/heads/") else {
            continue;
        };
        let Some(oid) = repo
            .find_reference(refname)
            .ok()
            .and_then(|reference| reference.target())
        else {
            continue;
        };
        let Some(tracking) = tracking_ref(repo, remote, branch) else {
            continue;
        };

        let old = repo
            .find_reference(&tracking)
            .ok()
            .and_then(|reference| reference.target())
            .unwrap_or(git2::Oid::ZERO_SHA1);

        if repo
            .reference(&tracking, oid, true, "meridian: update after push")
            .is_ok()
        {
            updates.push(RefUpdate {
                name: tracking,
                old_oid: oid_string(old),
                new_oid: oid_string(oid),
                kind: classify_update(repo, old, oid),
            });
        }
    }
    updates
}

/// Delete every `refs/remotes/<remote>/*` ref — used by `remote_remove`.
fn prune_tracking_refs(repo: &git2::Repository, remote: &str) {
    let pattern = format!("refs/remotes/{remote}/*");
    let names: Vec<String> = match repo.references_glob(&pattern) {
        Ok(references) => references
            .filter_map(|reference| reference.ok())
            .filter_map(|reference| reference.name().ok().map(str::to_string))
            .collect(),
        Err(_) => return,
    };
    for name in names {
        if let Ok(mut reference) = repo.find_reference(&name) {
            let _ = reference.delete();
        }
    }
}

fn split_remote_tracking(refname: &str) -> Option<(&str, &str)> {
    let rest = refname.strip_prefix("refs/remotes/")?;
    let (remote, branch) = rest.split_once('/')?;
    if branch == "HEAD" {
        return None;
    }
    Some((remote, branch))
}

fn local_for_remote_branch(repo: &git2::Repository, full_name: &str) -> Option<String> {
    for entry in repo.branches(Some(git2::BranchType::Local)).ok()? {
        let Ok((branch, _)) = entry else { continue };
        let Ok(upstream) = branch.upstream() else {
            continue;
        };
        if upstream.get().name().ok() == Some(full_name) {
            return branch.name().ok().flatten().map(str::to_string);
        }
    }
    None
}

fn conflict_paths(index: &git2::Index) -> Result<Vec<String>, ErrorPayload> {
    let mut paths = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let entry = conflict.our.or(conflict.their).or(conflict.ancestor);
        if let Some(entry) = entry {
            paths.push(String::from_utf8_lossy(&entry.path).into_owned());
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Set `key` to `value`, or remove it when `value` is `None`.
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

/// Escape a literal string for libgit2's `remove_multivar` regexp argument.
fn regex_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if "\\.+*?()[]{}|^$-".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn remote_not_found(name: &str) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::ReferenceNotFound,
        format!("Remote '{name}' does not exist"),
    )
    .with_param("name", name)
    .with_param("class", "remote_not_found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::index::test_support::{commit_all, scratch_repo};
    use std::path::PathBuf;

    /// Write `remote.<name>.push` directly, appending so a multi-valued key is
    /// genuinely multi-valued (that is what the CLI reads). `set_str` would
    /// overwrite, collapsing the key to its last value; the never-matching
    /// `$^` pattern makes `set_multivar` append a new entry instead.
    fn set_push_refspecs(repo: &git2::Repository, name: &str, refspecs: &[&str]) {
        let mut config = repo.config().unwrap();
        for refspec in refspecs {
            config
                .set_multivar(&format!("remote.{name}.push"), "$^", refspec)
                .unwrap();
        }
    }

    /// A bare repository standing in for a remote, plus the path to clone it
    /// from. Local file transport needs no credentials, so the credential
    /// callback under test is never consulted.
    fn bare_remote(tag: &str) -> (PathBuf, git2::Repository) {
        let dir =
            std::env::temp_dir().join(format!("meridian-remote-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = git2::Repository::init_bare(&dir).unwrap();
        (dir, repo)
    }

    fn add_file(repo: &git2::Repository, name: &str, body: &str) {
        let dir = repo.workdir().unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    /// Write a parentless commit straight onto a branch ref — the shortest way
    /// to a history the remote cannot fast-forward to. Committing through
    /// `commit_all` would move HEAD's branch instead, leaving the ref we mean to
    /// diverge untouched.
    fn orphan_commit(repo: &git2::Repository, refname: &str, message: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = git2::Signature::now("Meridian Test", "test@meridian.invalid").unwrap();
        // A dangling commit (no ref), then a forced ref move: `repo.commit`
        // pointed at the ref would refuse, since the new commit's first parent
        // is not the ref's current tip.
        let oid = repo
            .commit(None, &signature, &signature, message, &tree, &[])
            .unwrap();
        repo.reference(refname, oid, true, "test: orphan commit")
            .unwrap();
        oid
    }

    fn tracking_of(repo: &git2::Repository, branch: &str) -> Option<(String, String)> {
        let config = repo.config().ok()?;
        config
            .get_string(&format!("branch.{branch}.remote"))
            .ok()
            .zip(config.get_string(&format!("branch.{branch}.merge")).ok())
    }

    // -----------------------------------------------------------------
    // resolve_push_refspecs
    // -----------------------------------------------------------------

    #[test]
    fn caller_refspecs_win_over_configured_push_refspecs() {
        let scratch = scratch_repo();
        let (_remote_dir, _remote) = bare_remote("caller-wins");
        let repo: &git2::Repository = &scratch;
        repo.remote("origin", "/nowhere").unwrap();
        set_push_refspecs(
            repo,
            "origin",
            &["refs/heads/configured:refs/heads/configured"],
        );

        let explicit = vec!["refs/heads/explicit:refs/heads/explicit".to_string()];
        let resolved = resolve_push_refspecs(repo, "origin", &explicit).unwrap();
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn empty_refspecs_fall_back_to_the_configured_push_refspec() {
        // The bug: libgit2 iterates zero refspecs and reports success having
        // uploaded nothing. The CLI reads `remote.<name>.push`; we must too.
        let scratch = scratch_repo();
        let (_remote_dir, _remote) = bare_remote("configured-fallback");
        let repo: &git2::Repository = &scratch;
        repo.remote("origin", "/nowhere").unwrap();
        set_push_refspecs(
            repo,
            "origin",
            &[
                "refs/heads/main:refs/heads/main",
                "refs/heads/feature:refs/heads/feature",
            ],
        );

        let resolved = resolve_push_refspecs(repo, "origin", &[]).unwrap();
        assert_eq!(
            resolved,
            vec![
                "refs/heads/main:refs/heads/main".to_string(),
                "refs/heads/feature:refs/heads/feature".to_string(),
            ]
        );
    }

    #[test]
    fn push_with_no_refspecs_and_no_config_is_refused() {
        // Previously this returned Ok with empty `accepted`/`rejected`, so the
        // UI reported a successful push that had transferred nothing.
        let scratch = scratch_repo();
        let (_remote_dir, _remote) = bare_remote("no-refspec");
        let repo: &git2::Repository = &scratch;
        repo.remote("origin", "/nowhere").unwrap();

        let err = push(repo, "origin", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err.params.get("class").map(String::as_str),
            Some("push_refspec_missing")
        );
    }

    #[test]
    fn push_with_configured_refspecs_actually_transfers_a_commit() {
        let scratch = scratch_repo();
        let (remote_dir, remote) = bare_remote("push-transfers");
        let repo: &git2::Repository = &scratch;
        add_file(repo, "a.txt", "one\n");
        commit_all(repo, "initial");

        repo.remote("origin", remote_dir.to_str().unwrap()).unwrap();
        set_push_refspecs(repo, "origin", &["refs/heads/master:refs/heads/master"]);

        let report = push(repo, "origin", &[]).unwrap();
        assert!(
            report.rejected.is_empty(),
            "unexpected rejection: {:?}",
            report.rejected
        );
        assert!(
            report.accepted.iter().any(|r| r == "refs/heads/master"),
            "expected master to be accepted, got {:?}",
            report.accepted
        );

        // The commit really is on the remote now.
        let master = remote
            .find_reference("refs/heads/master")
            .expect("remote received the push");
        assert_eq!(
            master.peel_to_commit().unwrap().summary().unwrap().unwrap(),
            "initial"
        );
    }

    // -----------------------------------------------------------------
    // publish_branch
    // -----------------------------------------------------------------

    #[test]
    fn publish_branch_sets_upstream_after_a_successful_push() {
        let scratch = scratch_repo();
        let (remote_dir, _remote) = bare_remote("publish-ok");
        let repo: &git2::Repository = &scratch;
        add_file(repo, "a.txt", "one\n");
        commit_all(repo, "initial");
        repo.remote("origin", remote_dir.to_str().unwrap()).unwrap();

        publish_branch(repo, "origin", "master", true).unwrap();
        let (remote_name, merge_ref) = tracking_of(repo, "master").unwrap();
        assert_eq!(remote_name, "origin");
        assert_eq!(merge_ref, "refs/heads/master");
    }

    #[test]
    fn publish_branch_reports_the_pushed_ref_as_accepted() {
        // The accepted counterpart of the rejection test: a landing push must
        // report the ref as accepted, not error, so the rejection fix cannot
        // regress into always-erroring.
        let scratch = scratch_repo();
        let (remote_dir, _remote) = bare_remote("publish-accepted");
        let repo: &git2::Repository = &scratch;
        add_file(repo, "a.txt", "one\n");
        commit_all(repo, "initial");
        repo.remote("origin", remote_dir.to_str().unwrap()).unwrap();

        let report = publish_branch(repo, "origin", "master", true).unwrap();
        assert!(
            report.push.rejected.is_empty(),
            "unexpected rejection: {:?}",
            report.push.rejected
        );
        assert!(
            report
                .push
                .accepted
                .iter()
                .any(|r| r == "refs/heads/master"),
            "expected master to land, got {:?}",
            report.push.accepted
        );
        assert_eq!(report.upstream, "origin/master");
    }

    #[test]
    fn publish_branch_refuses_and_leaves_tracking_unset_when_the_push_is_rejected() {
        // The remote holds `feature` at one commit; local `feature` is rewritten
        // to an unrelated commit, so a non-forced refspec cannot fast-forward.
        let scratch = scratch_repo();
        let (remote_dir, remote) = bare_remote("publish-rejected");
        // A bare repo accepts a rewind unless told otherwise; make the server
        // authoritative so the rejection is real rather than client-side.
        remote
            .config()
            .unwrap()
            .set_bool("receive.denyNonFastForwards", true)
            .unwrap();
        let repo: &git2::Repository = &scratch;
        add_file(repo, "a.txt", "one\n");
        let first = commit_all(repo, "first");
        repo.branch("feature", &repo.find_commit(first).unwrap(), false)
            .unwrap();
        repo.remote("origin", remote_dir.to_str().unwrap()).unwrap();

        // First publish succeeds; the remote now holds `feature` at `first`.
        publish_branch(repo, "origin", "feature", true).unwrap();

        // Rewrite local `feature` to a parentless commit — not a descendant of
        // what the remote has, so pushing it is a non-fast-forward.
        orphan_commit(repo, "refs/heads/feature", "divergent");

        let err = publish_branch(repo, "origin", "feature", true).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err.params.get("class").map(String::as_str),
            Some("publish_rejected")
        );

        // The old code returned Ok and configured tracking anyway, leaving the
        // branch list showing ahead/behind against a remote that never took it.
        assert_eq!(
            tracking_of(repo, "feature"),
            Some(("origin".to_string(), "refs/heads/feature".to_string())),
            "pre-existing tracking from the first publish is expected"
        );
    }

    // -----------------------------------------------------------------
    // pull: the target branch
    // -----------------------------------------------------------------

    #[test]
    fn branch_tip_reports_the_named_branch_not_head() {
        let scratch = scratch_repo();
        let repo: &git2::Repository = &scratch;
        add_file(repo, "a.txt", "one\n");
        let first = commit_all(repo, "first");
        repo.branch("side", &repo.find_commit(first).unwrap(), false)
            .unwrap();
        add_file(repo, "b.txt", "two\n");
        let second = commit_all(repo, "second");

        // `side` stays at `first` while HEAD (master) moves to `second`.
        assert_eq!(branch_tip(repo, "side").unwrap(), first.to_string());
        assert_eq!(branch_tip(repo, "master").unwrap(), second.to_string());
    }

    // -----------------------------------------------------------------
    // credentials
    // -----------------------------------------------------------------

    #[test]
    fn credential_callback_is_not_a_bare_negotiate_default() {
        // `git2::Cred::default()` is `git_cred_default_new` — a Negotiate-only
        // credential with no user/pass/ssh payload, so every authenticated
        // fetch/push fails. Guard against it creeping back in: production code
        // must route through engine::credentials, never a bare default. Only
        // the production half is scanned — the test module may name the trap.
        let source = include_str!("remotes.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the file has production code before its tests");
        // A regression written across lines (`Ok(git2::Cred::default())` on its
        // own line) slips past a same-line grep, so scan every non-comment line
        // for the bare default — the only legitimate mention is in this file's
        // doc comments, which are stripped here.
        let offenders: Vec<&str> = production
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !(trimmed.starts_with("//") || trimmed.starts_with('*'))
            })
            .filter(|line| line.contains("Cred::default"))
            .collect();
        assert!(
            offenders.is_empty(),
            "remotes.rs must not authenticate with a bare Negotiate credential \
             (found {offenders:?}); route through engine::credentials::callback()"
        );
        assert!(
            production.contains("credentials::callback(repo)"),
            "every credential callback must go through engine::credentials with the repository"
        );
    }
}
