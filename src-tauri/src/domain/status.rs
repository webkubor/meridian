use serde::{Deserialize, Serialize};

use super::ids::RepoId;

/// The file-status families the evidence documents (docs/02 §4; evidence F2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Added,
    Copied,
    Deleted,
    Modified,
    Renamed,
    Unmerged,
    Untracked,
    Ignored,
}

/// One status entry, on exactly one axis: the index (`staged: true`) or the
/// working tree (`staged: false`). A file can legitimately appear twice — the
/// packed git2 status bitfield is 1:N over these (docs/02 §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryStatus {
    pub path: String,
    pub code: FileStatus,
    pub staged: bool,
    pub assume_unchanged: bool,
    /// The derived rendering (see [`UiStatusView`]). Shipped with the entry so
    /// the web layer never computes status strings itself.
    pub view: UiStatusView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Neutral,
    Positive,
    Info,
    Danger,
    Warning,
    Muted,
}

/// The single rendering the UI is allowed to use — "one enum, three
/// renderings": short letter, label, tone (docs/02 §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiStatusView {
    pub short: char,
    pub text: String,
    pub tone: Tone,
}

impl FileStatus {
    pub fn short(self) -> char {
        match self {
            FileStatus::Added => 'A',
            FileStatus::Copied => 'C',
            FileStatus::Deleted => 'D',
            FileStatus::Modified => 'M',
            FileStatus::Renamed => 'R',
            FileStatus::Unmerged => 'U',
            FileStatus::Untracked => '?',
            FileStatus::Ignored => '!',
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            FileStatus::Added => "Added",
            FileStatus::Copied => "Copied",
            FileStatus::Deleted => "Deleted",
            FileStatus::Modified => "Modified",
            FileStatus::Renamed => "Renamed",
            FileStatus::Unmerged => "Unmerged",
            FileStatus::Untracked => "Untracked",
            FileStatus::Ignored => "Ignored",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            FileStatus::Added | FileStatus::Renamed => Tone::Positive,
            FileStatus::Copied => Tone::Info,
            FileStatus::Modified => Tone::Warning,
            FileStatus::Deleted | FileStatus::Unmerged => Tone::Danger,
            FileStatus::Untracked => Tone::Neutral,
            FileStatus::Ignored => Tone::Muted,
        }
    }
}

impl EntryStatus {
    /// The only constructor: the derived `view` can never drift from the code.
    pub fn new(
        path: impl Into<String>,
        code: FileStatus,
        staged: bool,
        assume_unchanged: bool,
    ) -> Self {
        let view = UiStatusView {
            short: code.short(),
            text: code.text().to_string(),
            tone: code.tone(),
        };
        Self {
            path: path.into(),
            code,
            staged,
            assume_unchanged,
            view,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSummary {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

impl StatusSummary {
    pub fn of(entries: &[EntryStatus]) -> Self {
        let mut summary = Self::default();
        for entry in entries {
            match (entry.code, entry.staged) {
                (FileStatus::Untracked, _) => summary.untracked += 1,
                (FileStatus::Unmerged, _) => summary.conflicted += 1,
                (_, true) => summary.staged += 1,
                (_, false) => summary.unstaged += 1,
            }
        }
        summary
    }
}

/// Result envelope of every working-copy mutation: the fresh entry list plus a
/// summary, so the UI never re-runs status itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDelta {
    pub repo: RepoId,
    pub entries: Vec<EntryStatus>,
    pub summary: StatusSummary,
}
