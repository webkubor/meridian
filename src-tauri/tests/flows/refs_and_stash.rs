use meridian::commands::{
    BranchCheckout, BranchCheckoutRevision, BranchCreate, BranchDelete, BranchRename,
    IntegrationContinue, MergeAbort, MergeStart, RemoteAdd, RemoteFetch, RemotePublishBranch,
    RemotePull, RemotePush, StashApply, StashDrop, StashList, StashPop, StashSave, TagCreate,
    TagDelete, WcStagePaths, WcStatusLoad,
};
use meridian::domain::error::ErrorCode;
use serde_json::json;

use super::support::{
    assert_error, assert_error_events, assert_ok, changed, refs_changed, repos_changed, Fixture,
};

#[tokio::test]
async fn branch_create_checkout_rename_delete_and_invalid_cases_follow_wire_contract() {
    let fixture = Fixture::new();
    fixture.commit_file("base.txt", "base\n", "base");

    let create: BranchCreate = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "feature/no-checkout", "checkout": false
    }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    assert_eq!(fixture.repo().head().unwrap().shorthand().unwrap(), "main");

    let checkout: BranchCreate = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "feature/checked-out", "checkout": true
    }));
    let (result, events) = fixture.run(checkout).await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(
        fixture.repo().head().unwrap().shorthand().unwrap(),
        "feature/checked-out"
    );

    let invalid: BranchCreate =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "bad name" }));
    let (result, events) = fixture.run(invalid).await;
    assert_error(result, ErrorCode::InvalidReferenceName);
    assert_error_events(events, vec![]);

    let duplicate: BranchCreate =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "feature/no-checkout" }));
    let (result, events) = fixture.run(duplicate).await;
    assert_error(result, ErrorCode::ReferenceExists);
    assert_error_events(events, vec![]);

    let rename: BranchRename = fixture.wire(json!({
        "repo": fixture.repo_arg(), "old": "feature/no-checkout", "new": "feature/renamed"
    }));
    let (result, events) = fixture.run(rename).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    assert!(fixture
        .repo()
        .find_branch("feature/renamed", git2::BranchType::Local)
        .is_ok());

    let delete: BranchDelete = fixture.wire(json!({
        "repo": fixture.repo_arg(), "names": ["feature/renamed"]
    }));
    let (result, events) = fixture.run(delete).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
}

#[tokio::test]
async fn branch_checkout_supports_local_and_detached_head() {
    let fixture = Fixture::new();
    let base = fixture.commit_file("file.txt", "base\n", "base");
    fixture.commit_file("other.txt", "other\n", "other");

    let create: BranchCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side" }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);

    let checkout: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side", "opts": {} }));
    let (result, events) = fixture.run(checkout).await;
    let result = assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert_eq!(result.branch.as_deref(), Some("side"));
    assert!(!result.detached);

    let back: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "main", "opts": {} }));
    let (result, events) = fixture.run(back).await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );

    let missing: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "no-such-branch", "opts": {} }));
    let (result, events) = fixture.run(missing).await;
    assert_error(result, ErrorCode::ReferenceNotFound);
    assert_error_events(events, vec![]);

    let detach: BranchCheckoutRevision = fixture.wire(json!({
        "repo": fixture.repo_arg(), "revision": { "kind": "oid", "value": base }, "detach": true
    }));
    let (result, events) = fixture.run(detach).await;
    let result = assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );
    assert!(result.detached);
}

/// `branch_checkout` documents the safe strategy: local changes are never
/// clobbered and a conflicting checkout surfaces as `CheckoutConflicts`, with
/// HEAD left on `main` — what `git checkout side` does ("your local changes
/// would be overwritten"). The nested-path, staged and sibling-verb variants
/// live in `checkout.rs`.
#[tokio::test]
async fn branch_checkout_refuses_to_clobber_a_locally_modified_tracked_file() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "base\n", "base");

    let create: BranchCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side" }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    fixture.commit_on_branch("side", "file.txt", "side version\n", "side edit");

    fixture.write("file.txt", "dirty local edit\n");
    let dirty_checkout: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side", "opts": {} }));
    let (result, events) = fixture.run(dirty_checkout).await;
    assert_error(result, ErrorCode::CheckoutConflicts);
    assert_error_events(events, vec![]);
    assert_eq!(fixture.head_branch(), "main");
    assert_eq!(fixture.read("file.txt"), "dirty local edit\n");
}

/// An untracked file that the target branch tracks must not be taken over:
/// `git checkout side` refuses with "untracked working tree files would be
/// overwritten", and so does Meridian.
#[tokio::test]
async fn branch_checkout_refuses_to_overwrite_an_untracked_file() {
    let fixture = Fixture::new();
    fixture.commit_file("base.txt", "base\n", "base");

    let create: BranchCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side" }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    fixture.commit_on_branch("side", "side-only.txt", "tracked side\n", "add side file");

    fixture.write("side-only.txt", "MY UNTRACKED WORK\n");
    let checkout: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "side", "opts": {} }));
    let (result, events) = fixture.run(checkout).await;
    assert_error(result, ErrorCode::CheckoutConflicts);
    assert_error_events(events, vec![]);
    assert_eq!(fixture.head_branch(), "main");
    assert_eq!(fixture.read("side-only.txt"), "MY UNTRACKED WORK\n");
}

