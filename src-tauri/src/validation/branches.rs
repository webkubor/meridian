use crate::domain::error::ErrorCode;
use crate::domain::revision::Revision;
use crate::domain::sender::SenderTag;
use crate::engine::history;

use super::{ValidateCx, Violation};

/// `branch_create` preconditions (docs/03 §6): name valid; not existing; start
/// point resolves.
pub fn validate_create(cx: &ValidateCx<'_>, name: &str, start: &Revision) -> Result<(), Violation> {
    let name = name.trim();

    if name.is_empty() {
        return Err(Violation::new(
            ErrorCode::InvalidReferenceName,
            "Branch name is empty",
        ));
    }
    if !git2::Reference::is_valid_name(&format!("refs/heads/{name}")) {
        return Err(Violation::new(
            ErrorCode::InvalidReferenceName,
            format!("'{name}' is not a valid branch name"),
        )
        .with_param("name", name));
    }
    if cx.repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Err(Violation::new(
            ErrorCode::ReferenceExists,
            format!("Branch '{name}' already exists"),
        )
        .with_param("name", name));
    }
    history::resolve(cx.repo, start).map_err(|err| {
        Violation::new(ErrorCode::UnknownRevision, err.message.clone())
            .with_param("revision", revision_display(start))
    })?;

    // Sender-scoped precondition (docs/02 §3): toolbar and quick-action
    // invocations default the start point to HEAD, so an unborn HEAD has
    // nothing to start from — the full dialogue can still pass an explicit
    // start point, and that is the only difference the sender makes.
    let defaults_to_head = matches!(
        cx.sender,
        SenderTag::Toolbar | SenderTag::QuickActionPalette
    );
    if defaults_to_head && matches!(start, Revision::Head) && cx.repo.is_empty().unwrap_or(false) {
        return Err(Violation::new(
            ErrorCode::UnknownRevision,
            "The repository has no commits yet — choose an explicit start point",
        ));
    }

    Ok(())
}

/// `branch_checkout` preconditions (docs/03 §6): a working copy exists, no
/// integration is in progress, the branch exists, and it is not already HEAD.
pub fn validate_checkout(cx: &ValidateCx<'_>, name: &str) -> Result<(), Violation> {
    if cx.repo.is_bare() {
        return Err(Violation::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    if cx.repo.state() != git2::RepositoryState::Clean {
        return Err(Violation::new(
            ErrorCode::OperationInProgress,
            "Cannot switch branches while an operation is in progress",
        ));
    }
    let branch = cx
        .repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| {
            Violation::new(
                ErrorCode::ReferenceNotFound,
                format!("Branch '{name}' does not exist"),
            )
            .with_param("name", name)
        })?;
    if branch.is_head() {
        return Err(
            Violation::new(ErrorCode::InvalidRequest, format!("Already on '{name}'"))
                .with_param("name", name),
        );
    }
    Ok(())
}

fn revision_display(revision: &Revision) -> String {
    match revision {
        Revision::Head => "HEAD".to_string(),
        Revision::Branch(name) => name.clone(),
        Revision::Tag(name) => name.clone(),
        Revision::Oid(oid) => oid.clone(),
    }
}
