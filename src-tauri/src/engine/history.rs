//! History engine — the `GTGitLoadCommitsOperation` family (docs/03 §5).
//!
//! One revwalk per load, batched reads, no re-walking: the page carries the
//! parents of every row it returns, so the lane layout is computed from the
//! same pass ([`super::graph`]) and the decoration map is built with a single
//! sweep over the refs. Per-commit object reads happen only for rows that are
//! actually returned (plus whatever a search or path filter must inspect).
//!
//! Vocabulary (shared by every function here):
//!
//! * `from` — the commit to walk *from*, inclusive; defaults to `HEAD`.
//! * `to` — when given, the commit to hide together with its ancestors; the
//!   walk covers `from` down to (but excluding) `to`.
//!
//! That is the orientation the history store already uses (`from` is where
//! "load more" resumes), and it matches `history_revision_range_changeset_load`
//! only in spirit — there the pair names two revisions, not a walk window.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use git2::{Delta, DiffLineType, Oid, Sort};

use crate::domain::commit::{
    Changeset, ChangesetEntry, ChangesetStats, Coauthor, CommitDetail, CommitRow, Contributor,
    FileRevision, HistoryPage, HistoryQuery, RefDecoration, RefDecorationKind, SearchMode,
    SignatureState, Trailer, DEFAULT_HISTORY_LIMIT, MAX_HISTORY_LIMIT, MAX_HISTORY_SCAN,
    SHORT_OID_LEN,
};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::history::{HistorySort, Signature};
use crate::domain::revision::Revision;
use crate::domain::status::FileStatus;

use super::graph::{self, GraphNode};

pub use super::graph::draw_graph;

// ---------------------------------------------------------------------------
// Revisions
// ---------------------------------------------------------------------------

/// Resolve a user-supplied revision to a commit. Failure is an
/// `UnknownRevision` carrying the spec that did not resolve (docs/03 §5).
pub fn resolve<'repo>(
    repo: &'repo git2::Repository,
    revision: &Revision,
) -> Result<git2::Commit<'repo>, ErrorPayload> {
    let (spec, fallback) = match revision {
        Revision::Head => ("HEAD".to_string(), None),
        Revision::Branch(name) => (format!("refs/heads/{name}"), Some(name.clone())),
        Revision::Tag(name) => (format!("refs/tags/{name}"), Some(name.clone())),
        Revision::Oid(hex) => (hex.clone(), None),
    };

    match revparse_commit(repo, &spec) {
        Ok(commit) => Ok(commit),
        Err(primary) => match fallback {
            // A "branch" name may also be a remote-tracking branch
            // ("origin/main") or a bare tag: fall back to git's own resolution
            // before declaring the revision unknown.
            Some(name) => revparse_commit(repo, &name).map_err(|_| primary),
            None => Err(primary),
        },
    }
}

fn revparse_commit<'repo>(
    repo: &'repo git2::Repository,
    spec: &str,
) -> Result<git2::Commit<'repo>, ErrorPayload> {
    let object = repo
        .revparse_single(spec)
        .map_err(|err| ErrorPayload::from(err).with_param("revision", spec.to_string()))?;
    object
        .peel_to_commit()
        .map_err(|err| ErrorPayload::from(err).with_param("revision", spec.to_string()))
}

/// The tip of `HEAD`, or `None` while HEAD is unborn (a fresh repository).
fn head_tip<'repo>(
    repo: &'repo git2::Repository,
) -> Result<Option<git2::Commit<'repo>>, ErrorPayload> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_commit()?)),
        Err(err)
            if matches!(
                err.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

fn parse_oid(hex: &str) -> Result<Oid, ErrorPayload> {
    Oid::from_str(hex).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::UnknownRevision,
            format!("'{hex}' is not a commit hash"),
        )
        .with_param("revision", hex.to_string())
    })
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

