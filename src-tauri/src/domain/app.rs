//! Application-level vocabulary: the things that describe the app itself
//! rather than a repository.
//!
//! These types exist because the update, release-notes and feedback surfaces
//! belong to no repository. Every other command in the app is addressed to a
//! repository and travels through the `action → validator → operation` triad
//! (docs/02 §3); these deliberately do not, because `Command::repo_id()` and
//! `ExecCx` both require a resolved repository handle and there is none to give.
//! Inventing a fake one to satisfy the trait would be worse than a second,
//! clearly-labelled family of commands.
//!
//! Plain data only — no `git2`, no `tauri`.

use serde::{Deserialize, Serialize};

/// Which build this is. Debug builds never offer updates: the updater would
/// replace a working tree the developer is debugging.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProfile {
    Debug,
    Release,
}

/// Everything the About panel shows, and everything a bug report needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// Short SHA, when the build came from a git checkout.
    pub commit: Option<String>,
    pub build_profile: BuildProfile,
    pub rust_version: String,
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    /// `owner/name`. `None` when the build had no `origin` remote, which
    /// disables updates and feedback rather than guessing an address.
    pub repo_slug: Option<String>,
    /// Whether an update endpoint is configured in this build at all.
    pub updates_configured: bool,
    /// Whether this build can self-update (an unsigned build cannot).
    pub can_self_update: bool,
    /// The text behind "Copy diagnostics".
    ///
    /// Carried on the wire rather than assembled in the frontend because the
    /// frontend would have to reimplement the same formatting, and the two
    /// versions would drift — with the one the *user pastes into an issue*
    /// being the one nobody tests. `with_diagnostics` fills it at construction.
    /// Treat it as read-only.
    pub diagnostics: String,
}

impl AppInfo {
    /// Fills `diagnostics` from the other fields.
    ///
    /// Called once, immediately after construction. A struct cannot compute a
    /// field from its siblings in the literal, so this is the closest thing to
    /// "derived" that keeps a single source of truth.
    #[must_use]
    pub fn with_diagnostics(mut self) -> Self {
        let commit = self.commit.as_deref().unwrap_or("unrecorded");
        let repo = self.repo_slug.as_deref().unwrap_or("unconfigured");
        self.diagnostics = format!(
            "{name} {version} ({commit})\n{os} {os_version} · {arch} · rustc {rust}\nbuild: {profile:?}\nproject: {repo}",
            name = self.name,
            version = self.version,
            os = self.os_name,
            os_version = self.os_version,
            arch = self.arch,
            rust = self.rust_version,
            profile = self.build_profile,
        );
        self
    }
}

/// Which language of release notes to serve. The catalogues are compiled in,
/// so this is a selection between two bundled files, not a download.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotesLocale {
    ZhCn,
    En,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotes {
    /// The changelog verbatim, as Markdown.
    pub markdown: String,
    pub locale: NotesLocale,
    /// The version this build is, so the UI can mark where the reader is.
    pub current_version: String,
}

/// The result of asking the update endpoint what the latest release is.
///
/// `available` is derived, not stored: a build should never be able to report
/// "update available" for the version it is already running.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    /// Download size in bytes, when the endpoint states it.
    pub download_bytes: Option<u64>,
}

impl UpdateStatus {
    pub fn available(&self) -> bool {
        matches!(&self.latest_version, Some(latest) if latest != &self.current_version)
    }
}

/// A build that cannot check for updates still has to answer *why*, or the UI
/// shows a spinner forever.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum UpdateCheck {
    /// The endpoint answered.
    Checked { status: UpdateStatus },
    /// No endpoint is configured in this build.
    NotConfigured,
    /// This is a debug build; self-updating it would replace the developer's
    /// working tree.
    UnsupportedInDebugBuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Bug,
    Feature,
}

/// Which issue form to open, and the fields we can fill in honestly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackRequest {
    pub kind: FeedbackKind,
    /// What the user typed, if anything. Never sent anywhere by us: it becomes
    /// a query parameter on a GitHub URL the *user* opens and reviews.
    #[serde(default)]
    pub summary: Option<String>,
    /// The compact diagnostics text from the Diagnostics screen, already
    /// redacted there. Prefills the bug form's `diagnostics` field and nothing
    /// else; a feature request ignores it. Like `summary` it ends up in a URL
    /// the user opens and reviews, never in a request this app makes.
    #[serde(default)]
    pub context: Option<String>,
}

/// The URL the feedback action opens, plus what was put in it.
///
/// Returned to the frontend rather than opened in the backend so the caller can
/// show the user what is about to be attached before a browser opens.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackTarget {
    pub url: String,
    /// The diagnostic lines that were prefilled, so the UI can show them.
    pub prefilled: Vec<String>,
}
