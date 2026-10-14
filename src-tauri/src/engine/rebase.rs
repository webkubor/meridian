//! Rebase, the interactive todo engine, and the sequence machine shared with
//! cherry-pick / revert (`GTGitRebaseAction`,
//! `GTGitInteractiveRebaseMergeProcessInfo`, `GTGitRestackBranchOperation`).
//!
//! ## Why a sequence file instead of a native rebase
//!
//! libgit2's `Rebase` replays a fixed, linear range: it cannot reorder, drop,
//! squash or reword. Git answers that with the sequencer — a shell loop over a
//! todo file — so Meridian does the same in Rust and keeps its todo in
//! `<gitdir>/meridian/sequence`. Consequences worth stating plainly:
//!
//! - **Abort and continue read the sequence file, not git state.** A rebase
//!   paused here is a `CHERRY_PICK_HEAD` (each step *is* a cherry-pick) plus
//!   this file; [`super::merge::integration_state`] consults the file first so
//!   the UI is told "rebase", not "cherry-pick".
//! - **The file is not git data.** It lives outside refs and objects
//!   (docs/02 §9) and is deleted the moment an operation finishes, aborts or
//!   is superseded.
//! - Replay uses the real `Repository::cherrypick`, so conflict markers,
//!   `MERGE_MSG` and index stages are exactly what git produces.

use std::io::Write;

use git2::{build::CheckoutBuilder, Oid, ResetType, Sort};

use crate::domain::conflict::{
    IntegrationOutcome, IntegrationResult, PlanStep, SequenceAction, SequencePlan,
};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::history::Signature;
use crate::domain::repository::IntegrationState;
use crate::domain::revision::Revision;

use super::history;
use super::merge;

/// Which operation owns the sequence. One file format, four vocabularies — the
/// merge-process model is unified (evidence F5), so `integration_continue`
/// does not need to know which engine wrote the todo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceKind {
    /// `rebase_start`: a plain linear replay onto upstream.
    Rebase,
    /// `rebase_interactive_start`: a caller-supplied todo.
    Interactive,
    /// A cherry-pick sequence (docs/03 §13 `cherry_pick` with several revs).
    CherryPick,
    /// A revert sequence.
    Revert,
}

impl SequenceKind {
    fn token(self) -> &'static str {
        match self {
            SequenceKind::Rebase => "Rebase",
            SequenceKind::Interactive => "Interactive",
            SequenceKind::CherryPick => "CherryPick",
            SequenceKind::Revert => "Revert",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "Rebase" => Some(SequenceKind::Rebase),
            "Interactive" => Some(SequenceKind::Interactive),
            "CherryPick" => Some(SequenceKind::CherryPick),
            "Revert" => Some(SequenceKind::Revert),
            _ => None,
        }
    }

    pub fn integration_state(self) -> IntegrationState {
        match self {
            SequenceKind::Rebase => IntegrationState::Rebase,
            SequenceKind::Interactive => IntegrationState::RebaseInteractive,
            SequenceKind::CherryPick => IntegrationState::CherryPick,
            SequenceKind::Revert => IntegrationState::Revert,
        }
    }
}

/// Why a sequence is waiting for the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseKind {
    /// A replay step could not be applied cleanly: resolve, then continue.
    Conflict,
    /// An `edit` action: the commit is made and stopped on, so the user can
    /// amend or split it.
    Edit,
}

impl PauseKind {
    fn token(self) -> &'static str {
        match self {
            PauseKind::Conflict => "Conflict",
            PauseKind::Edit => "Edit",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "Conflict" => Some(PauseKind::Conflict),
            "Edit" => Some(PauseKind::Edit),
            _ => None,
        }
    }
}

/// The whole persisted rebase/cherry-pick/revert state.
#[derive(Clone, Debug)]
pub struct SequenceState {
    pub kind: SequenceKind,
    /// HEAD before the operation started — what abort restores.
    pub original_head: Oid,
    /// Checked-out branch (short name) before the operation, `None` when HEAD
    /// was detached. The finish and abort paths re-attach it.
    pub original_head_name: Option<String>,
    pub upstream: Option<Oid>,
    pub onto: Option<Oid>,
    /// Steps left to run, in order. The paused step (if any) is *not* here.
    pub steps: Vec<PlanStep>,
    pub applied: Vec<Oid>,
    pub dropped: Vec<Oid>,
    pub skipped: Vec<Oid>,
    pub pause: Option<PauseKind>,
    /// The step we are paused on.
    pub paused_step: Option<PlanStep>,
    /// The message of the current HEAD commit — the base a `squash`/`fixup`
    /// extends. Kept so a combined message survives a conflict pause.
    pub head_message: Option<String>,
    /// Cherry-pick/revert only: stage the results and do not commit
    /// (`--no-commit`). The sequence still runs, then unwinds with a soft
    /// reset so the net effect is a fully staged change.
    pub no_commit: bool,
    /// Cherry-pick only: append the `(cherry picked from commit …)` trailer.
    pub record_origin: bool,
    /// Cherry-pick/revert only: the parent to diff a merge commit against.
    pub mainline: u32,
}

impl SequenceState {
    pub fn integration_state(&self) -> IntegrationState {
        self.kind.integration_state()
    }
}

// ---------------------------------------------------------------------------
// State file (<gitdir>/meridian/sequence)
// ---------------------------------------------------------------------------

fn sequence_path(repo: &git2::Repository) -> std::path::PathBuf {
    merge::meridian_dir(repo).join("sequence")
}

pub fn sequence_present(repo: &git2::Repository) -> bool {
    sequence_path(repo).exists()
}

fn hex_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_decode(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    String::from_utf8(out).ok()
}

fn action_token(action: SequenceAction) -> &'static str {
    match action {
        SequenceAction::Pick => "pick",
        SequenceAction::Reword => "reword",
        SequenceAction::Squash => "squash",
        SequenceAction::Fixup => "fixup",
        SequenceAction::Edit => "edit",
        SequenceAction::Drop => "drop",
    }
}

fn action_from(token: &str) -> Option<SequenceAction> {
    match token {
        "pick" => Some(SequenceAction::Pick),
        "reword" => Some(SequenceAction::Reword),
        "squash" => Some(SequenceAction::Squash),
        "fixup" => Some(SequenceAction::Fixup),
        "edit" => Some(SequenceAction::Edit),
        "drop" => Some(SequenceAction::Drop),
        _ => None,
    }
}

fn oid_field(oid: Oid) -> String {
    oid.to_string()
}

fn opt_oid(oid: Option<Oid>) -> String {
    oid.map(oid_field).unwrap_or_else(|| "-".to_string())
}

fn text_field(text: Option<&str>) -> String {
    match text {
        Some(text) if !text.is_empty() => hex_encode(text),
        _ => "-".to_string(),
    }
}

fn opt_text(field: &str) -> Option<String> {
    if field == "-" {
        None
    } else {
        hex_decode(field)
    }
}

