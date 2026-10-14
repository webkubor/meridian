/**
 * Catalogue slice: diff viewer, commit list, commit graph and the App.vue
 * chrome ("shell").
 *
 * These four namespaces live in one file because they are one change: the diff
 * viewer, the commit list and the shell were three modules that each kept their
 * own English literals, and this slice is the single place their copy now comes
 * from. Splitting it further would spread one review across four files that
 * have to stay type-compatible with each other (the commit row's label is
 * assembled from a `commitList` key and a `graph` key).
 *
 * Deliberately NOT `as const`, matching `locales/en.ts`: literal types would
 * force every Chinese string to equal its English counterpart.
 *
 * Pluralisation: vue-i18n's `|` forms (`no lines | one line | {count} lines`).
 * Chinese has no plural inflection, so `zh-CN/diff.ts` writes a single form that
 * interpolates `count` — the same string then reads correctly for 0, 1 and n.
 *
 * Three entries are byte-identical in both catalogues ON PURPOSE, and the parity
 * test needs them in its `UNTRANSLATED_BY_DESIGN` allowlist:
 *  - `diff.hunkStats`, `commitList.overflowBadge` — `+12 −3` summaries. The
 *    signs are the information; the spec requires the same characters in both
 *    languages, and `+`/`−` are not words.
 *  - `shell.settings.git` — the name of the tool, like "Meridian".
 * Anything else that ends up identical is an untranslated string, not a claim.
 *
 * Terminology: diff 差异 · hunk 变更块 · file 文件 · line 行 · added 新增 ·
 * deleted 删除 · context 上下文 · word diff 按词差异 · whitespace 空白 ·
 * branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 · checkout 签出 ·
 * remote 远程 · fetch 抓取 · pull 拉取 · push 推送 · stash 储藏 · tag 标签 ·
 * repository 仓库 · graph 提交图 · lane 分道 · bubble 节点 · ref 引用.
 */

