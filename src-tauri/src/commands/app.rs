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

use serde::Deserialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

use crate::domain::app::{
    AppInfo, BuildProfile, FeedbackKind, FeedbackRequest, FeedbackTarget, NotesLocale,
    ReleaseNotes, UpdateCheck, UpdateStatus,
};
use crate::domain::error::{ErrorCode, ErrorPayload};

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
    let info = info_for(&app);

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
    let info = info_for(&app);
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
            push_param(&mut query, "title", summary);
        }
    }
    for (key, value) in prefilled.iter() {
        push_param(&mut query, key, value);
    }

    prefilled.retain(|(_, value)| !value.is_empty());

    Some(FeedbackTarget {
        url: format!("https://github.com/{slug}/issues/new?{query}"),
        prefilled: prefilled
            .into_iter()
            .map(|(key, value)| format!("{key}: {value}"))
            .collect(),
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
    let info = info_for(&app);
    feedback_target(&info, &args).ok_or_else(|| {
        ErrorPayload::new(
            ErrorCode::InvalidRequest,
            "This build does not know which project to report to.",
        )
    })
}

// --- error helpers ---------------------------------------------------------

fn update_error(error: &tauri_plugin_updater::Error) -> ErrorPayload {
    ErrorPayload::new(ErrorCode::Io, "Could not reach the update endpoint.")
        .with_detail(error.to_string())
}

fn invalid_endpoint(error: &dyn std::fmt::Display) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::Unclassified,
        "The update endpoint is not a valid URL.",
    )
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
            },
        )
        .expect("slug is set");

        assert!(target
            .url
            .contains("title=staging+%22foo%2Fbar%22+fails+%26+dies"));
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
}
