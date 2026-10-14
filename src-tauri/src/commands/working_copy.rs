//! The working-copy command surface (docs/03 §3, rows `wc_*`).
//!
//! Every verb of the evidence's working-copy area is declared here exactly
//! once as a [`Command`] — descriptor, pure `validate`, `execute` — and
//! exposed as a thin `#[tauri::command]` wrapper. The layers below
//! (`validation` → `engine`) own the git work; this file owns the argument
//! shapes, the refusals that are about *arguments* rather than about git, and
//! the events.
//!
//! Two output conventions, both from the evidence's own table:
//!
//! * a **read** answers with the full `WorkingTreeStatus` (rows that carry
//!   `oldPath` and submodule state, which the plain `StatusDelta` cannot);
//! * a **mutation** answers with the fresh `StatusDelta`, so the UI never
//!   issues a second `wc_status_load` after a stage/unstage/discard.
//!
//! `wc_snapshot_*`, `wc_merge_status_load`, `wc_conflicts_*` and `wc_undo_*`
//! belong to the snapshot and integrate modules (docs/03 §3 rows 121-128) and
//! are deliberately not declared here.

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx, Risk};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::status::{EntryStatus, StatusDelta};
use crate::domain::working_copy::{
    ChunkDirection, DiffDocument, DiffOptions, DiffPerspective, DiscardOptions, EntryDetail,
    HunkRef, IgnoreScope, ImageData, ImageSide, OpenTarget, PatchLocation, WorkingCopyEntry,
    WorkingCopyStatusOptions, WorkingTreeStatus, DEFAULT_CONTEXT_LINES,
};
use crate::engine;
use crate::events::{self, AppEvent};
use crate::state::AppState;
use crate::validation::{self, ValidateCx, Violation};

// ---------------------------------------------------------------------------
// Read commands
// ---------------------------------------------------------------------------

/// `wc_status_load` — the porcelain-v2-equivalent status (`GTGitStatusOption*`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcStatusLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub opts: WorkingCopyStatusOptions,
}

impl Command for WcStatusLoad {
    type Output = WorkingTreeStatus;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let entries = engine::status::load(cx.repo, &self.opts)?;
        Ok(WorkingTreeStatus::new(cx.repo_id.clone(), entries))
    }
}

/// `wc_entry_detail_load` — one entry's on-disk and index facts.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcEntryDetailLoad {
    pub repo: RepoId,
    pub path: String,
}

impl Command for WcEntryDetailLoad {
    type Output = EntryDetail;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::status::entry_detail(cx.repo, &self.path)
    }
}

/// `wc_diff_load` — one diff document, from any of the six perspectives.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcDiffLoad {
    pub repo: RepoId,
    #[serde(default)]
    pub path: Option<String>,
    pub perspective: DiffPerspective,
    #[serde(default)]
    pub opts: DiffOptions,
}

impl Command for WcDiffLoad {
    type Output = DiffDocument;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)?;
        if let Some(path) = &self.path {
            validation::working_copy::validate_paths(cx, std::slice::from_ref(path))?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let opts = engine::diff::EngineDiffOptions::from_wire(&self.opts);
        engine::diff::document(cx.repo, self.path.as_deref(), self.perspective, &opts)
    }
}

/// `wc_diff_hunks_load` — the hunk references a chunk operation takes back
/// (`FNGitDiffChunk`). The optional options only change how the hunks are
/// split; the coordinates always come from a document built the same way the
/// one the user clicked was.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcDiffHunksLoad {
    pub repo: RepoId,
    pub path: String,
    pub perspective: DiffPerspective,
    #[serde(default)]
    pub opts: DiffOptions,
}

impl Command for WcDiffHunksLoad {
    type Output = Vec<HunkRef>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let opts = engine::diff::EngineDiffOptions::from_wire(&self.opts);
        engine::diff::hunks(cx.repo, &self.path, self.perspective, &opts)
    }
}

/// `wc_image_data_load` — one side of an image comparison, base64 for IPC.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcImageDataLoad {
    pub repo: RepoId,
    pub path: String,
    pub side: ImageSide,
}

