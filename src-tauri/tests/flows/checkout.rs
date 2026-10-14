//! Switching branches with a dirty working copy, in the situations people are
//! actually in: an edit the target branch does not touch, an edit it does,
//! files inside directories, files the target lacks, and every verb that moves
//! HEAD (`branch_checkout`, `branch_checkout_revision`, `branch_checkout_remote`,
//! create-with-checkout, `branch_fast_forward`).
//!
//! Each case states what `git checkout` does, because that is the behaviour a
//! Git client has to match: refuse and change nothing when a local change would
//! be overwritten, and otherwise carry local changes across untouched.

use std::path::Path;

use meridian::commands::{
    BranchCheckout, BranchCheckoutRemote, BranchCheckoutRevision, BranchCreate, BranchFastForward,
    WcStagePaths,
};
use meridian::domain::error::{ErrorCode, ErrorPayload};
use meridian::domain::refs::CheckoutResult;
use meridian::events::AppEvent;
use serde_json::json;

use super::support::{
    assert_error, assert_error_events, assert_ok, changed, refs_changed, Fixture,
};

async fn make_branch(fixture: &Fixture, name: &str) {
    let create: BranchCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": name }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
}

async fn checkout(
    fixture: &Fixture,
    name: &str,
) -> (Result<CheckoutResult, ErrorPayload>, Vec<AppEvent>) {
    let command: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": name, "opts": {} }));
    fixture.run(command).await
}

fn status_of(fixture: &Fixture, path: &str) -> git2::Status {
    fixture
        .repo()
        .status_file(Path::new(path))
        .unwrap_or(git2::Status::CURRENT)
}

async fn stage(fixture: &Fixture, path: &str) {
    let command: WcStagePaths =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": [path] }));
    let (result, events) = fixture.run(command).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);
}

/// Refused: nothing moved, nothing written, no event for a change that did not
/// happen.
fn assert_refused(
    result: Result<impl std::fmt::Debug, ErrorPayload>,
    events: Vec<AppEvent>,
    fixture: &Fixture,
) {
    let error = result.expect_err("the checkout must be refused");
    assert_eq!(error.code, ErrorCode::CheckoutConflicts, "{error:?}");
    assert!(
        error
            .detail
            .as_deref()
            .is_some_and(|detail| !detail.is_empty()),
        "the refusal must name the paths in its detail: {error:?}"
    );
    assert_error_events(events, vec![]);
    assert_eq!(fixture.head_branch(), "main", "HEAD must not move");
}

// ---- carried across: git switches and keeps the edit -----------------------

/// P2. `git checkout side` succeeds and leaves `a.txt` modified, because `side`
/// only differs from `main` in `b.txt`.
#[tokio::test]
async fn a_dirty_file_the_target_does_not_change_is_carried_over() {
    let fixture = Fixture::new();
    fixture.commit_file("a.txt", "a1\n", "a");
    fixture.commit_file("b.txt", "b1\n", "b");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "b.txt", "b-from-side\n", "side edits b");

    fixture.write("a.txt", "my unsaved work\n");
    let (result, events) = checkout(&fixture, "side").await;
    let result = assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(result.branch.as_deref(), Some("side"));
    assert_eq!(fixture.head_branch(), "side");
    assert_eq!(fixture.read("a.txt"), "my unsaved work\n");
    assert_eq!(fixture.read("b.txt"), "b-from-side\n");
    assert!(status_of(&fixture, "a.txt").contains(git2::Status::WT_MODIFIED));
}

/// P4. The same with the edit already staged: git keeps it staged.
#[tokio::test]
async fn a_staged_edit_the_target_does_not_change_stays_staged() {
    let fixture = Fixture::new();
    fixture.commit_file("a.txt", "a1\n", "a");
    fixture.commit_file("b.txt", "b1\n", "b");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "b.txt", "b-from-side\n", "side edits b");

    fixture.write("a.txt", "staged work\n");
    stage(&fixture, "a.txt").await;
    let (result, events) = checkout(&fixture, "side").await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(fixture.head_branch(), "side");
    assert_eq!(fixture.read("a.txt"), "staged work\n");
    let status = status_of(&fixture, "a.txt");
    assert!(
        status.contains(git2::Status::INDEX_MODIFIED),
        "the edit must still be staged, got {status:?}"
    );
}

