//! The feedback button and the issue forms it opens must not drift apart.
//!
//! `commands::app::app_feedback_compose` builds a
//! `github.com/{slug}/issues/new?...` URL. GitHub prefills an issue *form* from
//! query parameters named after each field's `id`, so `?version=0.1.0` only
//! works while `.github/ISSUE_TEMPLATE/bug_report.yml` has a field with
//! `id: version`. Rename it there and the app keeps opening a form that is
//! silently blank — no compiler, no clippy lint, no runtime error, just a
//! reporter staring at empty boxes. This test is the only thing that notices.
//!
//! It checks both directions, because both fail quietly:
//!
//! * every id and template filename `commands/app.rs` writes must exist in the
//!   YAML — otherwise the prefill lands nowhere;
//! * the fields we deliberately *do not* prefill must still exist — otherwise
//!   the "we never fill this in for you" test in `commands/app.rs` is asserting
//!   the absence of a parameter that no longer has a field to land on.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// The `id:` of every field in an issue form. GitHub's syntax puts each under
/// a `- type: ...` entry at four spaces of indent; scanning for the key is
/// enough here and avoids taking a YAML parser dependency for one test.
fn field_ids(yaml: &str) -> BTreeSet<String> {
    yaml.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let value = trimmed.strip_prefix("id:")?;
            // Body fields are indented under `body:`; top-level keys like
            // `name:` / `description:` use the same `key: value` shape but are
            // never named `id`, so no indent check is needed.
            Some(value.trim().trim_matches(['"', '\'']).to_string())
        })
        .filter(|id| !id.is_empty())
        .collect()
}

/// Every `"..."` literal assigned to a `const ... : &str` in the command file.
/// These are the ids and template names the app actually sends.
fn string_constants(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("const ")?;
            let (_, literal) = rest.split_once('=')?;
            let literal = literal.trim().trim_end_matches(';');
            let value = literal.strip_prefix('"')?.strip_suffix('"')?;
            Some(value.to_string())
        })
        .collect()
}

/// Template filenames named anywhere in the command file. These sit in the
/// `feedback_target` match arms rather than in `const`s, so this scans the
/// whole source for `.yml` string literals.
///
/// Splitting on the quote character makes the odd-indexed pieces exactly the
/// string literals — scanning for quote *positions* instead would pair an
/// opening quote with the next closing one and read `template=bug_report.yml`
/// out of `"?template="` followed by `"bug_report.yml"`.
fn template_filenames(source: &str) -> BTreeSet<String> {
    source
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|literal| literal.ends_with(".yml"))
        .map(str::to_string)
        .collect()
}

/// Everything above the file's `#[cfg(test)]` module.
///
/// The scan has to run on production code only. `commands/app.rs` carries its
/// own unit tests, and those assert on expected URLs — `assert!(url.contains(
/// "template=bug_report.yml"))`. An expected value is not something the app
/// sends, but no amount of syntax reading can tell the two apart, so the test
/// module is cut off wholesale instead. Every literal the scanner reports then
/// comes from a line that runs in the shipped binary.
fn production_source(source: &str) -> &str {
    match source.find("#[cfg(test)]") {
        Some(offset) => &source[..offset],
        // A file with no test module is entirely production code. If the
        // scanner then picks up a literal it should not, the assertions below
        // fail loudly rather than passing on an empty set.
        None => source,
    }
}

const BUG_FORM: &str = ".github/ISSUE_TEMPLATE/bug_report.yml";
const FEATURE_FORM: &str = ".github/ISSUE_TEMPLATE/feature_request.yml";
const APP_COMMANDS: &str = "src-tauri/src/commands/app.rs";

/// Fields the bug form asks for that this app never fills in. Listed so that a
/// rename here is caught: the app's "we don't prefill sensitive-data" test
/// would otherwise be asserting the absence of a parameter with no field.
const BUG_FIELDS_LEFT_TO_THE_REPORTER: &[&str] = &[
    "what-happened",
    "what-expected",
    "steps-to-reproduce",
    "sensitive-data",
];

#[test]
fn every_form_field_the_app_prefills_still_exists() {
    let source = read(APP_COMMANDS);
    let sent = string_constants(production_source(&source));
    let bug_ids = field_ids(&read(BUG_FORM));

    for id in ["version", "macos-version", "architecture"] {
        assert!(
            sent.contains(id),
            "commands/app.rs no longer has a `const ... = \"{id}\"` for the bug \
             form's `{id}` field. If that field was renamed, rename the \
             constant too; if it was dropped, drop the prefill."
        );
        assert!(
            bug_ids.contains(id),
            "commands/app.rs prefills `{id}`, but {BUG_FORM} has no field with \
             `id: {id}`. GitHub silently ignores a parameter with no matching \
             field, so the reporter would get an empty box.\n  \
             the form has: {bug_ids:?}"
        );
    }
}

#[test]
fn the_forms_keep_the_fields_we_deliberately_leave_blank() {
    let bug_ids = field_ids(&read(BUG_FORM));

    for id in BUG_FIELDS_LEFT_TO_THE_REPORTER {
        assert!(
            bug_ids.contains(*id),
            "{BUG_FORM} no longer has a field with `id: {id}`.\n  \
             The most important of these is `sensitive-data`: the app opens the \
             form with it deliberately unchecked, and a unit test in \
             commands/app.rs asserts the URL never carries it. That test is \
             meaningless if there is no box to leave unchecked.\n  \
             the form has: {bug_ids:?}"
        );
    }

    let feature_ids = field_ids(&read(FEATURE_FORM));
    assert!(
        feature_ids.contains("problem"),
        "{FEATURE_FORM} lost its `problem` field — a capability request with no \
         space to describe the problem becomes a wish list.\n  \
         the form has: {feature_ids:?}"
    );
}

#[test]
fn both_forms_the_app_links_to_exist() {
    let source = read(APP_COMMANDS);
    let forms = template_filenames(production_source(&source));

    assert!(
        forms.contains("bug_report.yml") && forms.contains("feature_request.yml"),
        "expected commands/app.rs to name both issue forms, found {forms:?}"
    );

    for form in forms {
        let path = format!(".github/ISSUE_TEMPLATE/{form}");
        assert!(
            repo_root().join(&path).is_file(),
            "commands/app.rs opens `?template={form}`, but {path} does not \
             exist. GitHub falls back to an empty issue body."
        );
    }
}

#[test]
fn the_forms_are_actually_issue_forms() {
    // Without a `body:` list GitHub treats the file as legacy Markdown with
    // front matter, ignores `?version=`, and drops the field ids entirely.
    for form in [BUG_FORM, FEATURE_FORM] {
        let yaml = read(form);
        assert!(
            yaml.lines()
                .any(|line| line.trim_start().starts_with("body:")),
            "{form} has no `body:` — it is not an issue form, so none of its \
             field ids are addressable from a URL"
        );
        assert!(
            !field_ids(&yaml).is_empty(),
            "{form} declares no fields with an `id:`, so nothing the app sends \
             can prefill"
        );
    }
}