/// `history_commits_load` (docs/03 §5): one revwalk page — commit rows with
/// their parents, the lane layout for exactly those rows, and the pagination
/// envelope.
///
/// Filtering (`path`, `search`) happens *inside* the walk, so `skip` counts
/// matches, not scanned commits. A filtered walk is bounded by
/// [`MAX_HISTORY_SCAN`] commits; [`HistoryPage::truncated`] then means "the
/// walk stopped before the page (or the scan cap) was exhausted".
pub fn load_page(
    repo: &git2::Repository,
    query: &HistoryQuery,
) -> Result<HistoryPage, ErrorPayload> {
    let limit = query.limit.clamp(1, MAX_HISTORY_LIMIT);
    let path = normalise_path(query.path.as_deref());
    let needle = SearchNeedle::new(query);

    // A full object id never needs a walk.
    if let Some(page) = direct_lookup(repo, &needle, &path, query.skip, limit)? {
        return Ok(page);
    }

    let mut walk = repo.revwalk()?;
    walk.set_sorting(match query.sort {
        HistorySort::Date => Sort::TIME,
        HistorySort::Topo => Sort::TOPOLOGICAL | Sort::TIME,
    })?;
    if query.first_parent {
        walk.simplify_first_parent()?;
    }

    let tip = match &query.from {
        Some(revision) => resolve(repo, revision)?,
        None => match head_tip(repo)? {
            Some(commit) => commit,
            None => return Ok(empty_page(query.skip, limit)),
        },
    };
    walk.push(tip.id())?;
    if let Some(revision) = &query.to {
        walk.hide(resolve(repo, revision)?.id())?;
    }

    let decorations = decoration_map(repo);
    let mut watcher = path
        .as_ref()
        .map(|path| PathWatcher::new(path.clone(), query.follow_renames));

    let mut commits: Vec<CommitRow> = Vec::with_capacity(limit.min(DEFAULT_HISTORY_LIMIT));
    let mut scanned: u64 = 0;
    let mut matches: u64 = 0;
    let mut truncated = false;

    for oid in walk {
        let oid = oid?;
        // The page is full: one more commit exists to be had. With filters this
        // is deliberately optimistic — the alternative is scanning the rest of
        // the history before the UI may show "load more".
        if commits.len() >= limit {
            truncated = true;
            break;
        }
        scanned += 1;
        if scanned > MAX_HISTORY_SCAN {
            truncated = true;
            break;
        }

        if let Some(watcher) = watcher.as_mut() {
            if watcher.check(repo, oid)?.is_none() {
                continue;
            }
        }

        let commit = repo.find_commit(oid)?;
        if !needle.matches(&commit) {
            continue;
        }

        matches += 1;
        if matches <= query.skip as u64 {
            continue;
        }
        commits.push(row_from(&commit, &decorations));
    }

    let nodes: Vec<GraphNode> = commits
        .iter()
        .map(|row| GraphNode::new(row.oid.clone(), row.parents.clone()))
        .collect();
    let graph = graph::draw_graph(&nodes);
    // Computed before `commits` is moved into the page.
    let next_skip = truncated.then_some(query.skip + commits.len());

    Ok(HistoryPage {
        commits,
        graph,
        skip: query.skip,
        limit,
        next_skip,
        truncated,
        scanned,
    })
}

fn empty_page(skip: usize, limit: usize) -> HistoryPage {
    HistoryPage {
        commits: Vec::new(),
        graph: Default::default(),
        skip,
        limit,
        next_skip: None,
        truncated: false,
        scanned: 0,
    }
}

/// `history_graph_compute` (docs/03 §5): lane layout for an explicit oid list,
/// in the order it was given. One object read per oid, no walk.
pub fn graph_for_oids(
    repo: &git2::Repository,
    oids: &[String],
) -> Result<crate::domain::commit::GraphLayout, ErrorPayload> {
    let mut nodes = Vec::with_capacity(oids.len());
    for hex in oids {
        let oid = parse_oid(hex)?;
        let commit = repo.find_commit(oid)?;
        nodes.push(GraphNode::new(
            hex.clone(),
            commit
                .parent_ids()
                .map(|parent| parent.to_string())
                .collect(),
        ));
    }
    Ok(graph::draw_graph(&nodes))
}

/// `history_commit_hashes_load` (docs/03 §5): commit ids only — no objects are
/// loaded, which is what makes prefetching a page ahead cheap.
pub fn commit_hashes(
    repo: &git2::Repository,
    start: Option<&Revision>,
    skip: usize,
    count: usize,
) -> Result<Vec<String>, ErrorPayload> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    match start {
        Some(revision) => walk.push(resolve(repo, revision)?.id())?,
        None => match head_tip(repo)? {
            Some(commit) => walk.push(commit.id())?,
            None => return Ok(Vec::new()),
        },
    }

    let count = count.min(MAX_HISTORY_LIMIT);
    let mut hashes = Vec::with_capacity(count);
    for oid in walk.skip(skip).take(count) {
        hashes.push(oid?.to_string());
    }
    Ok(hashes)
}

/// `history_count_load` (docs/03 §5): the number of commits reachable from
/// `from` (default HEAD). Cached per `(gitdir, HEAD)` — a count is a full walk
/// and the value cannot change while HEAD does not.
pub fn count(repo: &git2::Repository, from: Option<&Revision>) -> Result<u64, ErrorPayload> {
    let cache_key = match (from, repo.path().to_str(), head_tip(repo)?) {
        (None, Some(gitdir), Some(head)) => Some(format!("{gitdir}#{}", head.id())),
        _ => None,
    };
    if let Some(key) = &cache_key {
        if let Some(hit) = count_cache_get(key) {
            return Ok(hit);
        }
    }

    let mut walk = repo.revwalk()?;
    match from {
        Some(revision) => walk.push(resolve(repo, revision)?.id())?,
        None => walk.push_head()?,
    }
    let mut total: u64 = 0;
    for oid in walk {
        oid?;
        total += 1;
    }

    if let Some(key) = cache_key {
        count_cache_put(key, total);
    }
    Ok(total)
}