impl Command for WcImageDataLoad {
    type Output = ImageData;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::diff::image_data(cx.repo, &self.path, self.side)
    }
}

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// `wc_stage_paths` (`GTGitStageEntriesAction`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcStagePaths {
    pub repo: RepoId,
    pub paths: Vec<String>,
}

impl Command for WcStagePaths {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::stage_paths(cx.repo, &self.paths)?;
        changed(cx)
    }
}

/// `wc_stage_all` (`GTGitStageAllEntriesAction`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcStageAll {
    pub repo: RepoId,
}

impl Command for WcStageAll {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::stage_all(cx.repo)?;
        changed(cx)
    }
}

/// `wc_unstage_paths` (`GTGitUnstageEntriesAction`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcUnstagePaths {
    pub repo: RepoId,
    pub paths: Vec<String>,
}

impl Command for WcUnstagePaths {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::unstage_paths(cx.repo, &self.paths)?;
        changed(cx)
    }
}

/// `wc_unstage_all` (`GTGitUnstageAllEntriesAction`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcUnstageAll {
    pub repo: RepoId,
}

impl Command for WcUnstageAll {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::unstage_all(cx.repo)?;
        changed(cx)
    }
}

// ---------------------------------------------------------------------------
// Chunk (hunk) operations
// ---------------------------------------------------------------------------

/// `wc_chunk_apply` — stage or unstage one hunk (`stageChunk:` /
/// `unstageChunk:`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcChunkApply {
    pub repo: RepoId,
    pub hunk: HunkRef,
    pub direction: ChunkDirection,
    /// Must match the context the hunk was listed with, or the coordinates
    /// would address a different hunk; absent means the default.
    #[serde(default)]
    pub context_lines: Option<u32>,
}

impl Command for WcChunkApply {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.hunk.path))?;
        let path = self.hunk.path.clone();
        precondition(engine::status::is_conflicted(cx.repo, &path), || {
            Violation::new(
                ErrorCode::CheckoutConflicts,
                format!("{path} has conflicts; resolve them first"),
            )
            .with_param("path", path.clone())
        })
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::apply_chunk(
            cx.repo,
            &self.hunk,
            self.direction,
            self.context_lines.unwrap_or(DEFAULT_CONTEXT_LINES),
        )?;
        changed(cx)
    }
}

/// `wc_chunk_discard` — throw one hunk away (`discardChunk:`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcChunkDiscard {
    pub repo: RepoId,
    pub hunk: HunkRef,
    #[serde(default)]
    pub context_lines: Option<u32>,
}

impl Command for WcChunkDiscard {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.hunk.path))?;
        let path = self.hunk.path.clone();
        precondition(engine::status::is_conflicted(cx.repo, &path), || {
            Violation::new(
                ErrorCode::CheckoutConflicts,
                format!("{path} has conflicts; resolve them first"),
            )
            .with_param("path", path.clone())
        })
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::discard_chunk(
            cx.repo,
            &self.hunk,
            self.context_lines.unwrap_or(DEFAULT_CONTEXT_LINES),
        )?;
        changed(cx)
    }
}

// ---------------------------------------------------------------------------
// Discard
// ---------------------------------------------------------------------------

/// `wc_paths_discard` (`GTDiscardLocalChangesDialogueViewController`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcPathsDiscard {
    pub repo: RepoId,
    pub paths: Vec<String>,
    #[serde(default)]
    pub opts: DiscardOptions,
}

impl Command for WcPathsDiscard {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Destructive
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)?;
        for path in &self.paths {
            precondition(engine::status::is_conflicted(cx.repo, path), || {
                Violation::new(
                    ErrorCode::CheckoutConflicts,
                    format!("{path} has conflicts; resolve them before discarding"),
                )
                .with_param("path", path.clone())
            })?;
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::discard_paths(cx.repo, &self.paths, &self.opts)?;
        changed(cx)
    }
}

