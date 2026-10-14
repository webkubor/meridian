use std::path::{Component, Path};

use crate::domain::error::{ErrorCode, ErrorDomain};

use super::Violation;

/// `repository_open` preconditions (docs/03 §2): path exists; is a directory.
pub fn validate_open_path(path: &Path) -> Result<(), Violation> {
    if !path.exists() {
        return Err(Violation::new(
            ErrorCode::Io,
            format!("Path does not exist: {}", path.display()),
        )
        .with_domain(ErrorDomain::Fs)
        .with_param("path", path.display().to_string()));
    }
    if !path.is_dir() {
        return Err(Violation::new(
            ErrorCode::Io,
            format!("Not a directory: {}", path.display()),
        )
        .with_domain(ErrorDomain::Fs)
        .with_param("path", path.display().to_string()));
    }
    Ok(())
}

/// `repository_init` preconditions (docs/03 §2): the path is empty or
/// creatable, and the initial branch name is a valid ref name.
pub fn validate_init_path(path: &Path, initial_branch: &str) -> Result<(), Violation> {
    if path.exists() {
        let mut entries = std::fs::read_dir(path).map_err(|err| {
            Violation::new(
                ErrorCode::Io,
                format!("Cannot read {}: {err}", path.display()),
            )
            .with_domain(ErrorDomain::Fs)
            .with_param("path", path.display().to_string())
        })?;
        if entries.next().is_some() {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("Directory is not empty: {}", path.display()),
            )
            .with_param("path", path.display().to_string()));
        }
    } else if let Some(parent) = path.parent() {
        // A missing parent means `create_dir_all` would fail — say so before
        // touching anything.
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(Violation::new(
                ErrorCode::Io,
                format!("Parent directory does not exist: {}", parent.display()),
            )
            .with_domain(ErrorDomain::Fs)
            .with_param("path", parent.display().to_string()));
        }
    }

    if !git2::Reference::is_valid_name(&format!("refs/heads/{initial_branch}")) {
        return Err(Violation::new(
            ErrorCode::InvalidReferenceName,
            format!("'{initial_branch}' is not a valid branch name"),
        )
        .with_param("name", initial_branch.to_string()));
    }

    // Guard against a path that would need `..` resolution to make sense.
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Violation::new(
            ErrorCode::InvalidRequest,
            format!("Path contains '..': {}", path.display()),
        )
        .with_param("path", path.display().to_string()));
    }

    Ok(())
}