/// Serialize [`SequenceState`]. Lines are space-separated with every free-text
/// field hex-encoded, so a message with newlines, tabs or unicode round-trips
/// byte-exactly.
fn encode(state: &SequenceState) -> String {
    let mut out = String::new();
    out.push_str("v1\n");
    out.push_str(&format!("kind {}\n", state.kind.token()));
    out.push_str(&format!("orig {}\n", oid_field(state.original_head)));
    out.push_str(&format!(
        "orig_name {}\n",
        text_field(state.original_head_name.as_deref())
    ));
    out.push_str(&format!("upstream {}\n", opt_oid(state.upstream)));
    out.push_str(&format!("onto {}\n", opt_oid(state.onto)));
    out.push_str(&format!(
        "pause {}\n",
        state.pause.map(PauseKind::token).unwrap_or("-")
    ));
    out.push_str(&format!(
        "head_message {}\n",
        text_field(state.head_message.as_deref())
    ));
    out.push_str(&format!("no_commit {}\n", u8::from(state.no_commit)));
    out.push_str(&format!(
        "record_origin {}\n",
        u8::from(state.record_origin)
    ));
    out.push_str(&format!("mainline {}\n", state.mainline));
    if let Some(step) = &state.paused_step {
        out.push_str(&format!(
            "paused {} {} {}\n",
            action_token(step.action),
            step.oid,
            text_field(step.new_message.as_deref())
        ));
    }
    for step in &state.steps {
        out.push_str(&format!(
            "step {} {} {}\n",
            action_token(step.action),
            step.oid,
            text_field(step.new_message.as_deref())
        ));
    }
    for oid in &state.applied {
        out.push_str(&format!("applied {}\n", oid_field(*oid)));
    }
    for oid in &state.dropped {
        out.push_str(&format!("dropped {}\n", oid_field(*oid)));
    }
    for oid in &state.skipped {
        out.push_str(&format!("skipped {}\n", oid_field(*oid)));
    }
    out
}

fn decode(raw: &str) -> Option<SequenceState> {
    let mut kind = None;
    let mut original_head = None;
    let mut original_head_name = None;
    let mut upstream = None;
    let mut onto = None;
    let mut pause = None;
    let mut head_message = None;
    let mut paused_step = None;
    let mut steps = Vec::new();
    let mut applied = Vec::new();
    let mut dropped = Vec::new();
    let mut skipped = Vec::new();
    let mut no_commit = false;
    let mut record_origin = false;
    let mut mainline = 0u32;

    for line in raw.lines().skip(1) {
        let mut parts = line.split(' ');
        match parts.next()? {
            "kind" => kind = SequenceKind::parse(parts.next()?),
            "orig" => original_head = Oid::from_str(parts.next()?).ok(),
            "orig_name" => original_head_name = opt_text(parts.next()?),
            "upstream" => {
                let field = parts.next()?;
                upstream = if field == "-" {
                    None
                } else {
                    Oid::from_str(field).ok()
                };
            }
            "onto" => {
                let field = parts.next()?;
                onto = if field == "-" {
                    None
                } else {
                    Oid::from_str(field).ok()
                };
            }
            "pause" => {
                let field = parts.next()?;
                pause = if field == "-" {
                    None
                } else {
                    PauseKind::parse(field)
                };
            }
            "head_message" => head_message = opt_text(parts.next()?),
            "no_commit" => no_commit = parts.next()? == "1",
            "record_origin" => record_origin = parts.next()? == "1",
            "mainline" => mainline = parts.next()?.parse().ok()?,
            "paused" => {
                let action = action_from(parts.next()?)?;
                let oid = parts.next()?;
                let message = opt_text(parts.next()?);
                paused_step = Some(PlanStep {
                    action,
                    oid: oid.to_string(),
                    new_message: message,
                    ..PlanStep::default()
                });
            }
            "step" => {
                let action = action_from(parts.next()?)?;
                let oid = parts.next()?;
                let message = opt_text(parts.next()?);
                steps.push(PlanStep {
                    action,
                    oid: oid.to_string(),
                    new_message: message,
                    ..PlanStep::default()
                });
            }
            "applied" => applied.push(Oid::from_str(parts.next()?).ok()?),
            "dropped" => dropped.push(Oid::from_str(parts.next()?).ok()?),
            "skipped" => skipped.push(Oid::from_str(parts.next()?).ok()?),
            _ => {}
        }
    }

    Some(SequenceState {
        kind: kind?,
        original_head: original_head?,
        original_head_name,
        upstream,
        onto,
        steps,
        applied,
        dropped,
        skipped,
        pause,
        paused_step,
        head_message,
        no_commit,
        record_origin,
        mainline,
    })
}

/// Read the current sequence, if one is in flight.
pub fn sequence_state(repo: &git2::Repository) -> Option<SequenceState> {
    let raw = std::fs::read_to_string(sequence_path(repo)).ok()?;
    decode(&raw)
}

fn write_sequence(repo: &git2::Repository, state: &SequenceState) -> Result<(), ErrorPayload> {
    let dir = merge::meridian_dir(repo);
    std::fs::create_dir_all(&dir).map_err(|err| {
        ErrorPayload::new(
            ErrorCode::Io,
            format!("Could not create {}: {err}", dir.display()),
        )
    })?;
    let path = sequence_path(repo);
    let mut file = std::fs::File::create(&path).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write {path:?}: {err}"))
    })?;
    file.write_all(encode(state).as_bytes()).map_err(|err| {
        ErrorPayload::new(ErrorCode::Io, format!("Could not write {path:?}: {err}"))
    })?;
    Ok(())
}

fn clear_sequence(repo: &git2::Repository) {
    let _ = std::fs::remove_file(sequence_path(repo));
}

// ---------------------------------------------------------------------------
// Plan computation
// ---------------------------------------------------------------------------

/// Commits reachable from `from` but not from `hide`, oldest first — the range
/// an interactive todo is built from. Topological order so a reorder produced
/// by the UI is already parent-before-child.
pub fn range_commits(
    repo: &git2::Repository,
    from: Oid,
    hide: Option<Oid>,
) -> Result<Vec<Oid>, ErrorPayload> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL)?;
    walk.push(from)?;
    if let Some(hide) = hide {
        walk.hide(hide)?;
    }
    let mut oids = Vec::new();
    for oid in walk {
        oids.push(oid?);
    }
    oids.reverse();
    Ok(oids)
}

fn step_for(
    repo: &git2::Repository,
    oid: Oid,
    action: SequenceAction,
) -> Result<PlanStep, ErrorPayload> {
    let commit = repo.find_commit(oid)?;
    let author = commit.author();
    Ok(PlanStep {
        action,
        oid: oid.to_string(),
        short_oid: merge::short(oid),
        summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
        message: commit.message().unwrap_or("").to_string(),
        author: Some(signature_of(&author)),
        new_message: None,
    })
}

/// `git2::Signature` → the domain shape. `domain` cannot name `git2`, so the
/// mapping lives here (docs/02 §1).
pub fn signature_of(sig: &git2::Signature<'_>) -> Signature {
    Signature {
        name: sig.name().unwrap_or("").to_string(),
        email: sig.email().unwrap_or("").to_string(),
        time: sig.when().seconds(),
    }
}

