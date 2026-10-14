# 00 — Evidence Summary

What the reverse-engineering study of **Git Tower 18.0 (macOS, arm64)** established, how it was
established, and — with equal weight — **what it could not establish**. Every statement in this
document and in docs/01–04 carries an evidence reference; nothing is asserted from memory of the
product.

The raw evidence lives in `/tmp/rea-evidence/` (working copies archived in this repository under
`docs/evidence/` as name-only extracts). The domain analyses live in `/tmp/rea-analysis/`. This
document is the contract between those sources and the Meridian build: **if a fact is not
traceable to one of the sources below, it is marked `INFERRED` or `UNVERIFIED` and no build
decision may rest on it alone.**

---

## 1. Subject and method

| Item | Value |
|---|---|
| Subject | `Tower.app` — Git Tower 18.0, macOS universal binary, arm64 slice (`lipo -thin arm64`) |
| Primary tool | REA (`rea-agents`) 6.4.0 |
| Fallback tools | `strings`, `nm`, `otool`, `assetutil`-style catalog metadata |
| Collection date | 2026-10-14 |
| Main binary | 46.3 MB |
| Companion binaries | `TowerAskpass` (credential prompt helper), bundled `git` 2.51.0, `git-flow-tower`, `git-lfs` |
| Own frameworks | FNGit, FNAppKit, FNFoundation, FNMarkdown, FNLicensing |
| Provider frameworks | GitHubAPI, GitLabAPI, BitbucketAPI, AzureDevOpsAPI, StashAPI, BeanstalkAPI, CustomServiceAPI |
| Third-party | Sparkle 1.23 (updates) — out of scope for Meridian |

**Method, stated precisely:** REA decoded the Interface Builder archives, the asset catalog
metadata and the type/string tables of the binary. **No decompiler ran on this host** (the Ghidra
decompiler binary for mac-arm64 does not exist in official Ghidra 12.1.4 releases; a source build
failed on missing binutils headers). Therefore the study produced **no native pseudocode and no
function-level call graphs**. Every conclusion is at the level of *type names, symbol names, string
constants, and decoded serialized archives*.

## 2. Evidence inventory

| File | Contents | Count verified by this build's own greps |
|---|---|---|
| `/tmp/rea-evidence/fngit-types.txt` | `FNGit*` / `GTGit*` type names | 996 lines |
| `/tmp/rea-evidence/tower-strings.txt` | printable strings, arm64 slice | 87 327 lines |
| `/tmp/rea-evidence/ui-documents.txt` | 403 IB archives: per-nib class lists | 403 `## …nib` sections, 9 273 lines |
| `/tmp/rea-evidence/ui-all-classes.txt` | mixed vocabulary: class names **and** UI copy | 1 212 lines |
| `/tmp/rea-evidence/ui-graph.json` | merged object graph | 19 924 nodes / 9 099 edges |
| `/tmp/rea-out/assets.json`, `assets2.json` | Assets.car rendition metadata (paged) | 1 698 records |
| `/tmp/rea-evidence/otool-L.txt` | linked libraries | 39 entries |
| `/tmp/rea-analysis/*.md` | 6 domain reports | see `/tmp/rea-analysis/00-index.md` |

Node-kind census of `ui-graph.json` (queried directly): `constraint` 5 364, `unknown` 3 719,
`resource` 3 430, `control` 3 332, `outlet` 1 770, `view` 1 229, `scene` 401, `action` 331,
`objc_selector` 322 (263 distinct selector names), `layout_guide` 16.

Asset-catalog census (queried directly): `Image` 1 024, `Vector Glyph` 268, `Color` 252,
`PackedImage` 70, `Icon Image` 35, `Vector` 28, `IconGroup` 12, others 9 — **89 distinct color
names, 50 distinct glyph names, 275 distinct image names**.

## 3. Evidence reference convention

Used throughout docs/01–04:

