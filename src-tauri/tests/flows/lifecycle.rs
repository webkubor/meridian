use std::fs;

use git2::Repository;
use meridian::commands::{RepositoryCloneArgs, RepositoryInitArgs, RepositoryOpenArgs};
use meridian::domain::error::ErrorCode;
use serde_json::json;

use super::support::{assert_error_events, repos_changed, Fixture};

#[tokio::test]
async fn repository_init_json_creates_a_repo_and_emits_repos_changed() {
    let fixture = Fixture::new();
    let destination = fixture._temp.path().join("new-repo");
    let args: RepositoryInitArgs = fixture.wire(json!({
        "path": destination,
        "bare": false,
        "initialBranch": "main",
        "createGitignore": true
    }));
    let sink = fixture.sink();
    let info = args
        .execute(&fixture.state, &sink)
        .expect("init via lifecycle command path");
    assert_eq!(info.head_branch.as_deref(), Some("main"));
    assert!(destination.join(".gitignore").exists());
    assert_eq!(fixture.take_events(&sink), vec![repos_changed()]);
}

#[tokio::test]
async fn repository_init_rejects_invalid_initial_branch() {
    let fixture = Fixture::new();
    let destination = fixture._temp.path().join("bad-repo");
    let bad_path = destination.clone();
    let args: RepositoryInitArgs = fixture.wire(json!({
        "path": destination,
        "initialBranch": "bad name"
    }));
    let sink = fixture.sink();
    let error = args
        .execute(&fixture.state, &sink)
        .expect_err("invalid branch must be rejected");
    assert_eq!(ErrorCode::InvalidReferenceName, error.code);
    assert!(!error.message.trim().is_empty());
    assert!(fixture.take_events(&sink).is_empty());
    assert!(!bad_path.exists());
}

#[test]
fn repository_open_supports_nested_repository_discovery_and_rejects_non_repo() {
    let fixture = Fixture::new();
    let nested = fixture.root.join("nested").join("folder");
    fs::create_dir_all(&nested).expect("create nested path");
    let args: RepositoryOpenArgs = fixture.wire(json!({ "path": nested }));
    let sink = fixture.sink();
    let info = args
        .execute(&fixture.state, &sink)
        .expect("discover parent repository");
    assert!(info.discovered);
    assert_eq!(info.id, fixture.repo_id);
    assert_eq!(fixture.take_events(&sink), vec![repos_changed()]);

    let not_repo = fixture._temp.path().join("not-a-repo");
    fs::create_dir_all(&not_repo).expect("create non-repository directory");
    let open_args: RepositoryOpenArgs = fixture.wire(json!({ "path": not_repo }));
    let sink = fixture.sink();
    let error = open_args
        .execute(&fixture.state, &sink)
        .expect_err("non-repository must fail visibly");
    assert_eq!(ErrorCode::InvalidRepository, error.code);
    assert!(!error.message.trim().is_empty());
    assert_error_events(fixture.take_events(&sink), vec![]);
}

#[test]
fn repository_open_accepts_bare_repositories() {
    let fixture = Fixture::new();
    let bare_path = fixture._temp.path().join("origin.git");
    Repository::init_bare(&bare_path).expect("create bare repository");
    let open_args: RepositoryOpenArgs = fixture.wire(json!({ "path": bare_path }));
    let sink = fixture.sink();
    let info = open_args
        .execute(&fixture.state, &sink)
        .expect("open bare repository");
    assert!(!info.discovered);
    assert!(info.is_bare);
    assert_eq!(fixture.take_events(&sink), vec![repos_changed()]);
}

#[tokio::test]
async fn repository_clone_from_local_bare_origin_succeeds_and_bad_origin_reports_error() {
    let fixture = Fixture::new();
    let source = fixture.commit_file("README", "origin\n", "initial");
    let bare = fixture._temp.path().join("origin.git");
    let mut builder = git2::build::RepoBuilder::new();
    builder.bare(true);
    builder
        .clone(fixture.root.to_string_lossy().as_ref(), &bare)
        .expect("create local bare origin from fixture repository");
    // git2 clone is the actual engine path used by the Tauri lifecycle wrapper.
    let cloned_path = fixture._temp.path().join("clone");
    let clone_args: RepositoryCloneArgs = fixture.wire(json!({
        "url": bare.to_string_lossy(), "destination": cloned_path, "initialBranch": "main"
    }));
    let sink = fixture.sink();
    let cloned_info = clone_args
        .execute(&fixture.state, &sink)
        .await
        .expect("clone local bare origin through the lifecycle command");
    assert_eq!(fixture.take_events(&sink), vec![repos_changed()]);
    let cloned = Repository::open(cloned_info.workdir.unwrap()).expect("open cloned repo");
    assert_eq!(
        cloned
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
            .to_string(),
        source
    );

    let error_path = fixture._temp.path().join("bad-clone");
    let bad_args: RepositoryCloneArgs = fixture.wire(json!({
        "url": "/definitely/not/a/repository", "destination": error_path
    }));
    let sink = fixture.sink();
    let error = bad_args
        .execute(&fixture.state, &sink)
        .await
        .expect_err("unavailable origin must return a visible error");
    assert!(!error.message.trim().is_empty());
    assert!(fixture.take_events(&sink).is_empty());
}
