# 01 — Product Specification

Meridian is a Git client for macOS: a Tauri (Rust + web UI) application built on a `git2`/libgit2
engine, specified from the reverse-engineering evidence summarised in `docs/00-evidence-summary.md`.
This document defines the product: who it is for, what ships, in what order, and the evidence behind
every capability.

**Tiering note (read first).** The `feature-scope` domain report catalogues **18 feature groups and
~95 capabilities** but does **not** define T0/T1/T2. The tiering below is Meridian's own engineering
triage of that evidence-derived catalogue: T0 = the daily-driver core without which a Git client is
not usable; T1 = professional depth that distinguishes a serious client; T2 = ecosystem surfaces
that depend on the core and on external hosts/workflows. Every capability row still carries its
evidence reference, so the tiering can be re-litigated without re-doing the reverse engineering.

---

## 1. Product thesis

Tower's product shape — established at name level in the evidence — is:

1. **A repository-centric desktop app**, not a commit-graph toy: bookmarks, multiple windows, a
   sidebar that is the navigation hub (`EV:analysis/ui-structure#F4`), master-detail panes
   everywhere (`EV:analysis/ui-structure#F18`).
2. **A working-copy tool first**: chunk-level staging (`EV:sel/stageChunk`), destructive-action
   confirmations, an editor-grade commit composer (`EV:analysis/ui-structure#F14`), and a
   default-action-plus-options interaction pattern (`quickPull:` / `quickPush:` / `quickSync:`,
   `EV:analysis/ui-structure#F17`).
3. **Safe by construction**: typed errors (`EV:analysis/git-semantics#F12`), validators that compute
   what would be lost before rewriting history (`EV:analysis/git-operations#F4`), working-tree
   snapshots plus a first-class undo family (`EV:analysis/git-operations#F3`), and explicit
   auto-behaviours (auto-fetch / auto-stash / auto-submodule-update) as *policies* with their own
   confirmations (`EV:analysis/git-operations#F6`).
4. **Never blocking on the network**: long work becomes observable Activities with progress,
   cancellation and a stall watchdog (`EV:analysis/git-operations#F5`).
5. **Tooling-integrated**: external diff/merge tools, terminal, editor, Finder, SSH/GPG programs,
   environment variables, a command-line tool, and deep links from other apps
   (`EV:analysis/feature-scope#G16`, `#G17`, `EV:analysis/services#F12`).

Meridian keeps that shape and re-implements it on a modern stack. It does **not** copy Tower's code,
icons, nibs, copy text or theme files (docs/00 §7).

## 2. Personas

**P1 — The daily committer.** Works in 2–10 repositories, stages hunks, writes commits, pulls and
pushes all day. Needs: fast open, sane defaults, undo. Lives in T0.

**P2 — The release/integration engineer.** Merges, rebases, resolves conflicts, cherry-picks onto
release branches, tags, and occasionally rewrites history. Needs: conflict UI, validators that warn
before destructive actions, snapshots, tags, submodules/worktrees. Lives in T0/T1.

**P3 — The platform/DevOps engineer.** Manages many repos across hosts, uses LFS, worktrees for
parallel branches, GPG/SSH signing, profiles for different identities, and expects the client to
integrate with the terminal and CI-adjacent tools. Lives in T1, reaches into T2.

**P4 — The occasional contributor.** Clones, branches, commits, opens a pull request. Needs the
clone path, the commit path, and the PR path without learning git internals. Lives in T0/T1 and is
the persona most served by T2's services layer.

Non-personas: git power users who want a TUI (out of scope — the product is a GUI), and users needing
Windows/Linux (macOS-only for v1; the architecture does not preclude ports, docs/02 §12).

## 3. Capability catalogue

Format: **ID · tier · capability · evidence.** "Observed" means a name/string/archive proves the
capability existed in Tower 18.0. Where we cannot tell whether a detail exists, the row says so.

### G1 Repository management and discovery — `EV:analysis/feature-scope#G1`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G1-01 | T0 | Open a repository (file panel, drag-and-drop, `gittower://` deep link) | `EV:analysis/feature-scope#G1`; `EV:analysis/services#F12` |
| CAP-G1-02 | T0 | Create a new repository | `EV:types/GTGitCreateRepositoryOperation` |
| CAP-G1-03 | T0 | Bookmarked repositories sidebar: folders + items, drop zone | `EV:ib/GTBookmarksSidebarViewController.nib`, `EV:types/GTBookmarksDropZoneView` |
| CAP-G1-04 | T0 | Recently cloned repositories list | `EV:analysis/feature-scope#G1` |
| CAP-G1-05 | T0 | Clone with URL field + autocomplete and a visible searching state, clone-to path, recents | `EV:ib/GTCloneGitRepositoryViewController.nib` (29 objects) |
| CAP-G1-06 | T0 | Clone options: bare, initialize submodules, use LFS clone | `EV:types/GTGitCloneRepositoryOptionBare`, `...InitializeSubmodules`, `...UseLFSClone` |
| CAP-G1-07 | T0 | Clone activity window with per-state cells (active / cancelled / error / finished) and cancel / restart / clear | `EV:ib/GTCloneActivity{Active,Cancelled,Error,Finished}CellView.nib`; `EV:types/GTGitCancelCloneActivitiesAction` |
| CAP-G1-08 | T0 | Default cloning directory setting + recently used cloning directories | `EV:strings/GTUserDefaultsDefaultCloningDirectory`, `GTUserDefaultsRecentCloningDirectories` |
| CAP-G1-09 | T1 | Quick Open palette over a background home-directory indexer (enable toggle, search paths, ignored paths, distinct searching/gathering phases) | `EV:types/GTGitRepositoryFinder`, `...FinderPathsManager`, `GTUserDefaultsQuickOpen*` |
| CAP-G1-10 | T1 | Multiple windows and repository tabs | `EV:sel/newWindowForTab:`, `EV:sel/openNewWindow:`; `EV:analysis/feature-scope#G1` |
| CAP-G1-11 | T1 | Sidebar workspace/refs organisation: bookmarks folder search, sidebar footer | `EV:ib/SidebarFooterViewController.nib`; `EV:assets/navbar_repositories` |

