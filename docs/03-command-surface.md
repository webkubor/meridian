# 03 — Command Surface

The complete Tauri command list for Meridian. Every command here is the third leg of the observed
triad — the `GTGit*Operation` counterpart (`EV:analysis/git-operations#F1`) — with its validator
preconditions named explicitly. The web UI may call nothing else: `bridges/commands.ts` wraps these
commands by hand, and `src-tauri/tests/handler_completeness.rs` fails the build if a
`#[tauri::command]` is declared but not registered in `lib.rs`'s `generate_handler!`.

## 0. Conventions

**Naming.** `<module>_<verb>` where `<module>` is the engine module and `<verb>` is the git verb:
`wc_stage_paths`, `branch_create`, `remote_push`, `stash_apply`, `merge_start`, `reflog_load`.
Modules follow the nine observed operation groups (`EV:types/GTGitOperationGroupBranches` …), plus
`commit`, `worktrees`, `integrate`, `security`, `services`, `workflows`, `settings`, `system`.

**Signature shape.** All commands:

```rust
#[tauri::command]
async fn <name>(state: State<'_, AppState>, args: <Args>) -> Result<<Out, ErrorPayload>;
```

`args` is a single struct (easier to evolve, one TS type per command). Every args struct carries
`repo: RepoId` where a repository is implied, and `sender: SenderTag` where the evidence shows
sender-scoped validation (`EV:analysis/git-operations#F2`). Long-running commands additionally take
`on_event: Channel<ProgressEvent>`.

**Risk and validation.** Each command declares `Risk::{None, Destructive, Rewrite}`. `Destructive`
and `Rewrite` commands take or link to a snapshot unless `opts.allow_without_snapshot` (default
false). Validation is a pure function over repository state; the table's *Validator* column lists
the preconditions in order.

**Error envelope** (docs/02 §14): `{ code, message, detail?, params, retryable, domain }`.

**Where the mapping says "child git"** the operation runs through `engine::child` (supervised `git`
process) because libgit2 does not implement it — the observed `FNGit*Command` counterpart
(`EV:analysis/git-operations#F8`). These are marked `⟨child⟩`.

---

## 1. Module index

| Module | Commands | Evidence group |
|---|---|---|
| `repository` | 21 | `GTGitOperationGroupRepository` |
| `working_copy` | 26 | `GTGitOperationGroupWorkingTree` |
| `commit` | 11 | (WorkingTree + Tower module) |
| `history` | 17 | `GTGitOperationGroupHistory` |
| `branches` | 26 | `GTGitOperationGroupBranches` |
| `remote_branches` | 5 | `GTGitOperationGroupRemoteBranches` |
| `remotes` | 27 | `GTGitOperationGroupRemotes` |
| `tags` | 7 | `GTGitOperationGroupTags` |
| `stashes` | 8 | `GTGitOperationGroupStashes` |
| `submodules` | 12 | `GTGitOperationGroupSubmodules` |
| `worktrees` | 11 | (Tower module: worktree actions) |
| `integrate` | 19 | (History/rewrite/integrate bucket) |
| `security` | 15 | (SSH/GPG/signing) |
| `services` | 26 | `GTService*Operation` (135 types) |
| `workflows` | 23 | (Graphite/GitFlow/SVN/Gerrit) |
| `settings` | 24 | `GTUserDefaults*` (190 keys) |
| `system` | 13 | (app shell, updates, CLI, deep links) |
| **Total** | **291** | |

Counts verified mechanically against the tables below (see §19).

---

## 2. `repository` — lifecycle and discovery

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `repository_open` | `path, sender` | `RepoInfo` | `Repository::open` → on failure `Repository::discover` | path exists; is a directory; documented "found parent repo" answer when discovered | `GTGitUpdateRepositoryOperation` |
| `repository_init` | `path, bare, initial_branch, create_gitignore` | `RepoInfo` | `Repository::init_opts` (`RepositoryInitOptions`) | path empty or creatable; branch name valid (`RefNameValidator`) | `GTGitCreateRepositoryOperation` |
| `repository_clone` | `url, dest, opts{bare, init_submodules, use_lfs, depth?}, on_event` | `RepoInfo` | `RepoBuilder` + `FetchOptions::remote_callbacks` | dest empty or missing; URL parse; writes default cloning dir when `dest` omitted | `GTGitClone{Repository,Operation}`, `GTUserDefaultsDefaultCloningDirectory` |
| `repository_clone_cancel` | `activity` | `()` | token cancel | activity running | `GTGitCancelCloneActivitiesAction` |
| `repository_discover` | `root, max_depth` | `Vec<RepoRoot>` | fs walk + `Repository::open` per candidate | root readable | `GTGitRepositoryFinderProcessResultsOperation` |
| `repository_state` | `repo` | `IntegrationState` | `Repository::state()` + merge/rebase info loaders | — | `GTGitMergeResultStateLoader` |
| `repository_refresh` | `repo` | `RepoInfo` | re-read HEAD, refs, config | — | `GTGitReloadRepositoryAction` |
| `repository_validate_path` | `path` | `PathCheck{kind, detail}` | `Repository::open_ext` probe | — | `GTGitValidateRepositoriesOperation` |
| `repository_config_load` | `repo, scope` | `ConfigSnapshot` | `Repository::config` / `Config::open_level` | scope valid | `GTGitReloadGlobalConfigOperation` |
| `repository_config_set` | `repo, scope, key, value` | `()` | `Config::set_str` + `GTGitSetGlobalConfigValues` path | key not in the protected managed set (see §17) | `GTGitSetGlobalConfigValuesOperation` |
| `repository_stats_load` | `repo` | `RepoStats{commits, contributors, size?}` | `GTGitUpdateRepositoryStatsOperation` counterpart: revwalk count | — | `GTGitUpdateRepositoryStatsOperation` |
| `repository_description_set` | `repo, text` | `()` | direct read/write of `.git/description` | — | `GTGitSetRepositoryDescriptionOperation` |
| `repository_git_binary_get` | — | `GitBinaryInfo{path, version, source}` | `engine::gitbin` discovery | — | `GTGitBinaryFinder` |
| `repository_git_binary_set` | `path` | `GitBinaryReport` | spawn `git --version`; version gate | executable; version ≥ minimum; else typed error | `GTGitBinaryValidator`, `GTGitBinaryMinimumRequiredVersionError` |
| `repository_bookmark_add` | `path, name?, group?` | `Bookmark` | SQLite | path is a repo | `GTBookmarksSidebarViewController` |
| `repository_bookmark_remove` | `id` | `()` | SQLite | exists | same |
| `repository_bookmark_move` | `id, group, index` | `()` | SQLite | exists | `GTBookmarksDropZoneView` |
| `repository_bookmark_group_create/rename/delete` | `…` | `BookmarkGroup` | SQLite | name non-empty | same |
| `repository_recents_list` | `limit?` | `Vec<RecentEntry>` | SQLite | — | "Recently Cloned Repositories" |
| `repository_recents_clear` | `()` | `()` | SQLite | — | same |
| `repository_quick_open_index` | `enable, search_paths[], ignored_paths[]` | `IndexerStatus` | `policy::finder` task control | paths readable | `GTGitRepositoryFinder`, `GTUserDefaultsQuickOpen*` |

**Discovery phases** (mirrors `GTGitRepositoryFinderDid{StartSearching,UpdateResults,StopGathering}Notification`):
the indexer emits `AppEvent::IndexerPhase { phase: Gathering|Processing|Searching }` then batched
`IndexerResults { roots, scanned_dirs, found }`.

---

