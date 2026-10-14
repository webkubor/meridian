//! Diff — the `FNGitDiff*` / `GTGitCompare*` counterpart (docs/02 §4,
//! docs/03 §3; evidence F4).
//!
//! One generator, six perspectives:
//!
//! | perspective | old side | new side | libgit2 call |
//! |---|---|---|---|
//! | `IndexToBase` | HEAD | index | `diff_tree_to_index` |
//! | `WorkingTreeToIndex` | index | workdir | `diff_index_to_workdir` |
//! | `WorkingTreeToBase` | HEAD | workdir+index | `diff_tree_to_workdir_with_index` |
//! | `MineToMergeBase` | stage 1 | stage 2 | `Patch::from_blobs` |
//! | `MineToTheirs` | stage 2 | stage 3 | `Patch::from_blobs` |
//! | `TheirsToMergeBase` | stage 1 | stage 3 | `Patch::from_blobs` |
//!
//! Nothing here depends on `tauri`; every return value is domain data. The
//! web layer renders hunks and never diffs anything itself.

use std::cell::Cell;
use std::path::Path;

use git2::{Blob, Delta, Diff, DiffDelta, DiffFormat, DiffOptions as Git2DiffOptions, Patch, Tree};

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::status::FileStatus;
use crate::domain::working_copy::{
    DiffDocument, DiffHunk, DiffLine, DiffLineKind, DiffOptions, DiffPerspective, FileChange,
    FileMode, FileModeChange, HunkRef, ImageData, ImageSide, PatchLocation, WhitespaceMode,
    DEFAULT_CONTEXT_LINES, MAX_CONTEXT_LINES, MAX_DOCUMENT_LINES, MAX_IMAGE_BYTES,
};

/// Context used by "show full diff": larger than [`MAX_DOCUMENT_LINES`], so the
/// whole file lands in a single hunk and the cap decides when to stop.
const FULL_CONTEXT_LINES: u32 = 100_000;

/// Engine-side diff options: the wire [`DiffOptions`] plus the richer
/// whitespace vocabulary git itself distinguishes (the switch the UI has is a
/// single boolean; the engine keeps the finer modes for callers that know
/// which one they want — chunk operations use [`WhitespaceMode::Show`] so that
/// coordinates always match the document the user clicked).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineDiffOptions {
    pub context_lines: u32,
    pub whitespace: WhitespaceMode,
    pub show_full: bool,
    /// Carried through untouched; the renderer picks the language.
    pub syntax: Option<String>,
}

impl Default for EngineDiffOptions {
    fn default() -> Self {
        Self {
            context_lines: DEFAULT_CONTEXT_LINES,
            whitespace: WhitespaceMode::Show,
            show_full: false,
            syntax: None,
        }
    }
}

impl EngineDiffOptions {
    pub fn new(context_lines: u32, whitespace: WhitespaceMode) -> Self {
        Self {
            context_lines,
            whitespace,
            ..Default::default()
        }
    }

    /// The wire options as the engine reads them. `ignore_whitespace` is the
    /// one boolean the UI has; it means "ignore all whitespace", which is what
    /// the evidence's toggle does (`ignoresWhitespaceChangesOptionToggled:`).
    pub fn from_wire(opts: &DiffOptions) -> Self {
        Self {
            context_lines: opts.context_lines,
            whitespace: if opts.ignore_whitespace {
                WhitespaceMode::IgnoreAll
            } else {
                WhitespaceMode::Show
            },
            show_full: opts.show_full,
            syntax: opts.syntax.clone(),
        }
    }

    /// The effective context, honoring the cap and `show_full`.
    fn effective_context(&self) -> u32 {
        if self.show_full {
            FULL_CONTEXT_LINES
        } else {
            self.context_lines.min(MAX_CONTEXT_LINES)
        }
    }

    /// Build the libgit2 option block. `path` narrows the diff to one pathspec
    /// — every document is per-path, and chunk operations rely on that.
    pub(crate) fn git2(&self, path: Option<&str>) -> Git2DiffOptions {
        let mut opts = Git2DiffOptions::new();
        opts.context_lines(self.effective_context())
            .interhunk_lines(if self.show_full { 0 } else { 1 })
            .include_typechange(true)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);

        match self.whitespace {
            WhitespaceMode::Show => {}
            WhitespaceMode::IgnoreAll => {
                opts.ignore_whitespace(true);
            }
            WhitespaceMode::IgnoreChange => {
                opts.ignore_whitespace_change(true);
            }
            WhitespaceMode::IgnoreEol => {
                opts.ignore_whitespace_eol(true);
            }
        }

        if let Some(path) = path {
            opts.pathspec(path);
        }
        opts
    }
}

// ---------------------------------------------------------------------------
// Low-level diff builders (shared with the chunk operations in `engine::index`)
// ---------------------------------------------------------------------------

/// The committed tree, or `None` on an unborn HEAD. Every perspective has to
/// survive the "no commits yet" repository: libgit2 reads a `None` old side as
/// the empty tree, which is exactly the porcelain behaviour.
pub(crate) fn head_tree_optional<'r>(
    repo: &'r git2::Repository,
) -> Result<Option<Tree<'r>>, ErrorPayload> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_commit()?.tree()?)),
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

/// The index as a tree. Writing it is what makes the staged state visible to
/// the tree-to-tree machinery (and it is harmless when nothing changed).
pub(crate) fn index_tree<'r>(repo: &'r git2::Repository) -> Result<Tree<'r>, ErrorPayload> {
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    Ok(repo.find_tree(oid)?)
}

/// index → workdir: the unstaged changes (`WorkingTreeToIndex`).
pub(crate) fn index_to_workdir_diff<'r>(
    repo: &'r git2::Repository,
    path: Option<&str>,
    opts: &EngineDiffOptions,
) -> Result<Diff<'r>, ErrorPayload> {
    let mut git = opts.git2(path);
    Ok(repo.diff_index_to_workdir(None, Some(&mut git))?)
}

/// HEAD → index: the staged changes (`IndexToBase`).
pub(crate) fn head_to_index_diff<'r>(
    repo: &'r git2::Repository,
    path: Option<&str>,
    opts: &EngineDiffOptions,
) -> Result<Diff<'r>, ErrorPayload> {
    let head = head_tree_optional(repo)?;
    let mut git = opts.git2(path);
    Ok(repo.diff_tree_to_index(head.as_ref(), None, Some(&mut git))?)
}

