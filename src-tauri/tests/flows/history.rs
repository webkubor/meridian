use meridian::commands::{HistoryChangesetLoad, HistoryCommitsLoad, HistoryMergeBase};
use meridian::domain::error::ErrorCode;
use serde_json::json;

use super::support::{assert_error, assert_ok, Fixture};

#[tokio::test]
async fn history_paging_search_merge_base_and_changeset_use_json_wire_args() {
    let fixture = Fixture::new();
    let first = fixture.commit_file("file.txt", "first\n", "first message");
    let second = fixture.commit_file("file.txt", "second\n", "second message");

    let first_page = fixture.wire(json!({
        "repo": fixture.repo_arg(), "limit": 1, "skip": 0,
        "search": "first", "searchMode": "message"
    }));
    let (result, events) = fixture.run::<HistoryCommitsLoad>(first_page).await;
    let page = assert_ok(result, events, vec![]);
    assert_eq!(page.commits.len(), 1);
    assert_eq!(
        page.commits[0].oid, first,
        "message search should return the matching older commit"
    );

    let second_page = fixture.wire(json!({ "repo": fixture.repo_arg(), "skip": 1, "limit": 1 }));
    let (result, events) = fixture.run::<HistoryCommitsLoad>(second_page).await;
    let page = assert_ok(result, events, vec![]);
    assert_eq!(page.commits.len(), 1);
    assert_eq!(page.commits[0].oid, first);

    let merge_base = fixture.wire(json!({
        "repo": fixture.repo_arg(),
        "left": { "kind": "oid", "value": first },
        "right": { "kind": "head" }
    }));
    let (result, events) = fixture.run::<HistoryMergeBase>(merge_base).await;
    let summary = assert_ok(result, events, vec![]);
    assert_eq!(summary.merge_base.as_deref(), Some(first.as_str()));
    assert_eq!(summary.ahead, 1);

    let changeset = fixture.wire(json!({ "repo": fixture.repo_arg(), "oid": second, "opts": {} }));
    let (result, events) = fixture.run::<HistoryChangesetLoad>(changeset).await;
    let changes = assert_ok(result, events, vec![]);
    assert!(changes.entries.iter().any(|entry| entry.path == "file.txt"));

    let invalid = fixture.wire(json!({ "repo": fixture.repo_arg(), "limit": 0 }));
    let (result, events) = fixture.run::<HistoryCommitsLoad>(invalid).await;
    assert_error(result, ErrorCode::InvalidRequest);
    assert!(events.is_empty());
}
