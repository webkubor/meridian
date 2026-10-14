use git2::Status;

use crate::domain::status::{EntryStatus, FileStatus};

/// Map one packed `git2::Status` into **one or two** domain entries.
///
/// The mapping is 1:N by design (docs/02 §4): libgit2 packs the index bits and
/// the working-tree bits into one value, while Meridian keeps the two axes as
/// distinct display rows.
pub fn from_git2(path: String, status: Status, assume_unchanged: bool) -> Vec<EntryStatus> {
    let mut entries = Vec::with_capacity(2);

    if let Some(code) = index_code(status) {
        entries.push(EntryStatus::new(path.clone(), code, true, assume_unchanged));
    }
    if let Some(code) = workdir_code(status) {
        entries.push(EntryStatus::new(path, code, false, assume_unchanged));
    }

    entries
}

fn index_code(status: Status) -> Option<FileStatus> {
    if status.contains(Status::CONFLICTED) {
        Some(FileStatus::Unmerged)
    } else if status.contains(Status::INDEX_NEW) {
        Some(FileStatus::Added)
    } else if status.contains(Status::INDEX_RENAMED) {
        Some(FileStatus::Renamed)
    } else if status.contains(Status::INDEX_DELETED) {
        Some(FileStatus::Deleted)
    } else if status.contains(Status::INDEX_MODIFIED) || status.contains(Status::INDEX_TYPECHANGE) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}

fn workdir_code(status: Status) -> Option<FileStatus> {
    if status.contains(Status::WT_NEW) {
        Some(FileStatus::Untracked)
    } else if status.contains(Status::WT_RENAMED) {
        Some(FileStatus::Renamed)
    } else if status.contains(Status::WT_DELETED) {
        Some(FileStatus::Deleted)
    } else if status.contains(Status::WT_MODIFIED) || status.contains(Status::WT_TYPECHANGE) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}
