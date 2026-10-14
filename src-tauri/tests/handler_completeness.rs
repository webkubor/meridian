//! Handler-completeness guard.
//!
//! `generate_handler!` expands to a `match` whose fallback arm is
//! `_ => { return false }`. A `#[tauri::command]` that is never listed there
//! therefore still compiles, still passes `cargo check`, `cargo clippy` and
//! `cargo test`, and only fails when the frontend invokes it — as
//! `Command <name> not found`. No compiler or existing test can catch it.
//!
//! 60 implemented commands were unreachable for exactly that reason. This test
//! closes the hole: it walks `src/commands/`, collects every declared command
//! (both the hand-written `#[tauri::command]` fns and the ones the
//! `tauri_command!` macro expands), and asserts each one appears in
//! `lib.rs`'s `generate_handler!` list.
//!
//! Adding a command wrapper without registering it fails here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Command names listed in `generate_handler!`, in `lib.rs`.
fn registered() -> BTreeSet<String> {
    let lib = fs::read_to_string(src_dir().join("lib.rs")).expect("lib.rs is readable");
    let body = lib
        .split_once("generate_handler![")
        .expect("lib.rs contains generate_handler![")
        .1
        .split_once("])")
        .expect("generate_handler![ is closed")
        .0;

    body.lines()
        .filter_map(|line| line.split("//").next())
        .filter_map(|line| line.trim().strip_prefix("commands::"))
        .map(|name| name.trim().trim_end_matches(',').to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Command names declared under `src/commands/`, from both declaration forms.
fn declared() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let commands_dir = src_dir().join("commands");
    let files: Vec<PathBuf> = fs::read_dir(&commands_dir)
        .expect("src/commands is readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();

    for file in files {
        let source = fs::read_to_string(&file).expect("command module is readable");
        let lines: Vec<&str> = source.lines().collect();

        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Form 1: a hand-written `#[tauri::command] pub fn name(`.
            // The attribute and the `fn` may be separated by doc comments or
            // other attributes, so scan forward to the first `fn`.
            if trimmed == "#[tauri::command]" {
                for next in lines.iter().skip(index + 1) {
                    let candidate = next.trim();
                    if candidate.starts_with("fn ")
                        || candidate.starts_with("pub fn ")
                        || candidate.starts_with("pub async fn ")
                    {
                        if let Some(name) = fn_name(candidate) {
                            names.insert(name);
                        }
                        break;
                    }
                    // Stop at the next item boundary rather than running past
                    // this function into a later one.
                    if candidate.starts_with("#[") && candidate != "#[tauri::command]" {
                        break;
                    }
                }
            }
        }

        // Form 2: `tauri_command!(name, Args, Output);` — the macro emits a
        // `#[tauri::command]` fn of the same name.
        //
        // Scanned over the whole source rather than line by line, because
        // rustfmt rewraps a long invocation onto several lines:
        //
        //     tauri_command!(
        //         branch_stacked_create,
        //         BranchStackedCreate,
        //         StackedBranchInfo
        //     );
        //
        // A line-oriented parser sees no `(` after the macro name on line one
        // and reports the command as undeclared — a false failure that says the
        // registration is broken when only its formatting changed.
        for (offset, _) in source.match_indices("tauri_command!(") {
            let rest = &source[offset + "tauri_command!(".len()..];
            let name: String = rest
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }

    names
}

/// The identifier from `fn name(` / `pub async fn name(`.
fn fn_name(signature: &str) -> Option<String> {
    let after_fn = signature.split_once("fn ")?.1;
    let name: String = after_fn
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

#[test]
fn every_declared_command_is_registered() {
    let declared = declared();
    let registered = registered();

    assert!(
        !declared.is_empty(),
        "found no command declarations — the parser is broken, not the crate"
    );

    let missing: Vec<&String> = declared.difference(&registered).collect();

    assert!(
        missing.is_empty(),
        "{} `#[tauri::command]` function(s) are missing from generate_handler! in \
         src-tauri/src/lib.rs and will fail at invoke time with `Command <name> not found`:\n  {}\n\
         Register each one in lib.rs in the same change that adds the wrapper.",
        missing.len(),
        missing
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

#[test]
fn no_registered_command_is_undeclared() {
    // Catches a stale or misspelled entry in the registration list.
    let declared = declared();
    let registered = registered();

    let orphans: Vec<&String> = registered.difference(&declared).collect();

    assert!(
        orphans.is_empty(),
        "generate_handler! lists {} name(s) with no matching declaration:\n  {}",
        orphans.len(),
        orphans
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

#[test]
fn registration_has_no_duplicates() {
    // A duplicate would not fail the match, but it signals a copy-paste merge
    // and hides the fact that some *other* command was left out.
    let lib = fs::read_to_string(src_dir().join("lib.rs")).expect("lib.rs is readable");
    let body = lib
        .split_once("generate_handler![")
        .expect("lib.rs contains generate_handler![")
        .1
        .split_once("])")
        .expect("generate_handler![ is closed")
        .0;

    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();

    for line in body.lines() {
        let Some(name) = line.split("//").next().map(str::trim) else {
            continue;
        };
        let Some(name) = name.strip_prefix("commands::") else {
            continue;
        };
        let name = name.trim_end_matches(',');
        if !seen.insert(name.to_string()) {
            duplicates.push(name);
        }
    }

    assert!(
        duplicates.is_empty(),
        "generate_handler! lists these commands more than once: {duplicates:?}"
    );
}