/// `rebase_interactive_plan` (docs/03 §13): compute the todo, change nothing.
pub fn plan(
    repo: &git2::Repository,
    upstream: &Revision,
    onto: Option<&Revision>,
) -> Result<SequencePlan, ErrorPayload> {
    let upstream_commit = history::resolve(repo, upstream)?;
    let onto_commit = match onto {
        Some(onto) => history::resolve(repo, onto)?,
        None => upstream_commit.clone(),
    };
    let head = merge::head_commit(repo)?;
    let oids = range_commits(repo, head.id(), Some(upstream_commit.id()))?;

    let mut steps = Vec::with_capacity(oids.len());
    for oid in oids {
        steps.push(step_for(repo, oid, SequenceAction::Pick)?);
    }

    Ok(SequencePlan {
        upstream: revision_label(upstream, upstream_commit.id()),
        onto: revision_label(onto.unwrap_or(upstream), onto_commit.id()),
        steps,
    })
}

fn revision_label(revision: &Revision, oid: Oid) -> String {
    match revision {
        Revision::Head => "HEAD".to_string(),
        Revision::Branch(name) => name.clone(),
        Revision::Tag(name) => name.clone(),
        Revision::Oid(_) => merge::short(oid),
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

/// Options `rebase_start` / `rebase_interactive_start` share.
#[derive(Clone, Copy, Debug, Default)]
pub struct RebaseRequest {
    pub autostash: bool,
    pub verify_signatures: bool,
}

/// `rebase_start` (docs/03 §13): replay HEAD's own commits onto `onto`
/// (default: `upstream`).
pub fn start(
    repo: &git2::Repository,
    upstream: &Revision,
    onto: Option<&Revision>,
    req: &RebaseRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    let upstream_commit = history::resolve(repo, upstream)?;
    let onto_id = match onto {
        Some(onto) => history::resolve(repo, onto)?.id(),
        None => upstream_commit.id(),
    };

    let plan = plan(repo, upstream, onto)?;
    let steps: Vec<PlanStep> = plan
        .steps
        .into_iter()
        .map(|step| PlanStep {
            action: SequenceAction::Pick,
            ..step
        })
        .collect();

    start_plan_for_kind(
        repo,
        SequenceKind::Rebase,
        Some(upstream_commit.id()),
        onto_id,
        steps,
        req,
    )
}

/// `rebase_interactive_start` (docs/03 §13): run a caller-supplied todo. The
/// plan's `onto` is honoured; `upstream` bounds the pick set.
pub fn start_interactive(
    repo: &git2::Repository,
    plan: &SequencePlan,
    req: &RebaseRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    let upstream = resolve_plan_ref(repo, &plan.upstream)?;
    let onto = resolve_plan_ref(repo, &plan.onto).unwrap_or(upstream);

    // Every step's oid must be a real commit and lie in the pick range; a todo
    // that names an unrelated commit would replay arbitrary history.
    let head = merge::head_commit(repo)?;
    let allowed: std::collections::HashSet<Oid> = range_commits(repo, head.id(), Some(upstream))?
        .into_iter()
        .collect();
    for step in &plan.steps {
        let oid = Oid::from_str(&step.oid).map_err(|_| {
            ErrorPayload::new(ErrorCode::UnknownRevision, "Todo step is not a commit id")
                .with_param("oid", &step.oid)
        })?;
        if !allowed.contains(&oid) {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "Todo step is outside the rebase range",
            )
            .with_param("oid", &step.oid));
        }
    }

    start_plan_for_kind(
        repo,
        SequenceKind::Interactive,
        Some(upstream),
        onto,
        plan.steps.clone(),
        req,
    )
}

fn resolve_plan_ref(repo: &git2::Repository, spec: &str) -> Result<Oid, ErrorPayload> {
    // Accept a short branch name, a full ref, or an oid — git's own rebase
    // arguments are equally loose.
    let revision = if spec == "HEAD" {
        Revision::Head
    } else if Oid::from_str(spec).is_ok() {
        Revision::Oid(spec.to_string())
    } else {
        Revision::Branch(spec.to_string())
    };
    Ok(history::resolve(repo, &revision)?.id())
}

fn start_plan_for_kind(
    repo: &git2::Repository,
    kind: SequenceKind,
    upstream: Option<Oid>,
    onto: Oid,
    steps: Vec<PlanStep>,
    req: &RebaseRequest,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }
    if req.verify_signatures {
        for step in &steps {
            if let Ok(oid) = Oid::from_str(&step.oid) {
                if repo.extract_signature(&oid, None).is_err() {
                    return Err(ErrorPayload::new(
                        ErrorCode::InvalidRequest,
                        "A commit in the plan is not signed; signature verification was requested",
                    )
                    .with_param("oid", &step.oid));
                }
            }
        }
    }

    let head = merge::head_commit(repo)?;
    let original_head = head.id();

    // Nothing to replay and nothing to move: already up to date.
    if steps.is_empty() {
        let base = repo.merge_base(onto, original_head)?;
        if base == onto {
            return Ok(IntegrationResult::new(IntegrationOutcome::UpToDate {
                head: original_head.to_string(),
            }));
        }
        // Fast-forward: HEAD is an ancestor of onto, so no rewrite happens.
        let mut notes = Vec::new();
        merge::autostash_save(repo, req.autostash, "rebase", &mut notes)?;
        merge::write_pseudoref(repo, "ORIG_HEAD", original_head)?;
        merge::fast_forward(repo, onto)?;
        merge::autostash_restore(repo, &mut notes)?;
        return Ok(IntegrationResult::new(IntegrationOutcome::FastForward {
            head: onto.to_string(),
            from: original_head.to_string(),
        })
        .with_note(
            notes.into_iter().next().unwrap_or_else(|| {
                "Rebase fast-forwarded; no commits needed replaying.".to_string()
            }),
        ));
    }

    let mut notes = Vec::new();
    merge::autostash_save(repo, req.autostash, "rebase", &mut notes)?;
    merge::write_pseudoref(repo, "ORIG_HEAD", original_head)?;

    // Detach at `onto`. From here every step is committed onto a detached
    // HEAD; the branch moves once, at finish.
    let onto_obj = repo.find_object(onto, None)?;
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(&onto_obj, Some(&mut checkout))?;
    repo.set_head_detached(onto)?;

    let original_head_name = branch_name(repo);

    let head_message = repo
        .find_commit(onto)?
        .message()
        .ok()
        .map(|message| message.to_string());

    let mut state = SequenceState {
        kind,
        original_head,
        original_head_name,
        upstream,
        onto: Some(onto),
        steps,
        applied: Vec::new(),
        dropped: Vec::new(),
        skipped: Vec::new(),
        pause: None,
        paused_step: None,
        head_message,
        no_commit: false,
        record_origin: false,
        mainline: 0,
    };

    run(repo, &mut state, notes)
}

