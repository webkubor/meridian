//! Probes for the headless command path itself.
//!
//! The rest of this suite proves *what* commands do. This module proves the
//! harness is trustworthy: `AppState::run_headless` must go through the same
//! registry, queue, validator and blocking executor as the Tauri wrappers, and
//! must answer the two things a user would otherwise never see — the emitted
//! events and the error payload.

use meridian::commands::{BranchList, WcStagePaths, WcStatusLoad};
use meridian::events::AppEvent;
use serde_json::json;

use super::support::{assert_error, assert_error_events, assert_ok, changed, refs_changed, Fixture};

/// A command whose `RepoId` was never registered must fail loudly instead of
/// silently answering an empty list.
#[tokio::test]
async fn an_unregistered_repository_is_refused_with_a_visible_error() {
    let fixture = Fixture::new();
    let stranger = json!({ "repo": "/definitely/not/registered/.git" });

    let status = fixture.wire::<WcStatusLoad>(json!({ "repo": stranger, "opts": {} }));
    let (result, events) = fixture.run(status).await;
    assert_error(result, meridian::domain::error::ErrorCode::InvalidRepository);
    assert_error_events(events, vec![]);
}

/// Read-only commands emit no events; mutating ones emit exactly their own.
#[tokio::test]
async fn the_headless_path_reports_the_same_events_the_stores_listen_for() {
    let fixture = Fixture::new();
    fixture.commit_file("file.txt", "base\n", "base");

    let status = fixture.wire::<WcStatusLoad>(json!({ "repo": fixture.repo_arg(), "opts": {} }));
    let (result, events) = fixture.run(status).await;
    let status = assert_ok(result, events, vec![]);
    assert!(status.entries.is_empty());

    fixture.write("file.txt", "changed\n");
    let stage = fixture.wire::<WcStagePaths>(json!({
        "repo": fixture.repo_arg(), "paths": ["file.txt"]
    }));
    let (result, events) = fixture.run(stage).await;
    assert_ok(result, events, vec![changed(&fixture.repo_id)]);

    let branches = fixture.wire::<BranchList>(json!({ "repo": fixture.repo_arg() }));
    let (result, events) = fixture.run(branches).await;
    let branches = assert_ok(result, events, vec![]);
    assert!(branches
        .iter()
        .any(|branch| branch.name == "main" && branch.is_head));
}

/// `AppEvent` must stay comparable, or every flow assertion above would have to
/// compare serialized strings instead of the value the stores actually receive.
#[test]
fn app_events_compare_by_value() {
    let fixture = Fixture::new();
    assert_eq!(
        refs_changed(&fixture.repo_id),
        refs_changed(&fixture.repo_id)
    );
    assert_ne!(
        changed(&fixture.repo_id),
        AppEvent::RefsChanged {
            repo: fixture.repo_id.clone()
        }
    );
    assert_eq!(AppEvent::ReposChanged, AppEvent::ReposChanged);
}