const diffEn = {
  diff: {
    regionLabel: "File diff",
    loading: "Loading diff…",
    retry: "Retry",
    failedTitle: "The diff could not be loaded",
    emptyTitle: "No changes",
    emptyBody: "This version is identical to the one it is compared against.",
    noHunks: "The file has no textual changes to show.",

    // Toolbar
    toolbarLabel: "Diff options",
    layoutLabel: "Diff layout",
    viewLabel: "Layout",
    viewUnified: "Unified",
    viewSplit: "Split",
    contextLabel: "Context",
    contextAria: "Context lines",
    contextFull: "Whole file",
    contextLines: "no lines | {count} line | {count} lines",
    ignoreWhitespace: "Ignore whitespace",
    ignoreWhitespaceShort: "Whitespace",
    showFullFile: "Show whole file",
    showFullFileShort: "Full file",
    syntaxColouring: "Syntax colouring",
    syntaxShort: "Syntax",
    wordDiff: "Word-level changes",
    wrap: "Wrap long lines",

    // Hunk chrome
    hunkLabel: "Hunk {header}",
    collapseHunk: "Collapse hunk",
    expandHunk: "Expand hunk",
    // Identical in zh-CN by design: `+12 −3` — see the file header.
    hunkStats: "+{added} −{removed}",
    stage: "Stage",
    unstage: "Unstage",
    discard: "Discard",
    stageThisHunk: "Stage this hunk",
    unstageThisHunk: "Unstage this hunk",
    discardThisHunk: "Discard this hunk",
    stageHunk: "Stage hunk",
    unstageHunk: "Unstage hunk",
    discardHunk: "Discard hunk",
    hunkBusy: "Working…",
    controlsDisabledReason: "Resolve the conflict before staging this file.",
    controlsDisabledReasonHunks: "Resolve the conflict before staging hunks.",

    // Gaps and elision (never silently truncate)
    expandAbove: "no hidden lines | Expand {count} hidden line | Expand {count} hidden lines",
    expandAboveUnknown: "Show more context",
    expandBelow: "Show more context below",
    hiddenBannerTitle: "Part of this diff is not shown",
    hiddenBannerBody: "Showing {shown} of {total} lines.",
    showAll: "Show all",
    // `count` picks the plural form; `lines` cannot pick one of its own inside
    // the same message, so it stays a plain number.
    backendElided:
      "The diff engine stopped early: {count} hunk ({lines} lines) was not generated. | The diff engine stopped early: {count} hunks ({lines} lines) were not generated.",
    requestFull: "Request full diff",
    hunkClipped: "This hunk is cut off by the render limit.",
    renderLimitNote:
      "Long diffs render the first {count} lines; nothing is hidden without this notice.",

    // Binary and modes
    binaryTitle: "Binary file",
    binaryNone: "This file has no textual diff.",
    binaryIdentical: "Both sides are byte-identical; only metadata differs.",
    binaryChanged: "The binary contents differ.",
    binaryOldSize: "Before: {size}",
    binaryNewSize: "After: {size}",
    before: "Before",
    after: "After",
    binaryOid: "{side} blob {oid}",
    modeChanged: "File mode changed from {from} to {to}.",
    modeAdded: "File created with mode {to}.",
    modeRemoved: "File deleted (was mode {from}).",

    // Image
    imageLoading: "Loading image data…",
    imageFailed: "Loading image data failed",
    imageUnavailable: "No image for this side",
    imageChecker: "Transparent areas are shown over a checkerboard.",

    // Renames
    renamedFrom: "Renamed from {path}",

    // Accessibility (docs/04 §9: per-row labels)
    lineLabel: "{where}, {role}: {text}",
    lineNumber: "line {number}",
    lineNumberUnknown: "line —",
    roleContext: "unchanged",
    roleAdded: "added",
    roleRemoved: "removed",
    roleNoNewline: "end of file",

    // States the viewer renders itself
    loadingAria: "Loading diff",
    tryAgain: "Try again",
    emptyPathTitle: "No changes to show",
    emptyNoFileTitle: "No file selected",
    emptyPathCaption: "This file is unchanged between the two sides of the comparison.",
    emptySelectCaption: "Select a file to see its diff.",
    gateTitle: "This diff is large",
    gateCaption: "{size} — above the {limit} automatic limit.",
    gateAction: "Show anyway",
    binaryCaption: "There is no textual diff for this file.",
    imageTitle: "Image diff",
    // Not git's literal `\ No newline at end of file`: this is a UI note in the
    // gutter, not a copy of the engine's stdout, so it is localised. The hunk
    // header itself (`@@ -1,4 +1,6 @@`) stays verbatim because it is data.
    noNewline: "\\ No newline at end of file",
  },

  commitList: {
    // The four entries below (`listLabel`, `listHint`, `overflowBadge`,
    // `skeletonsLabel`) are the commit list's own copy — the list chrome and
    // the overflow chip — read today through `commit-list/strings.ts`. They
    // live here so that file becomes a `t(...)` call site rather than a second
    // place copy is written.
    listLabel: "Commit history",
    listHint: "Use the arrow keys to move the selection.",
    today: "Today",
    yesterday: "Yesterday",
    weekOf: "Week of {date}",
    rowLabel: "{summary} — {author}, {when}",
    // Identical in zh-CN by design — see the file header.
    overflowBadge: "+{count}",
    skeletonsLabel: "Loading commits",
    skeletonAria: "Loading",
  },

  graph: {
    // What a row's topology is, for the row's accessible name. The canvas cell
    // is `aria-hidden`, so this is the only place the shape of the graph is
    // announced — the painted lanes and bubbles are not.
    merge: "merge commit",
    forkPoint: "fork point",
  },

  shell: {
    subtitle: {
      settings: "Settings",
      account: "Accounts",
      about: "About Meridian",
      repositories: "Repositories",
      on: "On {branch}",
    },

    sidebar: {
      workingCopy: "Working copy",
      history: "History",
      branches: "Branches",
      remotes: "Remotes",
      tags: "Tags",
      stashes: "Stashes",
      emptyWorkingCopy: "No local changes",
      emptyHistory: "No commits",
      emptyBranches: "No branches",
      emptyRemotes: "No remote branches",
      emptyTags: "No tags",
      emptyStashes: "No stashes",
      rowWorkingCopy: "Local changes",
      rowHistory: "All commits",
    },

    toolbar: {
      branch: "Branch",
      fetch: "Fetch",
      pull: "Pull",
      push: "Push",
      stash: "Stash",
      quickOpen: "Quick open",
      quickActions: "Quick actions",
      settings: "Settings",
      account: "Accounts",
      about: "About Meridian",
      newBranch: "New branch…",
      checkoutBranch: "Check out branch…",
      renameBranch: "Rename branch…",
      noRepository: "Open a repository first.",
      noRemote: "This repository has no remote configured.",
      detachedHead: "HEAD is detached — check out a branch to push.",
      activityUnavailable: "Activity tracking is not available in this build.",
    },

    // Workspace tabs. Also read by `router/routes.ts` as `labelKey`, so the
    // route table never becomes the place a label lives.
    tab: {
      workingCopy: "Working copy",
      history: "History",
      branches: "Branches",
      compare: "Compare",
    },

    palette: {
      refreshRepository: "Refresh repository",
      openSettings: "Open settings",
      openAccounts: "Open accounts",
      repositoryList: "Repository list",
      groupBranches: "Branches",
      groupRepository: "Repository",
      groupRepositories: "Repositories",
      groupApplication: "Application",
      groupViews: "Views",
      headingActions: "Quick actions",
      headingOpen: "Quick open",
      placeholderActions: "Run a command",
      placeholderOpen: "Search repositories, branches and views",
      empty: "No matches",
    },

    settings: {
      appearance: "Appearance",
      editor: "Editor",
      editorFontSize: "Editor font size",
      editorFontSizeHelp: "Applies to diff and blame views.",
      git: "Git",
      safety: "Safety",
      confirmCheckout: "Confirm before checking out with local changes",
      confirmCheckoutHelp: "Shows a confirmation whenever the working copy is dirty.",
      general: "General",
      services: "Services",
      advanced: "Advanced",
    },

    dialogue: {
      newBranchTitle: "New branch",
      newBranchLabel: "Branch name",
      newBranchHelp: "The branch starts at the current HEAD.",
      newBranchCheckoutOption: "Check out the new branch",
      renameBranchTitle: "Rename branch",
      renameBranchLabel: "New name",
      deleteBranchTitle: "Delete branch?",
      deleteBranchMessage: "Delete {branch}? This cannot be undone.",
      deleteBranchConfirm: "Delete branch",
      checkoutTitle: "Check out branch",
      checkoutDirtyTitle: "Check out with local changes?",
      checkoutDirtyMessage:
        "You have {count} uncommitted change. Checking out may fail or carry them over. | You have {count} uncommitted changes. Checking out may fail or carry them over.",
      checkoutDirtyConfirm: "Check out anyway",
      discardTitle: "Discard changes?",
      discardMessage: "This permanently discards the changes in this file. It cannot be undone.",
      discardConfirm: "Discard changes",
    },

    // The split action under the commit button.
    commitAndPush: "Commit & Push",

    toast: {
      linkFailed: "Could not open that link.",
      noBranchSelected: "Select a branch first.",
      feedbackOpened: "Opened GitHub with {count} detail filled in. | Opened GitHub with {count} details filled in.",
      revealed: "Revealed in the file manager.",
      copied: "Copied to the clipboard.",
      copyFailed: "Could not copy to the clipboard.",
      actionFailed: "The action failed.",
      fetched: "Fetched.",
      pulled: "Pulled.",
      pushed: "Pushed {branch}.",
      stashed: "Stashed the working copy.",
      branchCreated: "Created branch {branch}.",
      branchCreateFailed: "Could not create the branch.",
      branchRenamed: "Renamed to {branch}.",
      branchRenameFailed: "Could not rename the branch.",
      merged: "Merged {branch}.",
      rebased: "Rebased onto {branch}.",
      branchDeleted: "Deleted {branch}.",
      branchDeleteFailed: "Could not delete the branch.",
      checkedOut: "Checked out {branch}.",
      checkoutFailed: "Could not check out the branch.",
      discarded: "Discarded the file's changes.",
      discardFailed: "Could not discard the changes.",
      committed: "Committed.",
      commitFailed: "Could not create the commit.",
      committedAndPushed: "Committed and pushed.",
      conflictResolved: "Conflict resolved.",
      commitMessageRequired: "Write a commit message first.",
      operationContinued: "Operation continued.",
      operationAborted: "Operation aborted.",
      mergeContinued: "Merge continued.",
      mergeAborted: "Merge aborted.",
    },

    unavailable: {
      addSection: "Adding {noun} is not available in this build.",
      nounEntries: "entries",
      nounWorkingCopy: "working-copy entries",
      nounCommits: "commits",
      nounRemotes: "remotes",
      nounTags: "tags",
      nounStashes: "stashes",
      historyMergeFilter: "Filtering by merge status is not available in this build.",
      browseFolder: "Choosing a folder is not available in this build.",
      clone: "Cloning is not available in this build.",
      scanNearby: "Scanning for repositories is not available in this build.",
      compare: "Comparing revisions is not available in this build.",
      accounts: "Connecting and managing hosting accounts is not available in this build.",
    },
  activity: {
    title: "Activity",
    subtitle: "What Meridian is doing for you right now.",
    running: "{count} operation running | {count} operations running",
    idle: "Nothing running.",
    state: {
      running: "Running",
      done: "Done",
      failed: "Failed",
    },
    clear: "Clear finished",
    nothingToClear: "Nothing has finished yet.",
    empty: "No operations yet",
    emptyCaption: "Fetching, pulling, pushing and cloning show up here while they run.",
    privacy:
      "This list is built from what you clicked, in this window. Nothing is written to disk and nothing is sent anywhere.",
  },

  },
};

export type DiffEn = typeof diffEn;

export default diffEn;