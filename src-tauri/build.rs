use std::process::Command;

/// Capture build provenance as compile-time environment.
///
/// Two things are recorded here rather than hardcoded anywhere else:
///
/// 1. **The git commit.** A bug report that says "0.1.0" is nearly useless when
///    several builds share that version; the short SHA is what makes it
///    actionable. Absent in a source tarball — that is reported as `None`, not
///    as an empty string the UI would render as a blank line.
///
/// 2. **The project slug** (`owner/name`), read from the `origin` remote. The
///    repository URL is needed by the update endpoint, the feedback links and
///    the release-notes links, and it must not be duplicated into four files
///    where three of them can go stale. Deriving it here gives one source.
///    `MERIDIAN_REPO_SLUG` can override it, which is what a fork or a CI runner
///    without a git remote should set.
fn main() {
    tauri_build::build();

    let commit = git(&["rev-parse", "--short=12", "HEAD"]);
    println!(
        "cargo:rustc-env=MERIDIAN_GIT_COMMIT={}",
        commit.as_deref().unwrap_or("")
    );

    let slug = std::env::var("MERIDIAN_REPO_SLUG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(remote_slug);

    match slug {
        Some(slug) => println!("cargo:rustc-env=MERIDIAN_REPO_SLUG={slug}"),
        // Not fatal: a build without a slug still runs, it just cannot offer
        // updates or a feedback link, and says so in the UI rather than
        // pointing at a wrong address.
        None => {
            println!(
                "cargo:warning=no origin remote; MERIDIAN_REPO_SLUG is unset, \
                      so the update check and feedback links will be unavailable"
            );
            println!("cargo:rustc-env=MERIDIAN_REPO_SLUG=");
        }
    }

    // Re-derive when HEAD or the remote moves, otherwise the provenance is
    // whatever it was on the first build in this target directory.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/config");
    println!("cargo:rerun-if-env-changed=MERIDIAN_REPO_SLUG");
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// `owner/name` from `remote.origin.url`, across the three shapes it takes:
/// `https://host/owner/name.git`, `git@host:owner/name.git`, `ssh://…/owner/name`.
fn remote_slug() -> Option<String> {
    let url = git(&["remote", "get-url", "origin"])?;
    let trimmed = url.trim().trim_end_matches(".git");

    let path = if let Some(rest) = trimmed.split_once("://").map(|(_, rest)| rest) {
        // scheme://host/owner/name — drop the host.
        rest.split_once('/').map(|(_, path)| path)?
    } else if let Some((_, path)) = trimmed.split_once(':') {
        // scp-like: git@host:owner/name
        path
    } else {
        trimmed
    };

    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let owner = segments.next()?;
    let name = segments.next()?;
    Some(format!("{owner}/{name}"))
}
