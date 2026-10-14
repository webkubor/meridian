//! Working-copy domain vocabulary — the `FNGit*` counterpart of the staging /
//! index / diff engine (docs/02 §4, docs/03 §3).
//!
//! Plain data only: no `git2`, no `tauri`, no filesystem. The engine fills
//! these shapes; the web UI reads exactly these shapes and computes no status
//! strings, no hunk coordinates and no mode labels itself.

use serde::{Deserialize, Serialize};

use super::ids::RepoId;
use super::status::{FileStatus, StatusSummary, Tone, UiStatusView};

/// Context lines a diff carries when the caller names none — the observed
/// three-line default (`setContextLines:`).
pub const DEFAULT_CONTEXT_LINES: u32 = 3;

/// Upper bound for a caller-supplied context (`setContextLines:` is a free
/// number in the evidence; this is our sanity cap).
pub const MAX_CONTEXT_LINES: u32 = 2_000;

/// Safety cap on the rendered lines of one [`DiffDocument`]. When a diff
/// exceeds it, `truncated` is set and the UI shows its large-diff warning
/// (`LargeDiffWarningCellView` counterpart) instead of a frozen renderer.
pub const MAX_DOCUMENT_LINES: usize = 40_000;

/// Cap on the undecoded bytes [`ImageData`] will move over IPC (32 MiB).
pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// The knobs a status read honors — exactly the four observed
/// `GTGitStatusOptionInclude*` switches, under the names the UI toggles use
/// (`GTUserDefaultsWorkingCopyShow{Ignored,NonModified,AssumeUnchanged}Items`,
/// `...ShowConflictedItemsOnly`). Evidence F11.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkingCopyStatusOptions {
    /// Include `.gitignore`d files (`ShowIgnoredItems`).
    pub include_ignored: bool,
    /// Report an untracked directory as one row instead of recursing into it.
    pub include_dirs: bool,
    /// Include tracked-but-unmodified files (`ShowNonModifiedItems`).
    pub include_nonmodified: bool,
    /// Include assume-unchanged entries even though git suppresses them
    /// (`ShowAssumeUnchangedItems`).
    pub include_assume_unchanged: bool,
    /// Filter the result down to conflicted entries (`ShowConflictedItemsOnly`).
    pub conflicted_only: bool,
}

/// State of a gitlink (submodule) entry, computed for the status list so the
/// row can carry its badge without a second query. The richer taxonomy the
/// submodule group defines (ahead/behind/…) is its own module's type; status
/// reports the three facts a file row needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleState {
    /// Recorded in the index but its working directory is not populated.
    Uninitialized,
    /// Initialized and checked out at the recorded commit.
    Current,
    /// Initialized, but HEAD differs from the recorded commit.
    NewCommits,
    /// Initialized at the recorded commit, but with local modifications.
    Dirty,
}

/// The submodule facts a status row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleEntryState {
    pub state: SubmoduleState,
    /// The commit recorded in the parent index (the gitlink oid).
    pub recorded: Option<String>,
    /// The submodule's current HEAD, when it can be read.
    pub head: Option<String>,
}

/// One row of the working-copy list, on exactly one axis (index or working
/// tree), like the shared [`crate::domain::status::EntryStatus`] — but richer:
/// rename sources and submodule state travel with the row.
///
/// `code` is `None` for clean rows, which only appear when the caller asked
/// for them (`include_nonmodified` / `include_assume_unchanged`); those rows
/// still carry a complete `view`, so the UI never has to invent a rendering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<FileStatus>,
    pub staged: bool,
    pub assume_unchanged: bool,
    /// The derived rendering — "one enum, three renderings" (evidence F2).
    pub view: UiStatusView,
    /// Rename/copy source, when the code is `Renamed`/`Copied`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    /// Present for gitlink entries only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule: Option<SubmoduleEntryState>,
}

impl WorkingCopyEntry {
    /// A changed row; `view` is derived from the code so it cannot drift.
    pub fn changed(
        path: impl Into<String>,
        code: FileStatus,
        staged: bool,
        assume_unchanged: bool,
    ) -> Self {
        Self {
            path: path.into(),
            code: Some(code),
            staged,
            assume_unchanged,
            view: UiStatusView {
                short: code.short(),
                text: code.text().to_string(),
                tone: code.tone(),
            },
            old_path: None,
            submodule: None,
        }
    }