| Tag | Meaning |
|---|---|
| `EV:types/<Name>` | a type name present in `fngit-types.txt` and/or `tower-strings.txt` |
| `EV:strings/<literal>` | a literal string present in `tower-strings.txt` |
| `EV:ib/<Nib.nib>` | an Interface Builder archive in `ui-documents.txt` / `ui-graph.json` |
| `EV:sel/<selector>` | an `objc_selector` node in `ui-graph.json` |
| `EV:assets/<name>` | an asset-catalog record (color / glyph / image) |
| `EV:otool/<lib>` | a linked library |
| `EV:analysis/<domain>#F<n>` | finding *n* in the corresponding `/tmp/rea-analysis/` report |

Counts in this document were re-derived from the raw files at build time; where the domain reports
state a slightly different number (e.g. quick-action identifiers) both numbers are given and the
discrepancy is explained, never smoothed over.

## 4. Established facts

Each fact below is a *name-level* fact unless marked otherwise. "Established" means: the named
artifact exists in the evidence. It does **not** mean the behavior behind it is known.

### 4.1 Engine architecture

1. **The git engine is a libgit2-style binding, not shell git.** The engine namespace is `GTGit*`
   (951 of the 996 types); the domain layer is `FNGit*` (≈45 types). `EV:types/GTGitAddRemoteOperation`,
   `EV:analysis/git-semantics#F1`.
2. **Cross-layer enum conversion is explicit.** 21 transformer types bridge the two layers
   (`FNGitFileStatusToGTGitFileStatusTransformer`, `FNGitMergeModeToGTGitMergeModeTransformer`,
   `FNGitResetModeToGTGitResetModeTransformer`, `FNGitObjectTypeToGTGitObjectTypeTransformer`,
   `FNGitSubmodule*` trio, plus GTGit-side display transformers such as
   `GTGitFileStatusToShortStringTransformer`). `EV:analysis/git-semantics#F1`.
3. **Uniform triad `Action` → `ActionValidator` → `Operation`** at the engine layer:
   194 `GTGit*Operation`, 168 `GTGit*Action`, 105 `GTGit*ActionValidator` (counts independently
   re-verified). The triad is the *pattern*, not an invariant: `GTGitLoadCommits` has no action;
   `GTGitClone` has no validator observed. `EV:analysis/git-operations#F1`.
4. **App-side command layer**: `GTGitActionController`, `GTGitActionSenderValidator` and
   per-domain delegates (`GTGitPullActionDelegate`, `GTGitCheckoutActionDelegate`, …). The
   "sender" validation (`GTGitFlowActionSenderValidator`, `GTGitHistoryActionSenderValidator`,
   `GTGitLFSActionSenderValidator`) is how one action behaves differently per UI context.
   `EV:analysis/git-operations#F2`.
5. **Two execution mechanisms coexist**: libgit2-backed operations, plus a shell-out command
   family `FNGit{Archive,Checkout,Clone,FormatPatch,SVNClone,Submodule}Command` with
   `FNGitCommandDelegate` / `FNGitCommandProgress` / `FNGitCommandCancelling`. The bundle ships a
   full `git` 2.51.0. `EV:analysis/git-operations#F8`.
6. **The git binary is managed, validated and reportable**: `GTGitBinaryManager`, `GTGitBinaryFinder`,
   `GTGitBinaryValidator`, `GTGitBinaryReportCreator`; errors `GTGitBinaryPathErrorKey`,
   `GTGitBinaryVersionErrorKey`, `GTGitBinaryMinimumRequiredVersionErrorKey`,
   `GTGitInvalidGitBinaryError`, `GTGitUnsupportedGitBinaryError`; setting
   `GTUserDefaultsGitBinary`. `EV:types/GTGitBinaryManager`.
7. **Work is queued per repository**: `GTGitQueue`, `GTGitRepositoryQueue`,
   `GTGitRepositoryQueueManager`, `GTGitCloningQueue`, `GTGitUserProfileQueue`, `ThemeQueue`,
   `GTGitRepositoryEnqueueable`. `EV:analysis/git-operations#F7`.
8. **Long-running network work is modeled as Activities** with progress, cancellation and a stall
   watchdog: `GTGitRemoteActivityManager`, `GTGitCloneActivityManager`, `GTGitWorkingCopyActivityManager`,
   `GTGitRemoteActivityWatchdogController`, cancel actions
   (`GTGitCancel{Remote,Clone,StalledRemote,AllRemote}ActivitiesAction`), notifications
   `GTGitRepositoryDidStart/FinishRemoteOperationNotification`,
   `GTGitRemoteActivitiesDidUpdateProgressNotification`. `EV:analysis/git-operations#F5`.
