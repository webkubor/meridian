//! Meridian — application bootstrap and the module map.
//!
//! Layering (docs/02 §1, enforced by review and by module imports):
//!
//! ```text
//! commands   ← the Tauri command surface (docs/03); the only layer that
//!              knows Tauri. May depend on everything below.
//! validation ← pure precondition checks over a repository handle.
//! transform  ← pure git2 ↔ domain mappings.
//! engine     ← git2-backed operations; never depends on `tauri`.
//! domain     ← plain data; never depends on git2 / tauri / rusqlite.
//! ```
//!
//! Plus the infrastructure modules: `queue` (per-repo serialization),
//! `events` (the app-wide event bus) and `state` (the registry + run loop).
//!
//! ## Keeping `generate_handler!` complete
//!
//! `generate_handler!` expands to a `match` on the command name whose
//! fallback arm is `_ => return false`, so a `#[tauri::command]` that is not
//! listed here still compiles, still passes `cargo check`/`clippy`/`test`, and
//! still fails only at invoke time with `Command <name> not found`. Nothing in
//! the type system or the test suite can catch the omission — which is exactly
//! how 60 implemented commands went missing once already.
//!
//! So the list below is kept sorted within each family, and
//! `meridian::command_registration_is_complete` (a `#[test]` in
//! `tests/handler_completeness.rs`) walks the source tree and asserts that
//! every `#[tauri::command]` in `commands/` appears here. Add the wrapper and
//! the registration in the same change.

pub mod commands;
pub mod domain;
pub mod engine;
pub mod events;
pub mod queue;
pub mod state;
pub mod transform;
pub mod validation;

pub fn run() {
    tauri::Builder::default()
        // Opener backs "open in browser / Finder"; process backs the relaunch
        // after a self-update; updater is inert until `plugins.updater` in
        // tauri.conf.json carries a public key.
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            // --- application (no repository) --------------------------------
            // The only family not addressed to a repository; see
            // `commands/app.rs` for why it does not go through `Command`.
            commands::app_info_load,
            commands::app_release_notes_load,
            commands::app_update_check,
            commands::app_update_install,
            commands::app_relaunch,
            commands::app_feedback_compose,
            // --- repository -------------------------------------------------
            commands::repository_open,
            commands::repository_init,
            commands::repository_discover,
            commands::repository_validate_path,
            commands::repository_refresh,
            commands::repository_state,
            commands::repository_health_load,
            commands::repository_stats_load,
            commands::repository_remotes_load,
            commands::repository_worktrees_load,
            commands::repository_config_load,
            commands::repository_config_set,
            commands::repository_description_set,
            // --- working copy ----------------------------------------------
            commands::wc_status_load,
            commands::wc_stage_paths,
            commands::wc_unstage_paths,
            commands::wc_stage_all,
            commands::wc_unstage_all,
            commands::wc_paths_discard,
            commands::wc_entry_detail_load,
            commands::wc_entry_open,
            commands::wc_entry_ignore,
            commands::wc_entry_untrack,
            commands::wc_entry_assume_unchanged_set,
            // --- diff -------------------------------------------------------
            commands::wc_diff_load,
            commands::wc_diff_hunks_load,
            commands::wc_image_data_load,
            commands::wc_chunk_apply,
            commands::wc_chunk_discard,
            commands::wc_patch_apply,
            commands::wc_patch_save,
            // --- commit -----------------------------------------------------
            commands::commit_create,
            commands::commit_amend,
            // --- history ----------------------------------------------------
            commands::history_commits_load,
            commands::history_commit_load,
            commands::history_commit_hashes_load,
            commands::history_count_load,
            commands::history_graph_compute,
            commands::history_changeset_load,
            commands::history_contributors_load,
            commands::history_file_history_load,
            commands::history_revision_range_changeset_load,
            // --- branches ---------------------------------------------------
            commands::branch_list,
            commands::branch_create,
            commands::branch_checkout,
            commands::branch_load,
            commands::branch_stacked_create,
            commands::branch_stack_restack,
            commands::branch_checkout_remote,
            commands::branch_checkout_revision,
            commands::branch_rename,
            commands::branch_delete,
            commands::branch_update,
            commands::branch_fast_forward,
            commands::branch_set_upstream,
            commands::branch_unset_upstream,
            commands::branch_track_remote,
            commands::branch_untrack,
            // --- remotes ----------------------------------------------------
            commands::remote_branch_list,
            commands::remote_branch_delete,
            commands::remote_branch_prune,
            commands::remote_list,
            commands::remote_add,
            commands::remote_rename,
            commands::remote_delete,
            commands::remote_set_url,
            commands::remote_refspec_add,
            commands::remote_refspec_delete,
            commands::remote_set_fetch_mode,
            commands::remote_fetch,
            commands::remote_pull,
            commands::remote_push,
            commands::remote_force_push,
            commands::remote_prune,
            commands::remote_publish_branch,
            // --- tags -------------------------------------------------------
            commands::tag_list,
            commands::tag_detail_load,
            commands::tag_create,
            commands::tag_delete,
            commands::tag_publish,
            commands::tag_delete_remote,
            // --- stashes ----------------------------------------------------
            commands::stash_list,
            commands::stash_count_load,
            commands::stash_save,
            commands::stash_apply,
            commands::stash_pop,
            commands::stash_drop,
            commands::stash_show,
            // --- submodules & worktrees -------------------------------------
            commands::submodule_list,
            commands::submodule_init,
            commands::submodule_update,
            commands::submodule_sync,
            commands::submodule_settings_save,
            commands::worktree_list,
            commands::worktree_add,
            commands::worktree_remove,
            commands::worktree_lock,
            // --- integrate: merge / rebase / cherry / revert ------------------
            commands::merge_start,
            commands::merge_abort,
            commands::merge_status_load,
            commands::wc_merge_status_load,
            commands::merge_tool_invoke,
            commands::rebase_start,
            commands::rebase_interactive_plan,
            commands::rebase_interactive_start,
            commands::cherry_pick,
            commands::revert,
            commands::integration_continue,
            commands::integration_skip,
            commands::reset_to_revision,
            commands::reset_entries_to_revision,
            // --- integrate: history rewriting --------------------------------
            commands::rewrite_impact_load,
            commands::squash_revisions,
            commands::fixup_revisions,
            commands::move_revisions,
            commands::delete_revisions,
            // --- integrate: conflicts & external tools ----------------------
            commands::wc_conflicts_load,
            commands::wc_conflict_resolve,
            commands::conflict_hunks_load,
            commands::conflict_resolve_hunks,
            commands::diff_tool_invoke,
        ])
        .run(tauri::generate_context!())
        .expect("error while running meridian");
}