/// `history_contributors_load` (docs/03 §5): authors aggregated by email.
///
/// This is a full-history walk (`git shortlog -s` in effect) — the one load in
/// this module without a page bound, because its answer is the whole point.
pub fn contributors(
    repo: &git2::Repository,
    from: Option<&Revision>,
) -> Result<Vec<Contributor>, ErrorPayload> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TIME)?;
    match from {
        Some(revision) => walk.push(resolve(repo, revision)?.id())?,
        None => match head_tip(repo)? {
            Some(commit) => walk.push(commit.id())?,
            None => return Ok(Vec::new()),
        },
    }

    struct Acc {
        names: HashMap<String, u64>,
        commits: u64,
        first: i64,
        last: i64,
    }

    let mut by_identity: HashMap<String, Acc> = HashMap::new();
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        let author = commit.author();
        let name = author.name().unwrap_or_default().to_string();
        let email = author.email().unwrap_or_default().to_string();
        // An identity is an email; a nameless commit falls back to its name so
        // it is not silently dropped.
        let key = if email.is_empty() {
            name.clone()
        } else {
            email.to_ascii_lowercase()
        };
        if key.is_empty() {
            continue;
        }
        let time = author.when().seconds();
        let entry = by_identity.entry(key).or_insert_with(|| Acc {
            names: HashMap::new(),
            commits: 0,
            first: time,
            last: time,
        });
        *entry.names.entry(name).or_insert(0) += 1;
        entry.commits += 1;
        entry.first = entry.first.min(time);
        entry.last = entry.last.max(time);
    }

    let mut contributors: Vec<Contributor> = by_identity
        .into_iter()
        .map(|(email, acc)| Contributor {
            name: acc
                .names
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                .map(|(name, _)| name)
                .unwrap_or_else(|| email.clone()),
            email,
            commits: acc.commits,
            first_time: acc.first,
            last_time: acc.last,
        })
        .collect();
    contributors.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.email.cmp(&b.email))
    });
    Ok(contributors)
}

/// `history_file_history_load` (docs/03 §5): the commits that changed one
/// path, newest first.
///
/// With `follow_renames` the walk follows the path backwards through renames
/// (git's own greedy strategy: one name at a time).
pub fn file_history(
    repo: &git2::Repository,
    path: &str,
    skip: usize,
    limit: usize,
    follow_renames: bool,
) -> Result<Vec<FileRevision>, ErrorPayload> {
    let limit = limit.clamp(1, MAX_HISTORY_LIMIT);
    let mut watcher = PathWatcher::new(path.to_string(), follow_renames);

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    let Some(head) = head_tip(repo)? else {
        return Ok(Vec::new());
    };
    walk.push(head.id())?;

    let mut revisions = Vec::new();
    let mut matches = 0usize;
    let mut scanned: u64 = 0;
    for oid in walk {
        let oid = oid?;
        scanned += 1;
        if scanned > MAX_HISTORY_SCAN {
            break;
        }
        let Some(change) = watcher.check(repo, oid)? else {
            continue;
        };
        matches += 1;
        if matches <= skip {
            continue;
        }
        let commit = repo.find_commit(oid)?;
        revisions.push(FileRevision {
            oid: oid.to_string(),
            short_oid: short_oid(&oid.to_string()),
            path: change.path,
            old_path: change.old_path,
            status: change.status,
            insertions: change.insertions,
            deletions: change.deletions,
            binary: change.binary,
            summary: summary_of(&commit),
            author: signature(commit.author()),
            committer: signature(commit.committer()),
        });
        if revisions.len() >= limit {
            break;
        }
    }
    Ok(revisions)
}

// ---------------------------------------------------------------------------
// Commit detail and changesets
// ---------------------------------------------------------------------------

/// Options for the tree-diff reads. Rename detection is on by default, which is
/// what the changeset list wants; turning it off is the cheap mode.
#[derive(Clone, Copy, Debug)]
pub struct ChangesetOptions {
    pub detect_renames: bool,
}

impl Default for ChangesetOptions {
    fn default() -> Self {
        Self {
            detect_renames: true,
        }
    }
}

/// `history_commit_load` (docs/03 §5): one commit in full — parents (octopus
/// merges keep every parent), bodies, coauthors, trailers, signature state,
/// decorations and the diff stats against the first parent.
pub fn load_commit(repo: &git2::Repository, oid: Oid) -> Result<CommitDetail, ErrorPayload> {
    let commit = repo.find_commit(oid)?;
    let hex = oid.to_string();
    let refs = decoration_map(repo).remove(&oid).unwrap_or_default();

    let message = message_of(&commit);
    let (summary, body) = split_message(&message);
    let trailers = parse_trailers(&message);
    let coauthors = coauthors_from(&trailers);

    Ok(CommitDetail {
        short_oid: short_oid(&hex),
        oid: hex,
        parents: commit
            .parent_ids()
            .map(|parent| parent.to_string())
            .collect(),
        parent_count: commit.parent_count() as u32,
        tree: commit.tree_id().to_string(),
        summary,
        body,
        message,
        author: signature(commit.author()),
        committer: signature(commit.committer()),
        decorations: refs.iter().map(decoration_label).collect(),
        refs,
        signature: signature_state_of(&commit),
        coauthors,
        trailers,
        stats: commit_stats(repo, &commit, &ChangesetOptions::default())?,
    })
}