### G2 Working copy — staging, discarding, file actions — `EV:analysis/feature-scope#G2`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G2-01 | T0 | Stage / unstage whole file and whole selection | `EV:sel/stageAllItems:`, `EV:sel/unstageAllItems:`; `EV:types/GTGitStageEntriesOperation` |
| CAP-G2-02 | T0 | Stage / unstage a chunk (hunk) | `EV:sel/stageChunk:`; `EV:ib/GTWorkingCopyStagingCellView.nib` |
| CAP-G2-03 | T0 | Discard changes: file, chunk, all — with confirmation dialogues; "warn when discarding changes in a chunk" setting | `EV:sel/discardChunk:`, `EV:sel/discardLocalChangesForItems:`; `EV:ib/GTDiscardAllLocalChangesDialogueViewController.nib`; `EV:strings/GTUserDefaultsAlwaysDiscardDiffContent` |
| CAP-G2-04 | T0 | Rich file list with per-file status, and per-file detail + diff + image diff + unchanged-file views | `EV:ib/GTWorkingCopyFile{List,Detail,Diff,UnchangedFile}ViewController.nib`, `GTWorkingCopyDiffImageViewController.nib` |
| CAP-G2-05 | T0 | Status header with counts and merge status; conflicted-only filter | `EV:ib/GTWorkingCopyStatusHeaderViewController.nib`; `EV:strings/GTUserDefaultsWorkingCopyShowConflictedItemsOnly` |
| CAP-G2-06 | T1 | Show ignored / non-modified / assume-unchanged items | `EV:strings/GTUserDefaultsWorkingCopyShow{Ignored,NonModified,AssumeUnchanged}Items` |
| CAP-G2-07 | T1 | Assume-unchanged set/unset on entries | `EV:types/GTGitSetAssumeUnchangedEntriesAction`, `...UnsetAssumeUnchangedEntriesAction` |
| CAP-G2-08 | T1 | Ignore paths: custom pattern editor, repository ignores list, ignore tracked files dialogue | `EV:ib/GTAddCustomIgnorePatternViewController.nib`, `GTRepositoryIgnoresViewController.nib`, `GTIgnoreTrackedFilesDialogueViewController.nib` |
| CAP-G2-09 | T1 | Untrack entries | `EV:types/GTGitUntrackEntriesAction` |
| CAP-G2-10 | T1 | Apply patch to working copy; save patch (with omit-prefixes option) | `EV:ib/GTApplyPatchToWorkingCopyDialogueViewController.nib`; `EV:strings/GTUserDefaultsSavePatchOmitPrefixes` |
| CAP-G2-11 | T0 | Open in external editor / Terminal / Finder; reveal in Finder | `EV:sel/openInTerminal:`, `EV:sel/openInFinder:`; `EV:types/GTGitUndoCheckoutFilesAction` |
| CAP-G2-12 | T1 | Configurable double-click and spacebar actions on a file row | `EV:strings/GTUserDefaultsWorkingCopyFileStatusDoubleClickAction{OpenFile,OpenDiff,OpenSubmodules}`, `...WorkingCopySpacebarAction` |

### G3 Commit workflow and the commit editor — `EV:analysis/feature-scope#G3`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G3-01 | T0 | Commit composer: subject/body split with char-limit guides and counters | `EV:ib/GTWorkingCopyCommitViewController.nib`; `EV:strings/GTUserDefaultsSubjectCharacterLimit`, `...ShowsSubjectCharacterLimitCounter`, `...ShowsBodyLineCharacterLimitGuide` |
| CAP-G3-02 | T0 | Commit with sign-off toggle | `EV:sel/signOffToggled:`; `EV:strings/GTUserDefaultsCommitOptionSignOff` |
| CAP-G3-03 | T0 | Amend last commit | `EV:types/GTGitAmendRevisionAction`; `EV:sel/amendCheckbox` (outlet) |
| CAP-G3-04 | T0 | Undo commit (snapshot/undo family) | `EV:types/GTGitUndoCommitAction` |
| CAP-G3-05 | T1 | Commit templates: per-repo + global, subject template view, template button, presets menu, import/reload | `EV:ib/CommitSubjectTemplateView.nib`, `EditCommitTemplateDialogueViewController.nib`; `EV:types/GTGitSetRepositoryCommitTemplateFileAction` |
| CAP-G3-06 | T1 | Commit-message warnings and cleanup: strip comments, warn on empty commit, warn when template replaces message, subject/body line-limit dialogues | `EV:strings/GTUserDefaults{WarnIfCommitMessageContainsComments,StripCommentsFromCommitMessage,WarnIfCommitIsEmpty,WarnIfCommitMessageIsReplacedByTemplate}`, `EV:ib/EditLineCharacterLimitDialogueViewController.nib` |
| CAP-G3-07 | T1 | Edit the message of any commit; edit/rewrite revision | `EV:types/GTGitEditCommitMessageAction`, `EV:types/GTGitEditRevisionAction` |
| CAP-G3-08 | T1 | Autocomplete plugins in the commit editor: issue refs, file refs, commit refs, fixups, gitmoji, snippets | `EV:types/IssuesTextViewPlugin`, `FileReferenceTextViewPlugin`, `CommitReferenceTextViewPlugin`, `FixupTextViewPlugin`, `GitMojiPlugin`, `SnippetTextViewPlugin` |
| CAP-G3-09 | T1 | GPG signing toggle for commits (global/local config) | `EV:types/GTGitSetGPGSigningEnabledAction`, `GTGitUpdateGlobalGPGConfigAction`, `GTGitUpdateLocalGPGConfigAction` |
| CAP-G3-10 | T2 | AI commit message generation through a user-chosen agent CLI (`claude`, `codex`, `gemini`, `copilot`), with prompt CRUD, import/backup and context settings | `EV:analysis/feature-scope#G3`; `EV:types/AIAgentBinary*`, `AIPromptAction`, `BackupAIPromptsOperation` |
| CAP-G3-11 | T1 | Commit changeset view in the composer (file list of what is being committed) | `EV:ib/GTCommitChangesetViewController.nib` (36 objects) |