## 3. `working_copy` — the staging and file-action surface

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `wc_status_load` | `repo, opts{include_ignored, include_dirs, include_nonmodified, include_assume_unchanged, conflicted_only}` | `WorkingTreeStatus` | `Repository::statuses(StatusOptions)` | repo has workdir (`GTGitNoWorkingCopyError` otherwise) | `GTGitStatusOptionInclude*` |
| `wc_entry_detail_load` | `repo, path` | `EntryDetail{size, mode, mtime, oid?, conflict?}` | status + `Index`/`Odb` reads | path in workdir | `GTGitLoadWorkingTreeEntryDataOperation` |
| `wc_stage_paths` | `repo, paths[]` | `StatusDelta` | `Index::add_path`/`add_all` + `Index::write` | paths exist or were deleted (then `remove_path`) | `GTGitStageEntriesAction` |
| `wc_stage_all` | `repo` | `StatusDelta` | `Index::add_all(["."], IndexAddOption::DEFAULT)` + write | — | `GTGitStageAllEntriesAction` |
| `wc_unstage_paths` | `repo, paths[]` | `StatusDelta` | `Repository::reset_default(Some(HEAD), paths)` | HEAD exists (else no-op with explanation) | `GTGitUnstageEntriesAction` |
| `wc_unstage_all` | `repo` | `StatusDelta` | `reset_default(Some(HEAD), ["."])` | HEAD exists | `GTGitUnstageAllEntriesAction` |
| `wc_chunk_apply` | `repo, path, hunk, direction{Stage,Unstage}, opts{context?}` | `StatusDelta` | build a patch buffer from the hunk, `Diff::from_buffer` → `Repository::apply(diff, ApplyLocation::{Index,Workdir}, ApplyOptions)` | hunk belongs to a modified text file; file not conflicted | `stageChunk:` / `unstageChunk:` |
| `wc_chunk_discard` | `repo, path, hunk` | `StatusDelta` | inverse patch apply + checkout of the untouched hunks | same; requires confirm unless `AlwaysDiscardDiffContent` | `discardChunk:`, `GTUserDefaultsAlwaysDiscardDiffContent` |
| `wc_paths_discard` | `repo, paths[], include_untracked, confirm_token?` | `StatusDelta` | `CheckoutBuilder` (`checkout_head` for staged, `checkout_index` for workdir) | paths not conflicted; `confirm_token` required when untracked included | `GTDiscardLocalChangesDialogueViewController` |
| `wc_entry_ignore` | `repo, path, scope{Repo,Global}, pattern?` | `StatusDelta` | append to `.gitignore` / `core.excludesFile` | path not tracked (else `GTIgnoreTrackedFilesDialogueViewController` flow) | `GTAddCustomIgnorePatternViewController` |
| `wc_entry_assume_unchanged_set` | `repo, paths[], value` | `StatusDelta` | ⟨child⟩ `git update-index --[no-]assume-unchanged --` | libgit2 exposes no setter for this flag; paths tracked | `GTGitSet/UnsetAssumeUnchangedEntriesAction` |
| `wc_entry_untrack` | `repo, paths[]` | `StatusDelta` | `Index::remove_path` + write (keep workdir file) | paths tracked and clean or confirmed | `GTGitUntrackEntriesAction` |
| `wc_entry_open` | `repo, path, target{Editor,Terminal,Finder,DiffTool}` | `()` | `platform::*` spawn | configured tool exists, else typed error | `openInTerminal:`/`openInFinder:` |
| `wc_diff_load` | `repo, path?, perspective, opts{context_lines, ignore_whitespace, show_full, syntax}` | `DiffDocument` | `diff_tree_to_index` / `diff_index_to_workdir` / `diff_tree_to_workdir_with_index` + `DiffOptions` | perspective valid; path (if given) exists in one side | 6 perspectives, `EV:analysis/git-semantics#F4` |
| `wc_diff_hunks_load` | `repo, path, perspective` | `Vec<HunkRef>` | `Diff::foreach` + `Patch::from_diff` hunks | text file or binary-flagged | `FNGitDiffChunk` |
| `wc_image_data_load` | `repo, path, side{Old,New}` | `ImageData` | `Odb::read` at each side's blob | file has image extension | `GTWorkingCopyDiffImageViewController` |
| `wc_patch_save` | `repo, paths[], dest, omit_prefixes` | `()` | `Diff::print` to buffer, write file | at least one path | `GTGitSavePatchAction`, `GTUserDefaultsSavePatchOmitPrefixes` |
| `wc_patch_apply` | `repo, patch_bytes, location{Index,Workdir,Both}, three_way` | `StatusDelta` | `Diff::from_buffer` → `Repository::apply` | patch parses; on conflict, typed `ApplyFailed` with the rejected hunks | `GTGitApplyPatchOperation` |
| `wc_snapshot_create` | `repo, label, trigger{Manual,BeforeOp}` | `Snapshot` | `SnapshotStore` + tree write | repo clean state recorded even when dirty | `GTGitCreateWorkingTreeSnapshotOperation` |
| `wc_snapshot_restore` | `repo, snapshot, mode{Overwrite,Merge}` | `StatusDelta` | patch + `Repository::apply` | `Merge` mode requires user confirmation when divergence detected | `GTGitRestoreWorkingTreeSnapshotOperation` |
| `wc_snapshot_list` | `repo, limit?` | `Vec<Snapshot>` | SQLite | — | same |
| `wc_merge_status_load` | `repo, revision?` | `MergeStatus{state: Mergeable|Conflict|Merged, incoming?, base?}` | `merge_analysis` + trial merge into temp index | repo has workdir; no integration in progress | `GTGitLoadMergeStatusOperation` |
| `wc_conflicts_load` | `repo` | `Vec<ConflictEntry{path, kind, sides}>` | `Index::conflicts` + kind classification | integration in progress or unmerged entries exist | `GTGitMergeConflictNotAnUnmergedEntry` |
| `wc_conflict_resolve` | `repo, path, strategy{Ours,Theirs,Both,SaveVersions,MarkResolved}, save_to?` | `StatusDelta` | `Index::add_frombuffer` per strategy; `git2::merge::merge_file` where content merge is needed | entry is unmerged; strategy applies to its kind | 7 resolver classes, `EV:analysis/git-semantics#F3` |
| `wc_undo_list` | `repo` | `Vec<UndoEntry>` | `UndoRegistry` | — | `Undo*` actions |
| `wc_undo_apply` | `repo, undo_id` | `StatusDelta` | dispatch on `UndoOp` | entry still valid (HEAD unchanged since) | `GTGitUndo*Action` |

---

## 4. `commit`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `commit_create` | `repo, message, opts{sign_off, sign{Gpg\|Ssh\|None}, skip_hooks, template_used, committer?}` | `CommitInfo` | hooks → `Index::write_tree` + `Repository::commit` (or `commit_signed`) → update ref | index differs from HEAD parent tree; message not empty after strip-comments policy; no unmerged entries | `GTGitCommitAction`, `GTGitHookCache` |
| `commit_amend` | `repo, message?, opts{…}` | `CommitInfo` | new commit with HEAD's parents; `Reference::set_target` | HEAD exists; HEAD not pushed unless user confirmed rewrite (`ValidateUnpushedCommits`) | `GTGitAmendRevisionAction` |
| `commit_undo` | `repo` | `()` | `Repository::reset(HEAD~, Soft)` | HEAD is a Meridian-made commit or user confirms | `GTGitUndoCommitAction` |
| `commit_edit_message` | `repo, oid, message` | `CommitInfo` | recreate commit object, re-point each ref containing it | rewrite validation passed; refs computed first | `GTGitEditCommitMessageOperation` |
| `commit_hooks_probe` | `repo` | `HookState{pre_commit, commit_msg, prepare_msg, post_commit, sample?}` | fs stat of `.git/hooks` (cached, invalidated by `.git` watcher) | — | `GTGitHookCache`, "Skip hooks" |
| `commit_changeset_load` | `repo` | `Vec<ChangesetEntry>` (staged only) | `diff_tree_to_index(HEAD)` | HEAD may be unborn (then all staged files) | `GTCommitChangesetViewController` |
| `commit_templates_list` | `repo, scope{Repo,Global}` | `Vec<Template>` | file listing + parse | — | `CommitTemplatesLoader` |
| `commit_templates_save` | `repo, scope, name, subject, body, apply_by_default?` | `Template` | file write | name non-empty, unique | `CommitTemplateController` |
| `commit_templates_delete` | `repo, scope, name` | `()` | file delete | exists | `DeleteCommitTemplateAction` |
| `commit_templates_import` | `repo, source_path` | `ImportReport` | parse + copy | file readable, format valid (YAML front-matter + body) | `ImportCommitTemplatesOperation` |
| `commit_message_generate` | `repo, prompt_id, opts{include_unstaged, context_lines}, on_event` | `Channel text` | spawn configured agent CLI (`claude`/`codex`/`gemini`/`copilot`) with our prompt file; stream stdout | agent binary configured and executable; staged changes exist; agent support enabled | `AIAgentBinary*`, `GTUserDefaultsAIAgent*` (T2) |

**Hooks note.** libgit2 does not execute hooks, and this build does not run them either: no
`pre-commit` / `commit-msg` / `post-commit` runner is wired yet, so `commit_create` commits straight
from the index without the hook chain. `CommitOptions::skip_hooks` is accepted for the documented
argument shape but currently has no effect. The intended design (behind the observed "Skip hooks"
affordance and `GTGitHookCache`) is for `commit_create` to run the standard hook chain itself with a
git-compatible environment unless `skip_hooks`; hook semantics follow git's documented behaviour,
not reverse engineering.

---

