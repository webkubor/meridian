//! Commit-history value vocabulary — the counter for the `docs/03 §5`
//! `history_*` surface.
//!
//! Everything the history pane renders crosses the bridge as one of these
//! types: a list row, a commit detail, a changeset, the lane layout, or one of
//! the derived views (contributors, file history). Like the rest of `domain`
//! this module is plain data: serde in, serde out, no `git2` and no `tauri`.
//!
//! The graph types follow the contract in docs/04 §2.5 — the engine computes
//! segments `{lane, from_row, to_row, kind}` and the web layer only draws.

use serde::{Deserialize, Serialize};

use super::history::Signature;
use super::revision::Revision;
use super::status::FileStatus;

/// How many hex characters a `short_oid` carries. Seven is git's own default
/// abbreviation; the UI never re-derives it.
pub const SHORT_OID_LEN: usize = 7;

/// Page size used when a caller does not ask for one.
pub const DEFAULT_HISTORY_LIMIT: usize = 250;

/// Hard cap on one page. Larger requests are clamped, not refused: the history
/// list is virtualized and a page above this size stops being a page.
pub const MAX_HISTORY_LIMIT: usize = 5_000;

/// Hard cap on how many commits one filtered load will scan before giving up
/// ([`HistoryPage::truncated`] is then set and `scanned` reports the effort).
pub const MAX_HISTORY_SCAN: u64 = 200_000;

// ---------------------------------------------------------------------------
// List rows
// ---------------------------------------------------------------------------

/// One commit as the history list shows it: identity, parents, message,
/// both signatures and the ref chips pointing at it.
///
/// The first eight fields keep the shape the history store and the graph
/// renderer already consume (`src/bridges/bindings.ts` `CommitInfo`); the
/// fields after them are the richer view this engine added — structured
/// decorations, signature state, parent count.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub oid: String,
    pub short_oid: String,
    /// Parent ids in commit order; `len() > 1` is a merge (an octopus merge has
    /// three or more). Parents outside the loaded page are kept — the graph
    /// draws them as lines that continue below.
    pub parents: Vec<String>,
    pub parent_count: u32,
    pub summary: String,
    /// The full raw message (lossy-decoded when it is not UTF-8).
    pub message: String,
    /// Author — the person who wrote the change (`author` line).
    pub author: Signature,
    /// Committer — the person who created the commit object (`committer`
    /// line; different from the author after a rebase or cherry-pick).
    pub committer: Signature,
    /// Decoration strings in git's own spelling — `HEAD -> main`,
    /// `refs/heads/topic`, `refs/remotes/origin/main`, `tag: v1.0` — which is
    /// what `src/components/graph/refs.ts` parses.
    pub decorations: Vec<String>,
    /// The same decorations, typed.
    pub refs: Vec<RefDecoration>,
    pub signature: SignatureState,
}

/// A ref chip on a row ("main", "origin/main", "v1.2").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefDecorationKind {
    /// `HEAD` itself, or the local branch HEAD points at.
    Head,
    Branch,
    Remote,
    Tag,
    /// Any other ref namespace (`refs/stash`, `refs/notes/…`).
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefDecoration {
    /// Short display name: `main`, `origin/main`, `v1.2`.
    pub name: String,
    /// Full refname (`refs/heads/main`), for commands that take a refname.
    pub full_name: String,
    pub kind: RefDecorationKind,
    /// True for the branch HEAD currently points at (rendered `HEAD -> main`),
    /// and for the `Head` decoration of a detached HEAD.
    pub is_head: bool,
}

/// Whether the commit object carries a signature, and in which format.
/// Trust is *not* decided here — that is `security_signature_verify`'s job
/// (docs/03 §14); the list only renders the presence badge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignatureState {
    #[default]
    Unsigned,
    /// A `-----BEGIN PGP SIGNATURE-----` block in the `gpgsig` header.
    Pgp,
    /// An `-----BEGIN SSH SIGNATURE-----` block (`gpgsig` / `gpgsig-sha256`).
    Ssh,
    /// A signature block in a format we do not classify (it is present — the
    /// badge should still show).
    Other,
}

// ---------------------------------------------------------------------------
// Commit detail
// ---------------------------------------------------------------------------

/// Everything `history_commit_load` returns: the row plus the body, the
/// coauthor and trailer breakdown, and the tree stats.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub oid: String,
    pub short_oid: String,
    pub parents: Vec<String>,
    pub parent_count: u32,
    pub tree: String,
    /// First line of the message.
    pub summary: String,
    /// The message with `summary` and the trailing blank line removed.
    pub body: String,
    /// The full raw message (lossy-decoded when it is not UTF-8).
    pub message: String,
    pub author: Signature,
    pub committer: Signature,
    /// Decoration strings in git's own spelling, like [`CommitRow::decorations`]
    /// — this is what `src/components/graph/refs.ts` parses.
    pub decorations: Vec<String>,
    /// The same decorations, typed.
    pub refs: Vec<RefDecoration>,
    pub signature: SignatureState,
    /// `Co-authored-by:` trailers, parsed (git's own convention).
    pub coauthors: Vec<Coauthor>,
    /// The trailer block at the end of the message, in message order.
    pub trailers: Vec<Trailer>,
    /// Diff against the first parent (the root commit diffs against the empty
    /// tree); merge commits report against their first parent, like git.
    pub stats: ChangesetStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coauthor {
    pub name: String,
    pub email: String,
}

