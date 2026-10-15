//! Application-level commands: version, release notes, self-update, feedback.
//!
//! These are the only commands in the app that are **not** addressed to a
//! repository, which is why they are plain `#[tauri::command]` functions
//! rather than `Command` implementations: the trait's `repo_id()` and the
//! `ExecCx` it is handed both require a resolved `git2::Repository`, and there
//! is none. See `domain::app` for the same note from the data side.
//!
//! Nothing here contacts the network except `app_update_check` and
//! `app_update_install`, and both go to the project's own GitHub release
//! endpoint and carry nothing but the app version. That is a brand promise, not
//! a preference — see the disclosure text in `updates.disclosure`.

use std::env;
use std::time::Instant;

use serde::Deserialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

use crate::domain::app::{
    AppInfo, BuildProfile, FeedbackKind, FeedbackRequest, FeedbackTarget, NotesLocale,
    ReleaseNotes, UpdateCheck, UpdateStatus,
};
use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::logging;

/// The project slug, baked in by `build.rs` from the `origin` remote. Not a
/// literal anywhere in the source, so there is exactly one place that decides
/// where this project lives and no file can disagree with another.
const REPO_SLUG: &str = env!("MERIDIAN_REPO_SLUG");

/// The commit this binary was built from, or the empty string when the build
/// had no git checkout.
const GIT_COMMIT: &str = env!("MERIDIAN_GIT_COMMIT");

const CHANGELOG_EN: &str = include_str!("../../../CHANGELOG.md");
const CHANGELOG_ZH_CN: &str = include_str!("../../../CHANGELOG.zh-CN.md");

fn configured_slug() -> Option<&'static str> {
    let slug = REPO_SLUG.trim();
    (!slug.is_empty()).then_some(slug)
}

/// `https://github.com/{slug}/releases/latest/download/latest.json` — the
/// manifest the release workflow publishes beside each release.
fn update_endpoint(slug: &str) -> String {
    format!("https://github.com/{slug}/releases/latest/download/latest.json")
}

fn build_profile() -> BuildProfile {
    if cfg!(debug_assertions) {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    }
}

fn os_version() -> String {
    // `sw_vers` is the supported way to read the product version; the
    // alternative is linking against CoreFoundation for one string.
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    String::new()
}

/// Whether the updater plugin was given a public key. Without it `updater()`
/// fails, so the UI must be told up front rather than showing a button that
/// always errors.
fn has_pubkey<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|value| value.get("pubkey"))
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty())
}

fn info_for<R: Runtime>(app: &AppHandle<R>) -> AppInfo {
    let package = app.package_info();
    // A pubkey and a slug together are what an update check needs; either one
    // alone means the button cannot work.
    let configured = has_pubkey(app) && configured_slug().is_some();
    let profile = build_profile();

    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        commit: (!GIT_COMMIT.is_empty()).then(|| GIT_COMMIT.to_string()),
        build_profile: profile,
        rust_version: option_env!("CARGO_PKG_RUST_VERSION")
            .unwrap_or_default()
            .to_string(),
        os_name: env::consts::OS.to_string(),
        os_version: os_version(),
        arch: env::consts::ARCH.to_string(),
        repo_slug: configured_slug().map(str::to_string),
        updates_configured: configured,
        // A debug build must never replace itself: the updater would overwrite
        // the working tree the developer is running from.
        can_self_update: configured && profile == BuildProfile::Release,
        // Filled below from the fields above, so the text is built in exactly
        // one place.
        diagnostics: String::new(),
    }
    .with_diagnostics()
}

// --- app_info_load ---------------------------------------------------------

#[tauri::command]
pub fn app_info_load<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    info_for(&app)
}

// --- app_release_notes_load ------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotesLoad {
    /// The UI's active language. Unknown values fall back to English rather
    /// than failing: release notes are not worth an error screen.
    #[serde(default)]
    pub locale: Option<String>,
}

#[tauri::command]
pub fn app_release_notes_load<R: Runtime>(
    app: AppHandle<R>,
    args: ReleaseNotesLoad,
) -> ReleaseNotes {
    let wants_chinese = args
        .locale
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("zh-CN") || value.starts_with("zh"));

    let (markdown, locale) = if wants_chinese {
        (CHANGELOG_ZH_CN, NotesLocale::ZhCn)
    } else {
        (CHANGELOG_EN, NotesLocale::En)
    };

    ReleaseNotes {
        markdown: markdown.to_string(),
        locale,
        current_version: app.package_info().version.to_string(),
    }
}