/// index → HEAD: the mirror of [`head_to_index_diff`], used to unstage a hunk
/// (the patch then applies backwards, into the index).
pub(crate) fn index_to_head_diff<'r>(
    repo: &'r git2::Repository,
    path: Option<&str>,
    opts: &EngineDiffOptions,
) -> Result<Diff<'r>, ErrorPayload> {
    let index_tree = index_tree(repo)?;
    let head = head_tree_optional(repo)?;
    let mut git = opts.git2(path);
    Ok(repo.diff_tree_to_tree(Some(&index_tree), head.as_ref(), Some(&mut git))?)
}

/// HEAD → workdir with the index consulted: the "all changes" perspective
/// (`WorkingTreeToBase`), which blends staged and unstaged changes the way the
/// file list's summary row does.
pub(crate) fn head_to_workdir_diff<'r>(
    repo: &'r git2::Repository,
    path: Option<&str>,
    opts: &EngineDiffOptions,
) -> Result<Diff<'r>, ErrorPayload> {
    let head = head_tree_optional(repo)?;
    let mut git = opts.git2(path);
    Ok(repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut git))?)
}

fn axis_diff<'r>(
    repo: &'r git2::Repository,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<Diff<'r>, ErrorPayload> {
    match perspective {
        DiffPerspective::IndexToBase => head_to_index_diff(repo, path, opts),
        DiffPerspective::WorkingTreeToIndex => index_to_workdir_diff(repo, path, opts),
        DiffPerspective::WorkingTreeToBase => head_to_workdir_diff(repo, path, opts),
        DiffPerspective::MineToMergeBase
        | DiffPerspective::MineToTheirs
        | DiffPerspective::TheirsToMergeBase => Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("{perspective:?} is a conflict perspective and is built from index stages"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// `wc_diff_load` (docs/03 §3): the document for one path under one
/// perspective.
///
/// When `path` is `None` the document describes the first changed file of the
/// diff (the evidence's diff view is per selected file; a pathless call is the
/// caller asking for "whatever changed first"). A path with no differences —
/// or a perspective that is simply empty — answers with
/// [`DiffDocument::unchanged`], never an error.
pub fn document(
    repo: &git2::Repository,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    match perspective {
        DiffPerspective::MineToMergeBase
        | DiffPerspective::MineToTheirs
        | DiffPerspective::TheirsToMergeBase => {
            let path = path.ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    "A conflict perspective needs a path",
                )
            })?;
            conflict_document(repo, path, perspective, opts)
        }
        _ => {
            let diff = axis_diff(repo, path, perspective, opts)?;
            document_from_diff(&diff, path, perspective, opts)
        }
    }
}

/// A document for an explicit tree pair — the entry point the history module
/// uses for commit and commit..commit diffs. `perspective` is the caller's
/// label for the pair (it only travels through to the UI).
pub fn tree_pair_document(
    repo: &git2::Repository,
    old: Option<&Tree<'_>>,
    new: Option<&Tree<'_>>,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    let mut git = opts.git2(path);
    let diff = repo.diff_tree_to_tree(old, new, Some(&mut git))?;
    document_from_diff(&diff, path, perspective, opts)
}

/// A commit vs its first parent (or the empty tree for a root commit) — the
/// "this commit's changes" document.
pub fn commit_document(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    let new_tree = commit.tree()?;
    let parent_tree = match commit.parent(0) {
        Ok(parent) => Some(parent.tree()?),
        Err(_) => None,
    };
    tree_pair_document(
        repo,
        parent_tree.as_ref(),
        Some(&new_tree),
        path,
        perspective,
        opts,
    )
}

/// A commit pair (`commit..commit`).
pub fn commit_pair_document(
    repo: &git2::Repository,
    old: &git2::Commit<'_>,
    new: &git2::Commit<'_>,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    let old_tree = old.tree()?;
    let new_tree = new.tree()?;
    tree_pair_document(
        repo,
        Some(&old_tree),
        Some(&new_tree),
        path,
        perspective,
        opts,
    )
}