// ---- refused: git refuses and changes nothing ------------------------------

/// P1. Nearly all real code lives in directories, which is where the original
/// check could not see: `Tree::get_name` looks one level deep only.
#[tokio::test]
async fn a_dirty_file_inside_a_directory_the_target_changes_is_refused() {
    let fixture = Fixture::new();
    fixture.commit_file("src/deep/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "src/deep/file.txt", "side version\n", "side edit");

    fixture.write("src/deep/file.txt", "my unsaved work\n");
    let (result, events) = checkout(&fixture, "side").await;
    assert_refused(result, events, &fixture);
    assert_eq!(fixture.read("src/deep/file.txt"), "my unsaved work\n");
}

#[tokio::test]
async fn a_staged_edit_inside_a_directory_the_target_changes_is_refused() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "src/file.txt", "side version\n", "side edit");

    fixture.write("src/file.txt", "staged work\n");
    stage(&fixture, "src/file.txt").await;
    let (result, events) = checkout(&fixture, "side").await;
    assert_refused(result, events, &fixture);
    assert_eq!(fixture.read("src/file.txt"), "staged work\n");
    assert!(status_of(&fixture, "src/file.txt").contains(git2::Status::INDEX_MODIFIED));
}

#[tokio::test]
async fn an_untracked_file_inside_a_directory_the_target_tracks_is_refused() {
    let fixture = Fixture::new();
    fixture.commit_file("base.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "docs/guide/new.txt", "tracked on side\n", "add doc");

    fixture.write("docs/guide/new.txt", "MY UNTRACKED WORK\n");
    let (result, events) = checkout(&fixture, "side").await;
    assert_refused(result, events, &fixture);
    assert_eq!(fixture.read("docs/guide/new.txt"), "MY UNTRACKED WORK\n");
}

// ---- cleaned up: files the target does not have ----------------------------

/// P3. `git checkout main` removes `newdir/incoming.txt` (and the directory it
/// leaves empty) and reports a clean tree.
#[tokio::test]
async fn paths_the_target_lacks_are_removed_even_inside_directories() {
    let fixture = Fixture::new();
    fixture.commit_file("base.txt", "base\n", "base");
    let create: BranchCreate = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "incoming", "checkout": true
    }));
    let (result, events) = fixture.run(create).await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    fixture.commit_file("newdir/incoming.txt", "incoming\n", "add incoming file");

    let (result, events) = checkout(&fixture, "main").await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert!(!fixture.exists("newdir/incoming.txt"));
    assert!(!fixture.exists("newdir"), "the emptied directory goes too");
    assert!(
        fixture
            .repo()
            .statuses(None)
            .expect("statuses")
            .iter()
            .next()
            .is_none(),
        "switching back must leave a clean working copy"
    );
}

// ---- the other verbs that move HEAD ----------------------------------------

#[tokio::test]
async fn checking_out_a_revision_refuses_before_moving_head() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    let side_tip = fixture.commit_on_branch("side", "src/file.txt", "side version\n", "side edit");

    fixture.write("src/file.txt", "my unsaved work\n");
    let detach: BranchCheckoutRevision = fixture.wire(json!({
        "repo": fixture.repo_arg(),
        "revision": { "kind": "oid", "value": side_tip },
        "detach": true
    }));
    let (result, events) = fixture.run(detach).await;
    assert_refused(result, events, &fixture);
    assert_eq!(fixture.read("src/file.txt"), "my unsaved work\n");

    // The attached form refuses the same way.
    let attach: BranchCheckoutRevision = fixture.wire(json!({
        "repo": fixture.repo_arg(),
        "revision": { "kind": "branch", "value": "side" }
    }));
    let (result, events) = fixture.run(attach).await;
    assert_refused(result, events, &fixture);
}