## 5. `history`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `history_commits_load` | `repo, from?, to?, limit, sort{Date,Topo}, first_parent, path?` | `CommitPage{commits, truncated}` | `Revwalk` (`set_sorting`, `simplify_first_parent`) | `from` resolves (`UnknownRevisionError` otherwise) | `GTGitLoadCommitsOperation` |
| `history_graph_compute` | `repo, oids[]` | `GraphLayout{lanes, segments}` | lane assignment over the loaded page (our algorithm) | oids belong to one repo | `GTGitCommitGraphDrawer`, `GTGitCommitGraphLane` |
| `history_commit_load` | `repo, oid` | `CommitDetail` | `Repository::find_commit` + parents/refs pointing at it | oid exists | `GTGitLoadCommitOperation` |
| `history_changeset_load` | `repo, oid` | `Changeset{entries, stats}` | `diff_tree_to_tree(parent, commit)` (first parent) | oid exists; root commit handled | `GTGitLoadCommitChangesetOperation` |
| `history_revision_range_changeset_load` | `repo, from, to` | `Changeset` | `diff_tree_to_tree` | both resolve; `from` ancestor of `to` or user confirmed | `GTGitLoadRevisionRangeChangesetOperation` |
| `history_compare_revisions` | `repo, left, right, opts` | `Comparison{merge_base, behind, ahead, entries}` | `merge_base` + two diffs | both resolve | `compareRevisions:`, `GTGitComputeMergeBaseOperation` |
| `history_diff_document_load` | `repo, source{Commit(oid), Range(from,to)}, path, opts` | `DiffDocument` | diff path per §3 | — | `GTGitLoadCommitDiffsOperation` |
| `history_commit_hashes_load` | `repo, start, count` | `Vec<Oid>` | revwalk | — | `GTGitLoadCommitHashesOperation` |
| `history_count_load` | `repo, from?` | `u64` | revwalk count (cached per HEAD) | — | `GTGitLoadCommitCountOperation` |
| `history_fork_points_load` | `repo, tips[]` | `Vec<ForkPoint>` | `merge_base` per tip pair | ≥2 tips | `EV:strings/makeItemsFromCommits:behindCommitHashes:aheadCommitHashes:forkPointInfo:commitGraphInfos:` |
| `history_contributors_load` | `repo, range?` | `Vec<Contributor{name, email, commits}>` | revwalk + author aggregation | — | `GTGitContributorsUpdater` |
| `history_file_history_load` | `repo, path, limit, opts{follow_renames}` | `Vec<FileRevision>` | revwalk filtered by tree-diff touching `path`; rename detection when requested | path exists in at least one commit | `GTFileHistoryViewController` |
| `history_blame_load` | `repo, path, opts{range, ignore_whitespace, detect_moves}` `BlameOptions` | `BlameLines` | `Repository::blame_file` | path in HEAD | `GTGitLoadBlameOperation` |
| `history_reflog_load` | `repo, refname, limit, skip` | `Vec<ReflogEntry>` | `git2::Reflog` + `Reflog::iter` (entry point to be confirmed at implementation time: `Repository::reflog` if exposed, else the `Reflog` API over `logs/<ref>`) | refname valid | `GTGitLoadRefLogEntriesOperation` |
| `history_blob_load` | `repo, oid_or_rev:path` | `BlobView{kind: Text\|Binary\|Image\|LfsPointer, bytes\|rendered}` | `Odb::read` with size cap | path resolves | `GTBlobViewController`, `GTGitLoadBlobOperation` |
| `history_revision_archive` | `repo, oid, dest, format{Tar,Gz,Zip}` | `()` | ⟨child⟩ `git archive` | oid resolves; dest writable | `FNGitArchiveCommand` |
| `history_revision_patch_export` | `repo, oid, dest, count?` | `()` | ⟨child⟩ `git format-patch` | oid resolves | `FNGitFormatPatchCommand` |

---

## 6. `branches`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `branch_list` | `repo, filter{Local,Remote,All}, search?, sort` | `Vec<BranchInfo{name, oid, upstream, ahead, behind, pinned, archived, stale, fully_merged, parent?}>` | `Repository::branches` + `upstream` + ahead/behind via `graph_ahead_behind` | — | `GTGitLoadRefNamesOperation` |
| `branch_load` | `repo, name` | `BranchInfo` | as above, single | exists | same |
| `branch_create` | `repo, name, start: Revision, checkout, sender` | `BranchInfo` | `Repository::branch(name, commit, false)`; optional `set_head`+`checkout_tree` | name valid; not existing; start resolves | `GTGitCreateBranchAction`, `GTUserDefaultsDialogueOptionCheckoutCreatedBranch` |
| `branch_stacked_create` | `repo, name, parent, checkout` | `BranchInfo` | as `branch_create` + stack metadata write | parent exists; parent≠self; stack feature enabled | `GTGitCreateStackedBranchActionValidator` |
| `branch_checkout` | `repo, name, opts{autostash: Auto\|Always\|Never, force, detach?}` | `CheckoutResult{conflicts?}` | `set_head`/`set_head_detached` + `checkout_tree` (`CheckoutBuilder` safe) | no integration in progress; if dirty → policy ask (auto-stash); conflicts → `CheckoutConflicts` | `GTAutoStashDialogueViewController`, `GTGitCheckoutAction` |
| `branch_checkout_remote` | `repo, remote_branch, local_name?, track` | `BranchInfo` | create local branch at remote tip with upstream; checkout | remote branch exists; local name free | `GTGitCheckoutRemoteBranchAction` |
| `branch_checkout_revision` | `repo, revision, opts` | `CheckoutResult` | detached checkout | revision resolves | `GTCheckoutRevisionViewController` |
| `branch_rename` | `repo, old, new, opts{confirm_pr_branch}` | `BranchInfo` | `Branch::rename` | valid new name; if branch has an open PR (services) require `confirm_pr_branch` | `GTGitRenameBranchAction`; settings copy `EV:analysis/feature-scope#G5` |
| `branch_delete` | `repo, names[], opts{force, delete_remote_too, remote?}` | `DeleteReport{deleted, skipped[]}` | `Branch::delete`; remote part via push `:ref` | per branch: merged or upstream gone or `force`; unmerged → `GTGitDeleteUnmergedBranchError` unless forced | `GTGitDeleteBranchesAction`, `GTGitDeleteRemoteBranchAction` |
| `branch_fast_forward` | `repo, name, to?` | `BranchInfo` | `Reference::set_target` when ancestor | `to` is a descendant (else typed error) | `GTGitFastForwardBranchAction` |
| `branch_update` | `repo, name, mode{Merge,Rebase,Squash,MergeNoFF}, opts{autostash}` | `UpdateResult` | delegates to integrate module | branch checked out or worktree available; clean or autostash | `GTUserDefaultsUpdateBranchMergeMode*` |
| `branch_set_upstream` | `repo, name, upstream{local\|remote/ref}` | `BranchInfo` | `Branch::set_upstream` | upstream resolves; branch local | `GTGitSetUpstreamBranchAction` |
| `branch_unset_upstream` | `repo, name` | `BranchInfo` | `Branch::set_upstream(None)` | has upstream | `GTGitUnsetUpstreamBranchAction` |
| `branch_track_remote` | `repo, local, remote_branch, rename?` | `BranchInfo` | config `branch.<name>.remote/merge` writes | remote branch exists | `GTTrackRemoteBranchViewController` |
| `branch_untrack` | `repo, name` | `BranchInfo` | config removal | tracked | `GTGitRemoveBranchTrackingOperation` |
| `branch_pin_set` | `repo, name, pinned` | `BranchInfo` | SQLite | exists | `GTQuickActionName{Pin,Unpin}Branch` |
| `branch_review_load` | `repo, base, filter{All\|Contains\|Merged\|Unmerged}, sort{Attribute,Order}, search` | `ReviewModel{rows[], fully_merged[]}` | per-branch ahead/behind/merged relative to base (`CheckRevisionIsAncestorOfRevision`) | base resolves | `BranchesReviewListViewController`, `GTGitLoadFullyMergedBranchInfosOperation` |
| `branch_archivable_load` | `repo, opts{stale_months, require_merged, reviewed_only}` | `Vec<ArchiveCandidate{reason, last_commit_date}>` | SQLite + commit dates | — | `GTGitAutoArchiveBranchesOperation`, `GTUserDefaultsStaleBranchInterval` |
| `branch_archive_set` | `repo, names[], archived` | `()` | SQLite | exists | `GTQuickActionName{ArchiveBranch,UnarchiveBranch}` |
| `branch_auto_archive_flag_set` | `repo, name, flag{Include,Skip,Reviewed}` | `()` | SQLite | exists | `IncludeInAutoArchivingBranch` / `SkipAutoArchivingBranch` |
| `branch_auto_archive_run` | `repo, dry_run` | `ArchivePlan` | compute + (unless dry-run) apply flags | — | `runAutoArchiveBranches:` |
| `branch_stack_set_parent` | `repo, child, parent, force` | `StackState` | SQLite + validation | parent≠child; no cycle; `CanRestack` semantics | `SetGraphiteParentBranchViewController`, `GTGitCanRestackOperation` |
| `branch_stack_remove_parent` | `repo, child, force` | `StackState` | SQLite | has parent | `RemoveParentDialogueViewController` |
| `branch_stack_validate` | `repo` | `StackHealth{issues[]}` | SQLite + ancestry checks | — | `ValidateStackOperation` |
| `branch_stack_restack` | `repo, names[]` | `RestackReport` | rebase onto parent for each | stack consistent; no integration in progress | `GTGitRestackBranchOperation` |
| `branch_dependency_set` | `repo, parent, child` | `StackState` | alias of `branch_stack_set_parent` (Tower has both a "stack" and a "dependency" dialogue) | — | `SetBranchDependencyViewController` |

