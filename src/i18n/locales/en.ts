/**
 * English catalogue — the shape of record.
 *
 * `zh-CN.ts` is typed as `Catalog`, so a key added here and forgotten there is
 * a `vue-tsc` error rather than a string that silently renders in the wrong
 * language. That is the same guard the Rust side gets from
 * `handler_completeness.rs`: the drift is caught by a test, not by a user.
 *
 * Conventions
 *  - `{name}` interpolates. vue-i18n escapes by default; nothing here is HTML.
 *  - Keep `app.name` untranslated — a product name is not a translatable string.
 *  - `errors.byCode` is keyed by the *wire* code, which serde renames to
 *    snake_case, and is typed against `ErrorCode`, so adding a variant to the
 *    Rust enum and forgetting this catalogue is a `vue-tsc` error.
 *  - Terminology: "branch"/"branch" 分支, "commit" 提交, "working copy" 工作区,
 *    "stage" 暂存, "checkout" 签出, "remote" 远程.
 */

import type { ErrorCode } from "../../bridges/bindings";
import diffEn from "./parts/en/diff";
import panesEn from "./parts/en/panes";
import shellEn from "./parts/en/shell";

const en = {
  // Slices, kept as their own files so three parts of the UI can be
  // translated without three people editing the same object literal.
  ...diffEn,
  ...panesEn,
  ...shellEn,

  app: {
    name: "Meridian",
    tagline: "Line of no shadow",
  },

  common: {
    ok: "OK",
    cancel: "Cancel",
    close: "Close",
    save: "Save",
    delete: "Delete",
    retry: "Retry",
    copy: "Copy",
    copied: "Copied",
    loading: "Loading…",
    none: "None",
    search: "Search",
    refresh: "Refresh",
    learnMore: "Learn more",
    notAvailableInThisBuild: "Not available in this build.",
  },

  language: {
    label: "Language",
    note: "Takes effect immediately and is remembered on this Mac.",
    en: "English",
    zhCN: "简体中文",
  },

  about: {
    title: "About Meridian",
    version: "Version",
    commit: "Commit",
    commitUnknown: "not recorded in this build",
    buildProfile: "Build",
    buildProfileDebug: "development",
    buildProfileRelease: "release",
    runtime: "Runtime",
    platform: "Platform",
    license: "License",
    licenseValue: "MIT — free and open source",
    project: "Project",
    projectUnknown: "unknown — this build has no origin remote",
    openProjectPage: "Open the project page",
    diagnostics: "Diagnostics",
    copyDiagnostics: "Copy diagnostics",
    copyDiagnosticsHint:
      "Version, platform and build details. No repository names, no paths, no usage data.",
    copied: "Copied to the clipboard.",
    copyFailed: "Could not use the clipboard.",
    showReleaseNotes: "Release notes",
  },

  updates: {
    title: "Updates",
    currentVersion: "You are on {version}.",
    checkNow: "Check for updates",
    checking: "Checking…",
    upToDate: "Meridian {version} is the latest release.",
    available: "Meridian {version} is available.",
    publishedOn: "Published {date}.",
    downloadSize: "{size} to download.",
    releaseNotes: "What changed",
    install: "Download and install",
    installing: "Downloading… {percent}%",
    downloadingUnknown: "Downloading…",
    downloadingReceived: "Downloaded {received}.",
    preparing: "Preparing…",
    readyToRestart: "Ready. Meridian will restart to finish.",
    restartNow: "Restart now",
    restartLater: "Later",
    failed: "Could not check for updates.",
    failedDetail: "Nothing was changed. You can keep using this version.",
    installFailed: "The download did not finish.",
    installFailedDetail: "Nothing was replaced. This version still works.",
    lastChecked: "Last checked {when}.",
    neverChecked: "Never checked.",
    autoCheck: "Check automatically on launch",
    autoCheckNote: "Meridian checks once shortly after launch.",
    disclosureTitle: "What this sends",
    disclosure:
      "The check asks GitHub for this repository's latest release tag. The request carries the app version and nothing else — no repository names, no file paths, no usage data, no identifier for you or this machine. GitHub sees the request the same way it sees any visit to the project page. Turn the switch off and Meridian will not contact the network on its own.",
    unavailableInThisBuild:
      "This build has no update endpoint configured, so Meridian cannot check for releases. Builds published from the project's release pipeline do.",
    notConfiguredTitle: "Updates are not set up in this build",
    debugBuildTitle: "This is a development build",
    debugBuildDetail:
      "Meridian will not replace a build it is being developed from. Release builds update themselves.",
    checkingHint: "Asking the project's release page for the newest version.",
  },

  changelog: {
    title: "Release notes",
    subtitle: "What changed in each release.",
    empty: "No release notes in this build.",
    failed: "Could not read the release notes.",
    unreleased: "Unreleased",
    releasedOn: "Released {date}",
    thisBuild: "This build",
    languageOfNotes: "Release notes language",
    viewOnGitHub: "View on GitHub",
    bundledNote: "Shipped with this build; newer releases may not appear until you update.",
  },

  feedback: {
    title: "Feedback",
    subtitle: "Reports and requests go to the project's public issue tracker.",
    bug: "Report a problem",
    bugNote:
      "Opens a new issue on GitHub with the version and platform details filled in. You review and edit it there before anything is submitted.",
    feature: "Request a capability",
    featureNote: "Opens a new issue describing what you would like Meridian to do.",
    openOnGitHub: "Open on GitHub",
    summaryLabel: "Summary",
    summaryPlaceholder: "One line, if you have one. Optional.",
    bugTitle: "Something is wrong",
    featureTitle: "A capability request",
    composing: "Preparing…",
    failed: "Could not build the report.",
    failedDetail:
      "This build does not know which project to report to. You can still open the project page above.",
    privacyTitle: "What gets filled in",
    privacy:
      "Only the app version, your macOS version, and the machine architecture. Nothing is sent until you press Submit on GitHub, and you can delete any of it first.",
    warning:
      "Do not paste repository contents, credentials or tokens into an issue. They are public.",
  },

  errors: {
    title: "Something went wrong",
    detail: "Technical detail",
    retryable: "This is worth retrying.",
    copyCode: "Copy error code",
    byCode: {
      invalid_repository: "That folder is not a Git repository.",
      no_working_copy: "This repository is bare — it has no working copy.",
      io: "A file operation failed.",
      invalid_request: "Meridian refused that request.",
      index_locked:
        "Another Git process is using this repository. Close it and try again.",
      checkout_conflicts: "Checking out would overwrite local changes.",
      checkout_failed: "Checkout failed.",
      unknown_revision: "No such revision: {revision}",
      invalid_reference_name: "“{name}” is not a valid branch or tag name.",
      reference_exists: "“{name}” already exists.",
      reference_not_found: "“{name}” no longer exists.",
      operation_in_progress: "Another operation is still running in this repository.",
      cancelled: "Cancelled.",
      unclassified: "Something went wrong.",
    } satisfies Record<ErrorCode, string>,
  },
};

// Deliberately NOT `as const`: literal types here would make every Chinese
// string have to equal its English counterpart. The structural shape is what
// `Catalog` carries, and `satisfies Record<ErrorCode, string>` above keeps the
// error codes exhaustive.
export default en;
export type Catalog = typeof en;