### G4 History browsing and comparison — `EV:analysis/feature-scope#G4`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G4-01 | T0 | Commit list with drawable graph lanes (segment start/end indexes computed by the engine) | `EV:types/GTGitCommitGraphLane` (fields `laneID`, `startingIndex`, `endingIndex`), `GTGitDrawCommitGraphOperation` |
| CAP-G4-02 | T0 | Density modes (small / medium commit cells) and show/hide graph | `EV:ib/GTHistoryCommitList{Small,Medium}CellView.nib`; `EV:strings/GTUserDefaultsHistoryShowGraph`, `...HistoryGraphCommitViewSize` |
| CAP-G4-03 | T0 | Date group rows and fork-point rows | `EV:ib/GTHistoryCommitList{DateGroup,ForkPointMedium,ForkPointSmall}CellView.nib` |
| CAP-G4-04 | T0 | Commit detail: header, changeset file list, diff view | `EV:ib/GTHistoryDetailViewController.nib`, `GTHistoryCommitChangesetViewController.nib` |
| CAP-G4-05 | T0 | Diff options in the detail pane: context lines, ignore whitespace, full-diff toggle | `EV:sel/setContextLines:`, `EV:sel/ignoresWhitespaceChangesOptionToggled:`, `EV:sel/toggleShowsFullDiff:` |
| CAP-G4-06 | T0 | First-parent-only and date/topo sort modes; commit count limit | `EV:strings/GTUserDefaultsHistoryShowFirstParentOnly`, `...HistorySortByDate`, `...HistorySortByTopoOrder`, `...NumberOfCommitsInHistory` |
| CAP-G4-07 | T1 | Search / filter bar: date filter (absolute/relative), type, selection, contributor autocomplete | `EV:ib/GTHistorySearchFilterViewController.nib`; `EV:sel/historySearchFilter*` |
| CAP-G4-08 | T1 | Compare branches / revisions (RefComparison pane with summary, commits, changeset, no-data, large-diff warning) | `EV:ib/RefComparison*.nib`; `EV:sel/compareRevisions:` |
| CAP-G4-09 | T1 | Commit tree browser in the history pane | `EV:ib/GTHistoryCommitTreeViewController.nib` |
| CAP-G4-10 | T0 | Copy verbs: commit hash, file path, branch name, tag name, remote branch name | `EV:types/GTQuickActionNameCopy{CommitHash,FilePath,BranchName,TagName,RemoteBranchName}` |
| CAP-G4-11 | T0 | Large-diff warning before rendering | `EV:ib/LargeDiffWarningCellView.nib`; `EV:strings/GTUserDefaultsDiffWarningThreshold` |

### G5 Branch management — `EV:analysis/feature-scope#G5`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G5-01 | T0 | Create a branch, from HEAD or any commit, with a checkout-after-create option | `EV:ib/GTCreateNewBranchViewController.nib` (41 objects); `EV:types/GTGitCreateBranchAction`; `EV:strings/GTUserDefaultsDialogueOptionCheckoutCreatedBranch` |
| CAP-G5-02 | T0 | Checkout a revision / branch; checkout a remote branch and create tracking; auto-stash policy applies | `EV:ib/GTCheckoutRevisionViewController.nib`; `EV:types/GTGitCheckoutRemoteBranchAction` |
| CAP-G5-03 | T0 | Rename a branch (with a setting guarding branches that have an open GitHub PR) | `EV:types/GTGitRenameBranchAction`; `EV:analysis/feature-scope#G5` |
| CAP-G5-04 | T0 | Delete branches — local and remote — with an unmerged/unpushed guard | `EV:ib/GTDeleteBranchesViewController.nib`; `EV:types/GTGitDeleteUnmergedBranchError` |
| CAP-G5-05 | T1 | Fast-forward a branch | `EV:types/GTGitFastForwardBranchAction` |
| CAP-G5-06 | T1 | Update a branch with a merge mode (merge / rebase / squash / create-merge-commit) | `EV:strings/GTUserDefaultsUpdateBranchMergeMode{Merge,Rebase,Squash}`, `...UpdateBranchAlwaysGenerateMergeCommit`; `EV:types/GTGitUpdateBranchesOperation` |
| CAP-G5-07 | T1 | Pin / unpin a branch into a "Pinned" sidebar section | `EV:types/GTQuickActionName{Pin,Unpin}Branch` |
| CAP-G5-08 | T1 | Sidebar grouping settings: by parent / by trunk, grouped refs on top, refs in sidebar | `EV:strings/GTUserDefaults{GroupsBranchByParentInSidebar,GroupsBranchByTrunkInSidebar,GroupsRefsInSidebar,ShowGroupedRefsOnTop}` |
| CAP-G5-09 | T2 | Branch review pane: compare local branches to a chosen base, filter menu, sort attribute/order, search scopes, "fully merged" badge | `EV:ib/BranchesReview{Header,Detail}ViewController.nib`; `EV:types/GTGitLoadFullyMergedBranchInfosOperation` |
| CAP-G5-10 | T2 | Archive / unarchive branches, auto-archive policy (stale months threshold, fully-merged rule, per-branch include/skip, reviewed flag), stale + fully-merged sidebar badges | `EV:types/GTGitAutoArchiveBranchesOperation`, `GTQuickActionName{ArchiveBranch,UnarchiveBranch,IncludeInAutoArchivingBranch,SkipAutoArchivingBranch}`; `EV:strings/GTUserDefaultsAutomaticallyArchiveBranches`, `...StaleBranchInterval` |
| CAP-G5-11 | T2 | Stacked branches: create stacked branch, set/remove parent, restack, finish topic branch, repair stack, dependency validation | `EV:ib/GTCreateNewStackedBranchDialogueViewController.nib`, `GTRepairStackDialogueViewController.nib`; `EV:types/GTGit{CanRestack,Restack}BranchOperation`, `ValidateStackOperation` |
| CAP-G5-12 | T2 | Branch compare header in history (base-branch header) | `EV:ib/GTHistoryCommitListBaseBranchHeaderViewController.nib`; `EV:strings/GTUserDefaultsHistoryComparesBranches` |