// ---------------------------------------------------------------------------
// Entry actions
// ---------------------------------------------------------------------------

/// `wc_entry_ignore` (`GTAddCustomIgnorePatternViewController`). A tracked
/// path cannot be ignored — that is the evidence's
/// `GTIgnoreTrackedFilesDialogueViewController` flow, which untracks first.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcEntryIgnore {
    pub repo: RepoId,
    pub path: String,
    pub scope: IgnoreScope,
    #[serde(default)]
    pub pattern: Option<String>,
}

impl Command for WcEntryIgnore {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))?;
        let path = self.path.clone();
        precondition(engine::status::is_tracked(cx.repo, &path), || {
            Violation::new(
                ErrorCode::InvalidRequest,
                format!("{path} is tracked; untrack it before ignoring it"),
            )
            .with_param("path", path.clone())
        })
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::add_ignore_pattern(
            cx.repo,
            &self.path,
            self.scope,
            self.pattern.as_deref(),
        )?;
        changed(cx)
    }
}

/// `wc_entry_assume_unchanged_set` (`GTGitSet/UnsetAssumeUnchangedEntriesAction`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcEntryAssumeUnchangedSet {
    pub repo: RepoId,
    pub paths: Vec<String>,
    pub value: bool,
}

impl Command for WcEntryAssumeUnchangedSet {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::set_assume_unchanged(cx.repo, &self.paths, self.value)?;
        changed(cx)
    }
}

/// `wc_entry_untrack` (`GTGitUntrackEntriesAction`) — the file stays on disk.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcEntryUntrack {
    pub repo: RepoId,
    pub paths: Vec<String>,
}

impl Command for WcEntryUntrack {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        engine::index::untrack_paths(cx.repo, &self.paths)?;
        changed(cx)
    }
}

/// `wc_entry_open` (`openInTerminal:` / `openInFinder:` and their editor and
/// diff-tool siblings). The spawned process is detached: the command answers
/// as soon as the launch succeeded.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcEntryOpen {
    pub repo: RepoId,
    pub path: String,
    pub target: OpenTarget,
}

impl Command for WcEntryOpen {
    type Output = ();

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, std::slice::from_ref(&self.path))
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let repo = cx.repo;
        let path = self.path.as_str();
        let full = repo
            .workdir()
            .ok_or_else(|| {
                ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
            })?
            .join(path);

        match self.target {
            OpenTarget::Editor => open_in_editor(&full)?,
            OpenTarget::Terminal => open_in_terminal(repo.workdir(), &full)?,
            OpenTarget::Finder => reveal_in_file_manager(&full)?,
            OpenTarget::DiffTool => open_in_diff_tool(repo, path)?,
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Patches
// ---------------------------------------------------------------------------

/// `wc_patch_save` (`GTGitSavePatchAction`,
/// `GTUserDefaultsSavePatchOmitPrefixes`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcPatchSave {
    pub repo: RepoId,
    pub paths: Vec<String>,
    /// Absolute, or relative to the working directory.
    pub dest: String,
    #[serde(default)]
    pub omit_prefixes: bool,
}

impl Command for WcPatchSave {
    type Output = ();

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_paths(cx, &self.paths)?;
        if self.dest.trim().is_empty() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "No destination given for the patch",
            ));
        }
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let dest = resolve_in_workdir(cx.repo, &self.dest);
        engine::diff::save_patch(cx.repo, &self.paths, &dest, self.omit_prefixes)
    }
}

/// A patch as it arrives over IPC: a byte array, or the base64 text the file
/// picker hands back. Both shapes are accepted because the web layer has no
/// reason to care which it sends.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum PatchSource {
    Bytes(Vec<u8>),
    Base64(String),
}

impl PatchSource {
    fn into_bytes(self) -> Result<Vec<u8>, ErrorPayload> {
        match self {
            PatchSource::Bytes(bytes) => Ok(bytes),
            PatchSource::Base64(text) => base64_decode(&text),
        }
    }
}