/// `wc_diff_hunks_load` (docs/03 §3): the hunk references chunk operations
/// take back. Coordinates are carried twice (numbers plus header) so a stale
/// reference can be refused rather than misapplied.
pub fn hunks(
    repo: &git2::Repository,
    path: &str,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<Vec<HunkRef>, ErrorPayload> {
    let doc = document(repo, Some(path), perspective, opts)?;
    Ok(doc
        .hunks
        .iter()
        .enumerate()
        .map(|(index, hunk)| HunkRef {
            path: path.to_string(),
            perspective,
            index: index as u32,
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
            header: hunk.header.clone(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Document internals
// ---------------------------------------------------------------------------

fn document_from_diff(
    diff: &Diff<'_>,
    path: Option<&str>,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    let Some(index) = pick_delta(diff, path)? else {
        return Ok(DiffDocument::unchanged(
            path.map(str::to_string),
            perspective,
        ));
    };
    let raw_delta = diff
        .get_delta(index)
        .ok_or_else(|| ErrorPayload::new(ErrorCode::Unclassified, "Diff delta disappeared"))?;

    // Generating the patch is what makes libgit2 load the file content and
    // decide whether it is binary; the delta's flags read *before* this point
    // are not yet populated, which is a reliable way to be wrong about binary
    // files. So: generate first, then read the facts off the patch's delta.
    let patch = Patch::from_diff(diff, index)?;
    let delta = match patch.as_ref() {
        Some(patch) => patch.delta(),
        None => raw_delta,
    };

    let binary = is_binary(&delta);
    let mut doc = DiffDocument {
        path: delta_path(&delta, path),
        perspective,
        change: Some(file_change(&delta)),
        is_binary: binary,
        hunks: Vec::new(),
        additions: 0,
        deletions: 0,
        truncated: false,
        syntax: opts.syntax.clone(),
    };

    // A mode-only change has a delta and a mode change but no hunks at all.
    if !binary {
        if let Some(patch) = patch.as_ref() {
            let (_, additions, deletions) = patch.line_stats()?;
            doc.additions = additions.min(u32::MAX as usize) as u32;
            doc.deletions = deletions.min(u32::MAX as usize) as u32;
            let (hunks, truncated) = collect_hunks(patch);
            doc.hunks = hunks;
            doc.truncated = truncated;
        }
    }
    Ok(doc)
}

fn conflict_document(
    repo: &git2::Repository,
    path: &str,
    perspective: DiffPerspective,
    opts: &EngineDiffOptions,
) -> Result<DiffDocument, ErrorPayload> {
    let index = repo.index()?;
    let stage = |stage: i32| {
        crate::engine::status::lookup(&index, path, stage).map(|entry| (entry.id, entry.mode))
    };
    let base = stage(1);
    let ours = stage(2);
    let theirs = stage(3);
    if base.is_none() && ours.is_none() && theirs.is_none() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("No conflict stages for {path}"),
        )
        .with_param("path", path));
    }

    let (old, new) = match perspective {
        DiffPerspective::MineToMergeBase => (base, ours),
        DiffPerspective::MineToTheirs => (ours, theirs),
        DiffPerspective::TheirsToMergeBase => (base, theirs),
        _ => {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "Not a conflict perspective",
            ))
        }
    };

    let old_blob = blob_or_empty(repo, old)?;
    let new_blob = blob_or_empty(repo, new)?;
    let mut git = opts.git2(Some(path));
    let patch = Patch::from_blobs(
        &old_blob,
        Some(Path::new(path)),
        &new_blob,
        Some(Path::new(path)),
        Some(&mut git),
    )?;

    let delta = patch.delta();
    let binary = is_binary(&delta);
    let mut doc = DiffDocument {
        path: Some(path.to_string()),
        perspective,
        change: Some(file_change(&delta)),
        is_binary: binary,
        hunks: Vec::new(),
        additions: 0,
        deletions: 0,
        truncated: false,
        syntax: opts.syntax.clone(),
    };
    if !binary {
        let (_, additions, deletions) = patch.line_stats()?;
        doc.additions = additions.min(u32::MAX as usize) as u32;
        doc.deletions = deletions.min(u32::MAX as usize) as u32;
        let (hunks, truncated) = collect_hunks(&patch);
        doc.hunks = hunks;
        doc.truncated = truncated;
    }
    Ok(doc)
}

/// A blob for one conflict side; a missing stage becomes the empty blob, which
/// is how the pair reads as "added" / "deleted" rather than an error.
fn blob_or_empty<'r>(
    repo: &'r git2::Repository,
    side: Option<(git2::Oid, u32)>,
) -> Result<Blob<'r>, ErrorPayload> {
    match side {
        Some((oid, _)) => Ok(repo.find_blob(oid)?),
        // A missing stage reads as the empty blob, which is what makes the
        // pair a plain add or delete instead of an error.
        None => {
            let empty = repo.blob(b"")?;
            Ok(repo.find_blob(empty)?)
        }
    }
}

fn pick_delta(diff: &Diff<'_>, path: Option<&str>) -> Result<Option<usize>, ErrorPayload> {
    match path {
        Some(path) => {
            for index in 0..diff.deltas().len() {
                if let Some(delta) = diff.get_delta(index) {
                    let old = delta.old_file().path();
                    let new = delta.new_file().path();
                    if old == Some(Path::new(path)) || new == Some(Path::new(path)) {
                        return Ok(Some(index));
                    }
                }
            }
            Ok(None)
        }
        None => {
            for index in 0..diff.deltas().len() {
                if let Some(delta) = diff.get_delta(index) {
                    if !matches!(
                        delta.status(),
                        Delta::Unmodified | Delta::Ignored | Delta::Unreadable
                    ) {
                        return Ok(Some(index));
                    }
                }
            }
            Ok(None)
        }
    }
}

fn delta_path(delta: &DiffDelta<'_>, requested: Option<&str>) -> Option<String> {
    if let Some(path) = requested {
        return Some(path.to_string());
    }
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|path| path.to_string_lossy().into_owned())
}

fn is_binary(delta: &DiffDelta<'_>) -> bool {
    delta.flags().contains(git2::DiffFlags::BINARY)
        || delta.old_file().is_binary()
        || delta.new_file().is_binary()
}

fn file_change(delta: &DiffDelta<'_>) -> FileChange {
    let old_path = delta.old_file().path();
    let new_path = delta.new_file().path();
    let renamed = old_path != new_path;
    let old_mode = mode_of(&delta.old_file());
    let new_mode = mode_of(&delta.new_file());
    let status = delta_status(delta.status());

    let mode_change = match delta.status() {
        // An add or a delete has no "from" mode to compare against.
        Delta::Added | Delta::Deleted | Delta::Untracked => None,
        _ if old_mode != new_mode => Some(FileModeChange {
            old: old_mode,
            new: new_mode,
        }),
        _ => None,
    };

    FileChange {
        status,
        old_path: if renamed {
            old_path.map(|path| path.to_string_lossy().into_owned())
        } else {
            None
        },
        mode_change,
        old_size: delta.old_file().size(),
        new_size: delta.new_file().size(),
    }
}

fn mode_of(file: &git2::DiffFile<'_>) -> FileMode {
    crate::engine::status::mode_from_raw(u32::from(file.mode()))
}

fn delta_status(delta: Delta) -> FileStatus {
    match delta {
        Delta::Added => FileStatus::Added,
        Delta::Copied => FileStatus::Copied,
        Delta::Deleted => FileStatus::Deleted,
        Delta::Renamed => FileStatus::Renamed,
        Delta::Conflicted => FileStatus::Unmerged,
        Delta::Untracked => FileStatus::Untracked,
        Delta::Ignored => FileStatus::Ignored,
        Delta::Typechange | Delta::Modified => FileStatus::Modified,
        Delta::Unmodified | Delta::Unreadable => FileStatus::Modified,
    }
}

