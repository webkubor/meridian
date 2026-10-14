use std::fs;
use std::path::{Path, PathBuf};

use git2::{Repository, Signature};
use meridian::commands::Command;
use meridian::domain::error::{ErrorCode, ErrorPayload};
use meridian::domain::ids::RepoId;
use meridian::events::{AppEvent, CollectingEventSink};
use meridian::state::AppState;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tempfile::TempDir;

pub struct Fixture {
    pub _temp: TempDir,
    pub root: PathBuf,
    pub state: AppState,
    pub repo_id: RepoId,
}

/// libgit2 reads the user's global config, XDG config and system config, and
/// `core.excludesfile` among them. A developer whose global excludes contain
/// `*.txt` would see unrelated flow tests fail, so every search path points at
/// an empty directory before the first repository is opened. This is the only
/// place the process-wide search paths are touched, and `Once` makes sure it
/// happens before any fixture thread proceeds.
fn isolate_libgit2_config() {
    static ISOLATED: std::sync::Once = std::sync::Once::new();
    ISOLATED.call_once(|| {
        let empty =
            std::env::temp_dir().join(format!("meridian-flows-gitconfig-{}", std::process::id()));
        fs::create_dir_all(&empty).expect("create empty git config directory");
        for level in [
            git2::ConfigLevel::System,
            git2::ConfigLevel::XDG,
            git2::ConfigLevel::Global,
        ] {
            // SAFETY: documented as not thread-safe, which is why it runs once
            // under `Once` before any repository is opened in this process.
            unsafe { git2::opts::set_search_path(level, &empty) }
                .expect("point libgit2 config search path at an empty directory");
        }
    });
}

impl Fixture {
    pub fn new() -> Self {
        isolate_libgit2_config();
        let temp = tempfile::tempdir().expect("create isolated fixture directory");
        let root = temp.path().join("repo");
        fs::create_dir_all(&root).expect("create repository directory");
        let repo = meridian::engine::repository::init(
            &root,
            &meridian::engine::repository::InitOptions {
                bare: false,
                initial_branch: "main".to_string(),
                create_gitignore: false,
            },
        )
        .expect("init fixture repository");
        configure_identity(&repo);
        let repo_id = meridian::engine::repository::info(&repo)
            .expect("describe fixture repository")
            .id;
        let state = AppState::new();
        state
            .insert_repo(repo)
            .expect("register fixture repository");
        Self {
            _temp: temp,
            root,
            state,
            repo_id,
        }
    }

    pub fn repo(&self) -> Repository {
        Repository::open(&self.root).expect("open fixture repository")
    }

    pub fn commit_file(&self, path: &str, contents: &str, message: &str) -> String {
        self.write(path, contents);
        let repo = self.repo();
        let mut index = repo.index().expect("open fixture index");
        index.add_path(Path::new(path)).expect("stage fixture file");
        index.write().expect("write fixture index");
        let tree_id = index.write_tree().expect("write fixture tree");
        let tree = repo.find_tree(tree_id).expect("read fixture tree");
        let signature = Signature::now("Flow Test", "flow@example.test").expect("signature");
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents = parent.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("create fixture commit");
        // The index was written before the commit, so it already matches the
        // tree; the mixed reset only re-reads the committed state so a later
        // status read does not see a stale index entry.
        let commit = repo.find_commit(oid).expect("load fixture commit");
        repo.reset(commit.as_object(), git2::ResetType::Mixed, None)
            .expect("sync fixture index with committed tree");
        drop(commit);
        drop(tree);
        drop(index);
        drop(parents);
        drop(parent);
        drop(repo);
        let reopened = Repository::open(&self.root).expect("reopen repository after setup commit");
        self.state
            .insert_repo(reopened)
            .expect("refresh registered fixture repository after setup commit");
        oid.to_string()
    }