/// Switching back to a branch that lacks a file removes the file and its index
/// entry and leaves a clean tree, as `git checkout main` does. (Directories are
/// covered in `checkout.rs`.)
#[tokio::test]
async fn branch_checkout_clears_paths_the_target_branch_does_not_contain() {
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
    fixture.commit_file("incoming.txt", "incoming\n", "add incoming file");

    let back: BranchCheckout =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "main", "opts": {} }));
    let (result, events) = fixture.run(back).await;
    assert_ok(
        result,
        events,
        vec![refs_changed(&fixture.repo_id), changed(&fixture.repo_id)],
    );

    let status = fixture.wire(json!({ "repo": fixture.repo_arg(), "opts": {} }));
    let (result, events) = fixture.run::<WcStatusLoad>(status).await;
    let status = assert_ok(result, events, vec![]);
    assert!(
        status.entries.is_empty(),
        "switching to main must leave a clean working copy, got {:?}",
        status
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.code))
            .collect::<Vec<_>>()
    );
    assert!(!fixture.exists("incoming.txt"));
}

/// Fast-forward, a true merge commit, a conflict resolved with
/// `integration_continue`, and `merge_abort` with no integration in flight.
///
/// The divergent branches are built with `commit_on_branch`, which never leaves
/// the checked-out branch, so this flow stays about merging rather than about
/// switching branches.
#[tokio::test]
async fn merge_fast_forward_true_merge_conflict_abort_and_continue_are_exercised() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "base\n", "base");

    let branch: BranchCreate =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "incoming" }));
    let (result, events) = fixture.run(branch).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    fixture.commit_on_branch("incoming", "incoming.txt", "incoming\n", "incoming work");

    let fast_forward: MergeStart = fixture.wire(json!({
        "repo": fixture.repo_arg(),
        "revision": { "kind": "branch", "value": "incoming" },
        "opts": {}
    }));
    let (result, events) = fixture.run(fast_forward).await;
    assert_ok(
        result,
        events,
        vec![changed(&fixture.repo_id), refs_changed(&fixture.repo_id)],
    );
    assert!(
        fixture.exists("incoming.txt"),
        "fast-forward must land the file"
    );

    // A second, divergent branch exercises a true merge commit.
    let branch: BranchCreate =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "diverged" }));
    let (result, events) = fixture.run(branch).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    fixture.commit_on_branch("diverged", "diverged.txt", "diverged\n", "diverged work");
    fixture.commit_file("main.txt", "main\n", "main side");
    let merge: MergeStart = fixture.wire(json!({
        "repo": fixture.repo_arg(),
        "revision": { "kind": "branch", "value": "diverged" },
        "opts": { "alwaysMergeCommit": true }
    }));
    let (result, events) = fixture.run(merge).await;
    assert_ok(
        result,
        events,
        vec![changed(&fixture.repo_id), refs_changed(&fixture.repo_id)],
    );
    assert_eq!(
        fixture
            .repo()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent_count(),
        2
    );

    // Two conflicting descendants of one file: the integration stays visible
    // until the user resolves it and continues.
    let branch: BranchCreate =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "conflict-side" }));
    let (result, events) = fixture.run(branch).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    let theirs = fixture.commit_on_branch("conflict-side", "conflict.txt", "theirs\n", "theirs");
    fixture.commit_file("conflict.txt", "ours\n", "ours");

    let merge: MergeStart = fixture.wire(json!({
        "repo": fixture.repo_arg(), "revision": { "kind": "oid", "value": theirs }, "opts": {}
    }));
    let (result, events) = fixture.run(merge).await;
    let outcome = assert_ok(
        result,
        events,
        vec![changed(&fixture.repo_id), refs_changed(&fixture.repo_id)],
    );
    assert!(
        !outcome.conflicts.is_empty(),
        "the merge must report its conflict"
    );

    fixture.write("conflict.txt", "resolved\n");
    let stage: WcStagePaths = fixture.wire(json!({
        "repo": fixture.repo_arg(), "paths": ["conflict.txt"]
    }));
    let (result, events) = fixture.run(stage).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let continue_merge: IntegrationContinue = fixture.wire(json!({ "repo": fixture.repo_arg() }));
    let (result, events) = fixture.run(continue_merge).await;
    assert_ok(
        result,
        events,
        vec![changed(&fixture.repo_id), refs_changed(&fixture.repo_id)],
    );

    let abort_missing: MergeAbort = fixture.wire(json!({ "repo": fixture.repo_arg() }));
    let (result, events) = fixture.run(abort_missing).await;
    assert_error(result, ErrorCode::OperationInProgress);
    assert_error_events(events, vec![]);
}