/// Walk a patch into domain hunks, stopping at [`MAX_DOCUMENT_LINES`] so a
/// million-line generated file cannot freeze the renderer. The second value is
/// the `truncated` flag.
fn collect_hunks(patch: &Patch<'_>) -> (Vec<DiffHunk>, bool) {
    let mut hunks = Vec::new();
    let mut budget = MAX_DOCUMENT_LINES;
    let mut truncated = false;

    for hunk_index in 0..patch.num_hunks() {
        let Ok((hunk, line_count)) = patch.hunk(hunk_index) else {
            continue;
        };
        let mut domain = DiffHunk {
            header: String::from_utf8_lossy(hunk.header())
                .trim_end()
                .to_string(),
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines: Vec::with_capacity(line_count),
        };
        for line_index in 0..line_count {
            if budget == 0 {
                truncated = true;
                break;
            }
            let Ok(line) = patch.line_in_hunk(hunk_index, line_index) else {
                continue;
            };
            domain.lines.push(convert_line(&line));
            budget -= 1;
        }
        hunks.push(domain);
        if truncated {
            break;
        }
    }
    (hunks, truncated)
}

/// One libgit2 patch line as domain data. libgit2 hands out the line *without*
/// its `+`/`-`/space sigil (the official printers re-add it); the end-of-file
/// marker lines carry the whole `\ No newline at end of file` text.
fn convert_line(line: &git2::DiffLine<'_>) -> DiffLine {
    use git2::DiffLineType;

    let kind = match line.origin_value() {
        DiffLineType::Addition => DiffLineKind::Added,
        DiffLineType::Deletion => DiffLineKind::Removed,
        DiffLineType::ContextEOFNL | DiffLineType::AddEOFNL | DiffLineType::DeleteEOFNL => {
            DiffLineKind::NoNewline
        }
        DiffLineType::Context
        | DiffLineType::HunkHeader
        | DiffLineType::FileHeader
        | DiffLineType::Binary => DiffLineKind::Context,
    };

    let mut text = String::from_utf8_lossy(line.content()).into_owned();
    if text.ends_with('\n') {
        text.pop();
    }

    DiffLine {
        kind,
        old_lineno: line.old_lineno(),
        new_lineno: line.new_lineno(),
        text,
    }
}

// ---------------------------------------------------------------------------
// Chunk application (shared with `engine::index`)
// ---------------------------------------------------------------------------

/// The coordinates of one hunk of the diff we are about to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HunkSpan {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
}

/// What the caller's reference looks like in the coordinate system of a diff
/// that runs the other way round (index → HEAD for an unstage): the old and
/// new sides swap over.
fn target_span(target: &HunkRef, mirrored: bool) -> HunkSpan {
    if mirrored {
        HunkSpan {
            old_start: target.new_start,
            old_lines: target.new_lines,
            new_start: target.old_start,
            new_lines: target.old_lines,
        }
    } else {
        HunkSpan {
            old_start: target.old_start,
            old_lines: target.old_lines,
            new_start: target.new_start,
            new_lines: target.new_lines,
        }
    }
}

/// Every hunk span of the diff (one path ⇒ one delta ⇒ its hunks).
fn spans(diff: &Diff<'_>) -> Vec<HunkSpan> {
    let mut out = Vec::new();
    for index in 0..diff.deltas().len() {
        let Ok(Some(patch)) = Patch::from_diff(diff, index) else {
            continue;
        };
        for hunk_index in 0..patch.num_hunks() {
            let Ok((hunk, _)) = patch.hunk(hunk_index) else {
                continue;
            };
            out.push(HunkSpan {
                old_start: hunk.old_start(),
                old_lines: hunk.old_lines(),
                new_start: hunk.new_start(),
                new_lines: hunk.new_lines(),
            });
        }
    }
    out
}

/// Which hunk of `diff` the caller's reference names.
///
/// First an exact coordinate match. If that fails, the reference was rendered
/// with a different amount of context than the diff we just regenerated with,
/// so a span that consistently contains the reference (or is contained by it)
/// is accepted — but only when exactly one hunk qualifies, so an ambiguous
/// reference is refused rather than guessed at.
fn resolve_hunk(diff: &Diff<'_>, target: &HunkRef, mirrored: bool) -> Option<HunkSpan> {
    let wanted = target_span(target, mirrored);
    let candidates = spans(diff);

    if let Some(found) = candidates.iter().find(|span| **span == wanted) {
        return Some(*found);
    }

    let containing: Vec<HunkSpan> = candidates
        .into_iter()
        .filter(|span| contains(span, &wanted) || contains(&wanted, span))
        .collect();
    match containing.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// Does `outer` cover `inner` on both sides? Zero-length sides (a pure
/// insertion or deletion) are compared as one-line spans, which is what makes
/// containment meaningful for them.
fn contains(outer: &HunkSpan, inner: &HunkSpan) -> bool {
    outer.old_start <= inner.old_start
        && outer.new_start <= inner.new_start
        && outer.old_start + outer.old_lines.max(1) >= inner.old_start + inner.old_lines.max(1)
        && outer.new_start + outer.new_lines.max(1) >= inner.new_start + inner.new_lines.max(1)
}

/// The coordinates of a hunk, `@@ -a,b +c,d @@`, built from the numeric fields.
///
/// `HunkRef::header` is libgit2's verbatim hunk header, whose tail is the
/// nearest enclosing function line *from the file* — source code. A stale hunk
/// is a routine outcome (stage a hunk after the file changed) and its error is
/// logged at error level, so the text after the closing `@@` must never reach a
/// message, a param or a log line. The numbers carry everything a bug report
/// needs.
fn hunk_coordinates(target: &HunkRef) -> String {
    format!(
        "@@ -{},{} +{},{} @@",
        target.old_start, target.old_lines, target.new_start, target.new_lines
    )
}

/// The error a stale hunk reference earns: refuse, never misapply.
pub(crate) fn hunk_mismatch(target: &HunkRef) -> ErrorPayload {
    let coordinates = hunk_coordinates(target);
    ErrorPayload::new(
        ErrorCode::InvalidRequest,
        format!(
            "The hunk no longer exists in {} ({coordinates}); reload the diff",
            target.path
        ),
    )
    .with_param("path", target.path.clone())
    .with_param("hunk", coordinates)
}

/// Apply exactly one hunk of `diff` to `location`; every other hunk is
/// skipped. libgit2's `hunk_callback` is the selective-apply hook: return
/// `true` to apply, `false` to skip. A reference that matches nothing is a
/// typed error — a hunk that has since moved must never be guessed at.
pub(crate) fn apply_selected_hunk(
    repo: &git2::Repository,
    diff: &Diff<'_>,
    target: &HunkRef,
    mirrored: bool,
    location: git2::ApplyLocation,
) -> Result<(), ErrorPayload> {
    let Some(span) = resolve_hunk(diff, target, mirrored) else {
        return Err(hunk_mismatch(target));
    };

    let matched = Cell::new(false);
    let mut options = git2::ApplyOptions::new();
    options.hunk_callback(|hunk| {
        let Some(hunk) = hunk else { return true };
        if hunk.old_start() == span.old_start
            && hunk.old_lines() == span.old_lines
            && hunk.new_start() == span.new_start
            && hunk.new_lines() == span.new_lines
        {
            matched.set(true);
            true
        } else {
            false
        }
    });

    repo.apply(diff, location, Some(&mut options))?;

    if !matched.get() {
        return Err(hunk_mismatch(target));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Patches
// ---------------------------------------------------------------------------

/// `wc_patch_save` (docs/03 §3): write the working-copy changes of the given
/// paths out as a patch file. `omit_prefixes` reproduces
/// `GTUserDefaultsSavePatchOmitPrefixes` — the `a/`…`b/` prefixes are dropped
/// by regenerating with empty prefixes rather than by editing the text.
pub fn save_patch(
    repo: &git2::Repository,
    paths: &[String],
    dest: &Path,
    omit_prefixes: bool,
) -> Result<(), ErrorPayload> {
    let mut git = Git2DiffOptions::new();
    git.include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true)
        .include_typechange(true);
    if omit_prefixes {
        git.old_prefix("").new_prefix("");
    }
    for path in paths {
        git.pathspec(path.as_str());
    }

    let head = head_tree_optional(repo)?;
    let diff = repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut git))?;

    let mut buffer: Vec<u8> = Vec::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => buffer.push(line.origin() as u8),
            _ => {}
        }
        buffer.extend_from_slice(line.content());
        true
    })?;

    std::fs::write(dest, &buffer).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write {dest:?}"))
            .with_detail(err.to_string())
            .with_param("dest", dest.to_string_lossy().into_owned())
    })?;
    Ok(())
}

