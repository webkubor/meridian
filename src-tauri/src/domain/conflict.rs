//! Integration vocabulary — the counter for the `docs/03 §13 integrate`
//! surface: merge/rebase/cherry-pick/revert/reset and the conflict model.
//!
//! The conflict axis follows the evidence: seven kind classes, one resolver
//! class each (`GTGit{AddedByUs,AddedByThem,BothAdded,BothDeleted,
//! BothModified,DeletedByUs,DeletedByThem}MergeConflictResolver`), plus the
//! observed save-versions operation (`GTGitSaveMergeConflictVersionsOperation`)
//! and the one unmerged-entry error (`GTGitMergeConflictNotAnUnmergedEntry`).
//!
//! Like the rest of `domain` this module is plain data: serde in, serde out,
//! no `git2` and no `tauri`. The mode enums the integrate commands serialize
//! (`MergeStrategy`, `ResetMode`, `SequenceAction`) live here for the same
//! reason `Revision` lives in `domain/revision.rs`: the web layer must be able
//! to name every axis of every command. When `domain/merge.rs` lands, these
//! move there wholesale — see the note in docs/02 §4.

use serde::{Deserialize, Serialize};

use super::history::Signature;
use super::status::StatusDelta;

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

/// The seven conflict kinds, one per two-letter `git status` code
/// (`EV:analysis/git-semantics#F3`): `AU`, `UA`, `AA`, `DD`, `UD`, `DU`, `UU`.
///
/// Classification is a pure function of which index stages exist:
///
/// | stages (base/ours/theirs) | code | kind |
/// |---|---|---|
/// | `- A -` | AU | [`ConflictKind::AddedByUs`] |
/// | `- - A` | UA | [`ConflictKind::AddedByThem`] |
/// | `- A A` | AA | [`ConflictKind::BothAdded`] |
/// | `D - -` | DD | [`ConflictKind::BothDeleted`] |
/// | `D A -` | UD | [`ConflictKind::DeletedByThem`] |
/// | `D - A` | DU | [`ConflictKind::DeletedByUs`] |
/// | `D A A` | UU | [`ConflictKind::BothModified`] |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    AddedByUs,
    AddedByThem,
    BothAdded,
    BothDeleted,
    BothModified,
    DeletedByUs,
    DeletedByThem,
}

impl ConflictKind {
    /// The `git status --short` code the kind corresponds to.
    pub fn code(self) -> &'static str {
        match self {
            ConflictKind::AddedByUs => "AU",
            ConflictKind::AddedByThem => "UA",
            ConflictKind::BothAdded => "AA",
            ConflictKind::BothDeleted => "DD",
            ConflictKind::DeletedByThem => "UD",
            ConflictKind::DeletedByUs => "DU",
            ConflictKind::BothModified => "UU",
        }
    }

    /// Which sides of this conflict carry file content at all. `BothDeleted`
    /// has none; the deleted-by-one-side kinds have exactly one.
    pub fn is_content_conflict(self) -> bool {
        matches!(self, ConflictKind::BothModified | ConflictKind::BothAdded)
    }
}

/// One side (base / ours / theirs) of an unmerged index entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictVersion {
    /// Blob id of the stage as `hex`.
    pub oid: String,
    /// Raw git file mode (`0o100644`, `0o100755`, `0o120000`, `0o160000`, …).
    pub mode: u32,
    pub size: u64,
}

/// The three index stages of one conflicted path, in the naming the evidence
/// uses for the conflict menu (`GTGitMergeConflictVersion`): `base` is the
/// merge base (stage 1); `ours` the checked-out side (stage 2); `theirs` the
/// incoming side (stage 3). `None` means that side has no entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSides {
    pub base: Option<ConflictVersion>,
    pub ours: Option<ConflictVersion>,
    pub theirs: Option<ConflictVersion>,
}

/// One conflict region inside a text conflict file, as produced by a
/// diff3-style three-way merge. Line contents are kept as strings (with their
/// trailing newline) so per-hunk resolution can rebuild the file byte-exactly
/// for the chosen side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictHunk {
    /// Zero-based position of the region in the file, in merge-output order.
    /// Stable for as long as the working-tree content is untouched: per-hunk
    /// resolutions address hunks by this index.
    pub index: usize,
    /// The "ours" lines, conflict region only.
    pub ours: String,
    /// The merge-base lines (empty when the region is an add/add).
    pub base: String,
    /// The "theirs" lines, conflict region only.
    pub theirs: String,
}