// --- app_update_check ------------------------------------------------------

#[tauri::command]
pub async fn app_update_check<R: Runtime>(app: AppHandle<R>) -> Result<UpdateCheck, ErrorPayload> {
    // These commands run outside `AppState::run`, so each logs its own outcome:
    // an update that fails to check or install is exactly the failure that used
    // to leave nothing in the log.
    let started = Instant::now();
    let result = update_check(&app).await;
    logging::instrument("app_update_check", started, &result);
    result
}

async fn update_check<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateCheck, ErrorPayload> {
    let info = info_for(app);

    if !info.updates_configured {
        return Ok(UpdateCheck::NotConfigured);
    }
    if info.build_profile == BuildProfile::Debug {
        return Ok(UpdateCheck::UnsupportedInDebugBuild);
    }

    let slug = configured_slug().expect("checked by updates_configured");
    let endpoint = update_endpoint(slug)
        .parse()
        .map_err(|error| invalid_endpoint(&error))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .and_then(|builder| builder.build())
        .map_err(|error| update_error(&error))?;

    let found = updater
        .check()
        .await
        .map_err(|error| update_error(&error))?;

    Ok(UpdateCheck::Checked {
        status: UpdateStatus {
            current_version: info.version,
            latest_version: found.as_ref().map(|update| update.version.clone()),
            notes: found.as_ref().and_then(|update| update.body.clone()),
            published_at: found
                .as_ref()
                .and_then(|update| update.date)
                .map(|date| date.to_string()),
            download_bytes: None,
        },
    })
}

// --- app_update_install ----------------------------------------------------

/// Progress is pushed on a channel rather than polled, so a 60 MB download does
/// not become 60 invocations.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum UpdateProgress {
    Downloading {
        received: u64,
        total: Option<u64>,
    },
    Downloaded,
    Installing,
    /// macOS install completed; the app must be relaunched to run the new build.
    ReadyToRestart,
}

#[tauri::command]
pub async fn app_update_install<R: Runtime>(
    app: AppHandle<R>,
    on_progress: tauri::ipc::Channel<UpdateProgress>,
) -> Result<(), ErrorPayload> {
    let started = Instant::now();
    let result = update_install(&app, on_progress).await;
    logging::instrument("app_update_install", started, &result);
    result
}

async fn update_install<R: Runtime>(
    app: &AppHandle<R>,
    on_progress: tauri::ipc::Channel<UpdateProgress>,
) -> Result<(), ErrorPayload> {
    let info = info_for(app);
    if !info.can_self_update {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This build cannot update itself.",
        ));
    }

    let slug = configured_slug().expect("implied by can_self_update");
    let endpoint = update_endpoint(slug)
        .parse()
        .map_err(|error| invalid_endpoint(&error))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .and_then(|builder| builder.build())
        .map_err(|error| update_error(&error))?;

    let found = updater
        .check()
        .await
        .map_err(|error| update_error(&error))?
        .ok_or_else(|| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "No update is available for this version.",
            )
        })?;

    let progress = on_progress.clone();
    let mut received_so_far: u64 = 0;
    let finished = on_progress.clone();
    found
        .download_and_install(
            move |chunk_len, total| {
                received_so_far += chunk_len as u64;
                let _ = progress.send(UpdateProgress::Downloading {
                    received: received_so_far,
                    total,
                });
            },
            move || {
                let _ = finished.send(UpdateProgress::Downloaded);
            },
        )
        .await
        .map_err(|error| update_error(&error))?;

    let _ = on_progress.send(UpdateProgress::Installing);
    let _ = on_progress.send(UpdateProgress::ReadyToRestart);
    Ok(())
}

// --- app_relaunch ----------------------------------------------------------

/// Restarts the process.
///
/// Exists as a command rather than pulling in `@tauri-apps/plugin-process` on
/// the frontend: the plugin's Rust half is already linked, so exposing this one
/// verb keeps the webview's dependency surface at what it already has. The
/// frontend calls it only after an update has installed, since macOS leaves the
/// old process running the old code until it comes back up.
#[tauri::command]
pub fn app_relaunch<R: Runtime>(app: AppHandle<R>) {
    // Deliberately no return value and no error path: the process is about to
    // be replaced, so a failure here has no one left to report to. The frontend
    // treats the call as fire-and-forget.
    app.restart();
}

