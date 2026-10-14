//! The `commit` command surface (docs/03 §4).
//!
//! Two verbs share one engine operation: `commit_create` appends a commit to
//! HEAD, `commit_amend` replaces it. Everything git-shaped — the tree, the
//! parents, the signature, the reflog — lives in `engine::index::commit`; this
//! file owns the argument shapes, the refusals that are about *arguments*
//! rather than about git, and the events.
//!
//! The preconditions are the second item of the tri-pattern (CLAUDE.md
//! §"Adding a command"). They are checked here, in `validate`, because this
//! change may not add `validation/commit.rs` (that module is owned elsewhere);
//! the checks are pure (they read the repository and never mutate it) and use
//! only the read-only helpers `engine::index` exposes, so lifting them into
//! `validation/commit.rs` later is a move, not a rewrite.
//!
//! **Not in this pass.** Two things docs/03 §4 lists are deliberately absent
//! rather than half-wired: the hook chain (`pre-commit` / `commit-msg` /
//! `post-commit`) and commit signing (`sign{Gpg|Ssh}`). Their fields are not
//! declared here, so a caller cannot mistake a parsed-but-ignored option for a
//! working feature.

use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx, Risk};
use crate::domain::commit::{CommitOptions, CommitRequest, CommitResult, CommitSign};
use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::ids::RepoId;
use crate::domain::sender::SenderTag;
use crate::engine;
use crate::events::{self, AppEvent};
use crate::state::AppState;
use crate::validation::{ValidateCx, Violation};

/// `commit_create` (docs/03 §4): commit what the index holds onto the current
/// branch, or start one when HEAD is unborn.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitCreate {
    pub repo: RepoId,
    pub message: String,
    #[serde(default)]
    pub opts: CommitOptions,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for CommitCreate {
    type Output = CommitResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn sender(&self) -> SenderTag {
        self.sender
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validate_commit(cx, false, Some(self.message.as_str()), &self.opts)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let request = CommitRequest {
            message: Some(self.message),
            amend: false,
            opts: self.opts,
        };
        finish(engine::index::commit(cx.repo, &request)?, cx)
    }
}

/// `commit_amend` (docs/03 §4): replace HEAD with a new commit that keeps its
/// parents. `message` absent reuses the message being replaced.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitAmend {
    pub repo: RepoId,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub opts: CommitOptions,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for CommitAmend {
    type Output = CommitResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn sender(&self) -> SenderTag {
        self.sender
    }

    fn risk(&self) -> Risk {
        // Replacing a commit rewrites the branch tip the ref points at.
        Risk::Rewrite
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validate_commit(cx, true, self.message.as_deref(), &self.opts)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let request = CommitRequest {
            message: self.message,
            amend: true,
            opts: self.opts,
        };
        finish(engine::index::commit(cx.repo, &request)?, cx)
    }
}

/// Both verbs move HEAD and the index-vs-HEAD relationship: announce refs and
/// working copy.
fn finish(result: CommitResult, cx: &ExecCx<'_>) -> Result<CommitResult, ErrorPayload> {
    events::emit(
        cx.app,
        AppEvent::RefsChanged {
            repo: cx.repo_id.clone(),
        },
    );
    events::emit(
        cx.app,
        AppEvent::WorkingCopyChanged {
            repo: cx.repo_id.clone(),
        },
    );
    Ok(result)
}

/// The shared preconditions (docs/03 §4): no unrelated integration in progress,
/// no unresolved conflicts, a resolvable identity, and something to commit.
fn validate_commit(
    cx: &ValidateCx<'_>,
    amend: bool,
    message: Option<&str>,
    opts: &CommitOptions,
) -> Result<(), Violation> {
    let repo = cx.repo;
    let provided = message.map(str::trim).filter(|message| !message.is_empty());

    if !amend && provided.is_none() {
        return Err(Violation::new(
            ErrorCode::InvalidRequest,
            "A commit message is required",
        ));
    }

    // An amend replaces HEAD, so HEAD must exist (docs/03 §4). The engine
    // enforces this too, but refusing here keeps the failure a validation
    // violation rather than a half-run command.
    if amend && repo.head().is_err() {
        return Err(Violation::new(
            ErrorCode::OperationInProgress,
            "There is no commit to amend: HEAD does not point at a commit",
        ));
    }

    // Signing is not implemented this pass: a request for it is refused loudly
    // rather than answered with an unsigned commit.
    if matches!(
        opts.sign.as_ref(),
        Some(CommitSign::Gpg { .. }) | Some(CommitSign::Ssh { .. })
    ) {
        return Err(Violation::new(
            ErrorCode::InvalidRequest,
            "Commit signing is not supported in this build",
        ));
    }

    // A merge in progress *is* the commit path (it produces the two-parent
    // commit); every other integration must be finished or aborted first.
    match repo.state() {
        git2::RepositoryState::Clean | git2::RepositoryState::Merge => {}
        other => {
            return Err(Violation::new(
                ErrorCode::OperationInProgress,
                format!(
                    "{} is in progress; finish or abort it before committing",
                    operation_label(other)
                ),
            ));
        }
    }

    if repo
        .index()
        .map_err(|err| violation_from(ErrorPayload::from(err)))?
        .has_conflicts()
    {
        return Err(Violation::new(
            ErrorCode::CheckoutConflicts,
            "The index has unresolved conflicts; resolve them before committing",
        ));
    }

    // The identity is only needed where it cannot be overridden or inherited:
    // a create needs the author from config unless one was passed in, and every
    // commit needs the committer from config unless one was passed in.
    let needs_config = opts.committer.is_none() || (!amend && opts.author.is_none());
    if needs_config {
        if let Some(key) = engine::index::missing_identity(repo).map_err(violation_from)? {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("Git identity is not configured: set {key} (git config)"),
            )
            .with_param("key", key));
        }
    }

    let staged = engine::index::has_staged_changes(repo).map_err(violation_from)?;
    if amend {
        if !staged && provided.is_none() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                "Nothing to amend: there are no staged changes and no new message",
            ));
        }
    } else if !staged {
        return Err(Violation::new(
            ErrorCode::InvalidRequest,
            "No staged changes to commit",
        ));
    }

    Ok(())
}

fn operation_label(state: git2::RepositoryState) -> &'static str {
    use git2::RepositoryState::*;
    match state {
        Merge => "A merge",
        Revert | RevertSequence => "A revert",
        CherryPick | CherryPickSequence => "A cherry-pick",
        Bisect => "A bisect",
        Rebase | RebaseInteractive | RebaseMerge => "A rebase",
        ApplyMailbox | ApplyMailboxOrRebase => "An apply/rebase run",
        Clean => "No operation",
    }
}

/// A validator never carries an [`ErrorPayload`]; downgrade one to the
/// [`Violation`] the command boundary speaks.
fn violation_from(payload: ErrorPayload) -> Violation {
    Violation::new(payload.code, payload.message)
}

// ===========================================================================
// Tauri wrappers
// ===========================================================================

#[tauri::command]
pub async fn commit_create(
    state: State<'_, AppState>,
    app: AppHandle,
    args: CommitCreate,
) -> Result<CommitResult, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn commit_amend(
    state: State<'_, AppState>,
    app: AppHandle,
    args: CommitAmend,
) -> Result<CommitResult, ErrorPayload> {
    state.run(&app, args).await
}