/// The checked-out branch's short name, or `None` when HEAD is detached.
fn branch_name(repo: &git2::Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().ok().map(|shorthand| shorthand.to_string())
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Run (or resume) the sequence until it finishes or pauses. Notes are carried
/// through so an autostash decision survives the whole operation.
fn run(
    repo: &git2::Repository,
    state: &mut SequenceState,
    mut notes: Vec<String>,
) -> Result<IntegrationResult, ErrorPayload> {
    loop {
        if state.pause.is_some() {
            write_sequence(repo, state)?;
            return Ok(paused_result(repo, state, notes));
        }

        let Some(step) = state.steps.first().cloned() else {
            return finish(repo, state, notes);
        };
        state.steps.remove(0);

        if step.action == SequenceAction::Drop {
            if let Ok(oid) = Oid::from_str(&step.oid) {
                state.dropped.push(oid);
            }
            write_sequence(repo, state)?;
            continue;
        }

        apply_step(repo, state, &step, &mut notes)?;
    }
}

fn paused_result(
    repo: &git2::Repository,
    state: &SequenceState,
    notes: Vec<String>,
) -> IntegrationResult {
    let result = match state.pause {
        Some(PauseKind::Conflict) => IntegrationResult::new(IntegrationOutcome::Conflicted)
            .with_conflicts(merge::conflict_summaries(repo).unwrap_or_default()),
        Some(PauseKind::Edit) => IntegrationResult::new(IntegrationOutcome::Edited {
            commit: repo
                .head()
                .ok()
                .and_then(|head| head.target())
                .map(oid_field)
                .unwrap_or_default(),
        }),
        None => IntegrationResult::new(IntegrationOutcome::Staged),
    };
    result.also_notes(notes)
}

/// Apply one step: cherry-pick (or revert) its diff onto HEAD, then commit it
/// according to the action. On conflict the step is parked in
/// `state.paused_step`.
fn apply_step(
    repo: &git2::Repository,
    state: &mut SequenceState,
    step: &PlanStep,
    notes: &mut Vec<String>,
) -> Result<(), ErrorPayload> {
    let oid = Oid::from_str(&step.oid).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::UnknownRevision,
            "Sequence step is not a commit id",
        )
        .with_param("oid", &step.oid)
    })?;
    let source = repo.find_commit(oid)?;
    let is_revert = state.kind == SequenceKind::Revert;

    let mut checkout = CheckoutBuilder::new();
    checkout
        .safe()
        .allow_conflicts(true)
        .conflict_style_merge(true);

    if is_revert {
        let mut opts = git2::RevertOptions::new();
        opts.checkout_builder(checkout);
        if state.mainline > 0 {
            opts.mainline(state.mainline);
        } else if source.parent_count() > 1 {
            opts.mainline(1);
            notes.push(format!(
                "Merge commit {} was reverted against its first parent.",
                merge::short(oid)
            ));
        }
        repo.revert(&source, Some(&mut opts))?;
    } else {
        let mut opts = git2::CherrypickOptions::new();
        opts.checkout_builder(checkout);
        if state.mainline > 0 {
            opts.mainline(state.mainline);
        } else if source.parent_count() > 1 {
            opts.mainline(1);
            notes.push(format!(
                "Merge commit {} was flattened onto its first parent.",
                merge::short(oid)
            ));
        }
        repo.cherrypick(&source, Some(&mut opts))?;
    }

    if repo.index()?.has_conflicts() {
        state.pause = Some(PauseKind::Conflict);
        state.paused_step = Some(step.clone());
        return Ok(());
    }

    // An empty step (the change is already present) is dropped, exactly as
    // `git rebase` drops it — never an empty commit.
    {
        let head = merge::head_commit(repo)?;
        let tree_id = repo.index()?.write_tree()?;
        if head.tree_id() == tree_id {
            if !is_revert {
                notes.push(format!(
                    "{} {} had no net change and was dropped.",
                    action_token(step.action),
                    merge::short(oid)
                ));
            }
            state.dropped.push(oid);
            return Ok(());
        }
    }

    let source_message = if is_revert {
        revert_message(&source, oid)
    } else {
        let base = source.message().unwrap_or("").to_string();
        if state.record_origin {
            format!(
                "{}\n\n(cherry picked from commit {})\n",
                base.trim_end(),
                oid
            )
        } else {
            base
        }
    };
    // A revert is authored by whoever runs it; a cherry-pick keeps the source
    // author, which is what `git cherry-pick` does too.
    let author = if is_revert {
        merge::committer(repo)?
    } else {
        source.author()
    };

    match step.action {
        SequenceAction::Pick => {
            commit_pick(repo, &author, &source_message)?;
            state.head_message = Some(source_message);
            state.applied.push(oid);
        }
        SequenceAction::Reword => {
            let message = step
                .new_message
                .clone()
                .unwrap_or_else(|| source_message.clone());
            commit_pick(repo, &author, &message)?;
            state.head_message = Some(message);
            state.applied.push(oid);
        }
        SequenceAction::Edit => {
            commit_pick(repo, &author, &source_message)?;
            state.head_message = Some(source_message);
            state.applied.push(oid);
            state.pause = Some(PauseKind::Edit);
            state.paused_step = Some(step.clone());
        }
        SequenceAction::Squash => {
            let combined = step
                .new_message
                .clone()
                .unwrap_or_else(|| combine(state.head_message.as_deref(), &source_message));
            amend_head(repo, &combined)?;
            state.head_message = Some(combined);
            state.applied.push(oid);
        }
        SequenceAction::Fixup => {
            let message = step
                .new_message
                .clone()
                .or_else(|| state.head_message.clone())
                .unwrap_or_else(|| source_message.clone());
            amend_head(repo, &message)?;
            state.head_message = Some(message);
            state.applied.push(oid);
        }
        SequenceAction::Drop => {}
    }

    Ok(())
}

/// git's revert message, verbatim: `Revert "<subject>"` plus the explanatory
/// body every `git revert` writes.
fn revert_message(source: &git2::Commit<'_>, oid: Oid) -> String {
    let subject = source.summary().ok().flatten().unwrap_or("");
    format!("Revert \"{subject}\"\n\nThis reverts commit {oid}.\n")
}

/// Apply the diff of `source` onto HEAD and commit it with `author`.
/// `commit(None, …)` + an explicit HEAD move is used deliberately:
/// `commit(Some("HEAD"))` refuses an amend, and a squash/fixup would have no
/// way to reach git otherwise.
fn commit_pick(
    repo: &git2::Repository,
    author: &git2::Signature<'_>,
    message: &str,
) -> Result<Oid, ErrorPayload> {
    let tree_id = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head = merge::head_commit(repo)?;
    let committer = merge::committer(repo)?;
    let new = repo.commit(None, author, &committer, message, &tree, &[&head])?;
    move_head(repo, new)?;
    Ok(new)
}

/// Amend HEAD: same tree as the index, HEAD's own parents, HEAD's author.
fn amend_head(repo: &git2::Repository, message: &str) -> Result<Oid, ErrorPayload> {
    let tree_id = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head = merge::head_commit(repo)?;
    let parents: Vec<git2::Commit<'_>> = head.parents().collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
    let author = head.author();
    let committer = merge::committer(repo)?;
    let new = repo.commit(None, &author, &committer, message, &tree, &parent_refs)?;
    move_head(repo, new)?;
    Ok(new)
}

/// Point the current branch (or a detached HEAD) at `oid`.
fn move_head(repo: &git2::Repository, oid: Oid) -> Result<(), ErrorPayload> {
    let head = repo.head()?;
    if head.is_branch() {
        if let Ok(name) = head.name() {
            repo.reference(name, oid, true, "integrate: replay")?;
            return Ok(());
        }
    }
    repo.set_head_detached(oid)?;
    Ok(())
}