9. **Progress is a protocol with several granularities**: `FNGitCommandProgress`,
   `GTGitCloneOverallProgressDidUpdateNotification`,
   `GTGitWorkingCopyActivityDidUpdateProgressNotification`. `EV:analysis/git-operations#F17`.
10. **Nine operation groups mirror the sidebar**: `GTGitOperationGroup{Branches,History,RemoteBranches,
    Remotes,Repository,Stashes,Submodules,Tags,WorkingTree}` — 9 types, re-verified.
    `EV:analysis/git-operations#F15`.

### 4.2 Git semantics

11. **Typed errors**: 50 `GTGit*Error*` types spanning credentials/keychain
    (`GTGitRemoteConnectionKeychainItemNotFoundError`, `GTGitRemoteConnectionPrivateKeyNotFoundError`,
    `GTGitRemoteConnectionSSHAgentNotRunningError`), connectivity
    (`GTGitRemoteAuthenticationError`, `GTGitRemotePushRejectedError`), repository state
    (`GTGitIndexLockError`, `GTGitInvalidRepositoryError`, `GTGitNoWorkingCopyError`), validation
    (`GTGitRewriteUnpushedCommitsValidationError`, `GTGitRewriteMainlineCommitsValidationError`,
    `GTGitRewriteMergeCommitsValidationError`, `GTGitDeleteUnmergedBranchError`), with user-info
    keys (`GTGitErrorDomain`, `GTGitErrorMessageErrorKey`) rendered by one reusable controller
    (`GTGitErrorViewController` + accessory/scroll/text views). `EV:analysis/git-semantics#F12`.
12. **One parameterized integration state machine** for merge / rebase / cherry-pick / revert:
    `GTGitMergeMode{Merge,MergeSquash,Rebase,CherryPick,Revert}`,
    `GTGit{Merge,Rebase,CherryPick,Revert,InteractiveRebase}MergeProcessInfo`,
    `GTGitMergeProcessInfoManager`, `GTGitMergeSequenceStep`, `GTGitSequenceInstruction`,
    `GTGitMergeResultStateLoader`; ops `ContinueMerge`, `AbortMerge`, `SkipMerge`,
    `ReplayMergeSequence`. `EV:analysis/git-semantics#F5`.
13. **Conflict resolution is per conflict kind.** Seven resolver classes:
    `GTGit{AddedByThem,AddedByUs,BothAdded,BothDeleted,BothModified,DeletedByThem,DeletedByUs}MergeConflictResolver`;
    matching display strings ("Added By Us", "Both Modified", …); version handling via
    `GTGitMergeConflictFileVersion` / `GTGitSaveMergeConflictVersionsOperation`.
    `EV:analysis/git-semantics#F3`.
14. **Six diff perspectives** as validators: `GTGitCompare{IndexToBase,MineToMergeBase,MineToTheirs,
    TheirsToMergeBase,WorkingTreeToBase,WorkingTreeToIndex}Validator`; plus
    `GTGitWorkingTreeEntryDiffMode`. `EV:analysis/git-semantics#F4`.
15. **Status queries are option-driven**: `GTGitStatusOptionInclude{IgnoredFiles,Directories,
    NonModifiedFiles,AssumeUnchangedFiles}` mapping to UI toggles
    `GTUserDefaultsWorkingCopyShow{Ignored,NonModified,AssumeUnchanged}Items`,
    `...ShowConflictedItemsOnly`. `EV:analysis/git-semantics#F11`.
16. **File status is one enum rendered three ways**: short badge letter, text, color
    (`GTGitFileStatusTo{ShortString,Text,TextColor}Transformer`); assume-unchanged is an orthogonal
    boolean (`isAssumeUnchanged`, `canBeMarkedAssumeUnchanged`). `EV:analysis/git-semantics#F2`.