---

## 7. `remote_branches`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `remote_branch_list` | `repo, remote?, limit{recent_n\|all}` | `Vec<RemoteBranchInfo{ahead, behind, last_commit?}>` | `Repository::branches(Some(Remote))` + ahead/behind | — | `GTUserDefaultsNumberOfNewestRemoteBranches` |
| `remote_branch_delete` | `repo, remote, names[]` | `DeleteReport` | push refspec empty src (`:refs/heads/x`) via `Remote::push` | has push rights (else auth error) | `GTGitDeleteRemoteBranchAction` |
| `remote_branch_rename` | `repo, remote, old, new` | `()` | fetch new + push + delete old + reset upstream | not protected branch (config) | `GTGitRenameRemoteBranchAction` |
| `remote_branch_prune` | `repo, remote, dry_run` | `PruneReport{stale[]}` | `Remote::prune` with tip callbacks | fetch enabled | `GTPruneRemoteActionValidator` |
| `remote_branch_show_in_history` | `repo, refname` | `Oid` | `Reference::target` | resolves | `GTQuickActionNameShowCommitInHistory` |

---

## 8. `remotes`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `remote_list` | `repo` | `Vec<RemoteInfo{name, fetch_url, push_url, fetch_mode, refspecs}>` | `Repository::remotes` + `Remote::{url, pushurl, refspecs}` | — | `GTGitUpdateRemotesOperation` |
| `remote_add` | `repo, name, url` | `RemoteInfo` | `Repository::remote(name, url)` | name valid; not existing | `GTGitAddRemoteAction` |
| `remote_rename` | `repo, name, new` | `RemoteInfo` | `Remote::rename` | exists; new valid | `GTGitRenameRemoteAction` |
| `remote_set_url` | `repo, name, url, push: bool` | `RemoteInfo` | `Remote::set_url` / `set_pushurl` | exists; URL parse | `GTGitSetRemoteFetchURLAction` / `...PushURLAction` |
| `remote_delete` | `repo, name` | `()` | `Repository::remote_delete` | exists; branches tracking it get untracked with a report | `GTGitDeleteRemotesAction` |
| `remote_settings_load` | `repo, name` | `RemoteSettings{settings, refspecs, tracked_branches[], branch_filter?}` | read + `Remote::refspecs` | exists | `GTShowRemoteSettings` |
| `remote_refspec_add` | `repo, name, direction{Fetch,Push}, spec` | `Vec<Refspec>` | `Remote::add_fetch`/`add_push` | spec parses (`git2::Refspec::from_str`) | `GTGitAddFetchRefSpecAction` |
| `remote_refspec_update` | `repo, name, direction, index, spec` | `Vec<Refspec>` | config rewrite `remote.<n>.{fetch,push}` | parses | `GTGitUpdateFetchRefSpecAction` |
| `remote_refspec_delete` | `repo, name, direction, index` | `Vec<Refspec>` | config rewrite | index valid | `GTGitDeleteFetchRefSpecAction` |
| `remote_fetch` | `repo, remote?, opts{prune, tags: All\|Follow\|None, recurse_submodules, all_remotes}, on_event` | `FetchReport{updated_refs}` | `Remote::fetch` with refspecs, callbacks for progress + credentials; `Remote::update_tips` | remote exists; credentials resolvable (else prompt round-trip) | `GTGitFetchAction` |
| `remote_fetch_url` | `repo, url` | `Vec<RefInfo>` | `Remote::connect`/`ls` on ad-hoc URL | URL parse | `GTGitFetchURLOperation` |
| `remote_pull` | `repo, remote, branch?, opts{rebase, verify_signatures, autostash, ff_only}`, `on_event` | `PullResult{commits, merge_required}` | fetch → `merge_analysis` → fast-forward or merge/rebase delegate | clean or autostash; diverged → strategy required | `GTGitPullAction`, `GTUserDefaultsDialogueOptionPullUseRebase` |
| `remote_push` | `repo, remote, refspecs?, opts{tags, force_with_lease{oid?}, recurse_submodules, set_upstream}, on_event` | `PushReport{updated_refs, rejected[]}` | `Remote::push` with `PushOptions` + callbacks | remote exists; auth; rejection surfaces `PushRejected` with remote message | `GTGitPushAction` |
| `remote_force_push` | `repo, remote, branch, opts{lease: Oid\|Ref\|None, allow_unleased}` | `PushReport` | push refspec with `+`; default lease = current remote tip read just before | non-fast-forward without lease requires explicit `allow_unleased` confirmation | `GTForcePushDialogueViewController` |
| `remote_sync` | `repo, opts{fetch_all, update_current}`, `on_event` | `SyncResult` | fetch + fast-forward/rebase of current branch per settings | current branch has upstream | `GTGitSyncAction` |
| `remote_prune` | `repo, remote` | `PruneReport` | `Remote::prune` | — | `GTUserDefaultsAlwaysPruneObsoleteRemoteBranches` |
| `remote_validate_connection` | `repo, remote` | `DoctorReport{reachable, auth_method, ssh?, https?, suggestions[]}` | connect + ls with error classification (keychain item, private key, ssh agent, host key) | remote exists | `GTGitValidateRemoteConnectionOperation`, `GTGitRemoteConnectionDoctor` |
| `remote_set_fetch_mode` | `repo, remote, mode` | `RemoteSettings` | config `remote.<n>.fetch` policy flag | mode valid | `GTGitSetFetchModeOperation` |
| `remote_publish_branch` | `repo, branch, remote, opts{set_upstream}` | `PublishReport` | push + set upstream | remote exists; no existing remote branch of same name unless confirmed | `GTGitPublishBranchAction` |
| `remote_publish_tag` | `repo, tags[], remote` | `PublishReport` | push refspec `refs/tags/*` | tags exist | `GTGitPublishTagAction` |
| `remote_publish_repository` | `repo, account, spec{name, private, org?, access_type}` | `ServiceRepo` | services module create + remote add + push | account authenticated; name free | `PublishRepositoryAction` |
| `remote_activity_list` | `repo` | `Vec<Activity{kind, state, progress, started, stalled}>` | `ActivityRegistry` | — | `GTGitRemoteActivityManager` |
| `remote_activity_cancel` | `activity` | `()` | token cancel | active | `GTGitCancelRemoteActivitiesAction` |
| `remote_activity_cancel_stalled` | `repo` | `usize` | watchdog query + cancel | — | `GTGitCancelStalledRemoteActivitiesAction` |
| `remote_activity_clear` | `repo, scope{Finished,Expired,All}` | `()` | registry prune | — | `GTGitClearRemoteActivitiesAction` |
| `remote_activity_remove` | `activity` | `()` | registry remove | not running | `GTGitRemoveRemoteActivitiesAction` |
| `remote_activity_retry` | `activity` | `Activity` | re-run the stored command | original kind is retryable (clone, fetch) | "retry/restart clone" copy |

---

## 9. `tags`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `tag_list` | `repo, sort{Name,CreationDate,CommitDate,Author}` | `Vec<TagInfo{name, oid, annotated, message?, tagger?, verified?}>` | `tag_foreach` + `find_tag`/`find_reference` | — | `GTTagsSortSettingsMenuController` |
| `tag_create` | `repo, name, target: Revision, opts{annotated, message, sign, date_from_system}` | `TagInfo` | `Repository::tag` / `tag_lightweight` (signed: create tag object with `commit_signed`-style signature path via GPG) | name valid & free; target resolves | `GTGitCreateTagAction`, `GTUserDefaultsDialogueOptionSignTag` |
| `tag_delete` | `repo, names[]` | `DeleteReport` | `Repository::tag_delete` | exists | `GTGitDeleteTagsAction` |
| `tag_delete_remote` | `repo, remote, names[]` | `DeleteReport` | push `:refs/tags/<name>` | exists on remote | `GTGitDeleteRemoteTagAction` |
| `tag_publish` | `repo, names[], remote, opts{all}` | `PushReport` | push refspecs | names exist | "Push All Tags" |
| `tag_verify` | `repo, name` | `SignatureReport{trust, signer, key_id, timestamp}` | `Repository::extract_signature` + GPG verify (or ssh-key verify for ssh format) | annotated or signed commit exists | `GTGitVerifyTagOperation` |
| `tag_detail_load` | `repo, name` | `TagDetail` | tag object read | exists | `GTGitLoadTag*` (implied) |