fn combine(previous: Option<&str>, next: &str) -> String {
    let previous = previous.unwrap_or("").trim_end();
    let next = next.trim_end();
    if previous.is_empty() {
        next.to_string()
    } else if next.is_empty() {
        previous.to_string()
    } else {
        format!("{previous}\n\n{next}")
    }
}

// ---------------------------------------------------------------------------
// Continue / skip / abort
// ---------------------------------------------------------------------------

/// `integration_continue` for the sequence kinds (merge is handled by
/// [`super::merge::continue_merge`]).
pub fn continue_(repo: &git2::Repository) -> Result<IntegrationResult, ErrorPayload> {
    let Some(mut state) = sequence_state(repo) else {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "No rebase or sequence is in progress",
        ));
    };
    let notes = Vec::new();

    match state.pause {
        None => {}
        Some(PauseKind::Conflict) => {
            if repo.index()?.has_conflicts() {
                return Ok(paused_result(repo, &state, notes));
            }
            let Some(step) = state.paused_step.clone() else {
                return Err(ErrorPayload::new(
                    ErrorCode::Unclassified,
                    "The paused step is missing from the sequence",
                ));
            };
            let oid = Oid::from_str(&step.oid)?;
            let source = repo.find_commit(oid)?;
            let is_revert = state.kind == SequenceKind::Revert;
            let base_message = if is_revert {
                revert_message(&source, oid)
            } else {
                let base = source.message().unwrap_or("").to_string();
                if state.record_origin {
                    format!(
                        "{}\n\n(cherry picked from commit {})\n",
                        base.trim_end(),
                        oid
                    )
                } else {
                    base
                }
            };
            let author = if is_revert {
                merge::committer(repo)?
            } else {
                source.author()
            };

            match step.action {
                SequenceAction::Squash | SequenceAction::Fixup => {
                    let message = match step.action {
                        SequenceAction::Squash => step.new_message.clone().unwrap_or_else(|| {
                            combine(state.head_message.as_deref(), &base_message)
                        }),
                        _ => step
                            .new_message
                            .clone()
                            .or_else(|| state.head_message.clone())
                            .unwrap_or_else(|| base_message.clone()),
                    };
                    amend_head(repo, &message)?;
                    state.head_message = Some(message);
                }
                _ => {
                    let message = step
                        .new_message
                        .clone()
                        .unwrap_or_else(|| base_message.clone());
                    commit_pick(repo, &author, &message)?;
                    state.head_message = Some(message);
                }
            }
            state.applied.push(oid);
            state.paused_step = None;
            state.pause = None;
        }
        Some(PauseKind::Edit) => {
            // The commit exists; the user may have amended it. Just move on.
            if let Some(step) = &state.paused_step {
                if let Ok(oid) = Oid::from_str(&step.oid) {
                    if !state.applied.contains(&oid) {
                        state.applied.push(oid);
                    }
                }
            }
            state.paused_step = None;
            state.pause = None;
        }
    }

    run(repo, &mut state, notes)
}

/// `integration_skip`: drop the paused step and carry on.
pub fn skip(repo: &git2::Repository) -> Result<IntegrationResult, ErrorPayload> {
    let Some(mut state) = sequence_state(repo) else {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "No rebase or sequence is in progress",
        ));
    };
    if state.pause.is_none() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The sequence is not paused; there is nothing to skip",
        ));
    }

    // Discard the partial replay (reset --hard to the last good commit, which
    // is where HEAD already points), then forget the step.
    let current = repo.head()?.peel_to_commit()?.id();
    let current_obj = repo.find_object(current, None)?;
    repo.reset(&current_obj, ResetType::Hard, None)?;
    repo.cleanup_state()?;

    if let Some(step) = state.paused_step.take() {
        if let Ok(oid) = Oid::from_str(&step.oid) {
            state.skipped.push(oid);
        }
    }
    state.pause = None;

    run(repo, &mut state, Vec::new())
}

/// Abort a sequence: roll the tree and HEAD back to where the operation
/// started, re-attach the branch, and remove the state files.
pub fn abort(
    repo: &git2::Repository,
    notes: &mut Vec<String>,
) -> Result<IntegrationResult, ErrorPayload> {
    let Some(state) = sequence_state(repo) else {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "No rebase or sequence is in progress",
        ));
    };

    clear_sequence(repo);
    repo.cleanup_state()?;

    let target = repo.find_object(state.original_head, None)?;
    repo.reset(&target, ResetType::Hard, None)?;

    if let Some(name) = &state.original_head_name {
        let refname = format!("refs/heads/{name}");
        if repo.find_reference(&refname).is_ok() {
            repo.set_head(&refname)?;
        }
    }

    merge::autostash_restore(repo, notes)?;

    Ok(IntegrationResult::new(IntegrationOutcome::Aborted {
        head: state.original_head.to_string(),
    })
    .also_notes(std::mem::take(notes)))
}

/// All steps done: move the branch onto the replayed tip, re-attach HEAD,
/// clean up and report.
fn finish(
    repo: &git2::Repository,
    state: &mut SequenceState,
    mut notes: Vec<String>,
) -> Result<IntegrationResult, ErrorPayload> {
    let head_oid = repo.head()?.peel_to_commit()?.id();

    if let Some(name) = &state.original_head_name {
        let refname = format!("refs/heads/{name}");
        repo.reference(&refname, head_oid, true, "rebase: finish")?;
        repo.set_head(&refname)?;
    } else {
        repo.set_head_detached(head_oid)?;
    }

    repo.cleanup_state()?;
    clear_sequence(repo);

    // `--no-commit`: the picks were committed to keep each diff relative to
    // the previous one; unwind to where we started, leaving everything staged.
    if state.no_commit {
        let target = repo.find_object(state.original_head, None)?;
        repo.reset(&target, ResetType::Soft, None)?;
        merge::autostash_restore(repo, &mut notes)?;
        return Ok(IntegrationResult::new(IntegrationOutcome::Staged).also_notes(notes));
    }

    merge::autostash_restore(repo, &mut notes)?;

    let outcome = match state.kind {
        SequenceKind::Rebase | SequenceKind::Interactive => IntegrationOutcome::RebaseFinished {
            head: head_oid.to_string(),
            applied: state.applied.len(),
            dropped: state.dropped.len(),
        },
        SequenceKind::CherryPick | SequenceKind::Revert => sequence_finished(state),
    };

    Ok(IntegrationResult::new(outcome).also_notes(notes))
}

// ---------------------------------------------------------------------------
// Cherry-pick / revert sequences (used by cherry.rs)
// ---------------------------------------------------------------------------

/// Begin a multi-revision sequence. `steps` are executed in order against the
/// current (attached) HEAD; a conflict parks the sequence exactly like a
/// rebase step.
pub fn start_sequence(
    repo: &git2::Repository,
    kind: SequenceKind,
    steps: Vec<PlanStep>,
    no_commit: bool,
    record_origin: bool,
    mainline: u32,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }
    let head = merge::head_commit(repo)?;

    let mut state = SequenceState {
        kind,
        original_head: head.id(),
        original_head_name: branch_name(repo),
        upstream: None,
        onto: None,
        steps,
        applied: Vec::new(),
        dropped: Vec::new(),
        skipped: Vec::new(),
        pause: None,
        paused_step: None,
        head_message: head.message().ok().map(|message| message.to_string()),
        no_commit,
        record_origin,
        mainline,
    };
    run(repo, &mut state, Vec::new())
}

