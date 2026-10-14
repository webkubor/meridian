use std::path::{Component, Path};

use crate::domain::error::ErrorCode;

use super::{ValidateCx, Violation};

/// Every working-copy command needs a working copy.
pub fn validate_working_copy(cx: &ValidateCx<'_>) -> Result<(), Violation> {
    if cx.repo.is_bare() {
        return Err(Violation::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }
    Ok(())
}

/// Path arguments must be repository-relative and must not escape the working
/// tree — the security rule from docs/02 §16, enforced before any index write.
pub fn validate_paths(cx: &ValidateCx<'_>, paths: &[String]) -> Result<(), Violation> {
    validate_working_copy(cx)?;

    if paths.is_empty() {
        return Err(Violation::new(ErrorCode::InvalidRequest, "No paths given"));
    }

    for path in paths {
        let candidate = Path::new(path);
        let escapes = candidate.is_absolute()
            || candidate
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir));
        if escapes {
            return Err(Violation::new(
                ErrorCode::InvalidRequest,
                format!("Path leaves the repository: {path}"),
            )
            .with_param("path", path.clone()));
        }
    }

    Ok(())
}