/// `wc_patch_apply` (`GTGitApplyPatchOperation`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WcPatchApply {
    pub repo: RepoId,
    pub patch: PatchSource,
    #[serde(default = "default_patch_location")]
    pub location: PatchLocation,
    #[serde(default)]
    pub three_way: bool,
}

fn default_patch_location() -> PatchLocation {
    PatchLocation::Workdir
}

impl Command for WcPatchApply {
    type Output = StatusDelta;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        Risk::Destructive
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::working_copy::validate_working_copy(cx)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let bytes = self.patch.into_bytes()?;
        if bytes.is_empty() {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "The patch is empty",
            ));
        }
        engine::diff::apply_patch(cx.repo, &bytes, self.location, self.three_way)?;
        changed(cx)
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Every working-copy mutation answers with the fresh delta and announces the
/// change, so no window ever has to ask whether it is looking at stale state.
fn changed(cx: &ExecCx<'_>) -> Result<StatusDelta, ErrorPayload> {
    events::emit(
        cx.app,
        AppEvent::WorkingCopyChanged {
            repo: cx.repo_id.clone(),
        },
    );
    let entries = engine::status::load(cx.repo, &WorkingCopyStatusOptions::default())?;
    Ok(delta_of(cx, entries))
}

/// The mutation envelope. Rows without a status code are clean rows — they only
/// appear when the caller asked for them, and a delta is by definition "what
/// changed", so they are dropped here.
fn delta_of(cx: &ExecCx<'_>, entries: Vec<WorkingCopyEntry>) -> StatusDelta {
    let rows: Vec<EntryStatus> = entries
        .iter()
        .filter_map(|entry| {
            entry.code.map(|code| {
                EntryStatus::new(
                    entry.path.as_str(),
                    code,
                    entry.staged,
                    entry.assume_unchanged,
                )
            })
        })
        .collect();
    let summary = crate::domain::working_copy::summary_of(&entries);
    StatusDelta {
        repo: cx.repo_id.clone(),
        entries: rows,
        summary,
    }
}

fn resolve_in_workdir(repo: &git2::Repository, dest: &str) -> PathBuf {
    let candidate = Path::new(dest);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        repo.workdir()
            .map(|dir| dir.join(candidate))
            .unwrap_or_else(|| candidate.to_path_buf())
    }
}

// ---------------------------------------------------------------------------
// Launching external tools
// ---------------------------------------------------------------------------

/// The external tools `wc_entry_open` can launch.
///
/// The settings module owns the user's choices (docs/03 §3: "configured tool
/// exists, else typed error"); until it lands these are read from the
/// environment, which is what the same setting writes when Meridian is launched
/// from a shell. `None` means "not configured", which becomes a typed error
/// rather than a silent no-op.
struct ToolConfig {
    editor: Option<String>,
    terminal: Option<String>,
    diff_tool: Option<String>,
}

impl ToolConfig {
    fn read() -> Self {
        Self {
            editor: first_non_empty_env(&["MERIDIAN_EDITOR", "VISUAL", "EDITOR"]),
            terminal: first_non_empty_env(&["MERIDIAN_TERMINAL"]),
            diff_tool: first_non_empty_env(&["MERIDIAN_DIFF_TOOL"]),
        }
    }
}

fn first_non_empty_env(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn spawn_detached(program: &str, args: &[String]) -> Result<(), ErrorPayload> {
    ProcessCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|err| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("Could not launch {program}"),
            )
            .with_detail(err.to_string())
            .with_param("program", program)
        })
}

/// Split a configured command line ("code -w") into program and arguments.
fn split_command(line: &str) -> (String, Vec<String>) {
    let mut parts = line.split_whitespace().map(str::to_string);
    let program = parts.next().unwrap_or_default();
    (program, parts.collect())
}

