//! The two changelogs must stay in lockstep.
//!
//! The application ships `CHANGELOG.md` and `CHANGELOG.zh-CN.md` and renders
//! whichever matches the reader's language. Nothing in the type system connects
//! them: adding a release to one file and forgetting the other compiles, ships,
//! and is only noticed by a Chinese-speaking user reading English release
//! notes. This test is that connection.
//!
//! It deliberately checks *version headings*, not content. Translations are
//! allowed to read naturally rather than word-for-word; what has to match is
//! which releases exist.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .join(name)
}

fn read(name: &str) -> String {
    let path = repo_file(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// `## [1.2.3] - 2026-01-01` and `## [Unreleased]` both yield their label.
fn version_headings(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .filter_map(|rest| {
            let rest = rest.trim();
            let inner = rest.strip_prefix('[')?;
            let (label, _) = inner.split_once(']')?;
            Some(label.trim().to_string())
        })
        .collect()
}

#[test]
fn both_changelogs_list_the_same_versions_in_the_same_order() {
    let en = version_headings(&read("CHANGELOG.md"));
    let zh = version_headings(&read("CHANGELOG.zh-CN.md"));

    assert!(
        !en.is_empty(),
        "the English changelog has no `## [version]` headings — the parser is \
         broken, or the format changed"
    );

    let en_set: BTreeSet<String> = en.iter().cloned().collect();
    let zh_set: BTreeSet<String> = zh.iter().cloned().collect();

    let only_english: Vec<&String> = en_set.difference(&zh_set).collect();
    let only_chinese: Vec<&String> = zh_set.difference(&en_set).collect();

    assert!(
        only_english.is_empty() && only_chinese.is_empty(),
        "CHANGELOG.md and CHANGELOG.zh-CN.md document different releases.\n  \
         only in English: {only_english:?}\n  only in Chinese: {only_chinese:?}\n\
         A release has to be written in both, or the app shows English release \
         notes to a reader who chose Chinese."
    );

    assert_eq!(
        en, zh,
        "the two changelogs list the same versions in a different order"
    );
}

#[test]
fn the_changelogs_have_a_place_to_record_the_next_release() {
    let en = version_headings(&read("CHANGELOG.md"));

    assert!(
        en.iter().any(|label| label == "Unreleased"),
        "CHANGELOG.md has no `## [Unreleased]` section, so the next change has \
         nowhere to be recorded before it ships"
    );
}

#[test]
fn the_shipped_version_is_documented() {
    let version = env!("CARGO_PKG_VERSION");
    let en = version_headings(&read("CHANGELOG.md"));

    assert!(
        en.iter().any(|label| label == version),
        "Cargo.toml says this is {version}, but CHANGELOG.md never mentions \
         that version. Bump the crate and write the entry together.\n  \
         documented: {en:?}"
    );
}