    /// A clean row (unmodified file), shown only under the
    /// `include_nonmodified` / `include_assume_unchanged` options.
    pub fn clean(path: impl Into<String>, assume_unchanged: bool) -> Self {
        Self {
            path: path.into(),
            code: None,
            staged: false,
            assume_unchanged,
            view: UiStatusView {
                short: '\u{2022}',
                text: "Unmodified".to_string(),
                tone: Tone::Muted,
            },
            old_path: None,
            submodule: None,
        }
    }

    pub fn with_old_path(mut self, old_path: Option<String>) -> Self {
        self.old_path = old_path;
        self
    }

    pub fn with_submodule(mut self, submodule: Option<SubmoduleEntryState>) -> Self {
        self.submodule = submodule;
        self
    }
}

/// What `wc_status_load` answers, and what every working-copy mutation returns
/// so the UI never has to re-run status itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTreeStatus {
    pub repo: RepoId,
    pub entries: Vec<WorkingCopyEntry>,
    pub summary: StatusSummary,
}

impl WorkingTreeStatus {
    pub fn new(repo: RepoId, entries: Vec<WorkingCopyEntry>) -> Self {
        let summary = summary_of(&entries);
        Self {
            repo,
            entries,
            summary,
        }
    }
}

/// Counts the header shows; clean rows count nowhere.
pub fn summary_of(entries: &[WorkingCopyEntry]) -> StatusSummary {
    let mut summary = StatusSummary::default();
    for entry in entries {
        match (entry.code, entry.staged) {
            (None, _) => {}
            (Some(FileStatus::Untracked), _) => summary.untracked += 1,
            (Some(FileStatus::Unmerged), _) => summary.conflicted += 1,
            (Some(FileStatus::Ignored), _) => {}
            (Some(_), true) => summary.staged += 1,
            (Some(_), false) => summary.unstaged += 1,
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// Entry detail
// ---------------------------------------------------------------------------

/// The unmerged sides of an entry, by index stage (`base` = stage 1,
/// `ours` = stage 2, `theirs` = stage 3). The conflict *kind* classification
/// belongs to the conflict group; this is the raw side information the detail
/// pane needs to offer "save versions".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryConflict {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<String>,
}

/// `wc_entry_detail_load` — what the file-detail pane shows beside the diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDetail {
    pub path: String,
    /// Whether the file currently exists in the working tree.
    pub exists: bool,
    /// Working-tree size in bytes (0 when the file does not exist).
    pub size: u64,
    pub mode: FileMode,
    /// Working-tree mtime, seconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime: Option<i64>,
    /// The index entry's blob id, when the path is tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<EntryConflict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule: Option<SubmoduleEntryState>,
}

/// Git's file modes, in the vocabulary the evidence shows on disk
/// (`100644`, `100755`, `120000`, `040000`, `160000` — evidence strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
    Submodule,
    Tree,
    Unreadable,
}

impl FileMode {
    /// The octal rendering the UI shows ("File mode changed from 100644 to
    /// 100755").
    pub fn short(self) -> &'static str {
        match self {
            FileMode::Regular => "100644",
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
            FileMode::Submodule => "160000",
            FileMode::Tree => "040000",
            FileMode::Unreadable => "000000",
        }
    }
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

/// The six observed diff perspectives (`GTGitCompare{IndexToBase,
/// MineToMergeBase,MineToTheirs,TheirsToMergeBase,WorkingTreeToBase,
/// WorkingTreeToIndex}Validator`, evidence F4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffPerspective {
    /// HEAD → index: the staged changes.
    IndexToBase,
    /// Merge base → ours (stage 1 → stage 2); conflict contexts.
    MineToMergeBase,
    /// Ours → theirs (stage 2 → stage 3); conflict contexts.
    MineToTheirs,
    /// Merge base → theirs (stage 1 → stage 3); conflict contexts.
    TheirsToMergeBase,
    /// HEAD → working tree, staged and unstaged blended: "all changes".
    WorkingTreeToBase,
    /// Index → working tree: the unstaged changes.
    WorkingTreeToIndex,
}

/// The diff options the UI toggles (docs/03 §3; `setContextLines:`,
/// `ignoresWhitespaceChangesOptionToggled:`, `toggleShowsFullDiff:`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiffOptions {
    /// Lines of context per hunk.
    pub context_lines: u32,
    /// Drop whitespace-only changes (the "Ignore whitespace" toggle).
    pub ignore_whitespace: bool,
    /// Render the whole file as one hunk ("Show full diff").
    pub show_full: bool,
    /// Syntax override for the renderer; when absent the UI infers from the
    /// path. Carried through untouched by the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            context_lines: DEFAULT_CONTEXT_LINES,
            ignore_whitespace: false,
            show_full: false,
            syntax: None,
        }
    }
}