---

## 10. `stashes`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `stash_list` | `repo` | `Vec<StashInfo{index, message, date, author, files?}>` | `Repository::stash_foreach` (or reflog read) | — | `GTGitUpdateStashesOperation` |
| `stash_save` | `repo, message, opts{include_untracked, keep_index, paths?: Partial, snapshot}` | `StashInfo` | `Repository::stash_save_ext` (`StashSaveOptions` for path filtering) / `stash_save` with `INCLUDE_UNTRACKED` | workdir or index dirty (else `SaveStashNoChangesError`) | `GTGitSaveStashOperation`, `GTGitSaveStashNoChangesError` |
| `stash_apply` | `repo, index, opts{delete_after, restore_index, reinstate_index}` | `ApplyResult{conflicts?}` | `Repository::stash_apply(index, StashApplyOptions)` | index valid; clean enough or policy ask | `GTGitApplyStashOperation`, `GTUserDefaultsApplyStashOption*` |
| `stash_drop` | `repo, indexes[]` | `()` | `Repository::stash_drop` per index (descending) | each index valid; bulk confirmation supplied by UI | `GTGitDeleteStashesOperation` |
| `stash_show` | `repo, index` | `StashDetail{entries, diff}` | stash commit tree vs parent | index valid | `GTQuickActionNameShowStash` |
| `stash_snapshot_quick` | `repo, label?` | `Snapshot` | snapshot + stash of dirty state | dirty | `GTQuickActionNameCreateStashSnapshot`, `GTGitUndoCreateStashSnapshotAction` |
| `stash_auto_policy_set` | `repo?, opts{offer, skip_all, reapply}` | `()` | settings | — | `GTUserDefaultsAlwaysSkipAutoStash`, `...AutoStashAlwaysReapplyChanges` |
| `stash_count_load` | `repo` | `u64` | reflog read of `refs/stash` | — | `GTUserDefaultsShowNumberOfStashesInSidebar` |

---

## 11. `submodules`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `submodule_list` | `repo` | `Vec<SubmoduleInfo{path, url, head, recorded, state: Uninitialized\|Current\|Ahead\|Behind\|Modified, ignore, strategy, recurse}>` | `Repository::submodules` + `Submodule::status`-equivalent via compare | — | `GTGitSubmoduleStateManager`, `GTGitSubmoduleStateComparator` |
| `submodule_add` | `repo, url, path, opts{force, branch}` | `SubmoduleInfo` | `Submodule::add_to_index` + `add_finalize` + clone into path | path free (or force); URL reachable | `GTGitAddSubmoduleOperation` |
| `submodule_init` | `repo, paths?` | `()[]` | `Submodule::init(false)` | module registered | `GTGitInitSubmodulesOperation` |
| `submodule_update` | `repo, paths?, opts{init, recursive, strategy{Checkout,Rebase,Merge,None}}` | `UpdateReport` | `Submodule::update(init, SubmoduleUpdateOptions)` | — | `GTUserDefaultsDialogueOptionUpdateSubmoduleRecursive` |
| `submodule_sync` | `repo, paths?` | `()` | `Submodule::sync` | — | `GTGitSynchronizeSubmodulesOperation` |
| `submodule_fetch` | `repo, paths?` | `FetchReport` | per-submodule `Remote::fetch` (recursive depth-first) | initialized | `GTFetchSubmoduleViewController` |
| `submodule_restore` | `repo, paths` | `StatusDelta` | checkout recorded commit into submodule | workdir clean or confirmed | `GTRestoreSubmoduleViewController` |
| `submodule_delete` | `repo, paths[], opts{keep_git_dir, keep_worktree}` | `()` | deinit + index remove + fs cleanup per options | paths are submodules; settings remembered per `GTUserDefaultsDeleteSubmoduleOption*` | same |
| `submodule_move` | `repo, path, new_path, opts{rename: bool}` | `SubmoduleInfo` | `.gitmodules` rewrite + `git mv` semantics (index + fs) | new path free | `GTGitMoveSubmoduleOperation` |
| `submodule_set_url` | `repo, path, url` | `SubmoduleInfo` | `Submodule::set_url` + `.gitmodules` write | path exists | `GTGitSetSubmoduleURLOperation` |
| `submodule_settings_save` | `repo, path, ignore{None,Untracked,Dirty,All}\|unset, strategy\|unset, recurse\|unset` | `SubmoduleInfo` | `Submodule::set_ignore`, `set_update`, config writes | values valid | `GTGitSetSubmoduleIgnoreModeOperation` + siblings |
| `submodule_verify_before_push` | `repo` | `VerifyReport{unpushed_commits[]}` | compare recorded vs remote-tracking per submodule | — | "Verify Submodules before Push" |

---

## 12. `worktrees`

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `worktree_list` | `repo` | `Vec<WorktreeInfo{name, path, head, locked, valid, is_main}>` | `Repository::worktrees()` + `Repository::find_worktree` + `Worktree::{path, is_locked, validate, is_prunable}` | — | `GTWorktreesUpdater`, `WorktreeSidebarCellView` |
| `worktree_add` | `repo, name, path, start: Revision, opts{detached, force, checkout}` | `WorktreeInfo` | `Repository::worktree(name, path)` + `Worktree::add`-equivalent (create from ref; detached by writing HEAD) | path free; name valid; start resolves; already-checked-out start requires `force` | `GTAddWorktreeDialogueViewController`, `GTGitAddWorktreeAction` |
| `worktree_move` | `repo, name, new_path` | `WorktreeInfo` | mv + gitdir repair | target writable | `GTGitMoveWorktreeAction` |
| `worktree_relocate` | `repo, name, new_path` | `WorktreeInfo` | same as move with repair of the `gitdir` pointer | same | `GTRelocateWorktreeDialogueViewController` |
| `worktree_remove` | `repo, name, opts{force}` | `()` | `Worktree::prune(WorktreePruneOptions{valid(true), working_tree(true), locked(force)})` | not locked unless force (`DeleteWorktreeLockedError`); no uncommitted changes unless force (`DeleteWorktreeHasChangesError`) | both error types observed |
| `worktree_repair` | `repo, names[]` | `RepairReport` | re-run `Worktree::validate`; repair `gitdir`/`commondir` files | name registered | `GTGitRepairWorktreeAction` |
| `worktree_lock` | `repo, name, lock` | `WorktreeInfo` | `Worktree::lock`/`unlock` | valid | `Worktree::is_locked` counterpart |
| `worktree_detail_load` | `repo, name` | `WorktreeDetail{status, ahead, behind}` | open the worktree repo → status load | valid | `GTWorktreeDetailViewController` |
| `worktree_open` | `repo, name, target{Tab,Window,Terminal,Finder}` | `()` | app shell / platform spawn | valid; opened only if the target path is on the same volume | `GTQuickActionNameOpenWorktreeIn*` |
| `worktree_settings_load` | `repo` | `WorktreeSettings` | SQLite + config | — | `GTWorktreeGeneralSettingsViewController` |
| `worktree_settings_save` | `repo, settings` | `()` | SQLite | keys valid | same |

---

