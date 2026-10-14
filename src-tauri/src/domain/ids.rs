use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// Identity of a repository: the canonicalised **gitdir** path.
///
/// Two worktrees of one repository share a `RepoId` but have distinct working
/// directories, so the UI addresses the pair (docs/02 §5). Wrapping the string
/// keeps the queue key, the future SQLite key and the command argument
/// identical everywhere.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(String);

impl RepoId {
    /// Canonicalise when the path exists; fall back to the given path while it
    /// does not (e.g. a repository being initialised).
    pub fn new(gitdir: impl AsRef<Path>) -> Self {
        let raw = gitdir.as_ref();
        let canonical = std::fs::canonicalize(raw).unwrap_or_else(|_| raw.to_path_buf());
        Self(canonical.to_string_lossy().into_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