17. **Commit graph is computed engine-side, lane-wise**: `GTGitCommitGraphDrawer`,
    `GTGitCommitGraphInfo`, `GTGitCommitGraphLane` (fields `laneID`, `startingIndex`, `endingIndex`,
    `starting`, `ending`); nine lane color tokens exist (`*_history_graph_line_color`).
    `EV:analysis/git-semantics#F7`.
18. **Undo is a first-class operation family** backed by working-tree snapshots and patches:
    `Undo{A commit,Reset,SaveStash,ApplyStash,CheckoutFiles,AbortMerge}` actions,
    `GTGitCreateWorkingTreeSnapshotOperation` / `GTGitRestoreWorkingTreeSnapshotOperation`,
    selector `performResetActionWithRevision:headRevision:mode:snapshot:`, error string
    "Reset to revision failed because creating patch failed". `EV:analysis/git-operations#F3`.
19. **Automatic behaviors are policy objects**: `GTGitAutoFetchController`, `GTGitAutoStashController`,
    `GTGitAutoUpdateSubmodulesController`, `GTGitAutoApplyStashController` with gating defaults
    (`GTUserDefaultsAlwaysFetchAllRemotes`, `...AlwaysSkipAutoStash`, `...AutoStashAlwaysReapplyChanges`,
    `...AlwaysAutoUpdateSubmodules`, `...AutoFetchTimeInterval`). `EV:analysis/git-operations#F6`.
20. **Ref management is refspec-centric**: `Add/Update/Delete{Fetch,Push}RefSpec` operation families,
    SVN refspec families, `GTGitRefSpec`, `GTGitRefSpecInfo`, updaters
    `GTGitRemoteFetchRefSpecsUpdater` / `GTGitRemotePushRefSpecsUpdater`, UI
    `GTAddOrEditRefSpecViewController.nib`. `EV:analysis/git-operations#F11`.
21. **On-disk compatibility is standard git**: observed markers `gitdir:` (worktree redirect),
    `.git/config`, `CHERRY_PICK_HEAD`, `refs/heads/%@:%@`, `refs/for/%@`, `refs/pullrequests/`,
    `git reset HEAD --`, `.git/.graphite_repo_config`. `MERGE_HEAD` / `REVERT_HEAD` / rebase-merge
    directories were **not observed** in the string slice — absence here is not proof of absence.
    `EV:analysis/git-semantics#F15`.
22. **LFS is pattern management plus transfer**: `GTGitLFSAddTrackedFilePatterns`,
    `...RemoveTrackedFilePatterns`, `...LoadTrackingEntries`, `...Create{FileFromPointer,PointerFromFile,
    FileDataFromPointer}`, `Install/Uninstall`, transfer ops. `EV:analysis/git-operations#F13`.
23. **Signing spans GPG and SSH** with a trust vocabulary: `GTGitGPGSignatureMissingError`,
    `...UntrustedError`, `...SigningFailedError`, `GTGitGPGBinaryNotFoundError`; strings
    "Ensure GPG key email matches committer email", "Allowed signers file:",
    "SSH signing program:", "Merge is aborted if the tip commit of the side branch is not signed
    with a valid key."; UI `GTGitSignaturePopoverViewController`, "Signature Good",
    "No Signature Found"; app class `GTSSHSigningInitializer` exists in the Tower module.
    `EV:analysis/git-semantics#F10`.

### 4.3 Services

24. **Providers are linked frameworks** (one per host): GitHub, GitLab, Bitbucket, Azure DevOps,
    Stash, Beanstalk, Custom. `EV:otool/GitHubAPI.framework` … `EV:otool/CustomServiceAPI.framework`.
25. **135 `GTService*Operation` types** (re-verified) across **11 host flavors**: GitHub, GitHub
    Enterprise, GitLab, GitLab CE/EE, GitSwarm, Bitbucket, Stash, Beanstalk, Azure DevOps, Azure
    DevOps Server, Custom. The capability matrix is account / repositories / org units
    (organizations|groups|teams|projects) / public keys / pull requests / PR comments, plus issue
    search. `EV:analysis/services#F4`.
26. **Auth is brokered** through `https://oauth.git-tower.com/tower2-mac/%@/{authorize,token,refresh}`
    with `state` / `authCode` / `access_token` / `refresh_token` keys; non-OAuth methods enumerated
    by `GTServiceAccountAuthenticationMethod`: "Personal Access Token", "OAuth Access Token",
    "Basic Authentication". `EV:analysis/services#F2`.