/// The terminal report for a finished cherry-pick/revert sequence. Called by
/// the sequence engines in place of [`finish`]'s rebase-specific outcome.
pub fn sequence_finished(state: &SequenceState) -> IntegrationOutcome {
    IntegrationOutcome::SequenceFinished {
        applied: state.applied.iter().map(|oid| oid_field(*oid)).collect(),
        skipped: state.skipped.iter().map(|oid| oid_field(*oid)).collect(),
        dropped: state.dropped.iter().map(|oid| oid_field(*oid)).collect(),
    }
}

// ---------------------------------------------------------------------------
// Branch update / restack
// ---------------------------------------------------------------------------

/// One branch was moved onto its upstream by `branch_update`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub name: String,
    pub previous: String,
    pub head: String,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// `branch_update` (docs/03 §6): bring `name` up to date with its upstream by
/// merging or rebasing. The branch need not be checked out — we act on its ref
/// directly, which is the whole point of an "update in the background".
pub fn update_branch(
    repo: &git2::Repository,
    name: &str,
    mode: &str,
    autostash: bool,
) -> Result<UpdateResult, ErrorPayload> {
    let refname = format!("refs/heads/{name}");
    let branch = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| {
            ErrorPayload::new(ErrorCode::ReferenceNotFound, "No such local branch")
                .with_param("name", name)
        })?;
    let upstream = branch.upstream().map_err(|_| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Branch has no upstream to update from",
        )
        .with_param("name", name)
    })?;
    let upstream_oid = upstream.get().peel_to_commit()?.id();
    let previous = branch.get().peel_to_commit()?.id();

    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }

    let checked_out = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(|name| name.to_string()));
    let is_current = checked_out.as_deref() == Some(name);

    // Fast-forward is safe for every mode and needs no checkout.
    let base = repo.merge_base(previous, upstream_oid)?;
    if base == previous {
        repo.reference(&refname, upstream_oid, true, "branch update: fast-forward")?;
        if is_current {
            let mut checkout = CheckoutBuilder::new();
            checkout.safe();
            let obj = repo.find_object(upstream_oid, None)?;
            repo.checkout_tree(&obj, Some(&mut checkout))?;
        }
        return Ok(UpdateResult {
            name: name.to_string(),
            previous: previous.to_string(),
            head: upstream_oid.to_string(),
            mode: mode.to_string(),
            notes: vec!["Fast-forwarded to upstream.".to_string()],
        });
    }
    if base == upstream_oid {
        return Ok(UpdateResult {
            name: name.to_string(),
            previous: previous.to_string(),
            head: previous.to_string(),
            mode: mode.to_string(),
            notes: vec!["Branch already contains its upstream.".to_string()],
        });
    }

    // Diverged: only the checked-out branch can be merged/rebased in place,
    // because both operations need a working tree.
    if !is_current {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Branch has diverged and is not checked out; check it out to update it",
        )
        .with_param("name", name));
    }

    match mode {
        "Rebase" => {
            let req = RebaseRequest {
                autostash,
                verify_signatures: false,
            };
            let outcome = start(
                repo,
                &Revision::Branch(
                    upstream
                        .name()
                        .ok()
                        .flatten()
                        .unwrap_or("upstream")
                        .to_string(),
                ),
                None,
                &req,
            )?;
            let head = repo
                .find_branch(name, git2::BranchType::Local)
                .ok()
                .and_then(|branch| branch.get().peel_to_commit().ok())
                .map(|commit| commit.id().to_string())
                .unwrap_or_else(|| outcome_head(&outcome));
            Ok(UpdateResult {
                name: name.to_string(),
                previous: previous.to_string(),
                head,
                mode: mode.to_string(),
                notes: outcome.notes,
            })
        }
        "Merge" | "MergeNoFF" => {
            let mut req = merge::MergeRequest::dialogue_defaults();
            req.no_ff = mode == "MergeNoFF";
            req.autostash = autostash;
            let revision = Revision::Branch(
                upstream
                    .name()
                    .ok()
                    .flatten()
                    .unwrap_or("upstream")
                    .to_string(),
            );
            let outcome = merge::start(repo, &revision, &req)?;
            Ok(UpdateResult {
                name: name.to_string(),
                previous: previous.to_string(),
                head: outcome_head(&outcome),
                mode: mode.to_string(),
                notes: outcome.notes,
            })
        }
        "Squash" => {
            let mut req = merge::MergeRequest::dialogue_defaults();
            req.squash = true;
            req.autostash = autostash;
            let revision = Revision::Branch(
                upstream
                    .name()
                    .ok()
                    .flatten()
                    .unwrap_or("upstream")
                    .to_string(),
            );
            let outcome = merge::start(repo, &revision, &req)?;
            Ok(UpdateResult {
                name: name.to_string(),
                previous: previous.to_string(),
                head: previous.to_string(),
                mode: mode.to_string(),
                notes: outcome.notes,
            })
        }
        other => Err(
            ErrorPayload::new(ErrorCode::InvalidRequest, "Unknown branch update mode")
                .with_param("mode", other),
        ),
    }
}

fn outcome_head(outcome: &IntegrationResult) -> String {
    match &outcome.outcome {
        IntegrationOutcome::UpToDate { head }
        | IntegrationOutcome::FastForward { head, .. }
        | IntegrationOutcome::Merged { commit: head, .. }
        | IntegrationOutcome::Edited { commit: head }
        | IntegrationOutcome::RebaseFinished { head, .. }
        | IntegrationOutcome::Aborted { head } => head.clone(),
        _ => String::new(),
    }
}