#[tokio::test]
async fn stash_save_list_apply_pop_and_drop_round_trip_working_changes() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "base\n", "base");
    fixture.write("file.txt", "stashed\n");

    let save: StashSave =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "opts": { "message": "local work" } }));
    let (result, events) = fixture.run(save).await;
    let stash = assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert_eq!(stash.message, "local work");

    let list: StashList = fixture.wire(json!({ "repo": fixture.repo_arg() }));
    let (result, events) = fixture.run(list).await;
    assert_eq!(assert_ok(result, events, vec![]).len(), 1);

    let apply: StashApply =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "index": 0, "opts": {} }));
    let (result, events) = fixture.run(apply).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert_eq!(fixture.read("file.txt"), "stashed\n");

    // Reset the restored content before pop; a successful pop both restores and
    // removes the stash entry.
    let repo = fixture.repo();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    let pop: StashPop = fixture.wire(json!({ "repo": fixture.repo_arg(), "index": 0, "opts": {} }));
    let (result, events) = fixture.run(pop).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    fixture.write("file.txt", "another stash\n");
    let save: StashSave = fixture.wire(json!({ "repo": fixture.repo_arg(), "opts": {} }));
    let (result, events) = fixture.run(save).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    let drop_stash: StashDrop = fixture.wire(json!({ "repo": fixture.repo_arg(), "indexes": [0] }));
    let (result, events) = fixture.run(drop_stash).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let bad_index: StashApply = fixture.wire(json!({ "repo": fixture.repo_arg(), "index": 42 }));
    let (result, events) = fixture.run(bad_index).await;
    assert_error(result, ErrorCode::ReferenceNotFound);
    assert_error_events(events, vec![]);
}

#[tokio::test]
async fn tag_create_delete_and_duplicate_name_are_visible() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "data\n", "initial");

    let create: TagCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "v1.0" }));
    let (result, events) = fixture.run(create).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);

    let duplicate: TagCreate = fixture.wire(json!({ "repo": fixture.repo_arg(), "name": "v1.0" }));
    let (result, events) = fixture.run(duplicate).await;
    assert_error(result, ErrorCode::ReferenceExists);
    assert_error_events(events, vec![]);

    let delete: TagDelete = fixture.wire(json!({ "repo": fixture.repo_arg(), "names": ["v1.0"] }));
    let (result, events) = fixture.run(delete).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
}

#[tokio::test]
async fn remote_add_fetch_pull_push_publish_and_non_fast_forward_are_local_only() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "base\n", "base");
    let bare_path = fixture._temp.path().join("origin.git");
    let mut clone_builder = git2::build::RepoBuilder::new();
    clone_builder.bare(true);
    clone_builder
        .clone(fixture.root.to_string_lossy().as_ref(), &bare_path)
        .expect("create local bare origin");

    let add: RemoteAdd = fixture.wire(json!({
        "repo": fixture.repo_arg(), "name": "origin", "url": bare_path.to_string_lossy()
    }));
    let (result, events) = fixture.run(add).await;
    assert_ok(result, events, vec![repos_changed()]);

    let fetch: RemoteFetch =
        fixture.wire(json!({ "repo": fixture.repo_arg(), "remote": "origin", "refspecs": [] }));
    let (result, events) = fixture.run(fetch).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);

    let publish: RemotePublishBranch = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remote": "origin", "branch": "main", "setUpstream": true
    }));
    let (result, events) = fixture.run(publish).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);

    fixture.commit_file("local.txt", "local\n", "local");
    let push: RemotePush = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remote": "origin",
        "refspecs": ["refs/heads/main:refs/heads/main"]
    }));
    let (result, events) = fixture.run(push).await;
    assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);

    // Push a sibling commit to the bare origin, then a distinct local commit;
    // the normal push must return a rejection rather than force-update origin.
    let peer_path = fixture._temp.path().join("peer");
    let peer = git2::Repository::clone(bare_path.to_string_lossy().as_ref(), &peer_path).unwrap();
    std::fs::write(peer_path.join("peer.txt"), "peer\n").unwrap();
    let mut index = peer.index().unwrap();
    index.add_path(std::path::Path::new("peer.txt")).unwrap();
    index.write().unwrap();
    let tree = peer.find_tree(index.write_tree().unwrap()).unwrap();
    let signature = git2::Signature::now("Peer", "peer@example.test").unwrap();
    let parent = peer.head().unwrap().peel_to_commit().unwrap();
    peer.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "peer",
        &tree,
        &[&parent],
    )
    .unwrap();
    let mut remote = peer.find_remote("origin").unwrap();
    remote
        .push(&["refs/heads/main:refs/heads/main"], None)
        .unwrap();

    fixture.commit_file("local-diverged.txt", "local\n", "local diverged");
    let rejected: RemotePush = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remote": "origin",
        "refspecs": ["refs/heads/main:refs/heads/main"]
    }));
    let (result, events) = fixture.run(rejected).await;
    let report = assert_ok(result, events, vec![refs_changed(&fixture.repo_id)]);
    assert!(!report.rejected.is_empty());

    // Pull against the local origin after the peer advanced it. Fast-forward
    // is the normal path; a named branch mismatch is a typed validation error.
    let pull: RemotePull = fixture.wire(json!({
        "repo": fixture.repo_arg(), "remote": "origin", "branch": "main",
        "opts": { "mode": "fastForwardOnly" }
    }));
    let (result, events) = fixture.run(pull).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);
}