/// `history_changeset_load` (docs/03 §5): the file list of one commit against
/// its first parent (a root commit against the empty tree), with per-file
/// status and line counts.
pub fn load_changeset(
    repo: &git2::Repository,
    oid: Oid,
    opts: &ChangesetOptions,
) -> Result<Changeset, ErrorPayload> {
    let commit = repo.find_commit(oid)?;
    let parent = commit.parent(0).ok();
    let diff = diff_commits(repo, parent.as_ref(), &commit, opts)?;
    let entries = collect_entries(diff_entries(&diff)?);

    Ok(Changeset {
        oid: oid.to_string(),
        parent: parent.map(|parent| parent.id().to_string()),
        from: None,
        entries: entries.0,
        stats: entries.1,
    })
}

/// `history_revision_range_changeset_load` (docs/03 §5): the net changeset of a
/// revision range, diffed tree to tree.
pub fn load_range_changeset(
    repo: &git2::Repository,
    from: &Revision,
    to: &Revision,
    opts: &ChangesetOptions,
) -> Result<Changeset, ErrorPayload> {
    let to_commit = resolve(repo, to)?;
    let from_commit = resolve(repo, from)?;
    let diff = diff_trees(
        repo,
        Some(&from_commit.tree()?),
        Some(&to_commit.tree()?),
        opts,
    )?;
    let entries = collect_entries(diff_entries(&diff)?);

    Ok(Changeset {
        oid: to_commit.id().to_string(),
        parent: None,
        from: Some(from_commit.id().to_string()),
        entries: entries.0,
        stats: entries.1,
    })
}

fn commit_stats(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    opts: &ChangesetOptions,
) -> Result<ChangesetStats, ErrorPayload> {
    let parent = commit.parent(0).ok();
    let diff = diff_commits(repo, parent.as_ref(), commit, opts)?;
    let stats = diff.stats()?;
    Ok(ChangesetStats {
        files_changed: stats.files_changed() as u32,
        insertions: stats.insertions() as u32,
        deletions: stats.deletions() as u32,
    })
}

// ---------------------------------------------------------------------------
// Diff plumbing
// ---------------------------------------------------------------------------

fn diff_commits<'repo>(
    repo: &'repo git2::Repository,
    parent: Option<&git2::Commit<'_>>,
    commit: &git2::Commit<'_>,
    opts: &ChangesetOptions,
) -> Result<git2::Diff<'repo>, ErrorPayload> {
    let new_tree = commit.tree()?;
    let old_tree = match parent {
        Some(parent) => Some(parent.tree()?),
        None => None,
    };
    diff_trees(repo, old_tree.as_ref(), Some(&new_tree), opts)
}

fn diff_trees<'repo>(
    repo: &'repo git2::Repository,
    old: Option<&git2::Tree<'_>>,
    new: Option<&git2::Tree<'_>>,
    opts: &ChangesetOptions,
) -> Result<git2::Diff<'repo>, ErrorPayload> {
    let mut diff_opts = git2::DiffOptions::new();
    // Only line counts are read back out of these diffs; context lines are
    // pure cost.
    diff_opts.context_lines(0);
    let mut diff = repo.diff_tree_to_tree(old, new, Some(&mut diff_opts))?;
    if opts.detect_renames {
        let mut find = git2::DiffFindOptions::new();
        find.renames(true);
        diff.find_similar(Some(&mut find))?;
    }
    Ok(diff)
}

/// One delta of a diff, with its line counts filled in by
/// [`diff_entries`].
struct EntryAcc {
    path: String,
    old_path: Option<String>,
    status: FileStatus,
    binary: bool,
    insertions: u32,
    deletions: u32,
}