/// One `Key: value` trailer. Continuation lines (leading whitespace) are
/// folded into `value` with their newlines preserved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    pub token: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Changesets
// ---------------------------------------------------------------------------

/// One commit's (or one range's) file list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changeset {
    pub oid: String,
    /// The side diffed against: the first parent, or `None` when the commit is
    /// a root (or when the changeset comes from a range, where `from` carries
    /// the base).
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub entries: Vec<ChangesetEntry>,
    pub stats: ChangesetStats,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetEntry {
    /// Path after the change (for a deletion, the path that vanished).
    pub path: String,
    /// Set for renames and copies: the path before the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

// ---------------------------------------------------------------------------
// The query and the page
// ---------------------------------------------------------------------------

/// Which part of a row a search string is matched against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    /// Message, author name/email and hash prefix.
    #[default]
    All,
    Message,
    Author,
    Hash,
}

/// The `history_commits_load` argument core, shared with
/// `history_file_history_load`.
///
/// `to` is the tip of the walk (inclusive, default: HEAD); `from`, when given,
/// is hidden together with its ancestors — the walk covers `from..to`, the
/// same orientation `history_revision_range_changeset_load` uses.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Revision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Revision>,
    /// Rows to drop from the front of the *result* (after filtering). Offset
    /// pagination, so it is stable against appends at the top only by
    /// re-running the load — which is what the UI does on `RefsChanged`.
    #[serde(default)]
    pub skip: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort: super::history::HistorySort,
    #[serde(default)]
    pub first_parent: bool,
    /// Restrict to commits touching this path (git-log semantics: the path is
    /// matched as a pathspec, so a directory matches everything below it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Follow renames while path-filtering (`git log --follow` semantics).
    #[serde(default)]
    pub follow_renames: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default)]
    pub search_mode: SearchMode,
}

fn default_limit() -> usize {
    DEFAULT_HISTORY_LIMIT
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            skip: 0,
            limit: DEFAULT_HISTORY_LIMIT,
            sort: Default::default(),
            first_parent: false,
            path: None,
            follow_renames: false,
            search: None,
            search_mode: Default::default(),
        }
    }
}

/// One page of history: the rows, the lane layout computed for exactly those
/// rows, and the pagination envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub commits: Vec<CommitRow>,
    /// Lane layout aligned index-for-index with `commits`.
    pub graph: GraphLayout,
    /// The `skip` this page was produced with.
    pub skip: usize,
    /// The `limit` this page was produced with (after clamping).
    pub limit: usize,
    /// Offset for the next page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_skip: Option<usize>,
    /// True when rows remain that this page did not reach. This is *not* an
    /// error — it is how the list knows to keep a "load more" affordance.
    pub truncated: bool,
    /// How many commits the walk actually visited (loaded or not). Lets the
    /// UI explain a slow, filtered page instead of looking hung.
    pub scanned: u64,
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

/// Lane layout for one list of rows (docs/04 §2.5).
///
/// The shape is the contract declared by the renderer in
/// `src/components/graph/types.ts`: `lanes` is parallel to the commit array the
/// engine was asked about, `segments` are the strokes to draw, and row indices
/// in both are positions in that same array.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLayout {
    /// Bubble lane per layout row. `-1` marks a row with no commit (defensive;
    /// the engine only ever labels real commits).
    pub lanes: Vec<i32>,
    /// Every stroke needed to draw the page.
    pub segments: Vec<GraphLaneSegment>,
    /// Highest lane index used + 1.
    pub lane_count: u16,
}

/// Edge vocabulary from docs/04 §2.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphEdgeKind {
    /// First-parent edge that stays in the row's own lane (vertical run).
    Straight,
    /// First-parent edge that lands in a different lane because that lane
    /// already tracks the parent: the line bends into an existing line.
    Bend,
    /// Edge to a non-first parent that opens a new lane — one arm of a merge.
    Merge,
    /// Edge to a non-first parent whose line already exists (a merge whose
    /// other side was already drawn).
    Fork,
}