## 13. `integrate` — merge, rebase, cherry-pick, revert, reset, conflicts

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `merge_start` | `repo, revision, opts{always_merge_commit, auto_commit, verify_signatures, autostash}` | `IntegrationOutcome{Clean\|Conflicted}` | `merge_analysis` → `Repository::merge` with `MergeOptions`; commit when auto_commit | clean or autostash; no integration in progress (`IntegrationInProgress` otherwise); signature gate when `verify_signatures` | `GTGitMergeAction`, `GTUserDefaultsDialogueOptionMerge*` |
| `merge_status_load` | *see `wc_merge_status_load`* | — | — | — | `GTGitLoadMergeStatusOperation` |
| `merge_abort` | `repo` | `StatusDelta` | `Repository::cleanup_state` (mode-aware) | an integration is in progress | `GTGitAbortMergeAction` |
| `rebase_start` | `repo, upstream, onto?, opts{autostash, verify_signatures}` | `IntegrationOutcome` | `Repository::rebase(branch, upstream, onto, RebaseOptions)`; drive `Rebase::next`/`commit` | clean or autostash; branch checked out; not published history unless confirmed | `GTGitRebaseAction` |
| `rebase_interactive_plan` | `repo, upstream, onto?` | `SequencePlan{steps[{action: Pick\|Reword\|Edit\|Squash\|Fixup\|Drop, oid, message}], commits}` | revwalk computation only — no state change | commits resolve; no integration in progress | `GTGitInteractiveRebaseMergeProcessInfo` |
| `rebase_interactive_start` | `repo, plan, on_event` | `IntegrationOutcome` | sequential cherry-pick engine honoring the plan (libgit2 rebase with custom operations via `Rebase::next` + `commit`) | plan steps valid; conflicts pause the sequence | same |
| `cherry_pick` | `repo, revisions[], opts{ff, always_merge_commit, no_commit}` | `IntegrationOutcome` | `Repository::cherrypick_commit` per revision + commit | revisions resolvable, in one repo; not already applied unless confirmed | `GTGitCherryPickRevisionsAction`, `GTUserDefaultsDialogueOptionCherryPick*` |
| `revert` | `repo, revisions[], opts{commit_after, mainline?}` | `IntegrationOutcome` | `Repository::revert` per revision | merge commits need `mainline` | `GTRevertRevisionsDialogueViewController` |
| `reset_to_revision` | `repo, revision, mode{Soft,Mixed,Hard}, opts{confirm}` | `StatusDelta` + `Snapshot` | snapshot → `Repository::reset(object, kind, None)` | Hard requires confirm; snapshot always taken first | `GTGitResetAction`, `performResetActionWithRevision:headRevision:mode:snapshot:` |
| `reset_entries_to_revision` | `repo, paths[], revision` | `StatusDelta` | `Repository::reset_default(rev, paths)` | paths tracked at revision | `GTResetEntriesToRevisionDialogueViewController` |
| `rewrite_impact_load` | `repo, plan{branch, revisions}` | `RewriteImpact{unpushed[], mainline[], merge_commits[]}` | ancestry checks per revision (`ancestor_of`, `remote tip contains`) | branch local | three observed validation errors |
| `squash_revisions` | `repo, base, tips[]` | `IntegrationOutcome` | sequence engine: reset --soft base, commit once | all tips descendants of base | `GTGitSquashRevisionsAction` |
| `fixup_revisions` | `repo, target, fixups[]` | `IntegrationOutcome` | sequence engine with `fixup!`/`squash!` semantics | target ancestor of each fixup | `GTGitFixupRevisionsAction` |
| `move_revisions` | `repo, revisions[], meta{onto, direction}` | `IntegrationOutcome` | sequence + cherry-pick | impact check passed | `GTGitMoveRevisionsAction` |
| `delete_revisions` | `repo, revisions[]` | `IntegrationOutcome` | sequence engine | impact check passed | `GTGitDeleteRevisionsAction` |
| `integration_continue` | `repo` | `IntegrationOutcome` | mode from `Repository::state()` → continue path (merge commit / rebase next / cherry-pick sequence) | integration in progress; no unmerged entries | `GTGitContinueMergeAction` |
| `integration_skip` | `repo` | `IntegrationOutcome` | sequence skip | rebase/cherry-pick in progress | `GTGitSkipMergeAction` |
| `merge_tool_invoke` | `repo, paths[]` | `()` (exit report) | spawn configured merge tool with `$BASE/$LOCAL/$REMOTE/$MERGED` | tool configured and on disk; unmerged entries | `invokeDiffTool:` |
| `diff_tool_invoke` | `repo, left, right?, paths[], opts{directory_mode}` | `()` | spawn configured diff tool (`/usr/bin/opendiff` style args) | tool configured | `GTSelectDiffToolDialogueViewController` |

**Conflict flow.** `wc_conflicts_load` → `wc_conflict_resolve` (per kind) → `integration_continue`.
This mirrors the observed `MergeConflictsViewController` + `ContinueMerge` / `SkipMerge` / `AbortMerge`
surface (`EV:strings/"Continue the merge sequence"`).

---

## 14. `security` — SSH, GPG, signing, credentials

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `ssh_key_list` | `()` | `Vec<SshKeyInfo{path, type, fingerprint, comment, encrypted, has_public}>` | scan `~/.ssh` + store; parse with `ssh-key` crate | — | `GTSSHKeyImporter`, `GTSSHKeySerialization`, `GTSSHKeyTypeConverter` |
| `ssh_key_import` | `path, name?` | `SshKeyInfo` | read + parse + store metadata | file readable; parses as OpenSSH/PEM | same |
| `ssh_key_delete` | `id` | `()` | store delete (optionally fs) | exists; not the agent-loaded active key unless confirmed | `GTSSHKeysUpdater` |
| `ssh_agent_status` | `()` | `AgentStatus{sock, keys[]}` | connect to `SSH_AUTH_SOCK`; list keys | agent reachable (else `SSHAgentNotRunningError`) | `SSHSockProvider`, `EV:strings/SSH_AUTH_SOCK` |
| `ssh_program_get` | — | `{core_ssh_command, signing_program, format}` | config read (`core.sshCommand`, `gpg.format`, `gpg.ssh.program`) | — | `EV:strings/SSH signing program:` |
| `ssh_program_set` | `{ssh_command?, signing_program?}` | `()` | config write | program exists and is executable (`SSHProgramValidator`) | `SSHProgramValidator` |
| `ssh_program_validate` | `path` | `ValidationReport` | spawn `ssh -V` / `ssh-keygen -Y` | executable | same |
| `gpg_key_list` | `()` | `Vec<GpgKey{key_id, uids, emails, has_secret}>` | parse `gpg --list-secret-keys --with-colons` | gpg binary found (else `GTGitGPGBinaryNotFoundError`) | `GTGitGPGConfig`, `GTGitGPGConfigInfo` |
| `gpg_config_load` | `repo?` | `GpgConfig{signing_key, signing_enabled_repo, signing_enabled_global, key_matches_committer}` | config read | — | `GTGitUpdateLocalGPGConfigOperation` |
| `gpg_config_save` | `repo?, key_id?, email_must_match` | `GpgConfig` | config write | key exists; when `email_must_match` on, committer email ∈ key emails | `GTUserDefaultsEnsureGPGKeyEmailMatchesCommitterEmail` |
| `signing_enabled_set` | `repo, enabled` | `()` | `commit.gpgsign` local config | key configured | `GTGitSetRepositoryGPGSigningEnabledOperation` |
| `allowed_signers_select` | `repo, path?` | `()` | git config `gpg.ssh.allowedSignersFile` | file parses as ssh allowed-signers format | `GTGitSetRepositoryAllowedSignersFileOperation`, `AllowedSignersFilesFinder` |
| `signature_verify` | `repo, revision` | `SignatureReport{trust, signer, key, format, error?}` | `Repository::extract_signature` + verify (gpg or `ssh-key` for ssh format) | commit/tag exists and has a signature | `GTGitGPGSignature{Missing,Untrusted}Error` |
| `credential_forget` | `repo, remote` | `()` | keychain delete + credential-helper delete | exists | `GTGitRemoteConnectionKeychainItemNotFoundError` |
| `credential_store_set` | `repo, remote, method{Keychain,CredentialHelper,None}` | `()` | config + keychain | method valid | `GTUserDefaultsSSHSock` counterpart |

---

## 15. `services` — accounts, repositories, keys, pull requests, issues

All commands here are HTTP through the owning `ServiceAdapter` (docs/02 §10); none touch git2, and
all return the `ErrorCode::Service*` classes. `account` is `AccountId` (a local record), `host`
identifies the flavor.