// --- app_pick_directory ----------------------------------------------------

/// Asks the user for a folder, and returns its path.
///
/// This is the command the whole "open a repository" flow hangs off. Without it
/// the only way into a repository was typing its path by hand, and every
/// browse-shaped control in the UI was drawn, enabled, and inert.
///
/// Three decisions worth stating:
///
///  * **It returns `Ok(None)` when the user cancels.** Cancelling a picker is
///    not an error, and turning it into one is how a "choose folder" button
///    ends up showing an error toast to someone who simply changed their mind.
///  * **The dialog is opened on the main thread and awaited off it.** The
///    plugin's blocking API is a deadlock when called from a command's own
///    thread, and a sync command runs on the main thread — which would freeze
///    the window behind a modal that has not opened yet. So: schedule the
///    dialog where AppKit requires it, hand the answer back over a channel, and
///    await the channel here.
///  * **No capability entry.** The plugin is driven from Rust, so the webview
///    is never granted a dialog permission — same reasoning as the updater.
#[tauri::command]
pub async fn app_pick_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, ErrorPayload> {
    let started = Instant::now();
    let result = pick_directory(app).await;
    logging::instrument("app_pick_directory", started, &result);
    result
}

async fn pick_directory<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, ErrorPayload> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let dialog_app = app.clone();

    app.run_on_main_thread(move || {
        dialog_app
            .dialog()
            .file()
            .set_title("Choose a repository folder")
            .pick_folder(move |picked| {
                let answer = picked
                    .and_then(|path| path.into_path().ok())
                    .map(|path| path.to_string_lossy().into_owned());
                let _ = tx.send(answer);
            });
    })
    .map_err(|error| {
        ErrorPayload::new(
            ErrorCode::Io,
            format!("Could not open the folder picker: {error}"),
        )
    })?;

    Ok(rx.await.unwrap_or(None))
}

// --- app_feedback_compose --------------------------------------------------

/// Field ids from `.github/ISSUE_TEMPLATE/bug_report.yml`. A GitHub issue form
/// prefills from query parameters named after the field `id`, so these must
/// match that file exactly — a rename there silently stops prefilling here.
/// `tests/github_metadata.rs` reads the YAML and asserts every id below still
/// exists, and that the ids we deliberately do *not* prefill still exist too.
const FIELD_VERSION: &str = "version";
const FIELD_MACOS: &str = "macos-version";
const FIELD_ARCH: &str = "architecture";
/// The bug form's optional textarea for the Diagnostics report. Unlike the
/// three above it is prefilled from the caller's `context`, not from build facts.
const FIELD_DIAGNOSTICS: &str = "diagnostics";

/// Ceiling for the whole issue URL, in characters. Browsers and GitHub's edge
/// both refuse very long request lines — and they refuse with a bare error page,
/// not a trimmed form — so the report is cut to fit rather than sent whole and
/// lost. 7000 leaves a margin under the roughly 8 KB where that starts to happen.
const URL_BUDGET: usize = 7000;
/// A user-entered title can also contain arbitrarily long or multibyte text.
/// Keep a little room for it, then give the remainder to diagnostics.
const TITLE_BUDGET: usize = 768;
const TITLE_TRUNCATION_MARKER: &str = "[truncated]";

/// Appended, on its own line, when the context did not fit. A report that stops
/// mid-sentence with no sign of it is worse than none: the maintainer would
/// debug on evidence that is missing without knowing it.
const TRUNCATION_MARKER: &str = "[truncated — attach the exported file]";
/// Turns build facts into the two lines a maintainer actually needs.
///
/// Deliberately excludes: repository paths, remote URLs, branch names, the
/// user's git identity, and anything about what they were doing. A version and
/// a platform are enough to reproduce almost anything; the rest is the user's
/// business and they did not offer it.
fn architecture_label(info: &AppInfo) -> String {
    match info.arch.as_str() {
        "aarch64" => "Apple silicon".to_string(),
        "x86_64" => "Intel".to_string(),
        other => other.to_string(),
    }
}

