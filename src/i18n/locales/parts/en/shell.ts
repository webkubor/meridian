/**
 * English catalogue slice — layout, screens, dialogues, palette.
 *
 * Split out of `locales/en.ts` by *surface*, not by word: `src/layout/**`,
 * `src/components/screens/**`, `src/components/dialogues/**` and
 * `src/components/palettes/**` are the files this slice describes, so a
 * missing key points at the component that wanted it.
 *
 * Conventions inherited from `en.ts`:
 *  - `{name}` interpolates; vue-i18n escapes by default.
 *  - Product and vendor names (Meridian, GitHub, Bitbucket Server, HEAD) stay
 *    inline in the component rather than becoming keys: a catalogue entry whose
 *    two languages are byte-identical is noise, and `catalog.test.ts` reads a
 *    duplicate as an untranslated string.
 *
 * Two non-obvious choices, both deliberate:
 *  - `screens.operation.*` is shared by `RepositoryScreen` and `RepoSwitcher`.
 *    Both render the same git operation-state word, and two namespaces for one
 *    word would let the two surfaces drift apart.
 *  - `screens.choose` is likewise shared: "Choose…" is the same button in
 *    Settings and on the launch screen, and one word should be edited once.
 *
 * `@` is a reserved character in vue-i18n messages (a linked-message marker), so
 * a literal one is written `{'@'}`, and a literal brace `{'{'}` — the refname
 * errors quote git's own forbidden characters.
 */

const shell = {
  toolbar: {
    ariaLabel: "Repository actions",
    empty: "No repository actions available.",
    activity: "Activity",
    activityOpen: "Open the activity list",
    stalled: "Stalled",
  },

  sidebar: {
    repositoryLabel: "Repository",
    filterLabel: "Filter the sidebar",
    filterPlaceholder: "Filter",
    switchRepository: "Switch repository",
    noRepositoryOpen: "No repository open",
    chooseRepository: "Choose repository",
    recentRepositories: "Recent repositories",
    noOtherRepositories: "No other repositories.",
    allRepositories: "All repositories…",
    rowOptions: "Options for {name}",
    addToSection: "Add to {name}",
  },

  screens: {
    choose: "Choose…",
    operation: {
      state: {
        merging: "Merging",
        rebasing: "Rebasing",
        cherryPicking: "Cherry-picking",
        reverting: "Reverting",
        bisecting: "Bisecting",
        applyingPatches: "Applying patches",
        inProgress: "Operation in progress",
      },
      bannerHint: "{state}. Finish or abort the operation to continue.",
      continue: "Continue",
      abort: "Abort",
    },
    masterDetail: {
      resizeLabel: "Resize the detail pane",
    },
    toast: {
      // The toast body and its action label come from the caller that pushed
      // it; only this frame's own affordance is the component's to name.
      dismissNotification: "Dismiss notification",
    },
    settings: {
      title: "Settings",
      sectionsLabel: "Settings sections",
      shellChip: "Shell",
      searchPlaceholder: "Search settings",
      searchLabel: "Search settings",
      loading: "Loading settings…",
      tryAgain: "Try again",
      emptyTitle: "Nothing to configure yet",
      emptyCaption: "Settings become available as the backend commands land.",
      theme: "Theme",
      appearance: "Appearance",
      appearanceHelp: "Follows the system unless you pick a theme.",
      appearanceAuto: "Auto",
      appearanceLight: "Light",
      appearanceDark: "Dark",
      reset: "Reset",
      resetTitle: "Reset to the default",
      noMatchingGroup: "No matching settings.",
      noMatchingTitle: "No matching settings",
      nothingMatches: "Nothing matches “{query}”.",
    },
    account: {
      title: "Accounts",
      add: "Add account",
      loading: "Loading accounts…",
      emptyTitle: "No accounts connected",
      emptyCaption: "Connect a hosting service to browse remotes and pull requests.",
      notConnected: "Not connected",
      noneSelected: "No account selected",
      noneSelectedCaption: "Pick an account to see its details.",
      signOut: "Sign out",
      connect: "Connect",
      repositories: "Repositories",
      organizations: "Organizations",
      keys: "Keys",
      noKeys: "No keys stored for this account.",
      copyKeyId: "Copy the key id",
      remove: "Remove account",
      providers: {
        other: "Other service",
      },
    },
    repository: {
      tabsLabel: "Repository view",
      // The four tab names live in `router/routes.ts` (`WORKSPACE_TAB_LABEL_KEYS`)
      // so the command switch and this tab bar share one name per tab.
      noRepositoryOpen: "No repository open",
      noRepositoryCaption: "Open a repository from the repository list.",
    },
    repositories: {
      modeGroupLabel: "Repository action",
      open: "Open",
      clone: "Clone",
      create: "Create",
      urlLabel: "Repository URL to clone",
      newFolderPlaceholder: "A folder for the new repository",
      existingFolderPlaceholder: "A folder that contains a repository",
      newFolderLabel: "Folder for the new repository",
      openFolderLabel: "Folder to open",
      chooseFolder: "Choose a folder",
      recentsTitle: "Recent repositories",
      filterLabel: "Filter recent repositories",
      filterPlaceholder: "Filter",
      loadingRecents: "Loading recent repositories…",
      noRecents: "No recent repositories",
      noRecentsCaption: "Open, clone or create a repository to get started.",
      reveal: "Reveal in Finder",
      removeRecent: "Remove from recents",
      nearbyTitle: "Nearby repositories",
      scanAgain: "Scan again",
      nothingNearby: "Nothing found nearby",
      nothingNearbyCaption: "Repositories in the folders you have granted will appear here.",
      scanning: "Scanning for repositories…",
    },
  },

  dialogues: {
    prompt: {
      confirmLabel: "Create",
      errors: {
        valueRequired: "Enter a value.",
        nameRequired: "Enter a name.",
        nameAt: "A name cannot be just \"{'@'}\".",
        nameStartsWithDash: "Names cannot start with \"-\".",
        nameSlash: "Names cannot start or end with \"/\".",
        nameDot: "Names cannot start or end with \".\".",
        nameLock: "Names cannot end with \".lock\".",
        nameDoubleDot: "Names cannot contain \"..\".",
        nameDoubleSlash: "Names cannot contain \"//\".",
        nameAtBrace: "Names cannot contain \"{'@'}{'{'}\".",
        nameInvalidChars:
          "Names cannot contain spaces, ~ ^ : ? * [ \\ or control characters.",
      },
    },
    chooseBranch: {
      confirmLabel: "Choose",
      filterLabel: "Filter branches",
      filterPlaceholder: "Filter branches",
      listLabel: "Branches",
      empty: "No branches match “{query}”.",
    },
  },

  palette: {
    label: "Quick open",
    queryLabel: "Quick open query",
    resultsLabel: "Results",
    placeholder: "Search…",
    empty: "No matches",
  },
};

export default shell;
export type ShellEn = typeof shell;