| Command | Args | Returns | Upper mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `account_list` | — | `Vec<Account{id, host, username, avatar?, enterprise, methods[]}>` | account table | — | `GTServiceUpdate*AccountOperation` |
| `account_add` | `host, base_url?, method, on_event` | `Account` | adapter `probe_host` + `validate_credentials` | host reachable; URL shape valid (`GTServiceHostURLFormatValidator`); while probing, the OTP/2FA prompt path is available | `GTServiceProbeServerAPIVersionOperation`, `GTServiceHostValidationError` |
| `account_update` | `id, patch` | `Account` | adapter re-validate | exists | `GTServiceUpdateAccountOperation` |
| `account_delete` | `id, forget_credentials` | `()` | table + keychain | exists | `GTServiceDeleteAccountsOperation` |
| `account_authenticate` | `id, method, secret?, on_event` | `AuthResult` | OAuth device-flow / PAT validation | 2FA → `TwoFactorRequired` with prompt; rate limit → back-off | `GTServiceTwoFactorAuthenticationError`, `GTServiceRateLimitExceededError` |
| `account_probe_host` | `base_url` | `HostInfo{flavor, api_version, enterprise}` | adapter `probe_host` | URL parse; TLS trust errors surfaced as `ServerTrust` | `GTServiceServerTrustError` |
| `service_repositories_list` | `account, query{search, org?, page}` | `Vec<ServiceRepo>` | adapter `repositories` | authenticated | `GTServiceUpdate*RepositoriesOperation` |
| `service_repository_create` | `account, spec{name, private, org?, description?}` | `ServiceRepo` | adapter create | name valid for host | `GTServiceCreate*RepositoryOperation` |
| `service_org_units_list` | `account, kind(provider-noun), search?` | `Vec<OrgUnit>` | adapter `org_units` | — | orgs/groups/teams/projects ops |
| `service_public_keys_list` | `account` | `Vec<RemotePublicKey>` | adapter | capability `supports_public_keys` else `Unsupported` | `GTServicePublicKeyManager` |
| `service_public_key_create` | `account, key{title, key_material}, link_local_key?` | `RemotePublicKey` | adapter + local key link | key material parses; not duplicated | `GTServiceCreate*PublicKeyOperation` |
| `service_public_key_delete` | `account, key_id` | `()` | adapter | exists | `GTServiceDelete*PublicKeyOperation` |
| `pr_list` | `account, repo, filter{state, author?, search?}` | `Vec<PullRequest{id, number, title, state, draft, author, base, head, updated_at, branch_local?}>` | adapter | account linked to repo's remote | `GTServiceUpdate*OpenPullRequestsOperation` |
| `pr_load` | `account, repo, id` | `PullRequestDetail{body, commits[], comments[], changeset, checks?}` | adapter | id exists | `GTPullRequestsDetailViewController` |
| `pr_create` | `account, repo, draft{title, body, base, head, draft?}` | `PullRequest` | adapter | base≠head; head pushed | `GTServiceCreate*PullRequestOperation` |
| `pr_edit` | `account, repo, id, patch{title?, body?, state?}` | `PullRequest` | adapter | exists; not merged | `GTServiceEdit*PullRequestOperation` |
| `pr_checkout` | `account, repo, id, opts{local_name?, use_fork_remote}` | `BranchInfo` | adapter refspecs → engine fetch into local ref, fork remote ensure, checkout | PR head resolvable; local name free (or tracking exists) | `GTGitCheckoutPullRequestAction`, `GTGitCleanUpPullRequestForkRefsAction` |
| `pr_merge` | `account, repo, id, option{merge, squash, rebase, fast_forward_only?}, delete_branch_after?` | `PullRequest` | adapter merge with provider-shaped options | mergeable per adapter; option ∈ `capabilities.merge_options` | `GTServiceMerge*PullRequestsOperation` |
| `pr_close` | `account, repo, id` | `PullRequest` | adapter | not merged | `GTServiceClose*PullRequestsOperation` |
| `pr_comment_create` | `account, repo, id, body, thread?` | `Comment` | adapter | body non-empty | `GTServiceCreate*PullRequestCommentOperation` |
| `pr_comment_edit` | `account, repo, comment_id, body` | `Comment` | adapter | own comment | `GTServiceEdit*PullRequestCommentOperation` |
| `pr_comment_delete` | `account, repo, comment_id` | `()` | adapter | own comment | `GTServiceDelete*PullRequestCommentOperation` |
| `pr_comment_reply` | `account, repo, thread_id, body` | `Comment` | adapter | thread open | `GTServiceReplyTo*PullRequestCommentOperation` |
| `issue_search` | `account, repo, query` | `Vec<Issue>` | adapter | `supports_issue_search` else `Unsupported` | `GTServiceSearch*IssuesOperation` |
| `service_activity_list` | `account?, repo?` | `Vec<ServiceActivity>` | local log of service operations | — | `GTServiceActivityListViewController` |
| `service_sync_settings_set` | `account, opts{pr_interval, enabled}` | `()` | settings | interval ≥ 60s | `GTUserDefaultsAutomaticPullRequestsUpdateInterval` |

**PR refresh** is not a command: the poller (docs/02 §10) emits `AppEvent::PullRequestsChanged`.

---

## 16. `workflows` — Git Flow, GitFlow Next, custom, Graphite, Gerrit, SVN

| Command | Args | Returns | Engine mapping | Validator / preconditions | Evidence |
|---|---|---|---|---|---|
| `gitflow_config_load` | `repo` | `GitFlowConfig{branches{master, develop}, prefixes{feature,release,hotfix,support}, tag_prefix}` | config read (`gitflow.*`) | — | `GTGitFlowConfigManager` |
| `gitflow_config_save` | `repo, config` | `()` | config write + validate branches exist | branches exist; prefixes valid | `GTGitFlowSetConfigurationOperation` |
| `gitflow_config_unset` | `repo` | `()` | config removal | configured | `GTGitFlowUnsetConfigurationOperation` |
| `gitflow_start` | `repo, kind{Feature,Release,Hotfix}, name, base?` | `GitFlowState` | create branch per prefix + checkout | gitflow configured; free name | `GTGitFlowCreateFeatureBranchOperation` + siblings |
| `gitflow_finish` | `repo, kind, name, opts{merge_commit, preserve_merges, rebase, skip_tag, tag_message?}` | `GitFlowState` | merge/rebase into target + delete + optional tag | branch exists; clean | `GTGitFlowFinish*BranchOperation`, `GTUserDefaultsGitFlowFinish*` |
| `gitflow_next_config_load` | `repo` | `GitFlowNextConfig` | parse the workflow's config file (format per upstream project, **not from evidence**) | file exists | `GitFlowNextConfigParser` |
| `gitflow_next_config_save` | `repo, config` | `()` | write via upstream tool semantics | parses | `UpdateGitFlowNextWorkflowOperation` |
| `workflow_custom_list` | `repo` | `Vec<CustomWorkflow>` | SQLite | — | `WorkflowPickerDialogueViewController` |
| `workflow_custom_save` | `repo, workflow{name, base, prefixes, start_actions, finish_actions}` | `CustomWorkflow` | SQLite | name unique; prefix templates valid | `SaveCustomWorkflowAction` |
| `workflow_custom_delete` | `repo, id` | `()` | SQLite | exists | `DisableCustomWorkflowAction` |
| `topic_branch_start` | `repo, workflow_id, name, base?` | `TopicBranchState` | create + record | workflow exists; name free | `StartCustomTopicBranch` |
| `topic_branch_finish` | `repo, name, opts` | `TopicBranchState` | per-workflow finish | started; mergeability computed | `FinishTopicBranchAction`, `CheckMergeabilityOperation` |
| `graphite_enable` | `repo, enable, binary_path?, token?` | `GraphiteState` | shell-out `gt --version`; store config | binary exists and ≥ minimum version | `EnableGraphiteAction`, "An incompatible Graphite version has been found…" |
| `graphite_config_load` | `repo` | `GraphiteConfig` | read `.git/.graphite_repo_config` (plus local mirror) | initialized | `GetGraphiteConfigurationOperation` |
| `graphite_state_refresh` | `repo` | `GraphiteState{branches[{name, parent, pr?, sync_state}]}` | shell-out `gt` state queries; mirror locally | enabled | `UpdateGraphiteStateOperation` |
| `graphite_branch_op` | `repo, op{Create,Delete,Rename,Merge,Restack,Squash,Sync,Submit}, args` | `GraphiteState` | shell-out `gt <op>` with progress parsing | enabled; op-specific args | `GTQuickActionNameGraphite*` |
| `graphite_set_parent` | `repo, branch, parent, force` | `GraphiteState` | shell-out `gt` or local config write | cycle check | `SetGraphiteParentBranchViewController` |
| `graphite_remove_parent` | `repo, branch, force` | `GraphiteState` | same | has parent | `RemoveParentDialogueViewController` |
| `graphite_fetch_stack` | `repo, remote?` | `GraphiteState` | shell-out | enabled | `GraphiteFetchStackDialogueViewController` |
| `graphite_open_dashboard` | `repo` | `()` | open URL | enabled | `OpenGraphiteDashboardAction` |
| `gerrit_push` | `repo, remote, branch, opts{draft, topic?}` | `PushReport` | push with refspec `refs/for/<branch>` | gerrit support enabled; branch tracked | `GTGitPushToGerritRemoteActionValidator`, `refs/for/%@` |
| `svn_clone` | `url, dest, opts{trunk, branches, tags}, on_event` | `RepoInfo` | ⟨child⟩ `git svn clone` with layout options | URL reachable | `GTGitCloneSVNRepositoryOperation` |
| `svn_remote_add` | `repo, url, opts{trunk, branches, tags}` | `RemoteInfo` | config: svn-remote section + refspecs | repo is svn bridge or empty | `GTGitSVNAddRemoteOperation` |