### G6 Remotes, network operations, publishing — `EV:analysis/feature-scope#G6`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G6-01 | T0 | Remote CRUD: add, edit name, edit URL, delete | `EV:ib/GTAddRemoteToRepositoryViewController.nib`, `GTEditRemoteNameViewController.nib`, `GTEditRemoteURLViewController.nib`; `EV:types/GTGitAddRemoteOperation` |
| CAP-G6-02 | T0 | Fetch: one remote, all remotes, all tags; prune obsolete remote-tracking branches | `EV:sel/fetch:`, `EV:sel/fetchAllRemotes:`; `EV:strings/GTUserDefaultsAlwaysFetchAllRemotes`, `...AlwaysFetchAllTags`, `...AlwaysPruneObsoleteRemoteBranches` |
| CAP-G6-03 | T0 | Pull with options: rebase instead of merge, verify signatures; remote + branch selection | `EV:ib/GTPullFromRemoteViewController.nib`; `EV:strings/GTUserDefaultsDialogueOptionPullUseRebase`, `...DialogueOptionPullVerifySignatures` |
| CAP-G6-04 | T0 | Push with options: push all tags, recurse submodules; push to a chosen branch | `EV:ib/GTPushDialogueViewController.nib` (37 objects); `EV:strings/GTUserDefaultsDialogueOptionPushPushAllTags`, `...DialogueOptionPushRecurseSubmoduleMode` |
| CAP-G6-05 | T0 | Force push dialogue, defaulting to `--force-with-lease` behaviour | `EV:ib/GTForcePushDialogueViewController.nib` (34 objects); `EV:analysis/feature-scope#G6` |
| CAP-G6-06 | T0 | Sync (fetch + update) as a one-click action with a quick variant | `EV:sel/sync:`, `EV:sel/quickSync:`; `EV:types/GTGitSyncAction` |
| CAP-G6-07 | T0 | Remote activity window: list + detail, cancel, stall-watchdog surfacing, clear/remove finished, retry clone | `EV:ib/GTActivityWindowController.nib`; `EV:types/GTGitRemoteActivityWatchdogController`, `GTGitCancelStalledRemoteActivitiesAction` |
| CAP-G6-08 | T1 | Remote settings per remote: general, remote branches, refspec editor (fetch + push) | `EV:ib/GTAddOrEditRefSpecViewController.nib`; `EV:types/GTGitAddFetchRefSpecAction`, `...AddPushRefSpecAction` |
| CAP-G6-09 | T1 | Branch tracking add/remove; set upstream | `EV:types/GTGitAddBranchTrackingOperation`, `...RemoveBranchTrackingOperation`, `GTGitSetUpstreamBranchAction` |
| CAP-G6-10 | T1 | Publish branch / tag / repository (access type selection) | `EV:ib/GTPublishBranchViewController.nib`, `GTPublishTagDialogueViewController.nib`, `PublishRepositoryDialogueViewController.nib` |
| CAP-G6-11 | T1 | Auto-fetch policy with an interval | `EV:types/GTGitAutoFetchController`; `EV:strings/GTUserDefaultsAutoFetchTimeInterval` |
| CAP-G6-12 | T2 | Branch lease and refspec-level push control | `EV:types/GTGitSetBranchLeaseOperation`, `...RemoveBranchLeaseOperation`, `...UpdateLeaseValuesOperation` |

### G7 Integrate, rewrite, and recover — `EV:analysis/feature-scope#G7`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G7-01 | T0 | Merge a branch with options: always create merge commit, auto-commit, verify signatures | `EV:ib/GTMergeBranchDialogueViewController.nib` (34 objects); `EV:strings/GTUserDefaultsDialogueOptionMerge{AlwaysGenerateMergeCommit,AutoCommit,VerifySignatures}` |
| CAP-G7-02 | T0 | Conflict resolution: per-file conflict UI, the seven conflict kinds, popover, open in merge tool, save conflict versions, resolve/revert, continue/skip/abort sequence | `EV:ib/MergeConflictsViewController.nib`; `EV:types/GTGit{AddedByThem,AddedByUs,BothAdded,BothDeleted,BothModified,DeletedByThem,DeletedByUs}MergeConflictResolver`; `EV:sel/showMergeConflictsPopover:` |
| CAP-G7-03 | T1 | Merge status check with conflict/mergeable/merged overview art | `EV:ib/GTWorkingCopyMergeStatusViewController.nib`; `EV:types/GTGitLoadMergeStatusOperation`; `EV:assets/overview_merge_status_*` |
| CAP-G7-04 | T0 | Rebase branch / rebase onto a chosen revision, with a rebase option in pull | `EV:ib/GTRebaseBranchDialogueViewController.nib`, `GTRebaseOntoBranchDialogueViewController.nib`; `EV:types/GTGitInteractiveRebaseOntoOperation` |
| CAP-G7-05 | T1 | Cherry-pick multiple revisions with fast-forward / always-merge-commit options | `EV:ib/GTCherryPickRevisionsDialogueViewController.nib` (33 objects); `EV:strings/GTUserDefaultsDialogueOptionCherryPick*` |
| CAP-G7-06 | T1 | Revert revisions: commit-after-revert toggle; keep rolled-back changes in the working tree | `EV:ib/GTRevertRevisionsDialogueViewController.nib`; `EV:analysis/feature-scope#G7` |
| CAP-G7-07 | T1 | Reset to a revision (preserving uncommitted changes) with a snapshot taken first | `EV:ib/GTResetToRevisionDialogueViewController.nib`; `EV:sel/resetToHEAD:`; `EV:strings/performResetActionWithRevision:headRevision:mode:snapshot:` |
| CAP-G7-08 | T1 | Working-tree snapshots: create and restore | `EV:types/GTGitCreateWorkingTreeSnapshotOperation`, `...RestoreWorkingTreeSnapshotOperation` |
| CAP-G7-09 | T1 | Undo family: undo commit, undo reset, undo save/apply stash, undo checkout files, undo abort merge | `EV:types/GTGitUndo{Commit,Reset,SaveStash,ApplyStash,CheckoutFiles,AbortMerge}Action` |
| CAP-G7-10 | T2 | History rewriting: squash, fixup, move revisions, delete revisions, interactive rebase sequence data | `EV:types/GTGitSquashRevisionsAction`, `...FixupRevisionsAction`, `...MoveRevisionsAction`, `...DeleteRevisionsAction`; `EV:ib/GTInteractiveRebase*` (sequence classes) |
| CAP-G7-11 | T1 | Validation before rewriting: compute unpushed / mainline / merge commits that would be lost | `EV:types/GTGitValidate{Unpushed,Mainline,RewriteMerge}Commits*`; `EV:types/GTGitRewrite*ValidationError` |

