use std::fs;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::domain::diagnostics::{CrashNote, DebugState};
use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::logging;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogTailArgs {
    pub lines: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogWriteArgs {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSetArgs {
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub bytes: usize,
}

#[tauri::command]
pub fn diagnostics_log_tail(args: LogTailArgs) -> String {
    logging::log_tail(args.lines)
}

#[tauri::command]
pub fn diagnostics_log_write(args: LogWriteArgs) {
    logging::record_frontend(&args.level, &args.message, args.source.as_deref());
}

#[tauri::command]
pub fn diagnostics_debug_get() -> DebugState {
    logging::debug_state()
}

#[tauri::command]
pub fn diagnostics_debug_set<R: Runtime>(
    app: AppHandle<R>,
    args: DebugSetArgs,
) -> Result<DebugState, ErrorPayload> {
    let state = logging::set_debug(args.enabled)
        .map_err(|error| io_error("Could not save debug setting", error))?;
    // The title is the at-a-glance debug indicator; it must follow the switch
    // in this run, not only after the next launch.
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(title) = window.title() {
            let _ = window.set_title(&logging::debug_title(&title, state.enabled));
        }
    }
    Ok(state)
}

#[tauri::command]
pub fn diagnostics_reveal_logs<R: Runtime>(app: AppHandle<R>) -> Result<(), ErrorPayload> {
    let directory = logging::log_dir();
    if directory.as_os_str().is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "The diagnostics log folder is not available yet.",
        )
        .with_domain(ErrorDomain::Internal));
    }
    app.opener()
        .reveal_item_in_dir(&directory)
        .map_err(|error| io_error("Could not reveal diagnostics logs", error))
}

#[tauri::command]
pub fn diagnostics_export<R: Runtime>(
    app: AppHandle<R>,
    args: ExportArgs,
) -> Result<ExportResult, ErrorPayload> {
    // The app-info block is the same text `app_info_load` hands the About
    // pane, so a report and a bug report quote one version of the truth.
    let info = crate::commands::app_info_load(app.clone());
    let report = build_report(
        &info.diagnostics,
        &logging::debug_state(),
        logging::last_crash(),
        args.context.as_deref(),
        &logging::log_tail(2_000),
    );

    let download_dir = app
        .path()
        .download_dir()
        .map_err(|error| io_error("Could not find the Downloads folder", error))?;
    fs::create_dir_all(&download_dir)
        .map_err(|error| io_error("Could not prepare the Downloads folder", error))?;
    let path = download_dir.join(report_file_name());
    // Owner-only: the report quotes repository and branch names. The user
    // chooses where it goes next, and a world-readable copy in Downloads is not
    // that choice.
    logging::write_private(&path, report.as_bytes())
        .map_err(|error| io_error("Could not write diagnostics report", error))?;
    // The file exists now, so a failure to show it in Finder must not turn
    // into "export failed": the owner would never learn the path of a file
    // that is already on disk.
    if let Err(error) = app.opener().reveal_item_in_dir(&path) {
        log::warn!(target: "meridian::diagnostics", "exported the report but could not reveal it in Finder: {error}");
    }
    Ok(ExportResult {
        path: logging::redact(&path.display().to_string()),
        bytes: report.len(),
    })
}

/// `meridian-diagnostics-20261015-143022.txt`: local time, because a person
/// looking through Downloads thinks in their own clock, not in epoch seconds.
fn report_file_name() -> String {
    format!(
        "meridian-diagnostics-{}.txt",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

/// The exported report, assembled purely so its shape is testable. Every part
/// is redacted: the tail and the crash note were redacted when they were
/// written, and the caller-supplied context is redacted here because it is
/// free text the UI let the user type.
fn build_report(
    app_diagnostics: &str,
    debug: &DebugState,
    crash: Option<CrashNote>,
    context: Option<&str>,
    tail: &str,
) -> String {
    let mut report = String::new();
    report.push_str("Meridian diagnostics\n===================\n");
    report.push_str(app_diagnostics.trim_end());
    report.push_str(&format!(
        "\ndebug mode: {} (source: {:?})\nlog file: {}\n",
        debug.enabled, debug.source, debug.log_file
    ));
    if let Some(context) = context.filter(|context| !context.trim().is_empty()) {
        report.push_str("\nReported context\n----------------\n");
        report.push_str(context.trim_end());
        report.push('\n');
    }
    if let Some(crash) = crash {
        report.push_str("\nLast crash\n----------\n");
        match serde_json::to_string_pretty(&redacted_crash(crash)) {
            Ok(json) => report.push_str(&json),
            Err(error) => report.push_str(&format!("(could not format: {error})")),
        }
        report.push('\n');
    }
    report.push_str("\nRecent log (oldest first)\n-------------------------\n");
    report.push_str(tail.trim_end_matches('\n'));
    report.push('\n');
    logging::redact(&report)
}

#[tauri::command]
pub fn diagnostics_open_devtools<R: Runtime>(app: AppHandle<R>) -> Result<(), ErrorPayload> {
    if !logging::debug_state().enabled {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Webview developer tools require debug mode.",
        )
        .with_param("reason", "debug_off")
        .with_domain(ErrorDomain::Internal));
    }
    // The inspector is wired up when the window is created, so turning debug
    // mode on mid-session cannot add it. Refusing with a reason beats opening
    // nothing and looking like a dead button.
    if !logging::devtools_at_launch() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "Developer tools become available after Meridian is restarted.",
        )
        .with_param("reason", "restart_required")
        .with_domain(ErrorDomain::Internal));
    }
    let window = app.get_webview_window("main").ok_or_else(|| {
        ErrorPayload::new(ErrorCode::InvalidRequest, "The main window is unavailable.")
            .with_param("reason", "no_window")
            .with_domain(ErrorDomain::Internal)
    })?;
    window.open_devtools();
    Ok(())
}

