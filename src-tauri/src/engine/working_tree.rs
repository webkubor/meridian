use std::path::Path;

use crate::domain::error::{ErrorCode, ErrorPayload};
use crate::domain::status::EntryStatus;
use crate::transform::file_status;

/// Knobs for a status read — the options the UI actually toggles, and nothing
/// else; the engine never guesses (docs/03 §3).
#[derive(Clone, Copy, Debug, Default)]
pub struct StatusOptions {
    pub include_ignored: bool,
    pub include_dirs: bool,
}

pub fn status(
    repo: &git2::Repository,
    opts: &StatusOptions,
) -> Result<Vec<EntryStatus>, ErrorPayload> {
    if repo.is_bare() {
        return Err(ErrorPayload::new(
            ErrorCode::NoWorkingCopy,
            "Repository has no working copy",
        ));
    }

    let mut git_opts = git2::StatusOptions::new();
    git_opts
        .include_untracked(true)
        .include_ignored(opts.include_ignored)
        .include_unmodified(false)
        .recurse_untracked_dirs(!opts.include_dirs)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo.statuses(Some(&mut git_opts))?;

    let mut entries = Vec::new();
    for entry in statuses.iter() {
        let Ok(path) = entry.path() else { continue };
        // The assume-unchanged bit is read from the index in a later phase; P0
        // reports every entry as tracked normally.
        entries.extend(file_status::from_git2(
            path.to_string(),
            entry.status(),
            false,
        ));
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| b.staged.cmp(&a.staged)));
    Ok(entries)
}

/// `wc_stage_paths` (docs/03 §3): stage through `Index::add_all`, which also
/// records deletions (`Index::add_path` cannot).
pub fn stage_paths(repo: &git2::Repository, paths: &[String]) -> Result<(), ErrorPayload> {
    let mut index = repo.index()?;
    index.add_all(
        paths.iter().map(String::as_str),
        git2::IndexAddOption::DEFAULT,
        None,
    )?;
    index.write()?;
    Ok(())
}

/// `wc_unstage_paths` (docs/03 §3): reset the given paths against HEAD. On an
/// unborn HEAD there is no tree to reset to, so unstage means "remove from the
/// index" — the same end state the CLI produces.
pub fn unstage_paths(repo: &git2::Repository, paths: &[String]) -> Result<(), ErrorPayload> {
    match repo.head() {
        Ok(head) => {
            let target = head.peel(git2::ObjectType::Commit)?;
            repo.reset_default(Some(&target), paths.iter().map(String::as_str))?;
        }
        Err(_) => {
            let mut index = repo.index()?;
            for path in paths {
                let _ = index.remove_path(Path::new(path));
            }
            index.write()?;
        }
    }
    Ok(())
}