27. **Credentials have versioned serialization**: `GTServiceAccountCredentialsToSerializationV{1,2,3}FormatTransformer`,
    `GTServiceAccountCredentialsStore`, `GTServiceAccountsSerialization`; Security.framework linked.
    `EV:analysis/services#F3`.
28. **PR checkout reaches into the git layer**: `GTGitCheckoutPullRequestAction`,
    `GTGitCleanUpPullRequestForkRefs`, `refs/pullrequests/`, fork-data autofetch with a dedicated
    error string; per-provider merge-option transformers. `EV:analysis/services#F5`.
29. **Enterprise hosts are probed**: `GTServiceProbeServerAPIVersionOperation`,
    `GTServiceProbeGitLabServerAPIVersionOperation`, `GTServiceHostURLFormatValidator`,
    `GTServiceHostValidationError`, `GTServiceServerTrustError`; strings "Self-Hosted Services",
    "The host is not a GitHub Enterprise server." `EV:analysis/services#F6`.
30. **Error taxonomy is complete for UI states**: `GTServiceRateLimitExceededError`,
    `GTServiceTwoFactorAuthenticationError`, `GTServiceLoginAttemptLimitExceededError`,
    `GTServiceAccessTokenExpiredError`, `GTServicePermissionError`, plus `GTServiceAPIErrorMapper`
    with per-request-part keys. `EV:analysis/services#F7`.
31. **Graphite is shell-out integration**: ~33 app-level classes, config file
    `.git/.graphite_repo_config`, settings `GTUserDefaultsGraphiteBinaryPath`,
    `GTUserDefaultsGraphiteShowActionWarning`, strings "Graphite Binary:", "Authenticating
    Graphite…", minimum-version message. `EV:analysis/services#F10`.
32. **Gerrit is a push-refspec feature**: `GTGitPushToGerritRemoteActionValidator`,
    `GTPushToGerritDialogueViewController`, `GTToolbarItemIdentifierGerritPush`,
    `GTUserDefaultsEnableGerritSupport`, refspec `refs/for/%@`. `EV:analysis/services#F11`.