/// Walk a diff once and return one entry per file, with `+`/`-` counts.
///
/// The line callback cannot borrow the same state as the file callback, so the
/// accumulator is shared through a `RefCell` and the current index through a
/// `Cell` — one pass, no patch re-reading.
fn diff_entries(diff: &git2::Diff<'_>) -> Result<Vec<EntryAcc>, ErrorPayload> {
    let entries: RefCell<Vec<EntryAcc>> = RefCell::new(Vec::new());
    let current: Cell<usize> = Cell::new(0);

    {
        let mut file_cb = |delta: git2::DiffDelta<'_>, _progress: f32| -> bool {
            let mut entries = entries.borrow_mut();
            entries.push(entry_from_delta(&delta));
            current.set(entries.len().saturating_sub(1));
            true
        };
        let mut line_cb = |_delta: git2::DiffDelta<'_>,
                           _hunk: Option<git2::DiffHunk<'_>>,
                           line: git2::DiffLine<'_>|
         -> bool {
            let index = current.get();
            let mut entries = entries.borrow_mut();
            if let Some(entry) = entries.get_mut(index) {
                match line.origin_value() {
                    DiffLineType::Addition | DiffLineType::AddEOFNL => entry.insertions += 1,
                    DiffLineType::Deletion | DiffLineType::DeleteEOFNL => entry.deletions += 1,
                    _ => {}
                }
            }
            true
        };
        diff.foreach(&mut file_cb, None, None, Some(&mut line_cb))?;
    }

    Ok(entries.into_inner())
}

fn entry_from_delta(delta: &git2::DiffDelta<'_>) -> EntryAcc {
    let old = delta.old_file();
    let new = delta.new_file();
    let old_path = old.path().and_then(path_to_string);
    let new_path = new.path().and_then(path_to_string);
    // A deletion has no new side; the old path is the file's name.
    let path = new_path
        .clone()
        .or_else(|| old_path.clone())
        .unwrap_or_default();
    let status = file_status_from(delta.status());
    let old_path = match status {
        FileStatus::Renamed | FileStatus::Copied => old_path.filter(|old| *old != path),
        _ => None,
    };
    EntryAcc {
        path,
        old_path,
        status,
        binary: old.is_binary() || new.is_binary(),
        insertions: 0,
        deletions: 0,
    }
}

fn collect_entries(acc: Vec<EntryAcc>) -> (Vec<ChangesetEntry>, ChangesetStats) {
    let mut stats = ChangesetStats::default();
    let entries = acc
        .into_iter()
        .map(|entry| {
            stats.files_changed += 1;
            stats.insertions += entry.insertions;
            stats.deletions += entry.deletions;
            ChangesetEntry {
                path: entry.path,
                old_path: entry.old_path,
                status: entry.status,
                binary: entry.binary,
                insertions: entry.insertions,
                deletions: entry.deletions,
            }
        })
        .collect();
    (entries, stats)
}

fn file_status_from(delta: Delta) -> FileStatus {
    match delta {
        Delta::Added => FileStatus::Added,
        Delta::Deleted => FileStatus::Deleted,
        Delta::Renamed => FileStatus::Renamed,
        Delta::Copied => FileStatus::Copied,
        Delta::Conflicted => FileStatus::Unmerged,
        Delta::Ignored => FileStatus::Ignored,
        Delta::Untracked => FileStatus::Untracked,
        _ => FileStatus::Modified,
    }
}

fn path_to_string(path: &std::path::Path) -> Option<String> {
    path.to_str().map(str::to_string)
}

// ---------------------------------------------------------------------------
// Path filtering (walk-time) and the file-history watcher
// ---------------------------------------------------------------------------

/// One commit's change to the watched path.
struct PathChange {
    path: String,
    old_path: Option<String>,
    status: FileStatus,
    binary: bool,
    insertions: u32,
    deletions: u32,
}

/// Watches one path during a walk, following renames when asked.
struct PathWatcher {
    path: String,
    follow_renames: bool,
}

impl PathWatcher {
    fn new(path: String, follow_renames: bool) -> Self {
        Self {
            path,
            follow_renames,
        }
    }

    /// Does this commit touch the watched path? When it renamed the path and
    /// rename following is on, the watch continues under the old name.
    fn check(
        &mut self,
        repo: &git2::Repository,
        oid: Oid,
    ) -> Result<Option<PathChange>, ErrorPayload> {
        let commit = repo.find_commit(oid)?;
        let change = self.check_commit(repo, &commit)?;
        if self.follow_renames {
            if let Some(old_path) = change.as_ref().and_then(|change| change.old_path.as_ref()) {
                self.path = old_path.clone();
            }
        }
        Ok(change)
    }