---

## 17. `settings` — the typed settings surface

Settings are a *schema*, not free-form strings: each key from the evidence that we adopt gets a
typed field and a one-line doc comment. `settings_get_all` returns the whole schema (used to build
the preferences window generically, mirroring the observed `settingsItemsController` outlet).

| Command | Args | Returns | Storage | Validator | Evidence |
|---|---|---|---|---|---|
| `settings_get_all` | — | `SettingsDocument` | SQLite | — | `settingsItemsController` outlet, ×10 |
| `settings_get` | `key` | `serde_json::Value` | SQLite | key in schema | 190 observed keys |
| `settings_set` | `key, value` | `()` | SQLite + `AppEvent::SettingsChanged` | schema type; enum membership where observed | same |
| `settings_reset` | `key?` | `()` | SQLite | key in schema | `resetUserDefaults:` |
| `settings_schema_describe` | — | `SchemaDoc{key, type, default, group, doc}` | derived | — | same |
| `profile_list` | — | `Vec<UserProfile{id, name, user_name, user_email, signing_key?, repo_count}>` | SQLite | — | `GTUserPreferencesProfilesViewController` |
| `profile_create` | `name, user_name, user_email, signing_key?` | `UserProfile` | SQLite | email shape; name unique | `GTAddGitUserProfileDialogueViewController` |
| `profile_update` | `id, patch` | `UserProfile` | SQLite | exists | same |
| `profile_delete` | `id` | `()` | SQLite | not assigned to a repo unless confirmed | `removeUserProfile:` |
| `profile_set_repository` | `repo, profile_id?` | `()` | repo config (`user.name/email/signingkey`) | repo + profile exist | `GTGitSetRepositoryUserOperation` |
| `profile_set_global` | `profile_id` | `()` | global config | exists | `GTGitUpdateGlobalUserOperation` |
| `theme_list` | — | `Vec<ThemeInfo{id, name, author?, version, builtin, light_dark: Both\|Single}>` | fs + SQLite | — | `UserPreferencesThemes*` |
| `theme_import` | `path` | `ImportReport` | parse + validate + copy | format valid (`ThemeValidator`), else `InvalidThemes` report | `ImportThemeAction`, `InvalidThemesInfoView` |
| `theme_export` | `id, dest` | `()` | serialize | exists | `ExportThemeAction` |
| `theme_activate` | `id, appearance{Light,Dark,Both}` | `()` | settings + `SettingsChanged` | exists | `GTUserDefaults{Light,Dark}Theme` |
| `theme_delete` | `id` | `()` | fs + SQLite | not builtin | "This will delete the selected theme." |
| `appearance_set` | `mode{Auto,Light,Dark}` | `()` | settings | — | `AppAppearanceController` |
| `app_icon_set` | `mode{Auto,Light,Dark}` | `()` | settings + assets | — | `GTUserDefaultsAppIconUserMode` |
| `environment_load` | — | `EnvironmentModel{user_vars[], application_vars[], from_login_shell, effective[]}` | settings + live shell query | — | `ApplicationEnvironmentController` |
| `environment_set` | `scope{User,Application}, name, value?` | `EnvironmentModel` | settings | name non-empty, no NUL | `addEnvironmentVariablePressed:` |
| `environment_reload_login_shell` | `on_event` | `EnvironmentModel` | spawn login shell, capture env | shell runs (timeout 5s) | `ShellEnvironmentController`, "Automatically request environment variables from login shell" |
| `terminal_app_set` | `app_path` | `()` | settings | app exists | `GTUserDefaultsDefaultTerminalApplication` |
| `tool_diff_select` | `identifier, compare_directories` | `()` | settings | tool exists | `GTSelectDiffToolDialogueViewController` |
| `tool_merge_select` | `identifier` | `()` | settings | tool exists | `GTSelectMergeToolDialogueViewController` |

---

## 18. `system` — shell, updates, CLI, deep links

| Command | Args | Returns | Mapping | Validator | Evidence |
|---|---|---|---|---|---|
| `system_app_info` | — | `{version, build, os, git_binary, libgit2_version}` | compile-time + runtime probes | — | `GTGitBinaryReportCreator` |
| `system_logging_set` | `enabled, level?` | `()` | tracing filter | — | `GTUserDefaultsDetailedLoggingEnabled` |
| `system_log_export` | `dest, since?` | `()` | zip the rolling log | writable | `createReport:` |
| `system_diagnostic_report` | — | `Report` | environment + git binary + config summary | — | `createReport:` |
| `system_update_check` | `on_event` | `UpdateInfo?` | **our own** update source (not Sparkle, not Tower's channels) | — | `GTUserDefaultsUpdatesReleaseChannel` (mechanism only) |
| `system_update_settings_set` | `{check_automatically, channel}` | `()` | settings | — | same |
| `cli_tool_install` | `dest?` | `CliStatus` | write launcher script/binary + PATH hint | dest writable | "The command line tool lets you open existing Git repositories from the command line." |
| `cli_tool_uninstall` | — | `CliStatus` | remove | installed | "This will overwrite the existing meridian command." |
| `cli_tool_status` | — | `CliStatus{installed, path, version, in_path}` | fs probe | — | same |
| `system_deeplink_handle` | `url` | `DeepLinkOutcome` | closed verb table: `open`, `clone`, `checkout`, `pullrequest` | verb known; repo target resolvable; queue during startup | "request ignored: unimplemented verb" |
| `system_autostart_set` | `enabled` | `()` | launch agent | — | (our own; no evidence) |
| `system_notification_settings_set` | `{level, sound?}` | `()` | settings | — | notification managers observed |
| `system_telemetry_settings_set` | `{enabled}` | `()` | settings; **default off** | — | `GTUserDefaultsAnalyticsTrackingEnabled` (as design input only) |

---

## 19. Coverage and counts

Counts in §1 were generated with:

```bash
python3 -c "
import re
t=open('docs/03-command-surface.md',encoding='utf-8').read()
cur=None; n={}
for line in t.splitlines():
    m=re.match(r'## \d+\. \`(\w+)\`', line)
    if m: cur=m.group(1)
    if cur and re.match(r'\| \`[a-z_]+', line) and '— *alias row' not in line:
        n[cur]=n.get(cur,0)+1
import json; print(json.dumps(n,indent=1)); print('TOTAL', sum(n.values()))
"
```

The **TOTAL** figure in §1's table is the sum of the per-module command rows in this document
(alias rows excluded). The coverage rule: every T0 capability in docs/01 has ≥1 command here; T1/T2
capabilities are covered by their module's commands. Two capabilities have **no git2 mapping and no
command** by design: CAP-G18-05 ("What's New") and CAP-G18-07 (telemetry) are UI/settings-only, and
CAP-G18-06 updates are served by `system_update_check` with our own mechanism.

**Commands deliberately absent** (observed in Tower, not reproduced):
`emulateActivateLicenseEvent:`, the trial/upgrade screens and actions, `demoApplicationCrash:`,
`logNotificationCenterObservers:`, `resetUserDefaults:` (we keep per-key reset only),
`toggleProTrialBadge:`. See docs/00 §7.

## 20. Events and channels (the non-command surface)

| Surface | Payload | Used by |
|---|---|---|
| `Channel<ProgressEvent>` arg on long commands | `Started/Phase/Tick/RequiresInput/Finished/Failed` | clone, fetch, pull, push, sync, rebase-interactive, submodule fetch, Graphite ops, environment reload, update check, AI generation |
| `AppEvent::ReposChanged` | — | sidebar, windows |
| `AppEvent::RefsChanged{repo}` | — | sidebar, history, banners |
| `AppEvent::WorkingCopyChanged{repo}` | — | working copy pane |
| `AppEvent::QueueDepthChanged{repo, depth}` | — | toolbar/status area |
| `AppEvent::Activity{…}` | activity deltas | activity window, sidebar footer |
| `AppEvent::AccountsChanged` / `PullRequestsChanged{account}` | — | services pane |
| `AppEvent::SettingsChanged{key}` | — | preferences + consumers |
| `AppEvent::IndexerPhase{phase}` / `IndexerResults{batch}` | — | quick open |
| `AppEvent::PromptRequest{activity, prompt}` | credential / passphrase / 2FA asks | the prompt host; answers return as a command (`prompt_answer`) over the same channel contract |

`PromptRequest` is the only interactive round-trip in the surface: the engine pauses on a oneshot
channel while the UI shows the askpass-style dialogue (our counterpart of the observed
`TowerAskpass` helper). Prompts carry a `duration_ms` hint and expire to a cancellation.