### G8 Stash — `EV:analysis/feature-scope#G8`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G8-01 | T0 | Save a stash, optionally including untracked files | `EV:ib/GTSaveStashViewController.nib`; `EV:types/GTGitSaveStashAction`; `EV:strings/GTUserDefaultsSaveStashOptionIncludeUntrackedFiles` |
| CAP-G8-02 | T0 | Apply a stash with options: delete after apply, restore changes to index | `EV:ib/GTApplyStashViewController.nib`; `EV:strings/GTUserDefaultsApplyStashOption{DeleteStashAfterApply,RestoreStashedChangesToIndex}` |
| CAP-G8-03 | T0 | Delete stashes in bulk with a confirmation | `EV:types/GTGitDeleteStashesAction`; `EV:analysis/feature-scope#G8` |
| CAP-G8-04 | T0 | Stash list with detail; stash quick actions (apply / delete / show) | `EV:ib/GTStashDetailViewController.nib`; `EV:types/GTQuickActionName{ApplyStash,DeleteStash,ShowStash}` |
| CAP-G8-05 | T1 | Partial stash (choose paths) | `EV:ib/GTSavePartialStashViewController.nib` |
| CAP-G8-06 | T1 | Auto-stash policy: offer to stash, always skip, always re-apply after the operation | `EV:ib/GTAutoStashDialogueViewController.nib`; `EV:strings/GTUserDefaults{AlwaysSkipAutoStash,AutoStashAlwaysReapplyChanges}` |
| CAP-G8-07 | T1 | Stash count badge in the sidebar | `EV:strings/GTUserDefaultsShowNumberOfStashesInSidebar` |
| CAP-G8-08 | T2 | Quick stash snapshot ("Quick Snapshot N Items") with its own undo | `EV:types/GTGitUndoCreateStashSnapshotAction`; `EV:strings/GTQuickActionNameCreateStashSnapshot` |

### G9 Tags — `EV:analysis/feature-scope#G9`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G9-01 | T0 | Create a tag: annotated with message, sign option, "take date from system" | `EV:ib/GTCreateTagViewController.nib` (31 objects); `EV:strings/GTUserDefaultsDialogueOptionSignTag` |
| CAP-G9-02 | T0 | Delete local and remote tags | `EV:types/GTGitDeleteTagsAction`, `...DeleteRemoteTagAction` |
| CAP-G9-03 | T0 | Publish a tag | `EV:ib/GTPublishTagDialogueViewController.nib` |
| CAP-G9-04 | T1 | Verify tag signatures | `EV:types/GTGitVerifyTagOperation`; `EV:analysis/feature-scope#G9` |
| CAP-G9-05 | T1 | Tag sorting settings and "push all tags" | `EV:ib/GTTagsSortSettingsMenuController.nib`; `EV:strings/GTUserDefaultsTagsSortMode` |

### G10 Submodules — `EV:analysis/feature-scope#G10`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G10-01 | T1 | Add / clone a submodule (with force-add) | `EV:ib/GTAddSubmoduleDialogueViewController.nib`; `EV:types/GTGitAddSubmoduleAction` |
| CAP-G10-02 | T1 | Init and update submodules, recursively | `EV:ib/GTInitSubmoduleViewController.nib`, `GTUpdateSubmoduleViewController.nib`; `EV:strings/GTUserDefaultsDialogueOptionUpdateSubmoduleRecursive` |
| CAP-G10-03 | T1 | Synchronize submodules | `EV:types/GTGitSynchronizeSubmoduleAction` |
| CAP-G10-04 | T1 | Fetch submodules (single/all) | `EV:ib/GTFetchSubmoduleViewController.nib`; `EV:sel/fetchAllSubmodules:` |
| CAP-G10-05 | T1 | Delete submodules, optionally keeping `.git` dir or working tree | `EV:ib/GTDeleteSubmodulesDialogueViewController.nib`; `EV:strings/GTUserDefaultsDeleteSubmoduleOption{KeepGitDirectory,KeepWorkingTree}` |
| CAP-G10-06 | T1 | Submodule settings: ignore mode, update strategy, recurse; edit URL | `EV:ib/GTSubmoduleGeneralSettingsViewController.nib`, `GTSubmoduleConfigSettingsViewController.nib`, `GTEditSubmoduleURLViewController.nib` |
| CAP-G10-07 | T1 | Submodule detail pane and open-in-app behaviour | `EV:ib/GTSubmoduleDetailViewController.nib`, `GTSubmoduleViewController.nib` |
| CAP-G10-08 | T2 | Move / rename a submodule | `EV:ib/GTMoveSubmoduleViewController.nib`; `EV:strings/GTUserDefaultsMoveSubmoduleOptionRename` |
| CAP-G10-09 | T2 | Restore a submodule | `EV:ib/GTRestoreSubmoduleViewController.nib` |
| CAP-G10-10 | T2 | Ahead/behind sidebar badges for submodule freshness; "verify submodules before push" | `EV:assets/repository_sidebar_badge_submodule_update_{ahead,behind}`; `EV:analysis/feature-scope#G10` |
| CAP-G10-11 | T2 | Auto-update-submodules policy | `EV:types/GTGitAutoUpdateSubmodulesController`; `EV:strings/GTUserDefaultsAlwaysAutoUpdateSubmodules` |

### G11 Worktrees — `EV:analysis/feature-scope#G11`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G11-01 | T1 | Add a worktree for a starting revision, optionally detached, option to create even if already checked out elsewhere | `EV:ib/GTAddWorktreeDialogueViewController.nib` (34 objects); `EV:types/GTGitAddWorktreeAction` |
| CAP-G11-02 | T1 | Move / relocate a worktree | `EV:ib/GTMoveWorktreeDialogueViewController.nib`, `GTRelocateWorktreeDialogueViewController.nib` |
| CAP-G11-03 | T1 | Remove a worktree, guarding locked worktrees and worktrees with changes | `EV:types/GTGitDeleteWorktreeLockedError`, `...DeleteWorktreeHasChangesError` |
| CAP-G11-04 | T1 | Repair a worktree | `EV:types/GTGitRepairWorktreeAction` |
| CAP-G11-05 | T1 | Open a worktree: in new tab / new window / terminal / Finder | `EV:types/GTQuickActionNameOpenWorktreeIn{NewTab,NewWindow,Terminal}`, `...RevealWorktreeInFinder` |
| CAP-G11-06 | T1 | Worktree settings page and detail pane; sidebar section | `EV:ib/GTWorktreeGeneralSettingsViewController.nib`, `GTWorktreeDetailViewController.nib`, `WorktreeSidebarCellView.nib` |