/// `branch_stack_restack` (docs/03 §6): put every branch back on top of its
/// upstream. Only the checked-out branch can be rebuilt (it needs a working
/// tree); the others are reported as failures with a clear reason rather than
/// silently skipped.
pub fn restack(
    repo: &git2::Repository,
    names: &[String],
) -> Result<crate::domain::conflict::RestackReport, ErrorPayload> {
    use crate::domain::conflict::{RestackEntry, RestackFailure, RestackReport};

    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }

    let candidates: Vec<String> = if names.is_empty() {
        let mut locals = Vec::new();
        for branch in repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = branch?;
            if let Some(name) = branch.name().ok().flatten() {
                locals.push(name.to_string());
            }
        }
        locals
    } else {
        names.to_vec()
    };

    let checked_out = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(|name| name.to_string()));
    let mut report = RestackReport::default();

    for name in candidates {
        let branch = match repo.find_branch(&name, git2::BranchType::Local) {
            Ok(branch) => branch,
            Err(_) => {
                report.failed.push(RestackFailure {
                    name,
                    message: "No such local branch".to_string(),
                    conflicted: false,
                });
                continue;
            }
        };
        let upstream = match branch.upstream() {
            Ok(upstream) => upstream,
            Err(_) => {
                report.failed.push(RestackFailure {
                    name,
                    message: "Branch has no upstream".to_string(),
                    conflicted: false,
                });
                continue;
            }
        };
        let previous = branch.get().peel_to_commit()?.id();
        let upstream_oid = upstream.get().peel_to_commit()?.id();

        let base = repo.merge_base(previous, upstream_oid)?;
        if base == upstream_oid {
            report.up_to_date.push(name);
            continue;
        }
        if base == previous {
            // Fast-forward needs no working tree.
            repo.reference(
                &format!("refs/heads/{name}"),
                upstream_oid,
                true,
                "restack: fast-forward",
            )?;
            report.updated.push(RestackEntry {
                name: name.clone(),
                onto: upstream
                    .name()
                    .ok()
                    .flatten()
                    .unwrap_or("upstream")
                    .to_string(),
                previous: previous.to_string(),
                head: upstream_oid.to_string(),
                replayed: 0,
            });
            continue;
        }
        if checked_out.as_deref() != Some(name.as_str()) {
            report.failed.push(RestackFailure {
                name,
                message: "Branch has diverged and is not checked out".to_string(),
                conflicted: false,
            });
            continue;
        }

        let upstream_name = upstream
            .name()
            .ok()
            .flatten()
            .unwrap_or("upstream")
            .to_string();
        let req = RebaseRequest {
            autostash: true,
            verify_signatures: false,
        };
        match start(repo, &Revision::Branch(upstream_name.clone()), None, &req) {
            Ok(outcome) => {
                if matches!(outcome.outcome, IntegrationOutcome::Conflicted) {
                    report.failed.push(RestackFailure {
                        name,
                        message: "Restack hit conflicts".to_string(),
                        conflicted: true,
                    });
                    // Leave the sequence in place so the user can resolve.
                    break;
                }
                let head = repo
                    .find_branch(&name, git2::BranchType::Local)
                    .ok()
                    .and_then(|branch| branch.get().peel_to_commit().ok())
                    .map(|commit| commit.id().to_string())
                    .unwrap_or_else(|| outcome_head(&outcome));
                let replayed = match &outcome.outcome {
                    IntegrationOutcome::RebaseFinished { applied, .. } => *applied,
                    _ => 0,
                };
                report.updated.push(RestackEntry {
                    name,
                    onto: upstream_name,
                    previous: previous.to_string(),
                    head,
                    replayed,
                });
            }
            Err(err) => {
                report.failed.push(RestackFailure {
                    name,
                    message: err.message.clone(),
                    conflicted: false,
                });
            }
        }
    }

    Ok(report)
}

// ---------------------------------------------------------------------------
// Helpers shared with cherry.rs
// ---------------------------------------------------------------------------

/// Resolve a list of revisions to commit ids, preserving order.
pub fn resolve_revisions(
    repo: &git2::Repository,
    revisions: &[Revision],
) -> Result<Vec<Oid>, ErrorPayload> {
    let mut oids = Vec::with_capacity(revisions.len());
    for revision in revisions {
        oids.push(history::resolve(repo, revision)?.id());
    }
    Ok(oids)
}

/// Convert a commit's author to the domain shape (used when building steps).
pub fn author_of(repo: &git2::Repository, oid: Oid) -> Option<Signature> {
    repo.find_commit(oid)
        .ok()
        .map(|commit| signature_of(&commit.author()))
}

// ---------------------------------------------------------------------------
// Rewrite commands (docs/03 §13): squash / fixup / move / delete + impact
// ---------------------------------------------------------------------------

/// `rewrite_impact_load` (docs/03 §13): the three answers the UI shows before a
/// rewrite — computed by ancestry only, nothing moves.
pub fn rewrite_impact(
    repo: &git2::Repository,
    branch: &str,
    revisions: &[Revision],
) -> Result<crate::domain::conflict::RewriteImpact, ErrorPayload> {
    use crate::domain::conflict::RewriteImpact;

    let oids = resolve_revisions(repo, revisions)?;

    let mut remote_tips = Vec::new();
    for reference in repo.references()? {
        let reference = reference?;
        if let Ok(name) = reference.name() {
            if name.starts_with("refs/remotes/") {
                if let Some(target) = reference.target() {
                    remote_tips.push(target);
                }
            }
        }
    }
    let mut other_tips = Vec::new();
    for branch_ref in repo.branches(Some(git2::BranchType::Local))? {
        let (branch_ref, _) = branch_ref?;
        let name = branch_ref.name().ok().flatten().unwrap_or("").to_string();
        if name != branch {
            if let Some(target) = branch_ref.get().target() {
                other_tips.push(target);
            }
        }
    }

    let mut impact = RewriteImpact::default();
    for oid in oids {
        let pushed = remote_tips
            .iter()
            .any(|tip| repo.graph_descendant_of(*tip, oid).unwrap_or(false));
        if !pushed {
            impact.unpushed.push(oid.to_string());
        }
        let shared = other_tips
            .iter()
            .any(|tip| repo.graph_descendant_of(*tip, oid).unwrap_or(false));
        if shared {
            impact.mainline.push(oid.to_string());
        }
        if repo.find_commit(oid)?.parent_count() > 1 {
            impact.merge_commits.push(oid.to_string());
        }
    }
    Ok(impact)
}

/// The first parent of the oldest commit in `oids` — the base a rewrite of
/// that set is anchored to.
fn rewrite_base(repo: &git2::Repository, oids: &[Oid]) -> Result<Oid, ErrorPayload> {
    // The oldest member is the one that is an ancestor of every other member.
    let mut oldest = None;
    for candidate in oids {
        let is_ancestor = oids.iter().all(|other| {
            other == candidate
                || repo
                    .graph_descendant_of(*other, *candidate)
                    .unwrap_or(false)
        });
        if is_ancestor {
            oldest = Some(*candidate);
            break;
        }
    }
    let oldest = oldest.unwrap_or(oids[0]);
    let commit = repo.find_commit(oldest)?;
    commit.parent(0).map(|parent| parent.id()).map_err(|_| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The oldest revision has no parent to rewrite from",
        )
        .with_param("oid", oldest.to_string())
    })
}

/// `squash_revisions` (docs/03 §13): fold `base..HEAD` into a single commit —
/// `reset --soft base`, then one commit. Returns the new commit.
pub fn squash_revisions(
    repo: &git2::Repository,
    base: &Revision,
    tips: &[Revision],
    message: Option<&str>,
) -> Result<IntegrationResult, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if merge::integration_in_progress(repo) {
        return Err(ErrorPayload::new(
            ErrorCode::OperationInProgress,
            "An integration is already in progress",
        ));
    }
    let base_commit = history::resolve(repo, base)?;
    let tip_oids = resolve_revisions(repo, tips)?;
    for tip in &tip_oids {
        if *tip != base_commit.id() && !repo.graph_descendant_of(*tip, base_commit.id())? {
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "A tip is not a descendant of the base",
            )
            .with_param("oid", tip.to_string()));
        }
    }

    let head = merge::head_commit(repo)?.id();
    merge::write_pseudoref(repo, "ORIG_HEAD", head)?;

    let object = repo.find_object(base_commit.id(), None)?;
    repo.reset(&object, ResetType::Soft, None)?;

    let combined = match message {
        Some(message) => message.to_string(),
        None => {
            let mut parts = Vec::new();
            for oid in &tip_oids {
                if let Ok(commit) = repo.find_commit(*oid) {
                    if let Ok(message) = commit.message() {
                        parts.push(message.trim_end().to_string());
                    }
                }
            }
            if parts.is_empty() {
                format!("Squashed {} revisions", tip_oids.len())
            } else {
                parts.join("\n\n")
            }
        }
    };

    let tree_id = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = merge::committer(repo)?;
    let new = repo.commit(Some("HEAD"), &sig, &sig, &combined, &tree, &[&base_commit])?;

    Ok(IntegrationResult::new(IntegrationOutcome::Merged {
        commit: new.to_string(),
        parents: vec![base_commit.id().to_string()],
    }))
}