/// `wc_patch_apply` (docs/03 §3): parse a patch and apply it.
///
/// Plain application is a straight `Repository::apply`. With `three_way` the
/// engine first tries the same clean apply and, when it fails, merges each
/// modified file (`git apply --3way` semantics) using the pre- and post-image
/// blobs recorded in the patch's `index` line — those must resolve in this
/// repository, which is precisely the case a three-way apply is useful for.
/// Content that does not merge cleanly, or a patch whose blobs are absent,
/// fails with the paths named.
pub fn apply_patch(
    repo: &git2::Repository,
    patch_bytes: &[u8],
    location: PatchLocation,
    three_way: bool,
) -> Result<(), ErrorPayload> {
    let diff = Diff::from_buffer(patch_bytes).map_err(|err| {
        ErrorPayload::new(ErrorCode::InvalidRequest, "The patch could not be parsed")
            .with_detail(err.to_string())
    })?;

    let target = apply_location(location);

    // The clean apply is what a matching work tree gets, three-way or not.
    let clean = repo.apply(&diff, target, None);
    if clean.is_ok() || !three_way {
        return clean.map_err(|err| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("The patch did not apply to {location:?}"),
            )
            .with_detail(err.to_string())
        });
    }

    three_way_apply(repo, &diff, location)
}

fn apply_location(location: PatchLocation) -> git2::ApplyLocation {
    match location {
        PatchLocation::Index => git2::ApplyLocation::Index,
        PatchLocation::Workdir => git2::ApplyLocation::WorkDir,
        PatchLocation::Both => git2::ApplyLocation::Both,
    }
}

/// The three-way fallback (`git apply --3way` semantics): per file, merge
/// "what the patch wants" against "what is on disk now".
///
/// The merge needs three versions. *Base* is the patch's own preimage, which
/// git requires to be present in the object database; *ours* is the current
/// content of the target; *theirs* is the postimage, which the patch itself
/// only carries as hunks — so it is reconstructed by applying the file's patch
/// to the preimage through libgit2's own applier (`apply_to_tree` against a
/// one-file tree), never by hand.
fn three_way_apply(
    repo: &git2::Repository,
    diff: &Diff<'_>,
    location: PatchLocation,
) -> Result<(), ErrorPayload> {
    let mut unresolved: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    for index in 0..diff.deltas().len() {
        let Some(delta) = diff.get_delta(index) else {
            continue;
        };
        let Some(path) = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|path| path.to_string_lossy().into_owned())
        else {
            continue;
        };

        // Only content changes take part; adds and deletes keep the clean
        // semantics (there is nothing to merge them with).
        if !matches!(delta.status(), Delta::Modified | Delta::Typechange) {
            unresolved.push(path);
            continue;
        }

        let Some(preimage) = resolve_blob(repo, delta.old_file().id()) else {
            // git has the same requirement: without the preimage blob there is
            // nothing to merge against.
            unresolved.push(path);
            continue;
        };
        let base = preimage.content().to_vec();
        let base_oid = preimage.id();
        let Some(ours) = ours_bytes(repo, &path, location)? else {
            unresolved.push(path);
            continue;
        };
        let Some(theirs) = postimage(repo, diff, index, &path, &delta, base_oid) else {
            // The patch does not even apply to its own preimage.
            conflicts.push(path);
            continue;
        };

        let mut ancestor = git2::MergeFileInput::new();
        ancestor.path(path.as_str()).content(&base);
        let mut our_input = git2::MergeFileInput::new();
        our_input.path(path.as_str()).content(&ours);
        let mut their_input = git2::MergeFileInput::new();
        their_input.path(path.as_str()).content(&theirs);

        let merged = git2::merge_file(&ancestor, &our_input, &their_input, None)?;
        if !merged.is_automergeable() {
            conflicts.push(path);
            continue;
        }
        write_side(repo, &path, merged.content(), location)?;
    }

    if !unresolved.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "A three-way apply needs the patch's preimage blob, which this repository does not have",
        )
        .with_detail(format!("unresolved: {}", unresolved.join(", "))));
    }
    if !conflicts.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The patch conflicts with the current content",
        )
        .with_detail(format!("conflicts: {}", conflicts.join(", "))));
    }
    Ok(())
}