### G12 Services and pull requests — `EV:analysis/feature-scope#G12`, `EV:analysis/services`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G12-01 | T2 | Account management per provider, including enterprise variants; auth methods OAuth / PAT / basic | `EV:ib/GTServices{...}AccountViewController.nib` (11 provider flavored nibs); `EV:analysis/services#F2` |
| CAP-G12-02 | T2 | Enterprise/self-hosted hosts: URL validation, API-version probe, trust errors | `EV:types/GTServiceProbeServerAPIVersionOperation`, `GTServiceHostURLFormatValidator`, `GTServiceServerTrustError` |
| CAP-G12-03 | T2 | Repository browser and creator per provider | `EV:ib/GTServicesCloneRepositoryViewController.nib`; `EV:types/GTServiceCreate{Provider}RepositoryOperation` |
| CAP-G12-04 | T2 | Public key (SSH) management on the provider, with local key linking | `EV:ib/GTServicesPublicKeysViewController.nib`; `EV:types/GTService{Create,Delete}*PublicKeyOperation` |
| CAP-G12-05 | T2 | Pull-request list per repository, with automatic refresh interval and explicit reload | `EV:ib/GTPullRequestsListViewController.nib`; `EV:strings/GTUserDefaultsAutomaticPullRequestsUpdateInterval` |
| CAP-G12-06 | T2 | Pull-request detail: conversation (comments, threads), commits, changeset | `EV:ib/GTPullRequests{Conversation,Commits,Changeset,Detail}ViewController.nib` |
| CAP-G12-07 | T2 | Create a pull request | `EV:sel/createServicePullRequest:`; `EV:types/GTServiceCreatePullRequestOperation` |
| CAP-G12-08 | T2 | Checkout a pull request locally (fetch provider refs, handle forks, clean up fork refs) | `EV:sel/checkoutPullRequest:`; `EV:types/GTGitCheckoutPullRequestAction`, `...CleanUpPullRequestForkRefsAction` |
| CAP-G12-09 | T2 | Merge / close a pull request with provider-shaped merge options | `EV:ib/GTServicePullRequestMergeViewController.nib`; `EV:types/GTServiceMergePullRequestsOperation`, `...ClosePullRequestsOperation` |
| CAP-G12-10 | T2 | Pull-request comments: create, edit, reply, delete | `EV:types/GTService{Create,Edit,Delete,ReplyTo}PullRequestCommentOperation` |
| CAP-G12-11 | T2 | Issue search and account-driven issue linkification + autocomplete | `EV:types/GTServiceSearch*IssuesOperation`, `GTServiceFetchGitHubIssuesOperation`; `EV:analysis/services#F14` |
| CAP-G12-12 | T2 | Service activity list (per account) | `EV:ib/GTServiceActivityListViewController.nib`, `...DetailViewController.nib` |

### G13 Workflows: Git Flow, custom workflows, Graphite, Gerrit, SVN — `EV:analysis/feature-scope#G13`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G13-01 | T2 | Git Flow (classic): configure prefixes; create and finish feature/hotfix/release branches with finish options (merge commit, preserve merges, rebase, skip tagging) | `EV:ib/GTGitFlowFinish*BranchDialogueViewController.nib`; `EV:strings/GTUserDefaultsGitFlowFinishFeatureAlways{CreateMergeCommit,PreserveMergeCommits,Rebase}` |
| CAP-G13-02 | T2 | GitFlow Next: parse and update the workflow config | `EV:types/GitFlowNextConfigParser`, `GitFlowNextWorkflowUpdater`, `UpdateGitFlowNextWorkflowOperation` |
| CAP-G13-03 | T2 | Custom workflows: picker, configuration, save/disable; start/finish custom topic branch | `EV:ib/WorkflowPickerDialogueViewController.nib`, `ConfigureCustomWorkflowDialogueViewController.nib`; `EV:types/SaveCustomWorkflowAction` |
| CAP-G13-04 | T2 | Graphite integration: enable/disable, configure, branch create/delete/rename/merge/restack/squash/sync/submit, fetch stack, set/remove parent, dashboard, show PR, action warning, repair stack, minimum-version gate | `EV:analysis/services#F10`; `EV:ib/Graphite*.nib`, `ConfigureGraphiteFlowDialogueViewController.nib` (37 objects), `SubmitGraphiteBranchDialogueViewController.nib` (47 objects) |
| CAP-G13-05 | T2 | Gerrit support: enable, push refspec `refs/for/<branch>`, quick push, dedicated toolbar item and dialogue | `EV:types/GTGitPushToGerritRemoteActionValidator`; `EV:strings/GTUserDefaultsEnableGerritSupport`, `gerritPushRefSpecWithRefName:` |
| CAP-G13-06 | T2 | SVN interop: clone, add SVN remote, fetch/pull/push, publish branch/tag | `EV:ib/GTCloneSVNRepositoryViewController.nib`, `GTAddSVNRemoteToRepositoryViewController.nib`; `EV:types/GTGitSVN*Operation` |

### G14 File forensics: history, blame, reflog, blob — `EV:analysis/feature-scope#G14`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G14-01 | T1 | File history pane: timeline + detail for a path | `EV:ib/GTFileHistoryViewController.nib`, `GTFileHistoryTimelineViewController.nib`; `EV:types/GTQuickActionNameShowFileHistory` |
| CAP-G14-02 | T1 | Blame pane: annotated text, line numbers, commit popover | `EV:ib/GTBlameViewController.nib`, `GTBlameAnnotationCellView.nib`, `GTBlameCommitPopoverViewController.nib` |
| CAP-G14-03 | T1 | Blob viewer with binary handling | `EV:ib/GTBlobViewController.nib` (30 objects) |
| CAP-G14-04 | T1 | Reflog browser: sidebar + list with header/empty/loading/item cells; entry count setting | `EV:ib/GTRefLog*ViewController.nib`; `EV:strings/GTUserDefaultsNumberOfEntriesInReflog`, `...ShowReflogInSidebar` |
| CAP-G14-05 | T1 | Contributor statistics and contributor filter in history | `EV:types/GTGitContribution`, `GTGitContributorsUpdater`; `EV:ib/GTHistoryContributorAutocompleteCellView.nib` |

### G15 User profiles and identities — `EV:analysis/feature-scope#G15`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G15-01 | T1 | Git user profiles: create, edit, list, detail pane | `EV:ib/GTUserPreferencesProfilesViewController.nib`, `GTAddGitUserProfileDialogueViewController.nib` |
| CAP-G15-02 | T1 | Per-repository identity override; set as global user | `EV:types/GTGitSetRepositoryUserAction`, `...UpdateGlobalUserAction`; `EV:types/GTQuickActionNameSetAsCommitterIdentity` |
| CAP-G15-03 | T2 | Import identities from GPG configuration | `EV:types/GTGitUserProfileImporter` |