#[tokio::test]
async fn checking_out_a_remote_branch_refuses_and_leaves_no_local_branch_behind() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    let side_tip = fixture.commit_on_branch("side", "src/file.txt", "side version\n", "side edit");
    // Stand in for a fetched remote-tracking branch without a network: the
    // remote only has to exist so `track` can write its configuration.
    fixture
        .repo()
        .remote("origin", "https://example.invalid/team/repo.git")
        .expect("configure remote");
    fixture
        .repo()
        .reference(
            "refs/remotes/origin/feature",
            git2::Oid::from_str(&side_tip).expect("oid"),
            true,
            "test: pretend a fetch",
        )
        .expect("remote-tracking ref");

    fixture.write("src/file.txt", "my unsaved work\n");
    let command: BranchCheckoutRemote = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remoteBranch": "origin/feature", "track": true
    }));
    let (result, events) = fixture.run(command).await;
    assert_refused(result, events, &fixture);
    assert!(
        fixture
            .repo()
            .find_branch("feature", git2::BranchType::Local)
            .is_err(),
        "a refused checkout must not leave the new local branch behind"
    );
    assert_eq!(fixture.read("src/file.txt"), "my unsaved work\n");

    // With a clean tree the same request works, and tracks.
    fixture.write("src/file.txt", "base\n");
    let command: BranchCheckoutRemote = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remoteBranch": "origin/feature", "track": true
    }));
    let (result, events) = fixture.run(command).await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(fixture.head_branch(), "feature");
    assert_eq!(fixture.read("src/file.txt"), "side version\n");
}

#[tokio::test]
async fn create_with_checkout_refuses_and_does_not_create_the_branch() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "src/file.txt", "side version\n", "side edit");

    fixture.write("src/file.txt", "my unsaved work\n");
    let create: BranchCreate = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "from-side", "checkout": true,
        "start": { "kind": "branch", "value": "side" }
    }));
    let (result, events) = fixture.run(create).await;
    assert_refused(result, events, &fixture);
    assert!(
        fixture
            .repo()
            .find_branch("from-side", git2::BranchType::Local)
            .is_err(),
        "git checkout -b refuses without creating the branch"
    );
    assert_eq!(fixture.read("src/file.txt"), "my unsaved work\n");
}

#[tokio::test]
async fn fast_forwarding_the_current_branch_refuses_instead_of_clobbering() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "ahead").await;
    let ahead_tip =
        fixture.commit_on_branch("ahead", "src/file.txt", "ahead version\n", "ahead edit");
    let main_before = fixture
        .repo()
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id();

    fixture.write("src/file.txt", "my unsaved work\n");
    let command: BranchFastForward = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "main",
        "to": { "kind": "oid", "value": ahead_tip }
    }));
    let (result, events) = fixture.run(command).await;
    assert_refused(result, events, &fixture);
    assert_eq!(
        fixture
            .repo()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        main_before,
        "the branch must not move when its working copy cannot follow"
    );
    assert_eq!(fixture.read("src/file.txt"), "my unsaved work\n");
}

/// A refusal is retryable: once the blocker is gone the same request works.
#[tokio::test]
async fn a_refused_checkout_succeeds_once_the_local_change_is_discarded() {
    let fixture = Fixture::new();
    fixture.commit_file("src/file.txt", "base\n", "base");
    make_branch(&fixture, "side").await;
    fixture.commit_on_branch("side", "src/file.txt", "side version\n", "side edit");

    fixture.write("src/file.txt", "my unsaved work\n");
    let (result, events) = checkout(&fixture, "side").await;
    assert_error(result, ErrorCode::CheckoutConflicts);
    assert_error_events(events, vec![]);

    fixture.write("src/file.txt", "base\n");
    let (result, events) = checkout(&fixture, "side").await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(fixture.read("src/file.txt"), "side version\n");
}