/// A blob by id, tolerating the abbreviated ids a patch file carries.
///
/// `index <short>..<short>` lines only record an abbreviation, and libgit2's
/// patch parser keeps it as an oid zero-padded on the right. A full lookup
/// therefore misses; walking the abbreviation lengths finds the real object,
/// and an ambiguous length simply fails and moves on.
fn resolve_blob(repo: &git2::Repository, id: git2::Oid) -> Option<git2::Blob<'_>> {
    if let Ok(blob) = repo.find_blob(id) {
        return Some(blob);
    }
    let hex = id.to_string();
    for len in 7..=hex.len() {
        if let Ok(object) = repo.find_object_by_prefix(&hex[..len], Some(git2::ObjectType::Blob)) {
            if let Ok(blob) = object.into_blob() {
                return Some(blob);
            }
        }
    }
    None
}

/// The postimage of one delta: its own patch applied to its own preimage, done
/// by libgit2 (a one-file tree, `apply_to_tree`) so the result is exactly what
/// a clean apply would have produced.
fn postimage(
    repo: &git2::Repository,
    diff: &Diff<'_>,
    index: usize,
    path: &str,
    delta: &DiffDelta<'_>,
    base_oid: git2::Oid,
) -> Option<Vec<u8>> {
    let mut patch = Patch::from_diff(diff, index).ok()??;
    let buf = patch.to_buf().ok()?;
    let single = Diff::from_buffer(&buf).ok()?;

    let tree = tree_with_blob(repo, path, base_oid, delta.old_file().mode()).ok()?;
    let result = repo.apply_to_tree(&tree, &single, None).ok()?;
    let entry = result.get_path(Path::new(path), 0)?;
    let blob = repo.find_blob(entry.id).ok()?;
    Some(blob.content().to_vec())
}

/// A tree holding exactly one file, at `path` (directories included).
fn tree_with_blob<'r>(
    repo: &'r git2::Repository,
    path: &str,
    oid: git2::Oid,
    mode: git2::FileMode,
) -> Result<Tree<'r>, ErrorPayload> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err(ErrorPayload::new(ErrorCode::InvalidRequest, "Empty path"));
    }
    let root = write_tree_segments(repo, &segments, oid, u32::from(mode))?;
    Ok(repo.find_tree(root)?)
}

fn write_tree_segments(
    repo: &git2::Repository,
    segments: &[&str],
    oid: git2::Oid,
    mode: u32,
) -> Result<git2::Oid, ErrorPayload> {
    let (name, rest) = segments.split_first().expect("non-empty");
    let mut builder = repo.treebuilder(None)?;
    if rest.is_empty() {
        builder.insert(*name, oid, mode as i32)?;
    } else {
        let child = write_tree_segments(repo, rest, oid, mode)?;
        builder.insert(*name, child, 0o040000)?;
    }
    Ok(builder.write()?)
}

fn blob_bytes(repo: &git2::Repository, oid: git2::Oid) -> Option<Vec<u8>> {
    if oid.is_zero() {
        return None;
    }
    repo.find_blob(oid).ok().map(|blob| blob.content().to_vec())
}

/// "Ours" for a three-way apply: the index version when the patch is going
/// into the index, otherwise the file on disk.
fn ours_bytes(
    repo: &git2::Repository,
    path: &str,
    location: PatchLocation,
) -> Result<Option<Vec<u8>>, ErrorPayload> {
    if location == PatchLocation::Index {
        let index = repo.index()?;
        return Ok(crate::engine::status::lookup(&index, path, 0)
            .and_then(|entry| blob_bytes(repo, entry.id)));
    }
    let workdir = repo.workdir().ok_or_else(|| {
        ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
    })?;
    Ok(std::fs::read(workdir.join(path)).ok())
}

/// Land a merged side on the index, the working tree, or both. Used only by
/// the three-way fallback, so a plain `fs::write` plus an index update is
/// enough — the file's mode does not change under a content merge.
fn write_side(
    repo: &git2::Repository,
    path: &str,
    content: &[u8],
    location: PatchLocation,
) -> Result<(), ErrorPayload> {
    if matches!(location, PatchLocation::Workdir | PatchLocation::Both) {
        let workdir = repo.workdir().ok_or_else(|| {
            ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
        })?;
        std::fs::write(workdir.join(path), content).map_err(|err| {
            ErrorPayload::new(ErrorCode::Io, format!("Could not write {path}"))
                .with_detail(err.to_string())
        })?;
    }
    if matches!(location, PatchLocation::Index | PatchLocation::Both) {
        let mut index = repo.index()?;
        // `IndexEntry` has no `Clone`; the fields are public, so the merged
        // content goes in as a fresh entry built from the old one.
        if let Some(entry) = crate::engine::status::lookup(&index, path, 0) {
            let blob = repo.blob(content)?;
            let fresh = git2::IndexEntry {
                ctime: entry.ctime,
                mtime: entry.mtime,
                dev: entry.dev,
                ino: entry.ino,
                mode: entry.mode,
                uid: entry.uid,
                gid: entry.gid,
                file_size: content.len() as u32,
                id: blob,
                flags: entry.flags,
                flags_extended: entry.flags_extended,
                path: entry.path.clone(),
            };
            index.add(&fresh)?;
            index.write()?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Image diff
// ---------------------------------------------------------------------------

/// `wc_image_data_load` (docs/03 §3): one side of an image comparison,
/// base64-encoded for IPC.
///
/// There is no perspective argument in the evidence's command, so the two
/// sides are fixed: *old* is the version the repository holds (HEAD, falling
/// back to the index for a file that is not committed yet) and *new* is the
/// version on disk.
pub fn image_data(
    repo: &git2::Repository,
    path: &str,
    side: ImageSide,
) -> Result<ImageData, ErrorPayload> {
    let bytes = match side {
        ImageSide::Old => old_side_bytes(repo, path)?,
        ImageSide::New => new_side_bytes(repo, path)?,
    };
    let bytes = bytes.unwrap_or_default();
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!("The image is larger than {MAX_IMAGE_BYTES} bytes"),
        )
        .with_param("path", path));
    }
    Ok(ImageData {
        path: path.to_string(),
        side,
        mime: mime_for(path),
        size: bytes.len() as u64,
        data_base64: base64_encode(&bytes),
    })
}