    /// Commit a change onto another branch without checking it out.
    ///
    /// The work tree and index are put back exactly as the checked-out branch
    /// had them, so a test can build a divergent branch without paying for a
    /// branch switch (whose stale-index behaviour is a separate bug with its
    /// own ignored test).
    pub fn commit_on_branch(
        &self,
        branch: &str,
        path: &str,
        contents: &str,
        message: &str,
    ) -> String {
        let repo = self.repo();
        let full = format!("refs/heads/{branch}");
        let parent = repo
            .find_reference(&full)
            .expect("branch exists")
            .peel_to_commit()
            .expect("branch has a commit");

        self.write(path, contents);
        let mut index = repo.index().expect("open index");
        index.add_path(Path::new(path)).expect("stage scratch file");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().unwrap())
            .expect("read tree");
        let signature = Signature::now("Flow Test", "flow@example.test").expect("signature");
        let oid = repo
            .commit(
                Some(&full),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )
            .expect("commit on branch");
        drop(index);
        drop(tree);
        drop(parent);

        // Undo the scratch edit in the checked-out branch's terms: reset the
        // index to HEAD, then put the file back to HEAD's version or remove it
        // when HEAD does not track it.
        let head = repo
            .head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("HEAD commit");
        repo.reset(head.as_object(), git2::ResetType::Mixed, None)
            .expect("reset index to HEAD");
        if head
            .tree()
            .expect("HEAD tree")
            .get_path(Path::new(path))
            .is_ok()
        {
            let mut builder = git2::build::CheckoutBuilder::new();
            builder.force().path(path);
            repo.checkout_head(Some(&mut builder))
                .expect("restore HEAD version of the file");
        } else {
            fs::remove_file(self.root.join(path)).ok();
        }
        drop(head);
        drop(repo);

        let reopened = Repository::open(&self.root).expect("reopen repository");
        self.state
            .insert_repo(reopened)
            .expect("refresh registered fixture repository");
        oid.to_string()
    }

    pub fn write(&self, path: &str, contents: &str) {
        let full = self.root.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create fixture directory");
        }
        fs::write(full, contents).expect("write fixture file");
    }

    pub fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).unwrap_or_default()
    }

    pub fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }

    pub fn head_branch(&self) -> String {
        self.repo()
            .head()
            .expect("HEAD resolves")
            .shorthand()
            .expect("HEAD names a branch")
            .to_string()
    }

    pub fn wire<T: DeserializeOwned>(&self, args: Value) -> T {
        serde_json::from_value(args).expect("TypeScript-compatible JSON args deserialize")
    }

    pub fn repo_arg(&self) -> Value {
        json!(self.repo_id)
    }

    pub async fn run<C: Command>(
        &self,
        command: C,
    ) -> (Result<C::Output, ErrorPayload>, Vec<AppEvent>) {
        self.state.run_headless(command).await
    }

    pub fn sink(&self) -> CollectingEventSink {
        CollectingEventSink::new()
    }

    pub fn take_events(&self, sink: &CollectingEventSink) -> Vec<AppEvent> {
        sink.take()
    }
}

#[track_caller]
pub fn assert_ok<T>(
    result: Result<T, ErrorPayload>,
    events: Vec<AppEvent>,
    expected_events: Vec<AppEvent>,
) -> T {
    let value = result.unwrap_or_else(|error| panic!("expected success, got {error:?}"));
    assert_eq!(events, expected_events, "unexpected command events");
    value
}

pub fn assert_error_events(events: Vec<AppEvent>, expected_events: Vec<AppEvent>) {
    assert_eq!(
        events, expected_events,
        "failed command emitted stale events"
    );
}

pub fn assert_error(result: Result<impl std::fmt::Debug, ErrorPayload>, code: ErrorCode) {
    let error = result.expect_err("expected command to fail");
    assert_eq!(error.code, code, "unexpected error payload: {error:?}");
    assert!(
        !error.message.trim().is_empty(),
        "user-facing error is empty"
    );
}

pub fn changed(repo: &RepoId) -> AppEvent {
    AppEvent::WorkingCopyChanged { repo: repo.clone() }
}

pub fn refs_changed(repo: &RepoId) -> AppEvent {
    AppEvent::RefsChanged { repo: repo.clone() }
}

pub fn repos_changed() -> AppEvent {
    AppEvent::ReposChanged
}

fn configure_identity(repo: &Repository) {
    let mut config = repo.config().expect("open fixture config");
    config
        .set_str("user.name", "Flow Test")
        .expect("set fixture user");
    config
        .set_str("user.email", "flow@example.test")
        .expect("set fixture email");
}