### G16 SSH, signing and security — `EV:analysis/feature-scope#G16`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G16-01 | T1 | SSH key manager: import, list, serialize, type conversion; private-key password prompt | `EV:types/GTSSHKeyImporter`, `GTSSHKeySerialization`, `GTSSHKeyTypeConverter`, `GTSSHKeysUpdater`; `EV:ib/GTServicesPrivateSSHKeyPasswordViewController.nib` |
| CAP-G16-02 | T1 | SSH program selection with validation; SSH agent socket handling and diagnostics | `EV:types/SSHProgramValidator`, `SSHSockProvider`; `EV:strings/SSH_AUTH_SOCK` |
| CAP-G16-03 | T1 | GPG signing configuration: per-repo enable, key selection, key-email/committer-email match check | `EV:types/GTGitSetRepositoryGPGSigningEnabledAction`, `GTGitUpdateGlobalGPGConfigAction`; `EV:strings/GTUserDefaultsEnsureGPGKeyEmailMatchesCommitterEmail` |
| CAP-G16-04 | T1 | SSH signing support and allowed-signers file selection | `EV:types/GTSSHSigningInitializer`, `GTGitSetRepositoryAllowedSignersFileAction`; `EV:sel/selectAllowedSignersFile:` |
| CAP-G16-05 | T1 | Signature verification: trust states (good / missing / untrusted / failed), history badges and popover, merge gate option | `EV:types/GTGitGPGSignature{Missing,Untrusted}Error`; `EV:ib/GTSignaturePopoverViewController.nib`, `GTGitSignaturePopoverViewController.nib` |
| CAP-G16-06 | T1 | HTTPS credential guidance and remote connection doctor | `EV:types/GTGitRemoteConnectionDoctor`, `GTGitValidateRemoteConnectionOperation`; `EV:ib/GTGitRemoteConnectionViewController.nib` |

### G17 Preferences, environment, integrations — `EV:analysis/feature-scope#G17`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G17-01 | T1 | Preferences window with settings-schema-driven tabs: General, Git Config, Integration, Advanced, Appearance, Editor, Environment, Text, Profiles, Updates | `EV:analysis/ui-structure#F16`; outlet `settingsItemsController` (10 occurrences) |
| CAP-G17-02 | T1 | Diff tool and merge tool selection, including directory-diff tools | `EV:ib/GTSelectDiffToolDialogueViewController.nib`, `GTSelectMergeToolDialogueViewController.nib`; `EV:strings/GTUserDefaultsDiffToolIdentifier`, `...MergeToolIdentifier` |
| CAP-G17-03 | T1 | Terminal application selection; open in terminal | `EV:strings/GTUserDefaultsDefaultTerminalApplication`; `EV:sel/openInTerminal:` |
| CAP-G17-04 | T1 | Editor typography and tab settings; invisible characters; syntax colouring toggles | `EV:strings/GTUserDefaultsEditorFont`, `...EditorFontSize`, `...SpacesPerTab`, `...ShowInvisibleCharacters`, `...ColorSyntaxInDiffs` |
| CAP-G17-05 | T1 | Git binary selection with version validation and a diagnostics report | `EV:types/GTGitBinaryValidator`, `GTGitBinaryReportCreator`; `EV:strings/GTUserDefaultsGitBinary` |
| CAP-G17-06 | T2 | Environment variables: custom variables, request from login shell, application/ git environment layering | `EV:types/ApplicationEnvironmentController`, `UserEnvironmentController`, `ShellEnvironmentController`, `GitEnvironmentController`; `EV:ib/UserPreferencesEnvironmentViewController.nib` (48 objects) |
| CAP-G17-07 | T2 | Command-line tool install / update / uninstall (`gittower` equivalent) | `EV:analysis/feature-scope#G17`; `EV:sel/commandLineToolActionButtonPressed:` |
| CAP-G17-08 | T2 | Themes: import/export/validate, light+dark slots, versioned file format with migrations, per-theme syntax colouring | `EV:analysis/visual-design#F8`; `EV:strings/GTUserDefaultsDarkTheme`, `...LightTheme`, `ThemeV4toV5Migration` |
| CAP-G17-09 | T1 | Appearance mode (auto/light/dark) and app icon selection, reacting to OS accessibility display changes | `EV:assets/preferences_appearance_{auto,dark,light}`; `EV:sel/workspaceDidChangeAccessibilityDisplayOptions:` |

### G18 Platform features: quick actions, notifications, updates — `EV:analysis/feature-scope#G18`

| ID | Tier | Capability | Evidence |
|---|---|---|---|
| CAP-G18-01 | T1 | Quick actions: context-appropriate action sets over 20 object types (Branch, Commit, Revision, History, Remote, RemoteBranch, Stash(es), Submodule, Tag, TreeEntry, WorkingCopy, Worktree, PullRequests, Reflog, Settings, Theme, Appearance, UserProfile, BranchesReview) | `EV:types/GTQuickActionObjectName*` (20 types); `EV:ib/GTQuickActionViewController.nib` (31 objects) |
| CAP-G18-02 | T1 | Toolbar with global verbs; detail-tracking separator; per-pane list headers; sidebar footer with activity | `EV:analysis/visual-design#F13`; `EV:ib/SidebarFooterViewController.nib` |
| CAP-G18-03 | T1 | Notification plumbing per subsystem (activity, pull requests, accounts) | `EV:types/GTServiceActivitiesDidUpdateNotification`, `GTGitRepositoryDidStartRemoteOperationNotification` |
| CAP-G18-04 | T2 | Onboarding sequence (welcome, getting started, git user, tools, privacy consent, newsletter) | `EV:sel/showOnboarding*Screen:`; `EV:analysis/ui-structure#F15` |
| CAP-G18-05 | T2 | "What's New" window and release info handling | `EV:ib/WhatsNewWindow*`; `EV:strings/GTUserDefaultsReleaseInfoReadItemIDs` |
| CAP-G18-06 | T2 | Update checking with a release channel selection | `EV:strings/GTUserDefaultsUpdatesReleaseChannel`; `EV:ib/GTReleaseChannelAccessTokenWindowController.nib` — **Meridian designs its own update mechanism, not Sparkle-by-copy** |
| CAP-G18-07 | T2 | Crash reporting / usage analytics with explicit opt-out | `EV:strings/GTUserDefaultsAlwaysSendCrashReports`, `GTUserDefaultsAnalyticsTrackingEnabled` — **Meridian decides independently whether to ship any of this** |
| CAP-G18-08 | T2 | Multi-request deep-link handling with queueing during startup and per-verb validation | `EV:analysis/services#F12` |

**Counts:** 18 groups, **159 capability rows** (T0 = 47, T1 = 73, T2 = 39). Counted mechanically
from the tables above. The `feature-scope` report states the same catalogue as "~95 capabilities"
at coarser granularity; Meridian splits combined bullets (e.g. "submodule lifecycle" into eleven
rows) so each row is independently schedulable and testable. Neither number is a count of verified
code paths — both are counts of named evidence items.