#[tauri::command]
pub fn diagnostics_last_crash() -> Option<CrashNote> {
    logging::last_crash()
}

#[tauri::command]
pub fn diagnostics_dismiss_crash() -> Result<(), ErrorPayload> {
    logging::dismiss_crash().map_err(|error| io_error("Could not dismiss the crash report", error))
}

fn io_error(message: &str, error: impl std::fmt::Display) -> ErrorPayload {
    ErrorPayload::new(ErrorCode::Io, message)
        .with_detail(logging::redact(&error.to_string()))
        .with_domain(ErrorDomain::Fs)
}

fn redacted_crash(mut crash: CrashNote) -> CrashNote {
    crash.message = logging::redact(&crash.message);
    crash.location = crash.location.map(|value| logging::redact(&value));
    crash.backtrace_excerpt = crash.backtrace_excerpt.map(|value| logging::redact(&value));
    crash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::diagnostics::{CrashKind, DebugLevel, DebugSource};

    fn debug_state() -> DebugState {
        DebugState {
            enabled: true,
            source: DebugSource::Setting,
            log_dir: "~/Library/Logs/dev.meridian.app".to_string(),
            log_file: "~/Library/Logs/dev.meridian.app/meridian.log".to_string(),
            level: DebugLevel::Debug,
            devtools_ready: true,
        }
    }

    #[test]
    fn report_carries_every_section_in_order() {
        let crash = CrashNote {
            at: "2026-10-14T09:00:00.000Z".to_string(),
            message: "index out of bounds".to_string(),
            location: Some("src/engine/graph.rs:41:9".to_string()),
            backtrace_excerpt: None,
            kind: CrashKind::Panic,
        };
        let report = build_report(
            "Meridian 0.1.0 (abc123)",
            &debug_state(),
            Some(crash),
            Some("fetch button did nothing"),
            "2026-10-14T09:00:01.000Z [INFO] meridian::command: command=remote_fetch outcome=ok duration_ms=12",
        );
        let diagnostics = report.find("Meridian 0.1.0").expect("app info");
        let context = report.find("Reported context").expect("context");
        let crash_at = report.find("Last crash").expect("crash");
        let log_at = report.find("Recent log").expect("log");
        assert!(diagnostics < context && context < crash_at && crash_at < log_at);
        assert!(report.contains("index out of bounds"));
        assert!(report.contains("command=remote_fetch"));
        assert!(report.contains("debug mode: true (source: Setting)"));
    }

    #[test]
    fn report_omits_empty_optional_sections_and_redacts_the_context() {
        let report = build_report(
            "Meridian 0.1.0 (abc123)",
            &debug_state(),
            None,
            Some("clone failed with https://alice:secret@example.com/repo.git"),
            "",
        );
        assert!(!report.contains("Last crash"));
        assert!(report.contains("Reported context"));
        assert!(report.contains("clone failed with https://[redacted]@example.com/repo.git"));
        assert!(!report.contains("secret"));
    }

    #[test]
    fn report_without_a_context_has_no_context_section() {
        let report = build_report("Meridian 0.1.0 (abc123)", &debug_state(), None, None, "");
        assert!(!report.contains("Reported context"));
        assert!(!report.contains("Last crash"));
        assert!(report.contains("Recent log"));
    }

    #[test]
    fn the_report_file_name_is_a_readable_local_timestamp() {
        let name = report_file_name();
        let stamp = name
            .strip_prefix("meridian-diagnostics-")
            .and_then(|rest| rest.strip_suffix(".txt"))
            .expect("prefix and suffix");
        // 20261015-143022
        assert_eq!(stamp.len(), 15, "{name}");
        assert_eq!(stamp.as_bytes()[8], b'-');
        assert!(stamp
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 8 || byte.is_ascii_digit()));
    }
}