    fn check_commit(
        &self,
        repo: &git2::Repository,
        commit: &git2::Commit<'_>,
    ) -> Result<Option<PathChange>, ErrorPayload> {
        let new_tree = commit.tree()?;
        let old_tree = match commit.parent(0) {
            Ok(parent) => {
                if parent.tree_id() == commit.tree_id() {
                    return Ok(None);
                }
                Some(parent.tree()?)
            }
            // A root commit: everything it contains was added here.
            Err(_) => None,
        };

        let mut opts = git2::DiffOptions::new();
        opts.pathspec(self.path.as_str());
        opts.context_lines(0);
        let mut diff =
            repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
        if self.follow_renames {
            let mut find = git2::DiffFindOptions::new();
            find.renames(true);
            diff.find_similar(Some(&mut find))?;
        }

        let path = self.path.as_str();
        let Some(entry) = diff_entries(&diff)?
            .into_iter()
            .find(|entry| entry.path == path || entry.old_path.as_deref() == Some(path))
        else {
            return Ok(None);
        };

        Ok(Some(PathChange {
            path: entry.path,
            old_path: entry.old_path,
            status: entry.status,
            binary: entry.binary,
            insertions: entry.insertions,
            deletions: entry.deletions,
        }))
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// The compiled `search` argument. Matching is ASCII case-insensitive.
struct SearchNeedle {
    text: Option<String>,
    mode: SearchMode,
}

impl SearchNeedle {
    fn new(query: &HistoryQuery) -> Self {
        let text = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| text.to_string());
        Self {
            text,
            mode: query.search_mode,
        }
    }

    /// A full 40-character hash can skip the walk entirely.
    fn full_hash(&self) -> Option<&str> {
        if !matches!(self.mode, SearchMode::All | SearchMode::Hash) {
            return None;
        }
        let text = self.text.as_deref()?;
        (text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(text)
    }

    fn matches(&self, commit: &git2::Commit<'_>) -> bool {
        let Some(text) = self.text.as_deref() else {
            return true;
        };
        let needle = text.as_bytes();

        if matches!(self.mode, SearchMode::All | SearchMode::Hash)
            && contains_ascii_ci(commit.id().to_string().as_bytes(), needle)
        {
            return true;
        }
        if matches!(self.mode, SearchMode::All | SearchMode::Author) {
            let author = commit.author();
            if contains_ascii_ci(author.name_bytes(), needle)
                || contains_ascii_ci(author.email_bytes(), needle)
            {
                return true;
            }
        }
        if matches!(self.mode, SearchMode::All | SearchMode::Message)
            && contains_ascii_ci(commit.message_bytes(), needle)
        {
            return true;
        }
        false
    }
}

/// ASCII case-insensitive substring search; non-ASCII bytes compare exactly.
fn contains_ascii_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(h, n)| h.eq_ignore_ascii_case(n))
    })
}

/// Direct lookup for a full-hash search: no walk, no page arithmetic. Returns
/// `None` when the search is not a full hash, so the caller walks normally.
fn direct_lookup(
    repo: &git2::Repository,
    needle: &SearchNeedle,
    path: &Option<String>,
    skip: usize,
    limit: usize,
) -> Result<Option<HistoryPage>, ErrorPayload> {
    let Some(hex) = needle.full_hash() else {
        return Ok(None);
    };
    let Ok(oid) = Oid::from_str(hex) else {
        return Ok(Some(empty_page(0, limit)));
    };
    let commit = match repo.find_commit(oid) {
        Ok(commit) => commit,
        Err(err) if err.code() == git2::ErrorCode::NotFound => {
            return Ok(Some(empty_page(0, limit)))
        }
        Err(err) => return Err(err.into()),
    };

    // The path filter is part of the query, so it still applies.
    if let Some(path) = path {
        let watcher = PathWatcher::new(path.clone(), false);
        if watcher.check_commit(repo, &commit)?.is_none() {
            return Ok(Some(empty_page(0, limit)));
        }
    }
    if skip > 0 {
        return Ok(Some(empty_page(skip, limit)));
    }

    let row = row_from(&commit, &decoration_map(repo));
    let nodes = vec![GraphNode::new(row.oid.clone(), row.parents.clone())];
    Ok(Some(HistoryPage {
        graph: graph::draw_graph(&nodes),
        commits: vec![row],
        skip: 0,
        limit,
        next_skip: None,
        truncated: false,
        scanned: 1,
    }))
}

// ---------------------------------------------------------------------------
// Rows, signatures, decorations
// ---------------------------------------------------------------------------

fn row_from(
    commit: &git2::Commit<'_>,
    decorations: &HashMap<Oid, Vec<RefDecoration>>,
) -> CommitRow {
    let hex = commit.id().to_string();
    let refs = decorations.get(&commit.id()).cloned().unwrap_or_default();
    CommitRow {
        short_oid: short_oid(&hex),
        oid: hex,
        parents: commit
            .parent_ids()
            .map(|parent| parent.to_string())
            .collect(),
        parent_count: commit.parent_count() as u32,
        summary: summary_of(commit),
        message: message_of(commit),
        author: signature(commit.author()),
        committer: signature(commit.committer()),
        decorations: refs.iter().map(decoration_label).collect(),
        refs,
        signature: signature_state_of(commit),
    }
}

fn short_oid(hex: &str) -> String {
    hex.chars().take(SHORT_OID_LEN).collect()
}

fn signature(sig: git2::Signature<'_>) -> Signature {
    Signature {
        name: sig.name().unwrap_or_default().to_string(),
        email: sig.email().unwrap_or_default().to_string(),
        time: sig.when().seconds(),
    }
}

fn message_of(commit: &git2::Commit<'_>) -> String {
    String::from_utf8_lossy(commit.message_bytes()).into_owned()
}

fn summary_of(commit: &git2::Commit<'_>) -> String {
    match commit.summary() {
        Ok(Some(summary)) => summary.to_string(),
        _ => split_message(&message_of(commit)).0,
    }
}

