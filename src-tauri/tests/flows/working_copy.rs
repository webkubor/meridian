use std::fs;

use meridian::commands::{
    CommitAmend, CommitCreate, WcChunkApply, WcDiffHunksLoad, WcEntryAssumeUnchangedSet,
    WcPathsDiscard, WcStageAll, WcStagePaths, WcStatusLoad, WcUnstagePaths,
};
use meridian::domain::commit::CommitOptions;
use meridian::domain::error::ErrorCode;
use serde_json::json;

use super::support::{assert_error, assert_error_events, assert_ok, changed, Fixture};

#[tokio::test]
async fn status_stage_unstage_stage_all_and_discard_follow_wire_contract_and_emit_events() {
    let fixture = Fixture::new();
    fixture.commit_file("base.txt", "base\n", "base");
    fs::write(fixture.root.join("one.txt"), "one\n").unwrap();
    let args = fixture.wire(json!({ "repo": fixture.repo_arg(), "opts": {} }));
    let (result, events) = fixture.run::<WcStatusLoad>(args).await;
    let status = assert_ok(result, events, vec![]);
    assert!(status.entries.iter().any(|entry| entry.path == "one.txt"));

    let args = fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": ["one.txt"] }));
    let (result, events) = fixture.run::<WcStagePaths>(args).await;
    let delta = assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert!(delta
        .entries
        .iter()
        .any(|entry| entry.path == "one.txt" && entry.staged));

    let args = fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": ["one.txt"] }));
    let (result, events) = fixture.run::<WcUnstagePaths>(args).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    fs::write(fixture.root.join("two.txt"), "two\n").unwrap();
    let args = fixture.wire(json!({ "repo": fixture.repo_arg() }));
    let (result, events) = fixture.run::<WcStageAll>(args).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "paths": ["two.txt"],
        "opts": { "includeUntracked": true, "confirmToken": "confirmed" }
    }));
    let (result, events) = fixture.run::<WcPathsDiscard>(args).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert!(!fixture.root.join("two.txt").exists());
}

#[tokio::test]
async fn stage_missing_path_fails_with_visible_error_and_emits_no_event() {
    let fixture = Fixture::new();
    let args = fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": ["missing.txt"] }));
    let (result, events) = fixture.run::<WcStagePaths>(args).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);
}

#[tokio::test]
async fn untracked_discard_without_confirmation_is_refused_and_confirmed_discard_succeeds() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("untracked.txt"), "the only copy\n").unwrap();

    let args = fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": ["untracked.txt"] }));
    let (result, events) = fixture.run::<WcPathsDiscard>(args).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);
    assert!(fixture.root.join("untracked.txt").exists());

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "paths": ["untracked.txt"],
        "opts": { "includeUntracked": true, "confirmToken": "user-confirmed" }
    }));
    let (result, events) = fixture.run::<WcPathsDiscard>(args).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert!(!fixture.root.join("untracked.txt").exists());
}

#[tokio::test]
async fn chunk_stage_uses_a_wire_hunk_and_rejects_a_stale_hunk() {
    let fixture = Fixture::new();
    fixture.commit_file("chunk.txt", "before\n", "base");
    fs::write(fixture.root.join("chunk.txt"), "after\n").unwrap();

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "path": "chunk.txt",
        "perspective": "workingTreeToIndex", "opts": {}
    }));
    let (result, events) = fixture.run::<WcDiffHunksLoad>(args).await;
    let hunks = assert_ok(result, events, vec![]);
    assert_eq!(hunks.len(), 1);
    let hunk_wire = serde_json::to_value(&hunks[0]).unwrap();

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "hunk": hunk_wire,
        "direction": "stage", "contextLines": 3
    }));
    let (result, events) = fixture.run::<WcChunkApply>(args).await;
    let delta = assert_ok(result, events, vec![changed(&fixture.repo_id)]);
    assert!(delta
        .entries
        .iter()
        .any(|entry| entry.path == "chunk.txt" && entry.staged));

    let mut stale_hunk = serde_json::to_value(&hunks[0]).unwrap();
    stale_hunk["header"] = json!("@@ -900,1 +900,1 @@ stale");
    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "hunk": stale_hunk, "direction": "stage"
    }));
    let (result, events) = fixture.run::<WcChunkApply>(args).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);
}

#[tokio::test]
async fn assume_unchanged_requires_a_tracked_path_and_emits_after_success() {
    let fixture = Fixture::new();
    fixture.commit_file("tracked.txt", "base\n", "base");

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "paths": ["tracked.txt"], "value": true
    }));
    let (result, events) = fixture.run::<WcEntryAssumeUnchangedSet>(args).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let args = fixture.wire(json!({
        "repo": fixture.repo_arg(), "paths": ["untracked.txt"], "value": true
    }));
    let (result, events) = fixture.run::<WcEntryAssumeUnchangedSet>(args).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);
}

#[tokio::test]
async fn commit_create_validation_amend_and_sign_off_are_visible_and_correct() {
    let fixture = Fixture::new();
    let base = fixture.commit_file("base.txt", "base\n", "base");

    let empty = fixture.wire(json!({ "repo": fixture.repo_arg(), "message": "  " }));
    let (result, events) = fixture.run::<CommitCreate>(empty).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);

    let no_changes = fixture.wire(json!({ "repo": fixture.repo_arg(), "message": "no changes" }));
    let (result, events) = fixture.run::<CommitCreate>(no_changes).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);

    fs::write(fixture.root.join("staged.txt"), "staged\n").unwrap();
    let stage = fixture.wire(json!({ "repo": fixture.repo_arg(), "paths": ["staged.txt"] }));
    let (result, events) = fixture.run::<WcStagePaths>(stage).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let create = fixture.wire(json!({
        "repo": fixture.repo_arg(), "message": "feature", "opts": { "signOff": true }
    }));
    let (result, events) = fixture.run::<CommitCreate>(create).await;
    let commit = assert_ok(
        result,
        events,
        vec![
            meridian::events::AppEvent::RefsChanged {
                repo: fixture.repo_id.clone(),
            },
            changed(&fixture.repo_id),
        ],
    );
    assert!(commit
        .message
        .contains("Signed-off-by: Flow Test <flow@example.test>"));
    assert_ne!(commit.oid, base);

    let amend = fixture.wire(json!({ "repo": fixture.repo_arg(), "message": "amended" }));
    let (result, events) = fixture.run::<CommitAmend>(amend).await;
    let amended = assert_ok(
        result,
        events,
        vec![
            meridian::events::AppEvent::RefsChanged {
                repo: fixture.repo_id.clone(),
            },
            changed(&fixture.repo_id),
        ],
    );
    assert_eq!(amended.message, "amended");

    let sign = fixture.wire(json!({
        "repo": fixture.repo_arg(), "message": "signed",
        "opts": { "sign": { "kind": "gpg", "key": null } }
    }));
    let (result, events) = fixture.run::<CommitCreate>(sign).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert_error_events(events, vec![]);

    let _typed_options: CommitOptions = fixture.wire(json!({ "signOff": true }));
}