fn feedback_target(info: &AppInfo, request: &FeedbackRequest) -> Option<FeedbackTarget> {
    let slug = info.repo_slug.as_deref()?;

    let (template, mut prefilled) = match request.kind {
        FeedbackKind::Bug => {
            let version = match info.commit.as_deref() {
                Some(commit) => format!("{} ({commit})", info.version),
                None => info.version.clone(),
            };
            let os = if info.os_version.is_empty() {
                info.os_name.clone()
            } else {
                format!("{} {}", info.os_name, info.os_version)
            };
            (
                "bug_report.yml",
                vec![
                    (FIELD_VERSION, version),
                    (FIELD_MACOS, os),
                    (FIELD_ARCH, architecture_label(info)),
                ],
            )
        }
        FeedbackKind::Feature => ("feature_request.yml", vec![]),
    };

    // The sensitive-data checkbox in the bug form is intentionally left
    // unchecked: it is the reporter's confirmation that they removed private
    // material, and ticking it on their behalf would be a lie in their name.
    //
    // `title` is the one prefill parameter that does not correspond to a field
    // `id`; GitHub reads it as the issue title for any form, which is why the
    // summary goes here rather than into a body field.
    let mut query = String::new();
    push_param(&mut query, "template", template);
    if let Some(summary) = request.summary.as_deref().map(str::trim) {
        if !summary.is_empty() {
            // Redact first, truncate second: cutting first could split a
            // credential and leave a fragment the rules no longer recognise.
            let title = fit_title(&logging::redact(summary));
            push_param(&mut query, "title", &title);
        }
    }
    for (key, value) in prefilled.iter() {
        push_param(&mut query, key, value);
    }

    prefilled.retain(|(_, value)| !value.is_empty());
    let mut lines: Vec<String> = prefilled
        .into_iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect();

    // The diagnostics report goes last: everything else is a few dozen bytes,
    // so whatever room is left under the URL budget is the report's to use.
    // Only the bug form has the field; a feature request has nothing to attach
    // a log to, so it ignores `context` rather than sending a parameter GitHub
    // would drop.
    let origin = format!("https://github.com/{slug}/issues/new?");
    if let (FeedbackKind::Bug, Some(context)) = (request.kind, request.context.as_deref()) {
        // The report text is built in the webview, and this URL is what leaves
        // the process, so the Rust rules run here too — the only layer that
        // sees every report, whatever produced it.
        let context = logging::redact(context.trim());
        let context = context.as_str();
        if !context.is_empty() {
            // `&` + `diagnostics` + `=` is what `push_param` will add.
            let overhead = origin.len() + query.len() + encode(FIELD_DIAGNOSTICS).len() + 2;
            let room = URL_BUDGET.saturating_sub(overhead);
            match fit_context(context, room) {
                Fitted::Whole(text) => {
                    lines.push(format!(
                        "{FIELD_DIAGNOSTICS}: {} characters",
                        text.chars().count()
                    ));
                    push_param(&mut query, FIELD_DIAGNOSTICS, &text);
                }
                Fitted::Truncated(text) => {
                    lines.push(format!(
                        "{FIELD_DIAGNOSTICS}: {} of {} characters (truncated)",
                        text.chars().count(),
                        context.chars().count()
                    ));
                    push_param(&mut query, FIELD_DIAGNOSTICS, &text);
                }
                Fitted::Omitted => lines.push(format!(
                    "{FIELD_DIAGNOSTICS}: omitted, no room left in the link (attach the exported file)"
                )),
            }
        }
    }

    Some(FeedbackTarget {
        url: format!("{origin}{query}"),
        prefilled: lines,
    })
}

/// What became of the diagnostics text once the URL budget was applied.
#[derive(Debug, PartialEq, Eq)]
enum Fitted {
    /// It fit; sent unchanged.
    Whole(String),
    /// It did not fit. The text ends with `TRUNCATION_MARKER`.
    Truncated(String),
    /// Not even the marker fits — the title alone nearly fills the budget.
    /// Nothing is sent, and the caller says so instead of dropping it quietly.
    Omitted,
}