/// One unmerged path, as `wc_conflicts_load` / `conflict_load` report it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    pub path: String,
    pub kind: ConflictKind,
    pub sides: ConflictSides,
    /// True when no side contains NUL-free text (git's binary heuristic).
    pub binary: bool,
    /// False when a three-way text merge cannot produce a combined result
    /// (binary content, or a side that is a symlink/commit).
    pub mergeable: bool,
    /// The conflict regions of the text merge. Empty for non-mergeable
    /// conflicts and for conflicts that merge cleanly (mode-only cases).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hunks: Vec<ConflictHunk>,
}

/// The compact form the integration outcomes carry — enough for a banner or a
/// status line, cheap to produce mid-sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSummary {
    pub path: String,
    pub kind: ConflictKind,
}

impl From<&ConflictEntry> for ConflictSummary {
    fn from(entry: &ConflictEntry) -> Self {
        Self {
            path: entry.path.clone(),
            kind: entry.kind,
        }
    }
}

/// Which side of a conflict to take, for `wc_conflict_resolve` /
/// `conflict_resolve`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "side", content = "content")]
pub enum ResolutionStrategy {
    /// Take our side: the stage-2 blob. A file deleted on our side stays
    /// deleted when the entry has no stage 2.
    #[default]
    Ours,
    /// Take their side: the stage-3 blob; deleted when there is no stage 3.
    Theirs,
    /// Take the merge base: the stage-1 blob; requires a base entry.
    Base,
    /// Take the union of both sides (`git merge-file --union`).
    Both,
    /// Take content the caller supplies — the "manual" resolution: the user
    /// edited the file (or a hunk editor produced the text) and the exact
    /// bytes travel with the command.
    Manual { content: String },
    /// Write each existing side as a backup file next to the path
    /// (`<path>.ours`, `<path>.theirs`, `<path>.base`); the entry stays
    /// unmerged. Mirrors `GTGitSaveMergeConflictVersionsOperation` +
    /// `GTUserDefaultsKeepBackupFilesForMergeConflicts`.
    SaveVersions,
    /// Keep whatever is in the working tree right now and stage it — the
    /// resolution that ends with `git add` (`Index::add_path` collapses the
    /// conflict stages). The file must exist unless it was deleted on purpose.
    MarkResolved,
}

/// Which side one hunk of a text conflict takes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "choice", content = "content")]
pub enum HunkChoice {
    Ours,
    Theirs,
    Base,
    /// Replacement text for this region, verbatim.
    Manual {
        content: String,
    },
}

/// One per-hunk decision for [`ConflictHunk::index`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkResolution {
    pub hunk: usize,
    pub choice: HunkChoice,
}

// ---------------------------------------------------------------------------
// Merge / reset / sequence modes
// ---------------------------------------------------------------------------

/// The merge strategy knobs the UI exposes — the counterpart of
/// `GTUserDefaultsDialogueOptionMerge*`. `Default` runs a plain recursive
/// merge; the other three map onto libgit2's "favor" (`-X ours` and friends),
/// never onto a different merge driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeStrategy {
    #[default]
    Default,
    Ours,
    Theirs,
    Union,
}

/// `reset_to_revision` modes (docs/03 §13). `Soft` moves only HEAD; `Mixed`
/// also rewrites the index; `Hard` additionally discards tracked working-tree
/// changes — which is why it is the one mode with a confirmation
/// precondition (`performResetActionWithRevision:headRevision:mode:snapshot:`
/// is observed with an explicit snapshot argument).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    #[default]
    Mixed,
    Hard,
}

/// One entry of an interactive todo list — the actions the evidence's
/// `GTGitInteractiveRebaseMergeProcessInfo` carries through the merge-process
/// model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequenceAction {
    #[default]
    Pick,
    Reword,
    Squash,
    Fixup,
    Edit,
    Drop,
}

/// One planned (or in-flight) sequence step. When a caller submits a plan,
/// only `action`, `oid` and `new_message` are read; the descriptive fields are
/// echoed back by `rebase_interactive_plan` so the todo editor can render a
/// row per commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub action: SequenceAction,
    pub oid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub short_oid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Signature>,
    /// For `Reword` (and to override a squash's combined message): the message
    /// the rewritten commit should carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_message: Option<String>,
}

/// `rebase_interactive_plan` (docs/03 §13): the todo list a client may edit
/// and hand back to `rebase_interactive_start`. Computing it changes nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencePlan {
    /// The upstream the range was taken from, as given.
    pub upstream: String,
    /// The commit the sequence will be replayed onto (defaults to upstream).
    pub onto: String,
    pub steps: Vec<PlanStep>,
}

/// Which of the three answers `wc_merge_status_load` gives (docs/03 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeState {
    /// Merging the incoming revision would change something — but it can be
    /// done without conflicts.
    Mergeable,
    /// A trial merge produced conflicts: the merge dialogue should warn.
    Conflict,
    /// The incoming revision is already contained in HEAD (`Merged` in the
    /// docs table's vocabulary: "up to date").
    Merged,
}

