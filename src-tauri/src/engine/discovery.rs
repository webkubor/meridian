//! Repository discovery and path probing (docs/03 §2 `repository_discover`,
//! `repository_validate_path`).
//!
//! The walk here is the bounded, caller-driven one from the command table:
//! descend a root up to a depth limit and report every repository found. The
//! *streaming* indexer that watches configured search roots and emits
//! `IndexerPhase` / `IndexerResults` batches is `policy::finder` (docs/02 §8)
//! and lives elsewhere; this module has no state of its own.
//!
//! Discovery never follows directory symlinks, so it cannot loop, and it never
//! descends into a repository it found — the directories below a repository
//! root belong to it (that is also how nested submodule checkouts stay out of
//! the result until they are asked for explicitly).

use std::ffi::OsStr;
use std::path::Path;

use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::domain::repository::{PathCheck, PathKind, RepoRoot};

/// The descent limit `repository_discover` uses when the caller gives none.
pub const DEFAULT_MAX_DEPTH: u32 = 4;

/// Knobs for [`discover`] — the command layer maps its args onto this.
#[derive(Clone, Debug)]
pub struct DiscoverOptions {
    /// Depth below the root to inspect; `0` means no limit.
    pub max_depth: u32,
    /// Directory names never entered (exact matches).
    pub ignored: Vec<String>,
    /// Skip directories whose name starts with `.` (a repository is still
    /// detected when the *root itself* is hidden — only descent is skipped).
    pub ignore_hidden: bool,
    /// Report standalone bare repositories (`repo.git` layouts).
    pub include_bare: bool,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            ignored: Vec::new(),
            ignore_hidden: true,
            include_bare: true,
        }
    }
}

/// Walk `root` and return every repository root found, sorted by path.
///
/// The root itself is inspected first; when it is a repository the result is
/// exactly that one. Unreadable subdirectories are skipped — a bounded search
/// is best-effort — but an unreadable or missing *root* is an error, because
/// the caller asked for something the operation cannot deliver at all.
pub fn discover(root: &Path, opts: &DiscoverOptions) -> Result<Vec<RepoRoot>, ErrorPayload> {
    if !root.is_dir() {
        return Err(ErrorPayload::new(
            ErrorCode::Io,
            format!("Not a directory: {}", root.display()),
        )
        .with_domain(ErrorDomain::Fs)
        .with_param("path", root.display().to_string()));
    }
    std::fs::read_dir(root).map_err(|err| {
        ErrorPayload::new(
            ErrorCode::Io,
            format!("Cannot read {}: {err}", root.display()),
        )
        .with_domain(ErrorDomain::Fs)
        .with_param("path", root.display().to_string())
    })?;

    let mut roots = Vec::new();
    walk(root, 0, opts, &mut roots);
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(roots)
}

fn walk(dir: &Path, depth: u32, opts: &DiscoverOptions, out: &mut Vec<RepoRoot>) {
    if let Some(root) = probe_dir(dir, opts) {
        out.push(root);
        // Everything below a repository root belongs to it.
        return;
    }
    if opts.max_depth != 0 && depth >= opts.max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if opts.ignore_hidden && name.starts_with('.') {
            continue;
        }
        if opts.ignored.iter().any(|ignored| ignored == name) {
            continue;
        }
        // `file_type` does not follow symlinks: a symlinked directory is
        // skipped, never followed, so the walk cannot cycle.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            walk(&entry.path(), depth + 1, opts, out);
        }
    }
}

/// Probe exactly this directory (no upward search) and describe it as a root.
fn probe_dir(dir: &Path, opts: &DiscoverOptions) -> Option<RepoRoot> {
    let repo =
        git2::Repository::open_ext(dir, git2::RepositoryOpenFlags::NO_SEARCH, &[] as &[&OsStr])
            .ok()?;
    if repo.is_bare() && !opts.include_bare {
        return None;
    }
    Some(RepoRoot {
        name: dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir.to_string_lossy().into_owned()),
        path: dir.to_string_lossy().into_owned(),
        gitdir: repo.path().to_string_lossy().into_owned(),
        workdir: repo.workdir().map(|p| p.to_string_lossy().into_owned()),
        is_bare: repo.is_bare(),
        is_worktree: dir.join(".git").is_file(),
    })
}