/// Fits `context` into `room` characters of *percent-encoded* URL.
///
/// The budget is measured after encoding because that is what the URL costs:
/// a CJK character is three bytes and so nine characters on the wire, a newline
/// three. Measuring the raw text would let a Chinese report run to three times
/// the limit. The cut is made on `char` boundaries by construction — it walks
/// `char_indices` and slices only at an index it has just seen — so a multi-byte
/// character is kept whole or dropped whole, never split into invalid UTF-8.
fn fit_title(title: &str) -> String {
    if encode(title).len() <= TITLE_BUDGET {
        return title.to_string();
    }

    let available = TITLE_BUDGET - encode(TITLE_TRUNCATION_MARKER).len();
    let mut used = 0;
    let mut end = 0;
    for (index, character) in title.char_indices() {
        let cost = encode(character.encode_utf8(&mut [0; 4])).len();
        if used + cost > available {
            break;
        }
        used += cost;
        end = index + character.len_utf8();
    }

    let kept = title[..end].trim_end();
    format!("{kept}{TITLE_TRUNCATION_MARKER}")
}

fn fit_context(context: &str, room: usize) -> Fitted {
    if encode(context).len() <= room {
        return Fitted::Whole(context.to_string());
    }

    let marker = format!("\n{TRUNCATION_MARKER}");
    let Some(available) = room.checked_sub(encode(&marker).len()) else {
        return Fitted::Omitted;
    };

    let mut used = 0;
    let mut end = 0;
    for (index, character) in context.char_indices() {
        // Same encoder as the URL itself, so the two can never disagree.
        let cost = encode(character.encode_utf8(&mut [0; 4])).len();
        if used + cost > available {
            break;
        }
        used += cost;
        end = index + character.len_utf8();
    }

    let mut kept = context[..end].trim_end();
    // Prefer a section boundary: a report cut in the middle of a JSON block is
    // unreadable, while one that stops after a whole section is merely short.
    // Only when that keeps at least half of what fits — otherwise one huge
    // first section would be dropped entirely.
    if let Some(boundary) = kept.rfind("\n\n") {
        if boundary >= kept.len() / 2 {
            kept = kept[..boundary].trim_end();
        }
    }
    Fitted::Truncated(if kept.is_empty() {
        TRUNCATION_MARKER.to_string()
    } else {
        format!("{kept}{marker}")
    })
}

/// Minimal `application/x-www-form-urlencoded` encoding. `url::form_urlencoded`
/// is already in the tree via the updater plugin, but keeping this dependency
/// free makes the function trivially unit-testable.
fn push_param(query: &mut String, key: &str, value: &str) {
    if !query.is_empty() {
        query.push('&');
    }
    query.push_str(&encode(key));
    query.push('=');
    query.push_str(&encode(value));
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[tauri::command]
pub fn app_feedback_compose<R: Runtime>(
    app: AppHandle<R>,
    args: FeedbackRequest,
) -> Result<FeedbackTarget, ErrorPayload> {
    let started = Instant::now();
    let info = info_for(&app);
    let result = feedback_target(&info, &args).ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This build does not know which project to report to.",
        )
    });
    logging::instrument("app_feedback_compose", started, &result);
    result
}

// --- error helpers ---------------------------------------------------------

fn update_error(error: &tauri_plugin_updater::Error) -> ErrorPayload {
    // Service, not Git: nothing about the repository failed. A report that says
    // domain=git makes the reader look for a broken repo instead of a network.
    ErrorPayload::new(ErrorCode::Io, "Could not reach the update endpoint.")
        .with_domain(ErrorDomain::Service)
        .with_detail(error.to_string())
}

fn invalid_endpoint(error: &dyn std::fmt::Display) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::Unclassified,
        "The update endpoint is not a valid URL.",
    )
    .with_domain(ErrorDomain::Internal)
    .with_detail(error.to_string())
}