/// `wc_merge_status_load` result: `merge_analysis` plus a trial merge
/// (`merge_trees`, in memory) — nothing on disk moves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStatus {
    pub state: MergeState,
    /// The resolved incoming commit, when one was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incoming: Option<String>,
    /// The merge base, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// True when `git merge` would fast-forward — the UI can say so before
    /// the user commits to a merge.
    pub fast_forward: bool,
    /// Paths the trial merge could not combine. Only filled when `state` is
    /// [`MergeState::Conflict`], and capped: this is a preview, not a
    /// resolution list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_preview: Vec<String>,
}

/// `rewrite_impact_load` (docs/03 §13): the three observed validation
/// answers for a rewrite plan, computed before anything is touched.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteImpact {
    /// Commits in the plan that no remote-tracking ref reaches — rewriting
    /// them loses nothing that was published.
    pub unpushed: Vec<String>,
    /// Commits in the plan that are also ancestors of another local branch:
    /// rewriting them changes what that branch's history means.
    pub mainline: Vec<String>,
    /// Merge commits in the plan: rebasing flattens them.
    pub merge_commits: Vec<String>,
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// What an integrating command did. One enum for merge, rebase, cherry-pick,
/// revert, reset and sequence commands, because the merge-process model is
/// unified (evidence F5): Continue / Skip / Abort mean the same for all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "payload")]
pub enum IntegrationOutcome {
    /// The candidate was already contained in HEAD — nothing to do.
    UpToDate { head: String },
    /// A fast-forward moved HEAD (and the checked-out branch) to `head`.
    FastForward { head: String, from: String },
    /// A merge commit was created.
    Merged {
        commit: String,
        parents: Vec<String>,
    },
    /// Changes were merged into the index/working tree but deliberately not
    /// committed (`--squash`, or `auto_commit: false`, or a `no_commit`
    /// pick). The caller commits — or the user changes their mind and aborts.
    Staged,
    /// The operation paused on conflicts; resolve, then continue or abort.
    /// The conflict list travels on [`IntegrationResult::conflicts`].
    Conflicted,
    /// A sequence stepped paused on an `edit` action: the commit exists, the
    /// user may amend it, then continue.
    Edited { commit: String },
    /// A rebase finished: the branch (or detached HEAD) now points at `head`.
    RebaseFinished {
        head: String,
        applied: usize,
        dropped: usize,
    },
    /// A cherry-pick / revert sequence finished.
    SequenceFinished {
        applied: Vec<String>,
        skipped: Vec<String>,
        dropped: Vec<String>,
    },
    /// An integration was rolled back and the repository is clean again.
    Aborted {
        /// Where HEAD sits after the abort.
        head: String,
    },
}

/// The standard return envelope of an integrating command: the outcome, the
/// conflicts that stopped it (if any), and human-readable notes — chiefly the
/// auto-stash decisions, which never fail a command but must be visible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationResult {
    pub outcome: IntegrationOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl IntegrationResult {
    pub fn new(outcome: IntegrationOutcome) -> Self {
        Self {
            outcome,
            conflicts: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn with_conflicts(mut self, conflicts: Vec<ConflictSummary>) -> Self {
        self.conflicts = conflicts;
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// `reset_to_revision` result: the fresh status delta, plus the anchors the
/// undo surface needs. `undo_anchor` is the commit `ORIG_HEAD` pointed at
/// immediately **before** the reset — the value a "Undo Reset" action resets
/// back to.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    pub mode: ResetMode,
    pub head: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_head: Option<String>,
    pub undo_anchor: String,
    pub delta: StatusDelta,
}

/// One branch that `branch_restack` moved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestackEntry {
    pub name: String,
    /// The upstream the branch was rebuilt on.
    pub onto: String,
    pub previous: String,
    pub head: String,
    /// How many of the branch's own commits were replayed.
    pub replayed: usize,
}

/// One branch that could not be restacked without human input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestackFailure {
    pub name: String,
    pub message: String,
    /// Set when the failure is a content conflict: check the branch out and
    /// rebase it to resolve interactively.
    pub conflicted: bool,
}

/// `branch_stack_restack` (docs/03 §6) result.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestackReport {
    pub updated: Vec<RestackEntry>,
    /// Branches that were already built on their upstream.
    pub up_to_date: Vec<String>,
    pub failed: Vec<RestackFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// `merge_tool_invoke` / `diff_tool_invoke` exit report (docs/03 §13). The
/// command is fire-and-report: the tool owns the interaction, we report what
/// it exited with and what (if anything) we staged afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolReport {
    pub tool: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    /// Paths staged after a clean merge-tool exit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub staged: Vec<String>,
}