/// Classify a path without opening it as the current repository — the
/// `repository_validate_path` probe (docs/03 §2). Never fails: "this is not a
/// repository" is an answer, not an error, and the caller shows it as one.
pub fn probe_path(path: &Path) -> PathCheck {
    let path_str = path.to_string_lossy().into_owned();
    let bare_check = |kind: PathKind, detail: Option<String>| PathCheck {
        path: path_str.clone(),
        kind,
        detail,
        gitdir: None,
        workdir: None,
    };

    if !path.exists() {
        return bare_check(PathKind::Missing, None);
    }
    if !path.is_dir() {
        // A file can still live inside a working tree — say which repository
        // owns it, because "open the file's repository" is a real gesture.
        let parent = path.parent().unwrap_or(path);
        return match git2::Repository::discover(parent) {
            Ok(repo) => PathCheck {
                path: path_str,
                kind: PathKind::ParentRepository,
                detail: Some("Inside the working tree of a repository".to_string()),
                gitdir: Some(repo.path().to_string_lossy().into_owned()),
                workdir: repo.workdir().map(|p| p.to_string_lossy().into_owned()),
            },
            Err(_) => bare_check(
                PathKind::NotDirectory,
                Some("The path is a file".to_string()),
            ),
        };
    }
    if std::fs::read_dir(path).is_err() {
        return bare_check(
            PathKind::Unreadable,
            Some("The directory cannot be read".to_string()),
        );
    }

    match git2::Repository::open_ext(path, git2::RepositoryOpenFlags::NO_SEARCH, &[] as &[&OsStr]) {
        Ok(repo) => {
            let kind = if repo.is_bare() {
                PathKind::BareRepository
            } else if path.join(".git").is_file() {
                PathKind::Worktree
            } else {
                PathKind::Repository
            };
            PathCheck {
                path: path_str,
                kind,
                detail: None,
                gitdir: Some(repo.path().to_string_lossy().into_owned()),
                workdir: repo.workdir().map(|p| p.to_string_lossy().into_owned()),
            }
        }
        Err(_) => match git2::Repository::discover(path) {
            Ok(repo) => PathCheck {
                path: path_str,
                kind: PathKind::ParentRepository,
                detail: Some(format!(
                    "Inside the working tree of {}",
                    repo.workdir().unwrap_or_else(|| repo.path()).display()
                )),
                gitdir: Some(repo.path().to_string_lossy().into_owned()),
                workdir: repo.workdir().map(|p| p.to_string_lossy().into_owned()),
            },
            Err(_) => bare_check(PathKind::NoRepository, None),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "meridian-discovery-test-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn probe_classifies_paths_without_opening_them() {
        let dir = temp_dir("probe");
        let repo_dir = dir.join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git2::Repository::init(&repo_dir).unwrap();
        let sub = repo_dir.join("src");
        std::fs::create_dir_all(&sub).unwrap();
        let inside_file = repo_dir.join("tracked.txt");
        std::fs::write(&inside_file, "x").unwrap();
        let file = dir.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let bare_dir = dir.join("bare.git");
        git2::Repository::init_bare(&bare_dir).unwrap();

        assert_eq!(probe_path(&dir.join("nope")).kind, PathKind::Missing);
        assert_eq!(probe_path(&file).kind, PathKind::NotDirectory);
        assert_eq!(
            probe_path(&inside_file).kind,
            PathKind::ParentRepository,
            "a file inside a working tree reports its repository"
        );
        assert_eq!(probe_path(&repo_dir).kind, PathKind::Repository);
        assert_eq!(probe_path(&sub).kind, PathKind::ParentRepository);
        assert_eq!(probe_path(&bare_dir).kind, PathKind::BareRepository);
        assert_eq!(probe_path(&dir).kind, PathKind::NoRepository);

        let check = probe_path(&repo_dir);
        assert!(check.gitdir.is_some());
        assert!(check.workdir.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_walks_but_never_inside_a_repository() {
        let dir = temp_dir("walk");
        let root = dir.join("root");
        let a = root.join("a");
        let nested = root.join("b").join("c");
        for path in [&a, &nested] {
            std::fs::create_dir_all(path).unwrap();
            git2::Repository::init(path).unwrap();
        }
        // A repository inside a repository belongs to the outer one: the walk
        // reports `a`, never `a/inner`.
        let inner = a.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        git2::Repository::init(&inner).unwrap();

        let found = discover(&root, &DiscoverOptions::default()).unwrap();
        let paths: Vec<&str> = found.iter().map(|entry| entry.path.as_str()).collect();
        assert_eq!(found.len(), 2, "{paths:?}");
        assert!(paths.iter().any(|path| path.ends_with("/a")), "{paths:?}");
        assert!(paths.iter().any(|path| path.ends_with("/b/c")), "{paths:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_respects_the_depth_limit() {
        let dir = temp_dir("depth");
        let root = dir.join("root");
        let deep = root.join("one").join("two").join("three");
        std::fs::create_dir_all(&deep).unwrap();
        git2::Repository::init(&deep).unwrap();

        let shallow = discover(
            &root,
            &DiscoverOptions {
                max_depth: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(shallow.is_empty());

        // 0 means no limit.
        let full = discover(
            &root,
            &DiscoverOptions {
                max_depth: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(full.len(), 1);
        assert!(full[0].path.ends_with("three"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_reports_the_root_itself() {
        let dir = temp_dir("root");
        let repo_dir = dir.join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git2::Repository::init(&repo_dir).unwrap();

        let found = discover(&repo_dir, &DiscoverOptions::default()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "repo");
        assert!(!found[0].is_bare);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_refuses_an_unreadable_root() {
        let dir = temp_dir("missing");
        let err = discover(&dir.join("nope"), &DiscoverOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
        std::fs::remove_dir_all(&dir).ok();
    }
}