/// `fixup_revisions` (docs/03 §13): an interactive replay in which every
/// `fixup` folds into the target, everything else is a plain pick.
pub fn fixup_revisions(
    repo: &git2::Repository,
    target: &Revision,
    fixups: &[Revision],
) -> Result<IntegrationResult, ErrorPayload> {
    let target_oid = history::resolve(repo, target)?.id();
    let base = repo
        .find_commit(target_oid)?
        .parent(0)
        .map(|parent| parent.id())
        .map_err(|_| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "The target commit has no parent to rebase from",
            )
        })?;
    let fixup_oids: std::collections::HashSet<Oid> =
        resolve_revisions(repo, fixups)?.into_iter().collect();

    let head = merge::head_commit(repo)?;
    let steps = range_commits(repo, head.id(), Some(base))?
        .into_iter()
        .map(|oid| PlanStep {
            action: if fixup_oids.contains(&oid) {
                SequenceAction::Fixup
            } else {
                SequenceAction::Pick
            },
            oid: oid.to_string(),
            ..PlanStep::default()
        })
        .collect();

    let plan = SequencePlan {
        upstream: base.to_string(),
        onto: base.to_string(),
        steps,
    };
    start_interactive(repo, &plan, &RebaseRequest::default())
}

/// `delete_revisions` (docs/03 §13): an interactive replay that drops the given
/// commits.
pub fn delete_revisions(
    repo: &git2::Repository,
    revisions: &[Revision],
) -> Result<IntegrationResult, ErrorPayload> {
    if revisions.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No revisions were given to delete",
        ));
    }
    let oids = resolve_revisions(repo, revisions)?;
    let base = rewrite_base(repo, &oids)?;
    let drop: std::collections::HashSet<Oid> = oids.into_iter().collect();

    let head = merge::head_commit(repo)?;
    let steps = range_commits(repo, head.id(), Some(base))?
        .into_iter()
        .map(|oid| PlanStep {
            action: if drop.contains(&oid) {
                SequenceAction::Drop
            } else {
                SequenceAction::Pick
            },
            oid: oid.to_string(),
            ..PlanStep::default()
        })
        .collect();

    let plan = SequencePlan {
        upstream: base.to_string(),
        onto: base.to_string(),
        steps,
    };
    start_interactive(repo, &plan, &RebaseRequest::default())
}

/// `move_revisions` (docs/03 §13): reorder the given commits to sit before or
/// after `onto` in an otherwise unchanged replay.
pub fn move_revisions(
    repo: &git2::Repository,
    revisions: &[Revision],
    onto: &str,
    direction: &str,
) -> Result<IntegrationResult, ErrorPayload> {
    if revisions.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "No revisions were given to move",
        ));
    }
    let moving = resolve_revisions(repo, revisions)?;
    let onto_oid = resolve_plan_ref(repo, onto)?;

    let mut anchors = moving.clone();
    anchors.push(onto_oid);
    let base = rewrite_base(repo, &anchors)?;

    let head = merge::head_commit(repo)?;
    let full: Vec<Oid> = range_commits(repo, head.id(), Some(base))?;

    let moving_set: std::collections::HashSet<Oid> = moving.iter().copied().collect();
    let rest: Vec<Oid> = full
        .iter()
        .copied()
        .filter(|oid| !moving_set.contains(oid))
        .collect();

    let insert_at = rest
        .iter()
        .position(|oid| *oid == onto_oid)
        .map(|position| {
            if direction.eq_ignore_ascii_case("after") || direction.eq_ignore_ascii_case("below") {
                position + 1
            } else {
                position
            }
        })
        .ok_or_else(|| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "The 'onto' commit is not in the range",
            )
            .with_param("onto", onto_oid.to_string())
        })?;

    let mut ordered: Vec<Oid> = Vec::with_capacity(full.len());
    ordered.extend_from_slice(&rest[..insert_at]);
    ordered.extend_from_slice(&moving);
    ordered.extend_from_slice(&rest[insert_at..]);

    let steps = ordered
        .into_iter()
        .map(|oid| PlanStep {
            action: SequenceAction::Pick,
            oid: oid.to_string(),
            ..PlanStep::default()
        })
        .collect();

    let plan = SequencePlan {
        upstream: base.to_string(),
        onto: base.to_string(),
        steps,
    };
    start_interactive(repo, &plan, &RebaseRequest::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_arbitrary_text() {
        for sample in ["", "pick", "line\none\ttwo", "unicode ✓ 汉字"] {
            let encoded = hex_encode(sample);
            assert_eq!(hex_decode(&encoded).as_deref(), Some(sample));
        }
    }

    #[test]
    fn sequence_round_trips_through_the_file_format() {
        let oid = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
        let state = SequenceState {
            kind: SequenceKind::Interactive,
            original_head: oid,
            original_head_name: Some("feature/x".to_string()),
            upstream: Some(oid),
            onto: Some(oid),
            steps: vec![PlanStep {
                action: SequenceAction::Squash,
                oid: oid.to_string(),
                message: "keep\n\nthe newlines".to_string(),
                ..PlanStep::default()
            }],
            applied: vec![oid],
            dropped: Vec::new(),
            skipped: Vec::new(),
            pause: Some(PauseKind::Conflict),
            paused_step: Some(PlanStep {
                action: SequenceAction::Pick,
                oid: oid.to_string(),
                ..PlanStep::default()
            }),
            head_message: Some("subject\n\nbody".to_string()),
            no_commit: false,
            record_origin: true,
            mainline: 1,
        };

        let decoded = decode(&encode(&state)).expect("state decodes");
        assert_eq!(decoded.kind, SequenceKind::Interactive);
        assert_eq!(decoded.original_head_name.as_deref(), Some("feature/x"));
        assert_eq!(decoded.steps.len(), 1);
        assert_eq!(decoded.steps[0].action, SequenceAction::Squash);
        assert_eq!(decoded.pause, Some(PauseKind::Conflict));
        assert_eq!(decoded.head_message.as_deref(), Some("subject\n\nbody"));
        assert!(decoded.record_origin);
        assert_eq!(decoded.mainline, 1);
    }

    #[test]
    fn combine_joins_messages_with_a_blank_line() {
        assert_eq!(combine(Some("a"), "b"), "a\n\nb");
        assert_eq!(combine(None, "b"), "b");
        assert_eq!(combine(Some("a"), ""), "a");
    }
}