/// One drawn stroke, connecting `(from_row, from_lane)` to
/// `(to_row, to_lane)` in row/lane space.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLaneSegment {
    /// Layout row where the stroke begins (always a row inside the page).
    pub from_row: i64,
    /// Layout row where the stroke ends *for drawing*. A stroke whose parent
    /// is older than the loaded page is clamped to the page's last row, which
    /// is how the renderer knows to show the "continues below" marker there;
    /// `ending` distinguishes that clamp from a real termination.
    pub to_row: i64,
    /// The lane that owns the stroke: the child line that spawned it. Equal to
    /// `from_lane` for every stroke this engine produces.
    pub lane: u16,
    pub from_lane: u16,
    pub to_lane: u16,
    pub kind: GraphEdgeKind,
    /// True when the stroke opens a lane at `from_row` (merge arms).
    pub starting: bool,
    /// True when the stroke terminates at a commit row inside the page;
    /// false when it was clamped to the last row because its target is not
    /// loaded (see `to_row`).
    pub ending: bool,
}

// ---------------------------------------------------------------------------
// Derived views
// ---------------------------------------------------------------------------

/// `history_contributors_load` row: one author identity, aggregated.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    /// The most frequent name used with this email.
    pub name: String,
    pub email: String,
    pub commits: u64,
    /// Unix seconds of the oldest / newest commit attributed to this identity.
    pub first_time: i64,
    pub last_time: i64,
}

/// `history_file_history_load` row: one commit that changed the tracked path.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRevision {
    pub oid: String,
    pub short_oid: String,
    /// The path as of this commit (the rename target seen from `old_path`).
    pub path: String,
    /// Set when this revision renamed the path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub insertions: u32,
    pub deletions: u32,
    pub binary: bool,
    pub summary: String,
    pub author: Signature,
    pub committer: Signature,
}

// ---------------------------------------------------------------------------
// Commit — the mutation surface (docs/03 §4)
// ---------------------------------------------------------------------------

/// The signature mode a caller may ask for on a commit
/// (`src/bridges/bindings.ts` `CommitSign` — `{kind:"gpg"|"ssh"|"none"}`).
///
/// Only [`CommitSign::None`] (an unsigned commit) is honoured in this pass.
/// `Gpg` / `Ssh` are declared so that a request which wants them is **refused
/// loudly** by the engine and the validator, never silently ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommitSign {
    Gpg {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    Ssh {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    None,
}

/// Options the two commit verbs share (`commit_create` / `commit_amend`,
/// docs/03 §4). The shape mirrors the frontend's `CommitOptions`
/// (`src/bridges/bindings.ts`) so a wire round-trip carries every field.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitOptions {
    /// Append a `Signed-off-by:` trailer with the committer identity (git's
    /// `--signoff`). A trailer that is already present is left untouched.
    #[serde(default)]
    pub sign_off: bool,
    /// Requested signature mode. `Gpg` / `Ssh` are refused (see [`CommitSign`]);
    /// `None` or absent produces an unsigned commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign: Option<CommitSign>,
    /// Accepted for the documented argument shape. The hook runner
    /// (`pre-commit` / `commit-msg`) is **not** part of this pass — see the
    /// module note in `engine::index`; until it lands this flag has no effect.
    #[serde(default)]
    pub skip_hooks: bool,
    /// Provenance only: a template supplied the message. Nothing in this pass
    /// consumes it (the trailer policy it would drive does not exist yet); it
    /// is declared so the frontend's `CommitOptions` round-trips faithfully.
    #[serde(default)]
    pub template_used: bool,
    /// Override the author identity (passed through verbatim). Unset means the
    /// configured identity — or, for an amend, the author of the commit being
    /// replaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Signature>,
    /// Override the committer identity. Unset means the repository's configured
    /// `user.name` / `user.email`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committer: Option<Signature>,
}

/// The engine's commit request: what `commit_create` / `commit_amend` reduce to
/// once the descriptor has been read.
///
/// `message = None` with `amend = true` reuses the message of the commit being
/// replaced; with `amend = false` it is refused (a fresh commit needs a
/// message). The engine resolves every parent itself — the request carries no
/// parent, so no caller can hand it a graph it disagrees with.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Replace HEAD instead of appending to it (`commit_amend`).
    #[serde(default)]
    pub amend: bool,
    #[serde(default)]
    pub opts: CommitOptions,
}

/// The result of a `commit_create` / `commit_amend` (docs/03 §4): the new
/// commit plus where it landed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// The oid of the commit object that was written.
    pub oid: String,
    pub short_oid: String,
    /// Parent ids in commit order — two for a merge commit, empty for the
    /// initial commit.
    pub parents: Vec<String>,
    pub parent_count: u32,
    /// First non-empty line of the final message (sign-off included).
    pub summary: String,
    /// The message that was stored, verbatim.
    pub message: String,
    pub author: Signature,
    pub committer: Signature,
    /// The branch the commit landed on; `None` for a detached HEAD, where the
    /// commit moved HEAD itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// True while HEAD is detached (no branch to name). Mirrors
    /// `branch.is_none()` and is spelled out so the UI need not infer it.
    pub detached: bool,
    /// True when this replaced HEAD rather than appending to it.
    pub amend: bool,
}