/// Split a message into its subject line and the remaining body.
fn split_message(message: &str) -> (String, String) {
    let trimmed = message.trim_start_matches(['\n', '\r']);
    match trimmed.split_once('\n') {
        Some((subject, rest)) => (
            subject.trim_end_matches('\r').trim_end().to_string(),
            rest.trim_start_matches(['\n', '\r'])
                .trim_end_matches(['\n', '\r'])
                .to_string(),
        ),
        None => (trimmed.trim_end().to_string(), String::new()),
    }
}

/// Signature presence, read from the commit header already in memory — no
/// extra object read, and no verification (that is `signature_verify`).
fn signature_state_of(commit: &git2::Commit<'_>) -> SignatureState {
    let Ok(header) = commit.raw_header() else {
        return SignatureState::Unsigned;
    };
    if !header.lines().any(|line| line.starts_with("gpgsig")) {
        return SignatureState::Unsigned;
    }
    if header.contains("BEGIN SSH SIGNATURE") {
        SignatureState::Ssh
    } else if header.contains("BEGIN PGP SIGNATURE") {
        SignatureState::Pgp
    } else {
        SignatureState::Other
    }
}

/// Every commit's decorations, built with **one** sweep over the refs.
///
/// A local branch HEAD points at is `is_head` (git's `HEAD -> main`); a
/// detached HEAD is a `Head` decoration of its own.
pub fn decoration_map(repo: &git2::Repository) -> HashMap<Oid, Vec<RefDecoration>> {
    let mut map: HashMap<Oid, Vec<RefDecoration>> = HashMap::new();
    let head_branch = repo
        .find_reference("HEAD")
        .ok()
        .and_then(|head| head.symbolic_target().ok().flatten().map(str::to_string));
    let detached = repo.head_detached().unwrap_or(false);

    if let Ok(references) = repo.references() {
        for reference in references.flatten() {
            let Ok(full_name) = reference.name().map(str::to_string) else {
                continue;
            };
            let kind = if reference.is_branch() {
                RefDecorationKind::Branch
            } else if reference.is_tag() {
                RefDecorationKind::Tag
            } else if reference.is_remote() {
                RefDecorationKind::Remote
            } else if full_name == "refs/stash" {
                RefDecorationKind::Other
            } else {
                // Notes, replace refs, … are not decorations the history list
                // should show.
                continue;
            };
            let Some(name) = reference.shorthand().ok().map(str::to_string) else {
                continue;
            };
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            let is_head = kind == RefDecorationKind::Branch
                && head_branch.as_deref() == Some(full_name.as_str());
            map.entry(commit.id()).or_default().push(RefDecoration {
                name,
                full_name,
                kind,
                is_head,
            });
        }
    }

    if detached {
        if let Ok(head) = repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                map.entry(commit.id()).or_default().push(RefDecoration {
                    name: "HEAD".to_string(),
                    full_name: "HEAD".to_string(),
                    kind: RefDecorationKind::Head,
                    is_head: true,
                });
            }
        }
    }

    for refs in map.values_mut() {
        refs.sort_by(|a, b| decoration_sort_key(a).cmp(&decoration_sort_key(b)));
    }
    map
}

fn decoration_sort_key(decoration: &RefDecoration) -> (u8, &str) {
    let rank = match decoration.kind {
        RefDecorationKind::Head => 0,
        RefDecorationKind::Branch => 1,
        RefDecorationKind::Remote => 2,
        RefDecorationKind::Tag => 3,
        RefDecorationKind::Other => 4,
    };
    (rank, decoration.name.as_str())
}

/// The decoration string form, in git's own spelling — what
/// `src/components/graph/refs.ts` parses.
fn decoration_label(decoration: &RefDecoration) -> String {
    match decoration.kind {
        RefDecorationKind::Head => "HEAD".to_string(),
        RefDecorationKind::Branch if decoration.is_head => {
            format!("HEAD -> {}", decoration.name)
        }
        RefDecorationKind::Branch | RefDecorationKind::Remote | RefDecorationKind::Other => {
            decoration.full_name.clone()
        }
        RefDecorationKind::Tag => format!("tag: {}", decoration.name),
    }
}

// ---------------------------------------------------------------------------
// Message parsing: trailers and coauthors
// ---------------------------------------------------------------------------

