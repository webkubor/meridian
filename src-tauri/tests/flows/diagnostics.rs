//! What the log and the error payload say when a command fails.
//!
//! The rest of the flow suite proves what commands do. This module proves the
//! failure paths leave a trace: a failed `repository_open` (which bypasses
//! `AppState::run`) writes an outcome line, and a command that panics comes
//! back as an internal error — not a cancellation — without poisoning the
//! repository for every command after it.

use std::sync::{Mutex, Once};

use meridian::commands::{Command, RepositoryOpenArgs, WcStatusLoad};
use meridian::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use meridian::domain::ids::RepoId;
use meridian::validation::{ValidateCx, Violation};
use serde_json::json;

use super::support::{assert_error_events, assert_ok, Fixture};

/// Collects every record in this test binary. Only the lines a test asks for
/// are looked at, so concurrently running flows do not interfere.
struct Capture(Mutex<Vec<String>>);

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(format!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }
    fn flush(&self) {}
}

static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

fn install_capture() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        log::set_logger(&CAPTURE).expect("no other logger in the flow binary");
        log::set_max_level(log::LevelFilter::Debug);
    });
}

fn captured_containing(needle: &str) -> Vec<String> {
    CAPTURE
        .0
        .lock()
        .expect("capture lock")
        .iter()
        .filter(|line| line.contains(needle))
        .cloned()
        .collect()
}

#[test]
fn a_failed_repository_open_leaves_an_error_line_in_the_log() {
    install_capture();
    let fixture = Fixture::new();
    // The directory name is the needle: unique to this test, and kept in the
    // line by the error message, which names the path it refused.
    let not_repo = fixture._temp.path().join("not-a-repo-needle-4fa91c");
    std::fs::create_dir_all(&not_repo).expect("create directory");
    let args: RepositoryOpenArgs = fixture.wire(json!({ "path": not_repo }));
    let sink = fixture.sink();
    let error = args
        .execute(&fixture.state, &sink)
        .expect_err("a plain directory is not a repository");
    assert_eq!(error.code, ErrorCode::InvalidRepository);

    let lines = captured_containing("command=repository_open outcome=error");
    let line = lines
        .iter()
        .find(|line| line.contains("not-a-repo-needle-4fa91c"))
        .unwrap_or_else(|| panic!("no outcome line for the failed open; saw {lines:?}"));
    assert!(line.starts_with("[ERROR] meridian::command:"), "{line}");
    assert!(line.contains("code=invalid_repository"), "{line}");
    assert!(line.contains("duration_ms="), "{line}");
}

#[test]
fn a_successful_repository_open_logs_an_info_outcome() {
    install_capture();
    let fixture = Fixture::new();
    let args: RepositoryOpenArgs = fixture.wire(json!({ "path": fixture.root }));
    let sink = fixture.sink();
    args.execute(&fixture.state, &sink).expect("open");
    // Success is visible at info level too: "nothing in the log" must never
    // mean "did it run?".
    assert!(
        !captured_containing("[INFO] meridian::command: command=repository_open outcome=ok")
            .is_empty()
    );
}

/// A command that blows up inside `execute`.
struct Explode {
    repo: RepoId,
}

impl Command for Explode {
    type Output = ();

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, _cx: &meridian::commands::ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        panic!("deliberate test panic\nwith a second line");
    }
}

#[tokio::test]
async fn a_panicking_command_is_an_internal_error_and_the_repository_stays_usable() {
    install_capture();
    let fixture = Fixture::new();
    fixture.commit_file("a.txt", "a\n", "a");

    let (result, events) = fixture
        .run(Explode {
            repo: fixture.repo_id.clone(),
        })
        .await;
    let error = result.expect_err("the command panicked");
    // Not `Cancelled`: the UI renders that as "Cancelled.", which tells the
    // owner they did something when the app did.
    assert_eq!(error.code, ErrorCode::Unclassified, "{error:?}");
    assert_eq!(error.domain, ErrorDomain::Internal);
    assert!(
        error
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("deliberate test panic"),
        "the panic text must be in the detail: {error:?}"
    );
    assert_error_events(events, vec![]);

    // The panic poisoned the repository's mutex. Every later command used to
    // fail with "Repository lock poisoned" until the repository was reopened.
    let status: WcStatusLoad = fixture.wire(json!({ "repo": fixture.repo_arg(), "opts": {} }));
    let (result, events) = fixture.run(status).await;
    assert_ok(result, events, vec![]);

    let outcome = captured_containing("command=explode outcome=error");
    let line = outcome
        .first()
        .expect("an outcome line for the panicked command");
    assert!(
        line.contains("code=unclassified") && line.contains("domain=internal"),
        "{line}"
    );
    // One entry is one line, even when the panic message has two.
    assert!(!line.contains('\n'), "{line:?}");
    assert!(!captured_containing("outcome=panic detail=").is_empty());
}