fn open_in_editor(full: &Path) -> Result<(), ErrorPayload> {
    let config = ToolConfig::read();
    if let Some(line) = config.editor {
        let (program, args) = split_command(&line);
        let mut argv = args;
        argv.push(full.to_string_lossy().into_owned());
        return spawn_detached(&program, &argv);
    }

    // No editor configured: the platform's own "open with the default
    // application for this type" is the closest thing that always works.
    if cfg!(target_os = "macos") {
        return spawn_detached(
            "open",
            &["-t".to_string(), full.to_string_lossy().into_owned()],
        );
    }
    Err(not_configured(
        "editor",
        "Set MERIDIAN_EDITOR (or VISUAL/EDITOR) to the command that opens files",
    ))
}

fn open_in_terminal(workdir: Option<&Path>, fallback: &Path) -> Result<(), ErrorPayload> {
    let directory = workdir.unwrap_or_else(|| fallback.parent().unwrap_or(fallback));

    if let Some(line) = ToolConfig::read().terminal {
        let (program, args) = split_command(&line);
        let mut argv = args;
        argv.push(directory.to_string_lossy().into_owned());
        return spawn_detached(&program, &argv);
    }

    if cfg!(target_os = "macos") {
        return spawn_detached(
            "open",
            &[
                "-a".to_string(),
                "Terminal".to_string(),
                directory.to_string_lossy().into_owned(),
            ],
        );
    }
    Err(not_configured(
        "terminal",
        "Set MERIDIAN_TERMINAL to the terminal command to launch",
    ))
}

fn reveal_in_file_manager(full: &Path) -> Result<(), ErrorPayload> {
    let shown = full.to_string_lossy().into_owned();
    if cfg!(target_os = "macos") {
        // `-R` selects the file inside Finder; a directory is opened instead.
        let args = if full.is_dir() {
            vec![shown.clone()]
        } else {
            vec!["-R".to_string(), shown.clone()]
        };
        return spawn_detached("open", &args);
    }
    if cfg!(target_os = "windows") {
        return spawn_detached("explorer", &[format!("/select,{shown}")]);
    }
    let parent = full.parent().unwrap_or(full);
    spawn_detached("xdg-open", &[parent.to_string_lossy().into_owned()])
}

/// The external diff tool of `GIT_EXTERNAL_DIFF` shape: it is launched with
/// the pre-image and the post-image as its two arguments, so both sides are
/// materialised next to each other first. They live in a fresh temp directory
/// so a later clean or discard cannot leave a stale copy behind; the tool is
/// expected to copy them out if the user edits them.
fn open_in_diff_tool(repo: &git2::Repository, path: &str) -> Result<(), ErrorPayload> {
    let Some(line) = ToolConfig::read().diff_tool else {
        return Err(not_configured(
            "diff tool",
            "Set MERIDIAN_DIFF_TOOL to the external diff command",
        ));
    };

    let workdir = repo.workdir().ok_or_else(|| {
        ErrorPayload::new(ErrorCode::NoWorkingCopy, "Repository has no working copy")
    })?;

    let directory = temp_diff_dir(path)?;
    let base_path = directory.join("base");
    let new_path = directory.join("new");

    if let Some(base) = base_side_bytes(repo, path)? {
        std::fs::write(&base_path, base)
            .map_err(io_error(base_path.to_string_lossy().into_owned()))?;
    }
    if let Ok(new) = std::fs::read(workdir.join(path)) {
        std::fs::write(&new_path, new)
            .map_err(io_error(new_path.to_string_lossy().into_owned()))?;
    }

    let (program, args) = split_command(&line);
    let mut argv = args;
    argv.push(base_path.to_string_lossy().into_owned());
    argv.push(new_path.to_string_lossy().into_owned());
    spawn_detached(&program, &argv)
}