/// The trailer block at the end of a message: the last paragraph, provided
/// every line in it is a `Token: value` line (or a continuation of one).
///
/// This follows git's `interpret-trailers` shape closely enough for display:
/// blank line ends the block, a non-trailer line ends it, continuation lines
/// (leading whitespace) fold into the value, separators are `:` and the value
/// may be empty.
pub fn parse_trailers(message: &str) -> Vec<Trailer> {
    let mut trailers: Vec<Trailer> = Vec::new();
    let mut pending: Vec<String> = Vec::new();

    for line in message.trim_end_matches(['\n', '\r']).lines().rev() {
        if line.trim().is_empty() {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            pending.push(line.trim_start().to_string());
            continue;
        }
        let Some((token, value)) = split_trailer(line) else {
            break;
        };
        let mut value = value;
        while let Some(continuation) = pending.pop() {
            value.push('\n');
            value.push_str(&continuation);
        }
        trailers.push(Trailer { token, value });
    }

    // A block that ends in orphaned continuation lines was not a trailer
    // block at all.
    if !pending.is_empty() {
        return Vec::new();
    }
    trailers.reverse();
    trailers
}

/// `Token: value` — the token is alphanumeric-with-dashes and must be followed
/// by a space (or end the line), which is what keeps `https://…` in prose out.
fn split_trailer(line: &str) -> Option<(String, String)> {
    let colon = line.find(':')?;
    let token = &line[..colon];
    if token.is_empty()
        || token.len() > 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return None;
    }
    let rest = &line[colon + 1..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((
        token.to_string(),
        rest.trim_start_matches(' ').trim_end().to_string(),
    ))
}

/// `Co-authored-by:` trailers, parsed into name/email pairs (git's own
/// convention for shared commits).
pub fn coauthors_from(trailers: &[Trailer]) -> Vec<Coauthor> {
    trailers
        .iter()
        .filter(|trailer| trailer.token.eq_ignore_ascii_case("co-authored-by"))
        .filter_map(|trailer| parse_identity(&trailer.value))
        .collect()
}

fn parse_identity(value: &str) -> Option<Coauthor> {
    let open = value.find('<')?;
    let close = value[open..].find('>')? + open;
    let name = value[..open].trim().to_string();
    let email = value[open + 1..close].trim().to_string();
    (!email.is_empty()).then_some(Coauthor { name, email })
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn normalise_path(path: Option<&str>) -> Option<String> {
    path.map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| path.trim_start_matches("./").to_string())
}

/// The commit-count cache, keyed by `(gitdir, HEAD)`. Bounded so a long session
/// across many repositories cannot grow it without limit.
fn count_cache() -> &'static Mutex<HashMap<String, u64>> {
    static CACHE: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn count_cache_get(key: &str) -> Option<u64> {
    count_cache().lock().ok()?.get(key).copied()
}

fn count_cache_put(key: String, value: u64) {
    if let Ok(mut cache) = count_cache().lock() {
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailers_parse_the_last_paragraph() {
        let message = "Fix the thing\n\nBody line.\n\nSigned-off-by: Ada <ada@example.com>\nReviewed-by: Bob <bob@example.com>\n";
        let trailers = parse_trailers(message);
        assert_eq!(trailers.len(), 2);
        assert_eq!(trailers[0].token, "Signed-off-by");
        assert_eq!(trailers[0].value, "Ada <ada@example.com>");
        assert_eq!(trailers[1].token, "Reviewed-by");
    }

    #[test]
    fn trailers_fold_continuation_lines() {
        let message = "Subject\n\nCo-authored-by: Ada <ada@example.com>\n  with a note\n";
        let trailers = parse_trailers(message);
        assert_eq!(trailers.len(), 1);
        assert_eq!(trailers[0].value, "Ada <ada@example.com>\nwith a note");
    }

    #[test]
    fn prose_is_not_a_trailer_block() {
        let message = "Subject\n\nSee https://example.com/x for details.\n";
        assert!(parse_trailers(message).is_empty());
    }

    #[test]
    fn a_single_line_message_has_no_trailers() {
        assert!(parse_trailers("Just a subject").is_empty());
    }

    #[test]
    fn coauthors_come_from_their_trailer() {
        let trailers = parse_trailers(
            "Shipped\n\nCo-Authored-By: Ada Lovelace <ada@example.com>\nCo-Authored-By: Bob <bob@example.com>\n",
        );
        let coauthors = coauthors_from(&trailers);
        assert_eq!(coauthors.len(), 2);
        assert_eq!(coauthors[0].name, "Ada Lovelace");
        assert_eq!(coauthors[0].email, "ada@example.com");
    }

    #[test]
    fn message_split_keeps_the_body() {
        let (subject, body) = split_message("Subject line\n\nBody\nsecond line\n");
        assert_eq!(subject, "Subject line");
        assert_eq!(body, "Body\nsecond line");
    }

    #[test]
    fn ascii_search_matches_case_insensitively() {
        assert!(contains_ascii_ci(b"Fix the Parser", b"parser"));
        assert!(contains_ascii_ci(b"Fix the Parser", b"FIX"));
        assert!(!contains_ascii_ci(b"Fix the Parser", b"lexer"));
    }

    #[test]
    fn path_normalisation_trims_and_strips_the_dot_prefix() {
        assert_eq!(
            normalise_path(Some("  ./src/lib.rs ")).as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(normalise_path(Some("   ")), None);
        assert_eq!(normalise_path(None), None);
    }
}