/// How whitespace differences are treated while generating a diff — the
/// engine-side vocabulary behind the single `ignore_whitespace` wire switch
/// (git itself distinguishes all / change / at-eol).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhitespaceMode {
    #[default]
    Show,
    /// Ignore all whitespace (`-w`).
    IgnoreAll,
    /// Ignore changes in the amount of whitespace (`-b`).
    IgnoreChange,
    /// Ignore whitespace at end of line.
    IgnoreEol,
}

/// A file-mode change the document reports separately from its hunks
/// ("100644 → 100755"); content may be identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileModeChange {
    pub old: FileMode,
    pub new: FileMode,
}

/// The file-level facts of one delta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub status: FileStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_change: Option<FileModeChange>,
    pub old_size: u64,
    pub new_size: u64,
}

/// One rendered row inside a hunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    /// The `\ No newline at end of file` marker.
    NoNewline,
}

/// One rendered row (`HunkHeader` rows are carried by [`DiffHunk::header`];
/// the frontend renderer interleaves them).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Line number on the old side; `None` for added lines and markers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_lineno: Option<u32>,
    /// Line number on the new side; `None` for removed lines and markers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_lineno: Option<u32>,
    /// The line's text, without the leading `+`/`-`/space marker and without
    /// the trailing newline.
    pub text: String,
}

/// One hunk, addressable by its coordinates — the same shape the selectors
/// imply for chunk operations (docs/02 §13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    /// The `@@ -a,b +c,d @@` header, verbatim.
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// The whole document for one path under one perspective. `change: None`
/// means "no differences" (the unchanged-file view, CAP-G2-04); a binary
/// file carries `is_binary: true` and no hunks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub perspective: DiffPerspective,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<FileChange>,
    pub is_binary: bool,
    pub hunks: Vec<DiffHunk>,
    pub additions: u32,
    pub deletions: u32,
    /// Set when [`MAX_DOCUMENT_LINES`] cut the document short.
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
}

impl DiffDocument {
    /// A document with no differences at all.
    pub fn unchanged(path: Option<String>, perspective: DiffPerspective) -> Self {
        Self {
            path,
            perspective,
            change: None,
            is_binary: false,
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
            truncated: false,
            syntax: None,
        }
    }
}

/// A hunk *reference* — what `wc_diff_hunks_load` returns and what chunk
/// operations take back. Coordinates are carried twice (header + numbers) so
/// the engine can match a stale reference and refuse cleanly when it no
/// longer exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkRef {
    pub path: String,
    pub perspective: DiffPerspective,
    /// Position of the hunk within its document (0-based, in display order).
    pub index: u32,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
}

/// Which way a chunk operation moves a hunk (`stageChunk:` /
/// `unstageChunk:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChunkDirection {
    Stage,
    Unstage,
}

/// Where `wc_patch_apply` lands the patch (`git apply --cached` /
/// workdir / both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchLocation {
    Index,
    Workdir,
    Both,
}

// ---------------------------------------------------------------------------
// Image diff
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageSide {
    Old,
    New,
}

/// One side of an image comparison, base64-encoded for IPC
/// (`GTWorkingCopyDiffImageViewController` counterpart).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    pub path: String,
    pub side: ImageSide,
    pub mime: String,
    pub size: u64,
    /// Base64 of the raw file bytes; empty when that side does not exist.
    pub data_base64: String,
}

// ---------------------------------------------------------------------------
// File actions
// ---------------------------------------------------------------------------

/// Where `wc_entry_ignore` appends its pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IgnoreScope {
    /// The repository's own `.gitignore`.
    Repo,
    /// The user's global excludes file (`core.excludesFile`, or the XDG
    /// default `~/.config/git/ignore` when unset).
    Global,
}

/// What `wc_entry_open` launches (`openInTerminal:` / `openInFinder:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpenTarget {
    Editor,
    Terminal,
    Finder,
    DiffTool,
}

/// Args of `wc_paths_discard` beyond the paths themselves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiscardOptions {
    /// Also delete untracked files among the paths.
    pub include_untracked: bool,
    /// Required (non-empty) when `include_untracked` is on: the caller has
    /// shown the discard dialogue and the user confirmed
    /// (`GTDiscardLocalChangesDialogueViewController`).
    pub confirm_token: Option<String>,
}
