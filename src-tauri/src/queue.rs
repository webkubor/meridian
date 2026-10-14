use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::domain::ids::RepoId;

/// One serialized queue per repository key (docs/02 §5; evidence F7).
///
/// Mutating commands await their repository's turn, so two commands on the
/// same repository run in arrival order while different repositories stay
/// parallel. Read-only loads (`history_commits_load`, `wc_status_load`) bypass
/// the queue — they are idempotent and cheap to coalesce.
#[derive(Default)]
pub struct RepoQueueManager {
    queues: StdMutex<HashMap<RepoId, Arc<Mutex<()>>>>,
}

impl RepoQueueManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Await this repository's turn. The returned owned guard releases the
    /// queue when the command finishes (including on panic-unwind). Fairness
    /// within one repository comes from tokio's fair mutex.
    pub async fn enqueue(&self, repo: &RepoId) -> OwnedMutexGuard<()> {
        let queue = {
            let mut queues = self
                .queues
                .lock()
                .expect("repo queue registry poisoned by a panicking command");
            Arc::clone(queues.entry(repo.clone()).or_default())
        };
        queue.lock_owned().await
    }
}