/// The pre-image side: HEAD's copy, or the index's when the file is not
/// committed yet. `None` means the file is new — the tool then sees an empty
/// base file.
fn base_side_bytes(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, ErrorPayload> {
    if let Ok(head) = repo.head() {
        if let Ok(commit) = head.peel_to_commit() {
            if let Ok(tree) = commit.tree() {
                if let Ok(entry) = tree.get_path(Path::new(path)) {
                    if let Ok(blob) = repo.find_blob(entry.id()) {
                        return Ok(Some(blob.content().to_vec()));
                    }
                }
            }
        }
    }
    let index = repo.index()?;
    Ok(index
        .get_path(Path::new(path), 0)
        .and_then(|entry| repo.find_blob(entry.id).ok())
        .map(|blob| blob.content().to_vec()))
}

fn temp_diff_dir(path: &str) -> Result<PathBuf, ErrorPayload> {
    let stem = Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "diff".to_string());
    let directory = std::env::temp_dir().join(format!("meridian-diff-{stem}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory)
        .map_err(io_error(directory.to_string_lossy().into_owned()))?;
    Ok(directory)
}

fn io_error(path: impl Into<String>) -> impl FnOnce(std::io::Error) -> ErrorPayload {
    let shown = path.into();
    move |err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write {shown}"))
            .with_detail(err.to_string())
    }
}

fn not_configured(what: &str, hint: &str) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::InvalidRequest,
        format!("No {what} is configured"),
    )
    .with_detail(hint)
    .with_param("setting", what)
}

/// A validator reads the repository through the engine, which reports failures
/// as [`ErrorPayload`]; the boundary turns them into [`Violation`]s.
fn precondition(
    result: Result<bool, ErrorPayload>,
    refusal: impl FnOnce() -> Violation,
) -> Result<(), Violation> {
    match result {
        Ok(false) => Ok(()),
        Ok(true) => Err(refusal()),
        Err(payload) => Err(Violation::new(payload.code, payload.message)),
    }
}

// ---------------------------------------------------------------------------
// base64
// ---------------------------------------------------------------------------

/// Standard-alphabet base64 with padding — the same shape `ImageData` ships
/// its bytes in.
fn base64_decode(text: &str) -> Result<Vec<u8>, ErrorPayload> {
    fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
            b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let cleaned: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
        .collect();

    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    let mut accumulator: u32 = 0;
    let mut bits = 0u32;
    for byte in cleaned {
        let Some(six) = value(byte) else {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "The patch is not valid base64",
            ));
        };
        accumulator = (accumulator << 6) | six;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tauri entry points
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn wc_status_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcStatusLoad,
) -> Result<WorkingTreeStatus, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_entry_detail_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcEntryDetailLoad,
) -> Result<EntryDetail, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_stage_paths(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcStagePaths,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_stage_all(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcStageAll,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_unstage_paths(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcUnstagePaths,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_unstage_all(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcUnstageAll,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_chunk_apply(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcChunkApply,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_chunk_discard(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcChunkDiscard,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_paths_discard(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcPathsDiscard,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_entry_ignore(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcEntryIgnore,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_entry_assume_unchanged_set(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcEntryAssumeUnchangedSet,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_entry_untrack(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcEntryUntrack,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_entry_open(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcEntryOpen,
) -> Result<(), ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_diff_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcDiffLoad,
) -> Result<DiffDocument, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_diff_hunks_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcDiffHunksLoad,
) -> Result<Vec<HunkRef>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_image_data_load(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcImageDataLoad,
) -> Result<ImageData, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_patch_save(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcPatchSave,
) -> Result<(), ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn wc_patch_apply(
    state: State<'_, AppState>,
    app: AppHandle,
    args: WcPatchApply,
) -> Result<StatusDelta, ErrorPayload> {
    state.run(&app, args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("").unwrap(), b"");
        assert!(base64_decode("not base64!").is_err());
    }

    #[test]
    fn patch_source_carries_both_wire_shapes() {
        let from_bytes = PatchSource::Bytes(b"hi".to_vec());
        assert_eq!(from_bytes.into_bytes().unwrap(), b"hi");

        let from_text = PatchSource::Base64("aGk=".to_string());
        assert_eq!(from_text.into_bytes().unwrap(), b"hi");
    }

    #[test]
    fn a_configured_command_line_splits_into_program_and_arguments() {
        let (program, args) = split_command("code -w --reuse-window");
        assert_eq!(program, "code");
        assert_eq!(args, vec!["-w".to_string(), "--reuse-window".to_string()]);
    }
}