### Deliberately not built

- **License activation / trial / device activation / referral** — observed
  (`EV:strings/RequestProTrialAction`, `UpgradeToProAction`, activations URL) but a commercial
  mechanism we are not replicating. Perpetual local licence state is Meridian's own business decision,
  out of scope for this spec.
- **Perforce** — icons and one label observed (`services_icon_account_perforce`, "Perforce GitSwarm"),
  **no class-level wiring observed**. Listed here so nobody schedules it from evidence alone
  (`EV:analysis/services#F9`).
- **Tower's OAuth broker** — we will not depend on `oauth.git-tower.com`; each provider adapter
  uses device flow or PAT per provider capability (docs/03 §services).

## 4. Screens

Screens are how the capability catalogue is delivered. Each screen maps to panes observed in the
evidence; ids are ours.

| Screen | Kind | Delivers | Evidence |
|---|---|---|---|
| S1 Welcome/No-repository | main window, empty state | CAP-G1-01..05, quick-start recap | `EV:assets/empty_view_icon_*` |
| S2 Bookmarks sidebar + repository window | main window | CAP-G1-03/04/10, CAP-G18-02 | `EV:ib/GTMainWindowController.nib`, `EV:ib/GTBookmarksSidebarViewController.nib` |
| S3 Clone sheet + Clone activity window | sheet + utility window | CAP-G1-05..08 | `EV:ib/GTCloneGitRepositoryViewController.nib`, `GTActivityWindowController.nib` |
| S4 Quick Open palette | utility window | CAP-G1-09 | `EV:ib/GTQuickOpenViewController.nib` (25 objects) |
| S5 Repository sidebar | pane | CAP-G4/5/6/8/9/10/11 navigation, badges, grouping settings | `EV:ib/GTRepositorySidebarViewController.nib` (31 objects) |
| S6 Working copy pane | master-detail pane | G2, G3, CAP-G6-07, G7 conflict status | `EV:ib/GTWorkingCopy*` stack |
| S7 Commit composer | embedded in S6 | G3 | `EV:ib/GTWorkingCopyCommitViewController.nib` |
| S8 History pane | master-detail pane | G4, CAP-G5-12, CAP-G7-10 | `EV:ib/GTHistory*` stack |
| S9 Branches review pane | pane | CAP-G5-09 | `EV:ib/BranchesReview*` |
| S10 Services pane (dashboard, accounts, PRs) | top-level section | G12 | `EV:ib/GTServices*`, `GTPullRequests*` |
| S11 File history / Blame / Blob / Reflog panes | panes | G14 | `EV:ib/GTFileHistory*`, `GTBlame*`, `GTBlobViewController.nib`, `GTRefLog*` |
| S12 Activity window | utility window | CAP-G6-07, clone activities | `EV:ib/GTActivityWindowController.nib` |
| S13 Quick actions palette | utility window | CAP-G18-01 | `EV:ib/GTQuickActionViewController.nib` |
| S14 Preferences window | tabbed window | G17, G16, G15 | `EV:ib/GTUserPreferences*`, `UserPreferences*` |
| S15 Dialogue family (commit-confirm, merge, rebase, cherry-pick, revert, reset, push, pull, force-push, sync, publish, tag, worktree, submodule, stash, delete-branch, discard, refspec, diff-tool, merge-tool, SSH/GPG selection) | modal shell | most T0/T1 verbs | `EV:ib/GTDialogueViewController.nib`; per-verb nibs listed per capability |
| S16 Submodule / Worktree settings + detail | panes | G10, G11 | `EV:ib/GTSubmodule*`, `GTWorktree*` |
| S17 Onboarding / What's New | windows | CAP-G18-04/05 | `EV:analysis/ui-structure#F15` |

**Screen count:** 17 screens (S1–S17). Each screen's component breakdown is in docs/04.

## 5. Feature tiers, restated as a build plan

- **T0 — 47 capabilities.** A user can clone or open, stage hunks, commit (incl. amend/undo),
  browse history with a graph, branch/checkout/rename/delete, fetch/pull/push/sync/force-push with
  activities, merge/rebase and resolve conflicts, stash, tag, reset with a snapshot, and undo.
  Feature-complete for personas P1/P2.
- **T1 — 73 capabilities.** Worktrees, submodules, forensics, profiles, security/signing, LFS
  patterns, git-flow (classic), preferences/environment, quick actions, branch review, snapshots
  and undo depth, auto-behaviours (fetch/stash/submodules). Feature-complete for P3.
- **T2 — 39 capabilities.** Services/PRs, Graphite/Gerrit/SVN/GitFlow-Next/custom workflows,
  themes, deep links, onboarding/updates/telemetry, AI commit messages. Reaches P4 and the
  ecosystem.

Every tier boundary is a scope decision, not an evidence claim; the evidence list in §3 is the
constant.

## 6. Explicitly unverified (do not build on these)

| Item | Status | Why |
|---|---|---|
| Exact wording of any Tower UI copy | **unverified** | Copy recovered incidentally from IB archives; localization files not in scope (`EV:00 §5.7`). Meridian writes its own copy anyway. |
| Enum case sets (e.g. `SubmoduleIgnoreMode` values) | **inferred** | Derived from class names + adjacent strings (`EV:analysis/git-semantics` limits). |
| Sequencing inside any multi-step flow (e.g. exactly what "finish feature" does) | **inferred from dialogue copy** | No call graphs. |
| Whether hidden/undocumented defaults exist | **not observed** | Absence in the string slice is not proof of absence (MANIFEST). |
| Perforce integration | **not observed** | Icons + one label only. |
| `MERGE_HEAD` / `REVERT_HEAD` / rebase-merge on-disk markers | **not observed in the string slice** | We rely on standard git markers because they are the public on-disk format, not because Tower was seen using them. |
| Quick actions count | **84 (grep) vs 99 (report)** | Counted differently; both recorded. |
| Default values of settings | **mostly unknown** | Only keys observed; a handful of literal values appear near them. |

## 7. Out-of-scope for v1 (architecture allows later)

- Windows/Linux ports (macOS-first; docs/02 §12 lists what would change).
- Git hosting provider **server-side** features (webhooks, CI, org admin).
- Any use of Tower's name, iconography, or copy. Meridian names its own concepts and draws its own
  symbols (docs/04 §3).