33. **Deep links**: `gittower`, `github-mac`, `x-github-client`, `sourcetree` URL types; a verb
    dispatcher with queue-during-startup semantics and per-verb rule checks
    ("request dropped: more than %lu requests deferred during startup", "request ignored:
    unimplemented verb …"). `EV:analysis/services#F12`.
34. **Issue linkification is account-driven**: `IssuesReferenceLinkValueTransformer`,
    `IssuesAutocompleteItemController`, "Issue Tracker URL:", "Issue Number Prefix:".
    `EV:analysis/services#F14`.

### 4.4 UI structure

35. **One archive per pane**: `GTMainWindowController.nib` holds 7 objects, `GTMainContentViewController.nib`
    6; panes are separate nibs (`GTRepositoryViewController.nib`, `GTRepositorySidebarViewController.nib`,
    history/working-copy/services stacks). `EV:analysis/ui-structure#F1`.
36. **Eight window controllers**: Main, Dialogue, Activity, QuickAction, QuickOpen,
    ReleaseChannelAccessToken, Account, About. `EV:analysis/ui-structure#F3`.
37. **Sidebar section cell vocabulary** defines the information architecture: Branch, RemoteBranch,
    Remote, Tag, Stash, Submodule, Worktree, Workspace, Account, Bookmark (folder/item), plus
    `GTBookmarksDropZoneView`. `EV:analysis/ui-structure#F4`.
38. **The history pane is the most invested**: `GTHistoryCommitListViewController.nib` is the largest
    pane nib (50 objects), with fork-point / group / empty / loading cells at two densities,
    detail, commit tree, changeset and base-branch header. `EV:analysis/ui-structure#F6`.
39. **Staging is chunk-level**: selectors `stageChunk:`, `unstageChunk:`, `discardChunk:`,
    `stageAllItems:`, `unstageAllItems:`. `EV:sel/stageChunk`.
40. **One dialogue chrome plus a bare confirm**: `GTDialogueViewController.nib` (42 objects),
    `GTConfirmViewController.nib` (3), error accessory views; `DialogueView` / `DialogueWindowView` /
    `DialogueWrapperView`. `EV:analysis/ui-structure#F12`.
41. **The commit composer is an editor platform**: `GTWorkingCopyCommitViewController.nib` (41 objects)
    with `CommitMessageTextView`, `CommitSubjectTextView`, `CommitBodyTextView`,
    `CommitMessageActionBarViewController`, `CommitSubjectTemplateView`, autocomplete cell views
    (GitMoji / commit-template / fixup). `EV:analysis/ui-structure#F14`.
42. **Preferences is a tabbed, settings-schema-driven window**: General 49, Integration 53,
    Git Config 48, Advanced 40, Editor 36, Environment 48, Text 30 objects; outlet
    `settingsItemsController` recurs across tabs. `EV:analysis/ui-structure#F16`.
43. **Action surface is large and view-scoped**: 263 distinct IB selectors including
    `quickPull:` / `quickPush:` / `quickSync:` / `quickMergeRevision:` / `quickRebaseOnRevision:` —
    the "quick variant + options dropdown" pattern. `EV:analysis/ui-structure#F17`.
44. **Master-detail splits everywhere**: 5 364 constraint nodes, 16 layout guides, outlets named
    `masterView` / `detailView` / `splitView`, `contentViewSelectionDidChange:`,
    `NonRestorableSplitViewController`. `EV:analysis/ui-structure#F18`.
45. **Quick actions**: `GTQuickActionName*` identifiers. The domain report counts 99; this build's
    own extraction finds **84 distinct names** (the report's larger figure includes non-`Name`
    prefixed variants). 20 object types (`GTQuickActionObjectName{Branch,Commit,Revision,History,
    Remote,RemoteBranch,Stash,Stashes,Submodule,Tag,TreeEntry,WorkingCopy,Worktree,PullRequests,
    Reflog,Settings,Theme,Appearance,UserProfile,BranchesReview}`). `EV:analysis/feature-scope`
    + direct grep of `tower-strings.txt`.
46. **Toolbar**: 28 `GTToolbarItemIdentifier*` in the report; 27 re-derived here (Fetch, Pull, Push,
    PushTo, ForcePush, Sync, Merge, Rebase, CreateBranch, SaveStash, ApplyStash, Clone, Refresh,
    Search, QuickActions, QuickOpen, Workflow, Services, Bookmarks, OpenInFinder, OpenInTerminal,
    Prev, Next, LicenseInfo, ParentRepository, DetailTrackingSeparator, GerritPush).
    `EV:analysis/visual-design#F13`.

### 4.5 Visual system

47. **Colors are semantic slots, not raw colors** — 89 distinct names re-derived, e.g.
    `accent_primary_color`, `general_*` (background / secondary / header / border / hover-selection /
    info / warning / alternate-row), `{blue,gray,green,red,yellow}_{badge,file_status}_{background,text}_color`,
    `selected_*_file_status_background_color`, hint backgrounds, diff tokens
    (`repository_diff_added_block_{background,outline}_color`), nine
    `<hue>_history_graph_line_color` lanes, badge/glass tints, `text_view_*`. `EV:assets/accent_primary_color`.
48. **Light/dark is per-token**: 92 DarkAqua + 85 Aqua appearance-tagged renditions; each sampled
    token carries an unspecificed base record plus explicit Aqua and/or DarkAqua records; alpha is
    used systematically for hint/diff/badge fills. `EV:assets/general_background_color`.
49. **Icon system is role-organized**: families `navbar_*` (with `_active` variants), `toolbar_icon_*`,
    `quick_actions_*`, `repository_icon_*`, `repository_sidebar_*`, `services_logo_*` /
    `services_menu_icon_*` / `services_icon_{account,repository}_*` / `services_empty_view_logo_*`,
    `empty_view_icon_*` (19), `list_header_view_*`, `sidebar_icon_folder`. 275 distinct image names.
    `EV:analysis/visual-design#F3`.
50. **A `symbol_*` glyph vocabulary names git verbs** (commit, branch, merge, merge_warning, rebase,
    rebase_warning, fetch, pull, push, sync, remote, stashes, apply_stash, save_stash, create_branch,
    force_push, quick_open, finder, git_config, git_flow, gitflow_next, worktree, branch_worktree,
    submodule, user_profile, workflow, pull_request, graphite_get, graphite_submit, missing_parent,
    archived_branch_badge, archived_branch_worktree(_badge)) alongside system-style glyph names
    (`git`, `github.fill`, `gitlab.fill`, `graphite`, `stacked-branches`, `gitflow.next`).
    `EV:analysis/visual-design#F4`.
51. **Typography is system-only for chrome**; user-configurable only for editor surfaces
    (`GTUserDefaultsEditorFont`, `...EditorFontSize`, `...WorkingCopyFilePathFontSize`,
    `GTUserDefaultsSpacesPerTab`). `EV:analysis/visual-design#F5`.
52. **Commit graph: nine-color lane palette** cycling modulo 9, commits drawn as bordered bubbles
    (`repository_history_commit_bubble_{bg,border}_color`). `EV:analysis/visual-design#F6`.
53. **Diff visuals are tokenized**: added/deleted block background+outline with alpha ≈0.15/0.40,
    image-diff checkerboard, merge-conflict tile light/dark backgrounds, unmerged-row background,
    blob line-number background; toggles "Ignore Whitespace", "Show Full Contents",
    "Syntax Coloring in Diffs", "Show invisible characters". `EV:analysis/visual-design#F7`.
54. **Themes are versioned data files**: `GTTheme`, `GTThemeV5`, `ThemeReaderV01`/`V3`,
    `ThemeV1..V5DictionaryKeys`, `ThemeV4toV5Migration`, `ThemeV5toV6Migration`,
    `ThemeMigrationManager`, `ThemeQueue`, `ThemeValidator`; import/export actions; syntax
    highlighting is CSS-theme-driven. `EV:analysis/visual-design#F8`.
55. **Material/elevation primitives**: `OpaqueView`, `RoundedShadowView`, `TintableHintView`,
    `GTBadgesView`, badge glass tints, `general_hover_selection_background_color` (alpha 0.6),
    `Secondary/TertiaryContentBackgroundColor`. `EV:analysis/visual-design#F9`.
56. **Empty / loading / large-content warning is a systematic per-list trio**: 19 `empty_view_icon_*`,
    loading cells per list, `LargeDiffWarningCellView` with copy "This is a large diff. Do you want
    to show it?". `EV:analysis/visual-design#F10`.
57. **Appearance mode and app icon are separate settings with auto modes**, reacting to OS
    accessibility display changes (`workspaceDidChangeAccessibilityDisplayOptions:`).
    `EV:analysis/visual-design#F12`.

### 4.6 Behavioural defaults observed (settings surface)

58. **190 `GTUserDefaults*` keys** re-derived (the report says 191; the ±1 is demangling artifacts).
    They cover fetch intervals, tool identifiers, editor typography, sidebar grouping, history
    display, commit-message limits and warnings, stash/pull/push dialogue options, AI commit
    settings, themes, and release channels. `EV:analysis/feature-scope`, direct grep.
59. **AI commit messages are agent-CLI based**: `AIAgentBinary*` classes, supported CLI names
    `claude`, `codex`, `gemini`, `copilot`; settings `GTUserDefaultsAIAgent`,
    `...AIAgentSupportEnabled`, `...AICommits{ContextLines,IncludeUnstagedChanges,Prompt,HideSetupButton}`;
    prompt CRUD + import/backup (`AIPromptAction`, `BackupAIPromptsOperation`, `ImportAIPromptsOperation`);
    selector `generateCommitMessage:`. `EV:analysis/feature-scope#G3`.
60. **git subcommand surface, by occurrence frequency in the binary**: branch 423, remote 143,
    merge 84, pull 51, status 48, worktree 37, submodule 37, config 32, rebase 31, diff 25, push 23,
    fetch 23, tag 20, stash 17, checkout 17, show 12, notes 10, log 10, reset 9, clone 9, revert 8,
    blame 8, describe 5, cherry-pick 5, restore 4, lfs 4, apply 4, rev-parse 3, gc 2, credential 1.
    `EV:analysis/git-operations`, MANIFEST §3.

## 5. What the evidence cannot support — restated limits

These are not caveats to be waved away; they constrain what Meridian may claim.

1. **No native pseudocode, no call graphs.** The decompiler never ran (mac-arm64 `decompile` binary
   absent from Ghidra 12.1.4; source build failed on `<bfd.h>`). Consequence: *ordering, branching,
   exact parameters and internal algorithms are unknown.* Where docs/02–04 describe sequencing, it
   is our design, marked as such, not a recovered fact.
2. **Interface Builder archives are private serialized object graphs.** The decoder reported only
   recognizable keyed-archive objects and connections; **unrecognized classes and fields remain
   unknown** (3 719 `unknown` nodes in the graph). Runtime-visible or dynamically constructed UI and
   **pixel geometry are not recoverable**.
3. **Target-action selectors are observations of the archive**; they do not prove a matching
   implementation exists in the binary.
4. **`strings` hits prove a string exists, not that the code path is reachable.** Likewise a type
   name proves the type existed in this build, not that the feature is enabled or complete.
5. **Asset-catalog inspection reports rendition metadata only.** Image bytes were not decoded;
   **the look of any icon or artwork was never observed**.
6. **Provider API frameworks are separate dylibs**; their endpoints and payload shapes are outside
   the evidence set. Services conclusions stop at main-binary names/strings.
7. **No `.strings` localization files were in scope**, so the UI-copy inventory is partial and
   incidental.
8. **Enum case lists are inferred** from associated class names and adjacent literals; exact case
   sets and defaults are not certified unless a literal was observed.
9. **Counts are counts of distinct named items**, not of verified code paths.

## 6. Corrections applied to the evidence base

Recorded so later readers do not chase ghosts:

- `assets.json` / `assets2.json` are **not** in `/tmp/rea-evidence/`; they live in `/tmp/rea-out/`
  (offset 0 and offset 1000 of one 1 698-record catalog; both files self-report `truncated: true`).
- `ui-documents.txt` prints every `outlets:`/`actions:` block as `{}`. Connections exist only in
  `ui-graph.json` (`outlet` nodes + `outlet_to` edges; `action` nodes + `target_action` edges).
  **Use the graph, not the text, for connections.**
- `ui-all-classes.txt` is a **mixed vocabulary** of class names *and* UI copy, not a pure class list.
  Items containing spaces are copy, not classes.
- The Action→Validator→Operation triad is a pattern, not an invariant (see §4.1.3).
- The quick-action identifier count differs between the report (99) and a direct name-pattern grep
  (84); §4.4.45 records both.

## 7. Things we may legitimately reuse, and things we may not

**Reuse (mechanism):** layering and naming concepts, the triad pattern, queueing/activity/progress
architecture, semantic *slot lists* for tokens and icons, and the *taxonomy* of features — all
recorded in this document. Standard git on-disk formats and libgit2 semantics are public
knowledge and are implemented by our engine, not copied from Tower.

**Do not reuse (assets):** their code, their nib files, their icons or artwork, their UI copy,
their theme files, their OAuth broker, their licensing machinery. Meridian writes its own strings,
draws (or picks) its own icons, defines its own token values, and never ships Tower binaries.

**Deliberately out of scope** (observed in Tower, not replicated): license activation/trial/device
activation, the Mixpanel/Sentry pipelines, the Sparkle update channel scheme with access tokens.
Where a substitute is needed (e.g. update checking), Meridian designs its own.

## 8. Analysis provenance

| Domain report | Path | Used by |
|---|---|---|
| feature-scope | /tmp/rea-analysis/feature-scope.md | docs/01 |
| git-operations | /tmp/rea-analysis/git-operations.md | docs/02, docs/03 |
| git-semantics | /tmp/rea-analysis/git-semantics.md | docs/02, docs/03 |
| services | /tmp/rea-analysis/services.md | docs/02, docs/03 |
| ui-structure | /tmp/rea-analysis/ui-structure.md | docs/04 |
| visual-design | /tmp/rea-analysis/visual-design.md | docs/04 |
| index | /tmp/rea-analysis/00-index.md | all |