fn old_side_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    let head = head_tree_optional(repo)?;
    if let Some(tree) = head.as_ref() {
        if let Ok(entry) = tree.get_path(Path::new(path)) {
            if let Ok(blob) = repo.find_blob(entry.id()) {
                return Ok(Some(blob.content().to_vec()));
            }
        }
    }
    let index = repo.index()?;
    Ok(crate::engine::status::lookup(&index, path, 0).and_then(|entry| blob_bytes(repo, entry.id)))
}

fn new_side_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    let workdir = repo.workdir().ok_or_else(|| {
        ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
    })?;
    Ok(std::fs::read(workdir.join(path)).ok())
}

/// MIME by extension — the image pane only needs enough to hand the bytes to
/// the browser.
pub(crate) fn mime_for(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    let mime = match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

/// Standard base64 with padding. Hand-written because the workspace carries no
/// encoding crate and this is the only place that needs one.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::index::stage_paths;
    use crate::engine::index::test_support::{commit_all, scratch_repo};

    fn doc_for(repo: &git2::Repository, path: &str, perspective: DiffPerspective) -> DiffDocument {
        document(repo, Some(path), perspective, &EngineDiffOptions::default()).unwrap()
    }

    #[test]
    fn unstaged_document_reports_hunks_and_coordinates() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one\ntwo\nTHREE\nfour\nfive\n").unwrap();

        let doc = doc_for(&repo, "a.txt", DiffPerspective::WorkingTreeToIndex);
        assert_eq!(doc.hunks.len(), 1);
        assert_eq!(doc.additions, 1);
        assert_eq!(doc.deletions, 1);
        assert_eq!(doc.change.as_ref().unwrap().status, FileStatus::Modified);

        let hunk = &doc.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.new_start, 1);
        assert!(hunk.header.starts_with("@@ -1,5 +1,5 @@"));
        let changed: Vec<&DiffLine> = hunk
            .lines
            .iter()
            .filter(|line| line.kind != DiffLineKind::Context)
            .collect();
        assert_eq!(changed[0].kind, DiffLineKind::Removed);
        assert_eq!(changed[0].text, "three");
        assert_eq!(changed[0].old_lineno, Some(3));
        assert_eq!(changed[1].kind, DiffLineKind::Added);
        assert_eq!(changed[1].text, "THREE");
        assert_eq!(changed[1].new_lineno, Some(3));
    }

    #[test]
    fn staged_document_uses_the_head_to_index_perspective() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();

        let staged = doc_for(&repo, "a.txt", DiffPerspective::IndexToBase);
        assert_eq!(staged.additions, 1);

        // The unstaged side is already covered by the index: no differences.
        let unstaged = doc_for(&repo, "a.txt", DiffPerspective::WorkingTreeToIndex);
        assert!(unstaged.change.is_none());
        assert!(unstaged.hunks.is_empty());
    }

    #[test]
    fn binary_and_mode_changes_render_without_hunks() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("data.bin"), [0u8, 1, 2, 0, 3, 4]).unwrap();
        std::fs::write(dir.join("run.sh"), "echo hi\n").unwrap();
        stage_paths(&repo, &["data.bin".to_string(), "run.sh".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("data.bin"), [0u8, 9, 9, 9, 3, 4]).unwrap();

        let binary = doc_for(&repo, "data.bin", DiffPerspective::WorkingTreeToIndex);
        assert!(binary.is_binary);
        assert!(binary.hunks.is_empty());

        // Mode change only: same content, new executable bit. Skipped where
        // the filesystem does not carry the executable bit (core.filemode).
        let filemode = repo
            .config()
            .ok()
            .and_then(|config| config.get_bool("core.filemode").ok())
            .unwrap_or(false);
        if filemode {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(dir.join("run.sh")).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(dir.join("run.sh"), perms).unwrap();

            let mode = doc_for(&repo, "run.sh", DiffPerspective::WorkingTreeToIndex);
            assert!(!mode.is_binary);
            assert!(mode.hunks.is_empty());
            let change = mode.change.unwrap();
            assert_eq!(
                change.mode_change,
                Some(FileModeChange {
                    old: FileMode::Regular,
                    new: FileMode::Executable,
                })
            );
        }
    }

    #[test]
    fn unchanged_path_is_not_an_error() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        let doc = doc_for(&repo, "a.txt", DiffPerspective::WorkingTreeToBase);
        assert!(doc.change.is_none());
        assert!(!doc.is_binary);

        let missing = document(
            &repo,
            Some("does-not-exist.txt"),
            DiffPerspective::WorkingTreeToBase,
            &EngineDiffOptions::default(),
        )
        .unwrap();
        assert!(missing.change.is_none());
    }

    #[test]
    fn whitespace_ignoring_options_drop_the_hunk() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one   \ntwo\n").unwrap();

        let shown = doc_for(&repo, "a.txt", DiffPerspective::WorkingTreeToIndex);
        assert_eq!(shown.hunks.len(), 1);

        let ignored = document(
            &repo,
            Some("a.txt"),
            DiffPerspective::WorkingTreeToIndex,
            &EngineDiffOptions::new(DEFAULT_CONTEXT_LINES, WhitespaceMode::IgnoreEol),
        )
        .unwrap();
        assert!(ignored.hunks.is_empty());
    }

    #[test]
    fn hunks_are_addressable_for_chunk_operations() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        let body: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.join("a.txt"), &body).unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        let mut edited: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        edited[1] = "changed two\n".to_string();
        edited[17] = "changed eighteen\n".to_string();
        std::fs::write(dir.join("a.txt"), edited.concat()).unwrap();

        let refs = hunks(
            &repo,
            "a.txt",
            DiffPerspective::WorkingTreeToIndex,
            &EngineDiffOptions::default(),
        )
        .unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].index, 0);
        assert_eq!(refs[1].index, 1);
        assert_eq!(refs[0].path, "a.txt");
        assert_eq!(refs[0].perspective, DiffPerspective::WorkingTreeToIndex);
        assert_ne!(refs[0].old_start, refs[1].old_start);
    }

    #[test]
    fn save_patch_writes_a_parseable_patch() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        let dest = std::env::temp_dir().join(format!(
            "meridian-patch-{}-{}.patch",
            std::process::id(),
            "save"
        ));
        save_patch(&repo, &["a.txt".to_string()], &dest, false).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);

        assert!(text.contains("diff --git a/a.txt b/a.txt"));
        assert!(text.contains("+two"));
        // Regenerating with prefixes dropped produces the same hunks.
        let dest = std::env::temp_dir().join(format!(
            "meridian-patch-{}-{}.patch",
            std::process::id(),
            "omit"
        ));
        save_patch(&repo, &["a.txt".to_string()], &dest, true).unwrap();
        let bare = std::fs::read_to_string(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);
        assert!(bare.contains("--- a.txt"));
        assert!(!bare.contains("a/a.txt"));
    }

    #[test]
    fn apply_patch_round_trips_a_saved_patch() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();
        let dest = std::env::temp_dir().join(format!(
            "meridian-apply-{}-{}.patch",
            std::process::id(),
            "clean"
        ));
        save_patch(&repo, &["a.txt".to_string()], &dest, false).unwrap();
        let patch = std::fs::read(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);

        // Revert the file, then put the patch back.
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        apply_patch(&repo, &patch, PatchLocation::Workdir, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "one\nTWO\nthree\n"
        );

        // The same patch into the index only.
        apply_patch(&repo, &patch, PatchLocation::Index, false).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(Path::new("a.txt"), 0).unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        assert_eq!(blob.content(), b"one\nTWO\nthree\n");
    }

    #[test]
    fn apply_patch_refuses_a_patch_that_does_not_match() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();
        let dest = std::env::temp_dir().join(format!(
            "meridian-apply-{}-{}.patch",
            std::process::id(),
            "stale"
        ));
        save_patch(&repo, &["a.txt".to_string()], &dest, false).unwrap();
        let patch = std::fs::read(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);

        // The file moved on, so the preimage no longer matches.
        std::fs::write(dir.join("a.txt"), "totally\ndifferent\ncontent\n").unwrap();
        let err = apply_patch(&repo, &patch, PatchLocation::Workdir, false).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "totally\ndifferent\ncontent\n"
        );
    }

    #[test]
    fn apply_patch_three_way_merges_around_local_edits() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        let body: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.join("a.txt"), &body).unwrap();
        stage_paths(&repo, &["a.txt".to_string()]).unwrap();
        commit_all(&repo, "initial");

        // The patch changes an early line …
        let mut patched: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        patched[1] = "patched two\n".to_string();
        std::fs::write(dir.join("a.txt"), patched.concat()).unwrap();
        let dest = std::env::temp_dir().join(format!(
            "meridian-apply-{}-{}.patch",
            std::process::id(),
            "threeway"
        ));
        save_patch(&repo, &["a.txt".to_string()], &dest, false).unwrap();
        let patch = std::fs::read(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);

        // … while the working tree has meanwhile changed a line *inside the
        // patch's context window*, so the preimage no longer matches and only
        // a three-way merge can land it.
        let mut local: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        local[3] = "local four\n".to_string();
        std::fs::write(dir.join("a.txt"), local.concat()).unwrap();

        let clean = apply_patch(&repo, &patch, PatchLocation::Workdir, false);
        assert!(clean.is_err(), "a clean apply should not have applied");

        apply_patch(&repo, &patch, PatchLocation::Workdir, true).unwrap();
        let merged = std::fs::read_to_string(dir.join("a.txt")).unwrap();
        assert!(merged.contains("patched two"), "patch side lost: {merged}");
        assert!(merged.contains("local four"), "local side lost: {merged}");
        assert!(!merged.contains("<<<<<<<"), "unexpected conflict markers");
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(&[0xff, 0x00, 0x80]), "/wCA");
    }

    #[test]
    fn image_sides_read_committed_and_working_copies() {
        let repo = scratch_repo();
        let dir = repo.workdir().unwrap().to_path_buf();
        std::fs::write(dir.join("logo.png"), [1u8, 2, 3]).unwrap();
        stage_paths(&repo, &["logo.png".to_string()]).unwrap();
        commit_all(&repo, "initial");
        std::fs::write(dir.join("logo.png"), [4u8, 5, 6, 7]).unwrap();

        let old = image_data(&repo, "logo.png", ImageSide::Old).unwrap();
        assert_eq!(old.mime, "image/png");
        assert_eq!(old.size, 3);
        assert_eq!(old.data_base64, "AQID");

        let new = image_data(&repo, "logo.png", ImageSide::New).unwrap();
        assert_eq!(new.size, 4);
        assert_eq!(new.data_base64, "BAUGBw==");
    }

    /// libgit2's hunk header ends with the enclosing function line of the file,
    /// i.e. source code. A stale hunk is routine, and its error is logged, so
    /// none of that text may survive in the message, the params or the log line.
    #[test]
    fn a_stale_hunk_error_never_carries_source_text_from_the_header() {
        let source_line = "fn load_credentials(password: &str) -> Vec<u8> {";
        let target = HunkRef {
            path: "src/auth.rs".to_string(),
            perspective: DiffPerspective::WorkingTreeToIndex,
            index: 0,
            old_start: 8,
            old_lines: 7,
            new_start: 8,
            new_lines: 7,
            header: format!("@@ -8,7 +8,7 @@ {source_line}"),
        };
        let error = hunk_mismatch(&target);
        assert!(error.message.contains("@@ -8,7 +8,7 @@"));
        assert!(!error.message.contains("load_credentials"));
        assert_eq!(error.params["hunk"], "@@ -8,7 +8,7 @@");
        for value in error.params.values() {
            assert!(!value.contains("load_credentials"));
        }
        let (_, line) =
            crate::logging::outcome_line::<()>("wc_chunk_apply", Default::default(), &Err(error));
        assert!(!line.contains("load_credentials"), "log line: {line}");
        assert!(!line.contains("password"), "log line: {line}");
    }
}
