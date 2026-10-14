/**
 * English catalogue — the panes slice.
 *
 * One slice per area of the UI, merged into the app catalogue by
 * `locales/en.ts`. `PanesEn` is what `parts/zh-CN/panes.ts` is typed against,
 * so a key added here and forgotten there is a `vue-tsc` error rather than a
 * line that quietly renders in English.
 *
 * Conventions (the ones that cost something if forgotten)
 *  - `{name}` interpolates; vue-i18n escapes by default, nothing here is HTML.
 *  - A `_plural` twin exists only where English actually inflects
 *    ("1 file" / "2 files"). Chinese does not, so the two entries are the same
 *    sentence — the *pair* of keys must both exist for the type to line up.
 *  - Ref names, SHAs, author names and file paths are data, never translated.
 *  - The commit *list*'s own copy lives under `commitList.*` (the diff slice),
 *    not here — see `commit-list/strings.ts`, which is the only reader.
 *  - HEAD / origin stay literal; they are git vocabulary, not English.
 *
 * Terminology: branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 ·
 * checkout 签出 · remote 远程 · fetch 抓取 · pull 拉取 · push 推送 ·
 * diff 差异 · blame 溯源 · upstream 上游 · ancestor 祖先
 */

const panesEn = {
  commit: {
    // --- commit detail -----------------------------------------------------
    loadingCommit: "Loading commit…",
    noCommitSelected: "No commit selected",
    pickCommitHint: "Pick a commit from the history to see its details.",
    loadFailed: "Could not load the commit",
    copySummary: "Copy summary",
    copyHash: "Copy the full hash",
    browseFiles: "Browse files",
    browseTreeAt: "Browse the tree at {ref}",
    browseTreeHere: "Browse the tree at this commit",
    parent: "Parent",
    parents: "{count} parents",
    loadingFiles: "Loading files…",
    noChangedFiles: "No changed files",
    diffUnavailableTitle: "Commit diffs are not available yet",
    diffUnavailableCaption:
      "The command that loads one file's diff at a commit is not in this build; the changeset entry above is shown instead.",
    noFileSelected: "No file selected",
    noFileSelectedCaption: "Choose a file to see its diff.",

    // --- changeset ---------------------------------------------------------
    statusAdded: "Added",
    statusCopied: "Copied",
    statusDeleted: "Deleted",
    statusModified: "Modified",
    statusRenamed: "Renamed",
    statusUnmerged: "Unmerged",
    statusUntracked: "Untracked",
    statusIgnored: "Ignored",
    filesChanged: "{count} file",
    filesChanged_plural: "{count} files",
    binary: "binary",

    // --- signature badge ---------------------------------------------------
    signatureUnsigned: "Not signed",
    signaturePresent: "Signed (not verified)",
    signatureValid: "Signature verified",
    signatureUntrusted: "Signed by an untrusted key",
    signatureInvalid: "Signature invalid",
    signatureExpired: "Signature expired",

    // --- composer ----------------------------------------------------------
    submit: "Commit",
    amend: "Amend",
    amendTitle: "Amend commit",
    stagedCount: "{count} staged",
    unstagedCount: "{count} unstaged",
    runningHook: "Running hook",
    hooksOn: "Hooks on",
    subjectPlaceholder: "Commit subject",
    amendPlaceholder: "Amend the last commit",
    subjectLabel: "Commit subject",
    bodyLabel: "Commit body",
    bodyPlaceholder: "Describe the change (optional)",
    addDescription: "Add a description",
    unchangedMessage: "The message is unchanged since the last attempt.",
    signOff: "Sign-off",
    reasonBusy: "A commit is already running.",
    reasonNoCommitToAmend: "There is no commit to amend.",
    reasonAmendNeedsChange: "Change the message or stage some changes to amend.",
    reasonEnterSubject: "Enter a commit subject.",
    reasonStageToCommit: "Stage some changes to commit.",
  },

  history: {
    emptyTitle: "No commits",
    emptyCaption: "This repository has no history yet.",
    loading: "Reading history…",
    searchLabel: "Search commits",
    searchPlaceholder: "Search messages, authors, hashes",
    typeLabel: "Commit type",
    typeAll: "All commits",
    typeMerges: "Merge commits",
    typeNonMerges: "Non-merge commits",
    clear: "Clear",
    viewSettings: "View settings",
    sectionView: "View",
    showGraph: "Show graph",
    firstParentOnly: "First-parent only",
    sectionDensity: "Density",
    densityCompact: "Compact",
    densityRegular: "Regular",
    sectionGrouping: "Group by date",
    groupingOff: "Off",
    groupingDay: "Day",
    groupingWeek: "Week",
    groupingMonth: "Month",
    sectionSort: "Sort",
    sortDate: "Date order",
    sortTopo: "Topological",
  },

  workingCopy: {
    emptyTitle: "No local changes",
    emptyCaption: "Your working copy matches the last commit.",
    loading: "Reading the working copy…",
    refreshTitle: "Refresh the working copy",
    countStaged: "{count} staged",
    countModified: "{count} modified",
    countUntracked: "{count} untracked",
    conflictsOnly: "Conflicts only ({count})",

    // Section headers. Each one also feeds the bulk verb's tooltip through its
    // own noun, because "Stage all modified" reads as English grammar and has
    // no Chinese equivalent — see `noun*` below.
    sectionConflicts: "Conflicts",
    sectionStaged: "Staged",
    sectionModified: "Modified",
    sectionUntracked: "Untracked",
    emptyConflicts: "No conflicts",
    emptyStaged: "No staged changes",
    emptyModified: "No unstaged changes",
    emptyUntracked: "No untracked files",
    noFiles: "No files",

    stageAll: "Stage all",
    unstageAll: "Unstage all",
    bulkStageAll: "Stage all {noun}",
    bulkUnstageAll: "Unstage all {noun}",
    nounStaged: "staged changes",
    nounModified: "modified files",
    nounUntracked: "untracked files",

    stage: "Stage",
    unstage: "Unstage",
    discard: "Discard",
    discardChanges: "Discard changes",
    assumeUnchanged: "Assume unchanged",
    clearAssumeUnchanged: "Clear assume-unchanged and show this file again",
    selectFile: "Select {path}",

    mergeStatusLabel: "Merge status",
    merging: "Merging {label} — {remaining} of {total} conflicts left.",
    incomingChanges: "incoming changes",
    incomingTitle: "Incoming changes",
    conflictsResolved: "All conflicts resolved. Commit to finish the merge.",
    cleanMerge: "{label} can be merged cleanly ({commits} commit).",
    cleanMerge_plural: "{label} can be merged cleanly ({commits} commits).",
    commitMerge: "Commit merge",
    abortMerge: "Abort",

    openMergeTool: "Open the merge tool for this file",
    resolve: "Resolve",
    blame: "Blame",
    blameThisFile: "Blame this file",
    openInEditor: "Open in the default editor",
    conflictNote: "This file has conflicts. Hunk staging is disabled until it is resolved.",
    noFileSelected: "No file selected",
    noFileSelectedCaption: "Choose a file from the list to see its changes.",
  },

  branch: {
    emptyTitle: "No branches",
    emptyCaption: "Create a branch to start working.",
    emptyAction: "New branch…",
    newBranch: "New branch",
    loading: "Reading branches…",
    scopeLabel: "Branch scope",
    filterLocal: "Local",
    filterRemote: "Remote",
    filterAll: "All",
    searchLabel: "Search branches",
    searchPlaceholder: "Filter branches",
    tracking: "Tracking {ref}",
    merged: "Merged",
    copyName: "Copy the branch name",
    noSelectionTitle: "No branch selected",
    noSelectionCaption: "Pick a branch to see its actions.",
    factTip: "Tip",
    factUpstream: "Upstream",
    factLastAuthor: "Last author",
    factTracking: "Tracking",
    notTracking: "Not tracking",
    aheadBy: "Ahead by {count}",
    behindBy: "Behind by {count}",
    checkout: "Check out",
    checkedOut: "Checked out",
    mergeInto: "Merge into {name}",
    mergeIntoCurrent: "Merge into current",
    rebaseOnto: "Rebase onto",
    push: "Push",
    fetchIntoLocal: "Fetch into local",
    showInHistory: "Show in history",
    compare: "Compare…",
    rename: "Rename",
    delete: "Delete",
    reasonCheckedOut: "You cannot delete the checked-out branch.",
    reasonProtected: "This branch is protected.",
  },

  compare: {
    baseSide: "Base",
    compareSide: "Compare",
    chooseRef: "Choose a ref",
    swapTitle: "Swap base and target",
    noCommonAncestor: "No common ancestor",
    mergeBase: "merge base {base}",
    onlyIn: "In {ref} only",
    targetFallback: "target",
    baseFallback: "base",
    emptyTitle: "Choose two refs",
    emptyCaption: "Pick a base and a target to compare.",
    loading: "Comparing…",
  },

  blame: {
    showDiff: "Show diff",
    open: "Open",
    loading: "Blaming…",
    emptyTitle: "Nothing to blame",
    emptyCaption: "This file has no committed history.",
    annotationLabel: "Blame annotation",
    showCommit: "Show commit",
    copyHash: "Copy hash",
  },
};

export default panesEn;
// Deliberately not `as const`: literal types would force every Chinese string
// to equal its English counterpart. The structural shape is the contract.
export type PanesEn = typeof panesEn;