// --- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> AppInfo {
        AppInfo {
            name: "Meridian".into(),
            version: "0.1.0".into(),
            commit: Some("abc123def456".into()),
            build_profile: BuildProfile::Release,
            rust_version: "1.82".into(),
            os_name: "macos".into(),
            os_version: "14.5".into(),
            arch: "aarch64".into(),
            repo_slug: Some("example/meridian".into()),
            updates_configured: true,
            can_self_update: true,
            // The struct is built by hand here, so the derived text has to be
            // asked for explicitly — which is what makes this the same code
            // path the running app uses.
            diagnostics: String::new(),
        }
        .with_diagnostics()
    }

    #[test]
    fn update_endpoint_is_the_github_release_manifest() {
        assert_eq!(
            update_endpoint("example/meridian"),
            "https://github.com/example/meridian/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn architecture_is_reported_in_words_a_reporter_recognises() {
        let mut info = sample_info();
        assert_eq!(architecture_label(&info), "Apple silicon");
        info.arch = "x86_64".into();
        assert_eq!(architecture_label(&info), "Intel");
    }

    #[test]
    fn bug_url_prefills_version_platform_and_architecture() {
        let info = sample_info();
        let target = feedback_target(
            &info,
            &FeedbackRequest {
                kind: FeedbackKind::Bug,
                summary: None,
                context: None,
            },
        )
        .expect("slug is set");

        assert!(target
            .url
            .starts_with("https://github.com/example/meridian/issues/new?"));
        assert!(target.url.contains("template=bug_report.yml"));
        assert!(target.url.contains("version=0.1.0+%28abc123def456%29"));
        assert!(target.url.contains("macos-version=macos+14.5"));
        assert!(target.url.contains("architecture=Apple+silicon"));
    }

    #[test]
    fn bug_url_never_prefills_the_sensitive_data_confirmation() {
        let info = sample_info();
        let target = feedback_target(
            &info,
            &FeedbackRequest {
                kind: FeedbackKind::Bug,
                summary: Some("crash on stage".into()),
                context: None,
            },
        )
        .expect("slug is set");

        // The checkbox is the reporter's own confirmation that they removed
        // private material. Ticking it for them would be a lie in their name.
        assert!(!target.url.contains("sensitive-data"));
        assert!(target.url.contains("title=crash+on+stage"));
    }

    #[test]
    fn feature_url_uses_the_feature_form_and_carries_no_diagnostics() {
        let info = sample_info();
        let target = feedback_target(
            &info,
            &FeedbackRequest {
                kind: FeedbackKind::Feature,
                summary: None,
                context: None,
            },
        )
        .expect("slug is set");

        assert!(target.url.contains("template=feature_request.yml"));
        assert!(!target.url.contains("version="));
        assert!(!target.url.contains("macos-version="));
        assert!(
            target.prefilled.is_empty(),
            "a capability request needs no diagnostic prefill"
        );
    }

    #[test]
    fn no_feedback_url_is_produced_without_a_project_slug() {
        let mut info = sample_info();
        info.repo_slug = None;

        assert!(feedback_target(
            &info,
            &FeedbackRequest {
                kind: FeedbackKind::Bug,
                summary: None,
                context: None,
            }
        )
        .is_none());
    }

    #[test]
    fn quotes_and_spaces_in_a_summary_are_encoded_not_dropped() {
        let info = sample_info();
        let target = feedback_target(
            &info,
            &FeedbackRequest {
                kind: FeedbackKind::Bug,
                summary: Some("staging \"foo/bar\" fails & dies".into()),
                context: None,
            },
        )
        .expect("slug is set");

        assert!(target
            .url
            .contains("title=staging+%22foo%2Fbar%22+fails+%26+dies"));
    }

    // --- the diagnostics report in the bug URL ------------------------------

    /// Inverse of `encode`, for asserting on what a browser would hand to
    /// GitHub. Panics on bytes that are not UTF-8 — which is the point: a cut
    /// that split a character would decode to garbage there, so every test that
    /// reads a parameter back also proves the cut was on a char boundary.
    fn decode(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut out = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'+' => {
                    out.push(b' ');
                    index += 1;
                }
                b'%' => {
                    let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("hex pair");
                    out.push(u8::from_str_radix(hex, 16).expect("valid hex pair"));
                    index += 3;
                }
                other => {
                    out.push(other);
                    index += 1;
                }
            }
        }
        String::from_utf8(out).expect("the decoded parameter is valid UTF-8")
    }

    fn param(url: &str, key: &str) -> Option<String> {
        let query = url.split_once('?')?.1;
        query.split('&').find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == key).then(|| decode(value))
        })
    }

    fn bug_with(summary: Option<String>, context: Option<&str>) -> FeedbackTarget {
        feedback_target(
            &sample_info(),
            &FeedbackRequest {
                kind: FeedbackKind::Bug,
                summary,
                context: context.map(str::to_string),
            },
        )
        .expect("slug is set")
    }

    #[test]
    fn bug_url_carries_the_diagnostics_context() {
        let report = "Meridian 0.1.0\nlast failure: remote_fetch (network)\n";
        let target = bug_with(None, Some(report));

        // Trimmed, otherwise untouched: newlines survive the round trip.
        assert_eq!(
            param(&target.url, FIELD_DIAGNOSTICS).as_deref(),
            Some(report.trim())
        );
        // Whole, so no truncation marker anywhere.
        assert!(!target.url.contains(&encode(TRUNCATION_MARKER)));
        // The existing build-fact prefill is unchanged by the new field.
        assert!(target.url.contains("version=0.1.0+%28abc123def456%29"));
        assert!(target.url.contains("architecture=Apple+silicon"));
        // The UI is told something was attached, and how much — never the text.
        let line = target
            .prefilled
            .iter()
            .find(|line| line.starts_with("diagnostics:"))
            .expect("a diagnostics line is reported");
        assert!(line.contains("characters") && !line.contains("remote_fetch"));
        assert!(!target.url.contains("sensitive-data"));
    }

    #[test]
    fn an_oversized_cjk_context_is_cut_visibly_and_on_a_char_boundary() {
        let report = "诊断报告：推送失败，远程拒绝了这次请求。\n".repeat(600);

        // Three bytes per character means the encoded length steps by nine, so
        // sliding the budget one character at a time over a full cycle lands the
        // cut on every alignment — `decode` fails the test if any of them splits
        // a character.
        for title_len in 0..24 {
            let target = bug_with(Some("x".repeat(title_len)), Some(&report));
            let sent = param(&target.url, FIELD_DIAGNOSTICS).expect("diagnostics sent");

            assert!(
                sent.ends_with(TRUNCATION_MARKER),
                "the cut is announced (title of {title_len} characters)"
            );
            assert!(
                report.starts_with(sent.trim_end_matches(TRUNCATION_MARKER).trim_end()),
                "what is kept is a prefix of the original, not a rearrangement"
            );
            assert!(target
                .prefilled
                .iter()
                .any(|line| line.starts_with("diagnostics:") && line.contains("(truncated)")));
        }
    }

    #[test]
    fn the_whole_url_stays_inside_the_budget() {
        // Mixed worst cases for the encoder: 4-byte emoji (twelve characters
        // each on the wire), 3-byte CJK, newlines, and reserved ASCII.
        let reports = [
            "诊断".repeat(5000),
            "🧭\n".repeat(5000),
            "line with spaces & symbols = ? # / %\n".repeat(1000),
        ];
        for report in &reports {
            let target = bug_with(Some("push fails after rename".into()), Some(report));
            assert!(
                target.url.len() <= URL_BUDGET,
                "{} characters exceeds the {URL_BUDGET} budget",
                target.url.len()
            );
        }

        // And it is not over-trimmed: a pure-CJK report fills the budget to
        // within one encoded character plus the trimmed trailing space.
        let target = bug_with(None, Some(&reports[0]));
        assert!(
            target.url.len() > URL_BUDGET - 16,
            "{} leaves needless room under {URL_BUDGET}",
            target.url.len()
        );
    }

    #[test]
    fn a_context_that_fits_is_never_marked_truncated() {
        let report = "short report".repeat(10);
        let target = bug_with(None, Some(&report));

        assert_eq!(
            param(&target.url, FIELD_DIAGNOSTICS).as_deref(),
            Some(report.as_str())
        );
        assert!(!target.url.contains(&encode("truncated")));
    }

    #[test]
    fn an_oversized_title_is_bounded_and_visibly_truncated() {
        let title = "诊断标题".repeat(1000);
        let target = bug_with(Some(title), Some("any report"));
        let sent_title = param(&target.url, "title").expect("title is present");

        assert!(sent_title.ends_with(TITLE_TRUNCATION_MARKER));
        assert!(encode(&sent_title).len() <= TITLE_BUDGET);
        assert!(param(&target.url, FIELD_DIAGNOSTICS).is_some());
        assert!(target.url.len() <= URL_BUDGET);
    }

    #[test]
    fn a_feature_request_ignores_the_context() {
        let target = feedback_target(
            &sample_info(),
            &FeedbackRequest {
                kind: FeedbackKind::Feature,
                summary: None,
                context: Some("last failure: remote_fetch".into()),
            },
        )
        .expect("slug is set");

        assert!(target.url.contains("template=feature_request.yml"));
        assert!(!target.url.contains("diagnostics"));
        assert!(!target.url.contains("remote_fetch"));
        assert!(target.prefilled.is_empty());
    }

    #[test]
    fn no_context_means_no_diagnostics_parameter() {
        for context in [None, Some(""), Some("  \n\t ")] {
            let target = bug_with(None, context);
            assert!(
                !target.url.contains("diagnostics"),
                "context {context:?} must not produce a parameter: {}",
                target.url
            );
            assert!(target
                .prefilled
                .iter()
                .all(|line| !line.starts_with("diagnostics")));
        }
    }

    #[test]
    fn fit_context_is_exact_at_the_edges() {
        let marker_cost = encode(&format!("\n{TRUNCATION_MARKER}")).len();

        let long = "abcdefghij".repeat(20);

        assert_eq!(fit_context("abc", 3), Fitted::Whole("abc".into()));
        // One short of fitting, with no room for the marker either.
        assert_eq!(fit_context("abc", 2), Fitted::Omitted);
        // Too long, and room for the marker alone: the marker alone is sent.
        assert_eq!(
            fit_context(&long, marker_cost),
            Fitted::Truncated(TRUNCATION_MARKER.to_string())
        );
        // Room for exactly "ab" before the marker.
        assert_eq!(
            fit_context(&long, marker_cost + 2),
            Fitted::Truncated(format!("ab\n{TRUNCATION_MARKER}"))
        );
        // A character that does not fit is dropped whole: nine wire characters
        // for "诊", so eight spare ones keep only the "a" before it.
        assert_eq!(
            fit_context(&format!("a{}", "诊断".repeat(20)), marker_cost + 1 + 8),
            Fitted::Truncated(format!("a\n{TRUNCATION_MARKER}"))
        );
    }

    #[test]
    fn diagnostics_contain_no_repository_or_identity_information() {
        let info = sample_info();
        let text = &info.diagnostics;

        assert!(text.contains("Meridian 0.1.0"));
        assert!(text.contains("abc123def456"));
        assert!(text.contains("example/meridian"));
        // Nothing from a user's machine beyond platform and build facts.
        for forbidden in ["/Users/", "git@", "remote", "branch"] {
            assert!(
                !text.contains(forbidden),
                "diagnostics must not mention {forbidden:?}: {text}"
            );
        }
    }

    #[test]
    fn update_is_only_available_when_the_version_actually_differs() {
        let same = UpdateStatus {
            current_version: "0.1.0".into(),
            latest_version: Some("0.1.0".into()),
            notes: None,
            published_at: None,
            download_bytes: None,
        };
        assert!(!same.available());

        let newer = UpdateStatus {
            latest_version: Some("0.2.0".into()),
            ..same.clone()
        };
        assert!(newer.available());

        let unknown = UpdateStatus {
            latest_version: None,
            ..same
        };
        assert!(!unknown.available());
    }

    #[test]
    fn both_changelogs_are_embedded_and_non_empty() {
        assert!(CHANGELOG_EN.contains("# Changelog"));
        assert!(CHANGELOG_ZH_CN.contains("# 版本日志"));
    }

    /// The report is built in the webview but the URL is built here, and the
    /// URL is what leaves the process — so credentials are removed here too,
    /// before the budget cut can split one into an unrecognisable fragment.
    #[test]
    fn credential_shaped_text_never_reaches_the_issue_url() {
        // Built from fragments so this file holds no literal that looks like a
        // real credential.
        let token = ["ghp_", "abcdefghijklmnopqrstuvwxyz1234567890"].concat();
        let password = ["hunter", "2pass"].concat();
        let context = format!(
            "clone failed: https://alice:{password}@example.com/r.git\nAuthorization: Bearer {}\nmy token {token}",
            "abcdefghijklmnopqrstuvwxyz0123456789"
        );
        let target = bug_with(Some(format!("push failed with {token}")), Some(&context));

        let url = decode(&target.url);
        assert!(!url.contains(&password), "password reached the URL");
        assert!(!url.contains(&token), "token reached the URL");
        assert!(!url.contains("abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(url.contains("clone failed: https://[redacted]@example.com/r.git"));
        // The report is still a usable report: only the secret parts are gone.
        assert!(url.contains("Authorization: [redacted]"));
    }
}
