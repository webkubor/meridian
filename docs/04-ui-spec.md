# 04 — UI Specification

Screen-by-screen specification for the Meridian web UI, derived from the `ui-structure` and
`visual-design` evidence and implemented with **our own tokens, our own values, our own copy and our
own artwork**. The evidence constrains *structure* (what panes exist, how they are composed, which
states are modelled); it does not supply pixel geometry, motion or artwork — those are design
decisions recorded here and nowhere claimed as observations.

**What the evidence gives us, and what it does not.** Structure: one archive per pane, master-detail
everywhere, a sidebar whose section vocabulary is fixed, a cell-per-state convention, a shared
dialogue chrome, two palette windows, a tabbed preferences window (`EV:analysis/ui-structure#F1`–#F18).
Visual system: *semantic slot lists* for colours and icons, a nine-colour graph palette, a
diff-token set, empty/loading/large-warning states, and a versioned theme file format
(`EV:analysis/visual-design#F1`–#F13). Not available: any pixel geometry, any artwork bytes, any
animation timing. Where this document says "28 px", it is OURS.

---

## 1. Design principles

1. **Pane = component.** Each pane from the screen list (docs/01 §4) is one route-level component
   with its own store slice; the shell composes them. (Mirrors the one-nib-per-pane structure.)
2. **State is explicit.** Every asynchronous list renders exactly four states: `loading`, `empty`,
   `error`, `data` — plus the orthogonal `large-content warning` state where content is big
   (diff/list). The evidence shows this trio as a systematic convention; we make it a component
   contract (`<ListShell>`), so no list can forget a state.
3. **One dominant action per row, options behind a caret.** The observed `quickPull:` /
   `quickPush:` / `quickSync:` family is the pattern: default action on click, options dropdown on
   the caret, full dialogue when the user opens options. Implemented once as `<SplitAction>`.
4. **Destructive verbs are two-step and amber/red.** Discard, delete branch, force push, reset,
   hard reset, remove worktree, remove submodule: the confirm names the exact object and the exact
   loss. No typed confirmation (evidence shows simple confirms); the guard is that the primary
   button in a destructive confirm is styled `danger` and never has focus by default.
5. **Work is visible.** Any command over ~300 ms opens an activity chip in the sidebar footer;
   the Activity window shows list + detail; cancels are always reachable.
6. **Nothing blocks.** The UI never spins on a modal for network work; dialogs validate and hand
   off to activities.
7. **A11y is a gate, not a polish pass.** Full keyboard operation, visible focus, ≥4.5:1 text
   contrast in both themes, reduced-motion honoured. (Our standard; Tower's a11y is not observable.)

## 2. Design tokens

Tokens are CSS custom properties on `:root` with a `.theme-dark` override, exposed through a
`design/tokens.css`. Token **names** are ours; the **slot inventory** below is mapped to the
evidence's semantic slots so that no state found in the evidence lacks a token.

### 2.1 Colour slots (ours) and the evidence slot they cover

| Our token | Role | Evidence slot(s) covered |
|---|---|---|
| `--md-bg-canvas` | outermost window background | `general_background_color` |
| `--md-bg-surface` | panes, list backgrounds | `general_secondary_background_color` |
| `--md-bg-sunken` | wells, inactive areas, blob/line-number strips | `repository_blob_text_linenumber_background_color`, `text_view_off_page_background_color` |
| `--md-bg-header` | pane headers, list headers, toolbars | `general_header_background_color`, `general_secondary_header_background_color` |
| `--md-bg-alt-row` | alternating rows | `general_alternate_row_background_color` |
| `--md-bg-hover` | hover/selection hover | `general_hover_selection_background_color` |
| `--md-bg-selected` / `--md-bg-selected-muted` | selected row / selected-unfocused | `selected_*`, `repository_pullrequests_conversation_selection_color` |
| `--md-border` / `--md-border-strong` / `--md-border-inactive` | dividers, control borders, inactive window | `general_border_color`, `general_inactive_border_color`, `text_view_page_separator_color` |
| `--md-fg` / `--md-fg-muted` / `--md-fg-subtle` / `--md-fg-disabled` | text hierarchy | `text_view_{primary,secondary}_disabled_text_color` + observed text colours |
| `--md-accent` / `--md-accent-strong` / `--md-accent-contrast` | primary action colour | `accent_primary_color` |
| `--md-accent-secondary` | secondary accent (links in code, graph "you are here") | `accent_secondary_color` |
| `--md-success` / `--md-warning` / `--md-danger` / `--md-info` | semantic fills (85% alpha for badges) | `success_tint_color`, `warning_tint_color`, hint backgrounds, `red_badge_background_color` |
| `--md-bg-info-soft` / `-warning-soft` | hint banners (translucent) | `general_info_background_color`, `general_warning_bg_color`, `*HintBackgroundColor` |
| `--md-status-added-bg/-fg` … `--md-status-ignored-bg/-fg` | 7 file-status families (added/renamed, copied, modified, deleted, untracked, unmerged, ignored) | `{blue,gray,green,red,yellow}_file_status_{background,text}_color` + `default_file_status_text_color` |
| `--md-badge-bg` / `--md-badge-fg` / `--md-badge-selected-bg/-fg` | sidebar badges (counts, ahead/behind) | `sidebarBadgeBackgroundColor`, `general_sidebar_badge_bg_color`, `{red,yellow}_badge_glass_tint_color` |
| `--md-graph-lane-1..9` | commit-graph lanes (cycle modulo 9) | nine `*_history_graph_line_color` tokens |
| `--md-graph-bubble-bg` / `--md-graph-bubble-border` | commit bubbles | `repository_history_commit_bubble_{bg,border}_color` |
| `--md-diff-added-bg` / `-outline` / `--md-diff-deleted-bg` / `-outline` / `--md-diff-word-*` | diff blocks (alpha ~0.14 bg / ~0.38 outline) | `repository_diff_added_block_*`, `repository_diff_deleted_block_*` |
| `--md-diff-checker-a` / `-b` | image-diff checkerboard | `repository_diff_image_checkerboard_*` |
| `--md-conflict-tile` / `-tile-strong` | conflict block background | `repository_mergeconflict_tile_background_{light,dark}_color`, `repository_workingcopy_unmerged_item_bg_color` |
| `--md-shadow-1` / `-2` | elevation | `RoundedShadowView` mechanism (class observed; values ours) |

Values (ours — light `:root`, dark `.theme-dark`):

```
:root {
  --md-bg-canvas:#FAF9F7; --md-bg-surface:#FFFFFF; --md-bg-sunken:#F1EFEA; --md-bg-header:#F5F3EF;
  --md-bg-alt-row:#FBFBF9; --md-bg-hover:#EDEAE4; --md-bg-selected:#DFE7FB; --md-bg-selected-muted:#E9EDF7;
  --md-border:#E2DED6; --md-border-strong:#CFC9BE; --md-border-inactive:#EDEAE4;
  --md-fg:#1B1A17; --md-fg-muted:#6E6960; --md-fg-subtle:#948E83; --md-fg-disabled:#B7B1A6;
  --md-accent:#2F5BE0; --md-accent-strong:#2447B4; --md-accent-contrast:#FFFFFF; --md-accent-secondary:#0E8FA8;
  --md-success:#2E9E5B; --md-warning:#B57000; --md-danger:#C43D2E; --md-info:#2F5BE0;
  --md-bg-info-soft:rgba(47,91,224,.08); --md-bg-warning-soft:rgba(181,112,0,.10);
  --md-graph-lane-1:#2F5BE0; --2:#0E8FA8; --3:#2E9E5B; --4:#8A7300; --5:#C46A1E;
  --6:#C43D2E; --7:#B4437E; --8:#7A4FBD; --9:#4C5CB8;   /* full set incl. light/dark variants in tokens.css */
  --md-diff-added-bg:rgba(46,158,91,.14);  --md-diff-added-outline:rgba(46,158,91,.38);
  --md-diff-deleted-bg:rgba(196,61,46,.14); --md-diff-deleted-outline:rgba(196,61,46,.38);
}
.theme-dark {
  --md-bg-canvas:#17191C; --md-bg-surface:#1E2126; --md-bg-sunken:#121417; --md-bg-header:#24272C;
  --md-border:#33373E; --md-border-strong:#454A52;
  --md-fg:#E9E7E2; --md-fg-muted:#A5A199; --md-fg-subtle:#7E7A72;
  --md-accent:#6C8BFF; --md-accent-strong:#8AA2FF; --md-accent-contrast:#10131A;
  --md-success:#3FBF74; --md-warning:#E5A32B; --md-danger:#F0715F;
  /* …full list in tokens.css; every light token has a dark counterpart, no exceptions */
}
```

### 2.2 Typography

| Token | Value | Use |
|---|---|---|
| `--md-font-ui` | `system-ui, -apple-system, "SF Pro Text", sans-serif` | all chrome (evidence: system font only, three weights) |
| `--md-font-mono` | `ui-monospace, "SF Mono", Menlo, monospace` | diffs, blob, blame, hash rendering |
| `--md-text-xs/sm/md/lg` | 11 / 12 / 13 / 15 px | captions/badges, list rows, body, pane titles (11 px is the macOS small-control size; final sizes validated against platform metrics, our choice) |
| `--md-leading-tight/-normal` | 1.25 / 1.45 | list rows / prose |
| User overrides | `editor.font`, `editor.font-size`, `working-copy.path-font-size`, `editor.tab-width` | diff, blob, commit editor — scoped to editor surfaces only (evidence F5) |

### 2.3 Spacing, radii, density

- Spacing scale `--md-space-1..6` = 4 / 8 / 12 / 16 / 24 / 32 px.
- Radii: `--md-radius-sm` 5 px (badges), `--md-radius-md` 8 px (rows, buttons), `--md-radius-lg` 12 px (panes, dialogs).
- Row heights: compact 24 px / regular 28 px / comfortable 34 px; the history pane's small/medium
  densities map to compact/regular (evidence: two commit-cell densities).
- Chart/table paddings standardised to `--md-space-2` horizontal.

### 2.4 Elevation

Three levels only: `0` (flat), `1` (raised: popovers, carets), `2` (floating: dialogues, palettes).
Shadows are two-layer (1 px hairline + soft blur), theme-aware, and never the only affordance.

### 2.5 Graph rendering spec (mechanism from `EV:analysis/git-semantics#F7`)

- Lanes are computed in Rust (`history_graph_compute`) as segments `{lane, from_row, to_row, kind:
  Straight|Merge|Fork|Bend}`; the UI draws only.
- Lane width 14 px (regular density) / 10 px (compact); bubble radius 4 px with 1 px border.
- Colour = `--md-graph-lane-$((lane % 9) + 1)`; the nine hues are ours, the modulo-9 cycling is the
  evidence's mechanism.
- Fork-point rows and date-group rows are separate row kinds with their own styles.

## 3. Icon system

Role taxonomy (mirrors the observed families): `nav.*`, `toolbar.*`, `action.*` (quick actions /
row verbs), `repo.*` (object-type icons), `badge.*`, `empty.*`, `provider.*`.

Rendering rules:

- Chrome icons are SF Symbols via `<Icon name>` with our own role registry (role → symbol +
  weight + scale). Using system-provided symbols is not asset copying; the *mapping* is ours.
- Every role has a fallback symbol and a text fallback for symbol-less contexts (menus render
  labels anyway).
- Provider logos are **our own neutral marks** (monogram tiles in the provider's brand hue) —
  we do not copy provider artwork or Tower's `services_logo_*` renditions.

Role registry (excerpt; full table in `design/icons.ts`):

| Role | Symbol | Role | Symbol |
|---|---|---|---|
| `nav.repositories` | `square.grid.2x2` | `action.commit` | `checkmark.circle` |
| `nav.history` | `clock` | `action.branch` | `arrow.triangle.branch` |
| `nav.branch` | `arrow.triangle.branch` | `action.merge` | `arrow.triangle.merge` |
| `nav.workingCopy` | `doc.text` | `action.rebase` | `arrow.triangle.2.circlepath` |
| `nav.stashes` | `shippingbox` | `action.cherryPick` | `arrow.down.doc` |
| `nav.tags` | `tag` | `action.revert` | `arrow.uturn.backward` |
| `nav.submodule` | `square.on.square` | `action.fetch` | `tray.and.arrow.down` |
| `nav.worktree` | `square.stack.3d.up` | `action.pull` | `arrow.down.circle` |
| `nav.reflog` | `clock.arrow.circlepath` | `action.push` | `arrow.up.circle` |
| `nav.services` | `cloud` | `action.forcePush` | `arrow.up.circle.badge.exclamationmark` |
| `nav.settings` | `gearshape` | `action.sync` | `arrow.triangle.2.circlepath` |
| `nav.pullRequests` | `arrow.triangle.pull` | `action.stash` | `shippingbox` |
| `badge.unpushed` | `arrow.up` | `action.tag` | `tag` |
| `badge.unpulled` | `arrow.down` | `action.worktree` | `square.stack.3d.up` |
| `badge.stale` | `moon.zzz` | `action.submodule` | `square.on.square` |
| `badge.fullyMerged` | `checkmark.circle.fill` | `action.sign` | `signature` |
| `badge.conflict` | `exclamationmark.triangle` | `action.snapshot` | `camera` |
| `action.aiMessage` | `wand.and.stars` | `action.quickOpen` | `magnifyingglass` |

Empty states (`empty.*`): 14 roles from the evidence's 19 names — our set covers: no repository
selected, no branch selected, no commit selected, no file selected, multi-selection, no pull
request selected, no reflog entry, no stash selected, unchanged file, invalid submodule, cloning
queue, drop folder, fully merged branch, services empty. Each is a small line-drawn mark in
`--md-fg-subtle` plus a one-line caption + optional primary action.

## 4. Window and shell layout

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ Toolbar zone: [global verbs] … [quick open] [quick actions] [activity] […]     │
├──────────────┬────────────────────────────────────────────────────────────────┤
│ Sidebar      │  Pane header (title, base branch, list-header verbs)           │
│  bookmarks   ├──────────────────────────┬─────────────────────────────────────┤
│  sections:   │  Master list (virtual)   │  Detail                                  │
│   branches   │                          │                                         │
│   remotes    │                          │                                         │
│   tags       │                          │                                         │
│   stashes    │                          │                                         │
│   submodules │                          │                                         │
│   worktrees  │                          │                                         │
│   reflog     │                          │                                         │
│  ──────────  │                          │                                         │
│ footer:      │                          │                                         │
│  activity •  │                          │                                         │
│  settings    │                          │                                         │
└──────────────┴──────────────────────────┴─────────────────────────────────────┘
```

- **Toolbar** holds global verbs (observed identifier set, our own icons): fetch, pull, push,
  push-to, force-push, sync, merge, rebase, create-branch, save-stash, apply-stash, clone, refresh,
  search (in-pane), quick-open, quick-actions, workflow, services, bookmarks, reveal, terminal,
  prev/next, parent-repository, detail separator. Items disable (with tooltip reason) when their
  precondition fails.
- **List header** per pane: sort menu, filter menu, density toggle, view settings — the observed
  per-pane `*ViewSettingsMenuController` pattern.
- **Sidebar footer**: activity chip (spinner + count), settings, and the repository switcher.
- **Two utility windows**: Activity (list + detail + toolbar + search), and Quick Open / Quick
  Actions (palettes). Multi-window: repository windows are tabs on the main window; each tab owns
  one repository; `Command-T` opens a new window (`newWindowForTab:` counterpart).

### 4.1 Sidebar sections and rows

Sections (in order): **Pinned**, **Branches**, **Remotes**, **Remote branches**, **Tags**,
**Stashes**, **Submodules**, **Worktrees**, **Reflog**, **Working copy** (always present as a
pseudo-row), plus per-repository **Pull requests** when a service account is linked.
Rows per section follow the observed cell vocabulary; each row type defines: primary text, badge
slots (ahead/behind, unpushed/unpulled, stale, fully-merged, submodule ahead/behind, PR number),
and its default action + options.

Row interactions:

| Section | Default action | Options include |
|---|---|---|
| Branch | checkout (or "check working copy" when current) | rename, delete, pin, set upstream, archive, stack parent, merge into current, rebase current onto, create PR |
| Remote branch | checkout (tracks) | delete remote, rename remote branch, show in history, fetch this remote |
| Tag | show in history | publish, delete, delete on remote, verify signature |
| Stash | show | apply, apply+delete, delete, copy message |
| Submodule | open | init/update, sync, fetch, set URL, ignore mode, delete |
| Worktree | open | move, remove, repair, lock, terminal/Finder |

### 4.2 Sizing and behaviour

- Default window 1180×760, min 900×560; sidebar 216 px (resizable 180–320, persisted per workspace).
- Master/detail split is resizable with a 1 px hairline; remembered per pane.
- The detail pane collapses to a "no selection" state rather than disappearing.

## 5. Screen specifications

Screens S1–S17 are defined in docs/01 §4. Each entry: layout, components, states, interactions,
commands (docs/03 §), and the evidence anchors.

### S1 — Welcome / no repository selected
- Zones: sidebar shown; content area = empty state (`empty.noRepositorySelected`) with three
  actions: **Open…**, **Clone…**, **Create…**; below, "Recent" list (max 8) and — when the indexer
  is enabled and has results — "Found nearby: N repositories" with the first 5.
- States: indexer off / scanning / results / none.
- Commands: `repository_open`, `repository_clone`, `repository_init`, `repository_quick_open_index`.
- Evidence: `empty_view_icon_*` inventory; quick-open copy is analysed but our text is our own.

### S2 — Repository window shell
- Zones: toolbar (global verbs + repository switcher), sidebar, pane area.
- The window title shows repository name + current branch; the repository switcher (sidebar top)
  lists bookmarks + recents with search.
- Commands: `repository_refresh`, `repository_state`, `remote_activity_list`, settings reads.
- Evidence: `GTMainWindowController` (near-empty shell), `newWindowForTab:`.

### S3 — Clone sheet + Clone activity
- Sheet fields: URL (with paste-normalisation), destination (with "Use default" chip showing the
  configured default cloning dir + recent dirs), options disclosure: bare / initialise submodules /
  use LFS.
- Search-state: while typing a service URL, a "checking host" progress line appears (observed
  searching state); results offer account-based clone when an account matches the host.
- On submit: sheet closes, activity chip appears; the Clone activity window shows per-item rows
  with states active/cancelled/error/finished, cancel and restart buttons, clear-finished action.
- Commands: `repository_clone`, `repository_clone_cancel`, `remote_activity_*`,
  `service_repositories_list`.
- Evidence: `GTCloneGitRepositoryViewController.nib` (29 objects), clone option types,
  `GTCloneActivity*CellView`.

### S4 — Quick Open palette
- Input with scope chips (All / Bookmarks / Local repositories); grouped results (Bookmarks, Local);
  keyboard-first (↑/↓/Enter, Esc); footer hint "Indexing your home folder…" during the gathering
  phase (our copy).
- Empty states: index disabled (inline link to enable), no matches, still indexing.
- Commands: `repository_quick_open_index`, `repository_open`.
- Evidence: `GTQuickOpenViewController.nib`, finder phases.

### S5 — Repository sidebar
- Sections and rows as §4.1; sections collapse; badges as §3; contextual filter menu per section
  (e.g. branches: all/contains/merged/unmerged — mirrors the observed filter menu, our labels).
- Sidebar footer: activity chip → opens Activity window; settings → Preferences.
- Commands: `branch_list`, `remote_branch_list`, `tag_list`, `stash_list`, `submodule_list`,
  `worktree_list`, `history_reflog_load`, `branch_pin_set`, `branch_archive_set`.
- Evidence: section cell views inventory; `GTRepositorySidebarViewController.nib` (31 objects).

### S6 — Working copy pane
- Zones: header (status summary + conflicted-only filter + merge-status banner), file list
  (virtual), file detail (diff / image diff / unchanged / conflicted variants), action bar
  (stage/unstage/discard/open-with + options carets).
- File list row: status glyph (colour + shape, so colour is not the only cue), path (dir part
  muted), badges (assume-unchanged, submodule, LFS), trailing marker for staged/unstaged duality
  (two columns: "staged" and "unstaged" ticks).
- Chunk staging: hovering a hunk shows a stage/unstage/discard affordance; the diff is chunk-
  addressable; selection is sent as `(path, hunk, direction)`.
- Merge-status banner variants: mergeable / conflict / merged (`overview_merge_status_*`
  counterparts, our drawings).
- Commands: §3 `wc_*`, `commit_*`.
- Evidence: full `GTWorkingCopy*` controller stack; `stageChunk:`/`unstageChunk:`/`discardChunk:`.

### S7 — Commit composer (embedded in S6, collapsible)
- Zones: subject input (with counter/limit guide when enabled), body editor (resizable, remembered
  height, pinned option), action bar (commit button with split options, amend toggle, sign-off
  toggle, sign toggle, hooks chip with "skip this time"), changeset summary line
  ("N files, X added, Y removed"), warnings banner area.
- Autocomplete: typing `#` → issues (when linked), `@` → files/commits (our triggers), gitmoji
  picker, fixup autocomplete when the commit message starts with `fixup!`/`squash!`.
- AI: "Generate message" appears in the commit options when an agent CLI is configured; a preview
  panel shows the streamed message with **Insert** / **Replace** / **Cancel**; setup guidance when
  not configured (our copy), linking to Settings → AI.
- Templates: subject template view (mono preview) when the template is multi-line, plus
  "Insert template" menu.
- Warnings (each dismissible, each backed by a setting): comments stripped, empty commit,
  template will replace message, no staged files, hooks failed.
- Commands: `commit_create`, `commit_amend`, `commit_undo`, `commit_hooks_probe`,
  `commit_message_generate`, `commit_templates_*`.
- Evidence: `GTWorkingCopyCommitViewController.nib` (41 objects) + the `CommitMessageTextView`
  ecosystem; AI agent CLI inventory (`claude`/`codex`/`gemini`/`copilot`).

### S8 — History pane
- Zones: header (base-branch compare chip + view settings: graph on/off, density, first-parent,
  sort), search/filter bar (collapsible: text, author autocomplete, date range with absolute/
  relative modes, type), commit list (virtual, graph column + text), detail (commit header,
  changeset file list, diff).
- Commit row kinds: normal (small/medium density), merge commit, fork point, date group header,
  loading skeleton, empty.
- Detail: message (rendered as text, not markdown, with links detected), author/committer with
  avatar fallback, hash with copy affordances, ref chips, signature badge (trust states), changeset
  list with per-file +/- counts, diff with context-lines control, ignore-whitespace toggle,
  full-file toggle, syntax colouring toggle, large-diff gate ("Show anyway").
- Compare mode: a summary ribbon (N commits ahead / M commits behind base) + the same list/detail.
- Commands: §5 `history_*`.
- Evidence: `GTHistoryCommitListViewController.nib` (50 objects — largest pane),
  `GTHistorySearchFilterViewController.nib`, RefComparison cell inventory.

### S9 — Branches review pane
- Zones: header with base-branch selector ("Compare against:" — our copy), filter menu
  (all/contains/merged/unmerged + search scopes), branch list with review columns (last commit
  date, author, ahead/behind, fully-merged badge), detail with the branch's commits and actions.
- Commands: `branch_review_load`, `branch_fast_forward`, `branch_update`, `branch_delete`.
- Evidence: `BranchesReview{Header,Detail}ViewController.nib`, `LoadFullyMergedBranchInfosOperation`.

### S10 — Services pane
- Sub-screens: Dashboard (account tiles), Accounts list, Account detail (auth method, org units,
  public keys, repositories), Repository browser, Pull request list, Pull request detail
  (conversation tab, commits tab, changeset tab), PR create sheet, PR merge sheet.
- PR list row: number, title, author avatar, base←head, state chip (open/draft/merged/closed),
  checks indicator (only when the provider exposes it — adapter capability), updated-at.
- Conversation: timeline with grouped cells (author/body/comment/thread/commit/date-group/spacer/
  add-comment), each row virtualized; comment composer is a plain textarea with provider-specific
  markdown hint (our copy) and **no** rich-text (evidence: comment VC exists; editor capability
  not observed).
- Merge sheet: option set is provider-shaped (`capabilities.merge_options`), branch-deletion
  checkbox when supported.
- Empty states: no accounts (CTA to add), no repositories, no PRs, PR detail loading.
- Commands: §15 `account_*`, `service_*`, `pr_*`, `issue_search`.
- Evidence: `GTServices*` / `GTPullRequests*` cell and VC inventory (F8/F9 in ui-structure).

### S11 — Forensics panes
- **File history**: path header, timeline (commit rows compact), detail (changeset for that commit
  restricted to the path), follow-renames toggle.
- **Blame**: annotation gutter (author, date, relative time, hash on hover) + text with syntax
  colouring; clicking an annotation opens the commit popover (message, hash, actions: show in
  history, show file history, copy hash).
- **Blob**: read-only viewer with binary detection (hex preview + size + "Save as…") and LFS
  pointer rendering (pointer contents + "Fetch LFS object" action).
- **Reflog**: ref selector (HEAD + refs), list (entry rows: selector, action, hash, time), detail
  (affected commit + diff against previous entry).
- Commands: `history_file_history_load`, `history_blame_load`, `history_blob_load`,
  `history_reflog_load`, `history_revision_archive`.
- Evidence: `GTFileHistory*`, `GTBlame*`, `GTBlobViewController.nib`, `GTRefLog*` nibs.

### S12 — Activity window
- Toolbar: filter (all / remote / clone / service / indexer), search, clear-finished, cancel-all.
- List + detail: list rows show kind icon, title, repo, state, progress bar, elapsed; detail shows
  the operation's log lines (collapsible), the retry action when applicable, and the cancel button.
- Progress widget states: determinate (done/total), indeterminate, stalled (amber, with
  "Cancel stalled" appearing in the toolbar — the watchdog surface).
- Commands: `remote_activity_*`.
- Evidence: `GTActivityWindowController.nib` + `GTActivityWindowToolbarController`,
  watchdog and cancel actions.

### S13 — Quick actions palette
- Opened by the toolbar button or ⌘⇧A (ours). Content = the action set for the current selection
  context (one of the 20 observed object types): search box, grouped list, disabled actions show
  their unmet precondition as subtitle.
- Keyboard: type-to-filter, ↑/↓/Enter; Esc closes.
- Commands: dispatches the same commands as the context menus — the palette is a view, not a
  second implementation.
- Evidence: `GTQuickActionViewController.nib` (31 objects), 20 `GTQuickActionObjectName*` types.

### S14 — Preferences window
- Tabbed (General, Git Config, Integration, Appearance, Editor, Environment, Text, Profiles,
  Services, AI, Updates, Advanced), each rendered from the settings schema (rows of label +
  control + help), so tabs are data.
- Notable panes: **Profiles** (list + detail, "Set as global", per-repo assignment),
  **Integration** (diff tool, merge tool, terminal app, git binary, CLI tool install, deep-link
  registrations), **Environment** (user variables table, application variables, "reload from login
  shell" with progress), **Editor** (font/size/tab width with live preview), **Appearance**
  (mode, app icon, theme picker with import/export/validate report), **Updates** (our own
  mechanism), **Advanced** (logging, snapshot retention, snapshot/undo list with restore).
- Commands: §17 `settings_*`, `profile_*`, `theme_*`, `environment_*`, `tool_*`, `appearance_set`.
- Evidence: `GTUserPreferences*ViewController.nib` sizes (49/53/48/40/36/48/30 objects),
  `settingsItemsController` outlet.

### S15 — Dialogue family
- One shell: title, subtitle (object identity, always naming the exact ref/path), body zone
  (fields/summary), options zone (checkbox rows with remembered state + "remember" affordance
  where a policy exists), error accessory zone, action row (secondary ← primary, danger styling
  for destructive).
- Sizes: S (confirm, 360 px), M (single-column form, 460), L (two-column e.g. push dialog, 560).
- Validation: inline, on submit, with the validator's message; the primary button stays enabled but
  the click reports the violation (matches the engine's typed errors).
- Progress: long verbs (fetch/pull/push inside a dialogue) show a progress track with phase text and
  a **Background** button that moves the work to the Activity window.
- Confirms: `GTConfirmViewController` counterpart — icon, one sentence, optional "affected items"
  list, Cancel + destructive primary.
- Evidence: `GTDialogueViewController.nib` (42 objects), `DialogueView`/`DialogueWrapperView`
  classes, per-verb dialogue nib inventory (docs/01 §4 S15).

### S16 — Submodule and worktree panes
- Submodule detail: path header, state chip (uninitialised/current/ahead/behind/modified), recorded
  vs current commit with update action, URL, ignore mode / update strategy / recurse controls,
  "open submodule" affordance.
- Worktree detail: path, HEAD, locked state, validity warning with repair action, open targets
  (new tab/window, terminal, Finder), remove with the two guard messages (locked / has changes).
- Evidence: `GTSubmodule*` and `GTWorktree*` nib inventory.

### S17 — Onboarding / What's New
- Onboarding sequence (our copy): welcome → Git identity (name/email + profile creation) → tools
  (diff/merge/terminal detection) → services (optional account) → privacy. Skippable at each step;
  resumable; a "Getting started" checklist lives in the sidebar footer until dismissed.
- What's New: shown once per version, list of highlights, links to our changelog.
- Evidence: onboarding selector inventory (`showOnboarding*Screen:`), `GTUserDefaultsGettingStartedPassed`
  (mechanism only).

## 6. Component inventory and the state contract

| Component | Purpose | States |
|---|---|---|
| `<ListShell>` | standard list chrome: header, scroll, states | loading / empty / error / data / large-warning |
| `<VirtualList>` | windowed rows with sticky group headers | as above |
| `<SidebarSection>` / `<SidebarRow>` | section + typed row | collapsed, filtered-empty, normal, selected |
| `<SplitAction>` | default action + caret options | idle, busy, disabled(+reason) |
| `<RowBadge>` | counts/ahead-behind/state badges | variant per §3 |
| `<DiffView>` | diff document renderer | loading, text, binary, image, too-large, error |
| `<HunkControls>` | stage/unstage/discard per hunk | hidden (non-text), visible, disabled (conflicted) |
| `<GraphColumn>` | lane segment renderer | — |
| `<CommitMessageEditor>` | subject/body editor with plugins | plain, autocomplete-open, warning, generating |
| `<IntegrationBanner>` | merge/rebase/cherry-pick/revert progress banner | each mode; conflicted variant; continue/skip/abort |
| `<ProgressTrack>` | activity progress | indeterminate, determinate, stalled |
| `<EmptyState>` | icon + title + caption + action | per `empty.*` role |
| `<DialogueShell>` | modal chrome | S/M/L, error, progress, busy |
| `<SettingsRow>` | schema-driven preference row | bool/int/string/enum/path/color + help |
| `<PromptHost>` | credential/passphrase/2FA asks | queued prompts, expiry countdown |
| `<Toast>` | post-op notices (non-modal) | info, success, warning, danger (with Undo action when available) |

## 7. Theme file format (ours)

```jsonc
{
  "format": "meridian-theme",
  "version": 1,
  "meta": { "id": "uuid", "name": "Arctic", "author": "", "createdAt": "…" },
  "appearance": "dark",              // "light" | "dark" — one file per appearance
  "tokens": {                        // full or partial token map; hex/rgba strings
    "bg-canvas": "#0E1114", "fg": "#E8ECF1", "accent": "#59A8FF", "graph-lane-3": "#3FBF74"
  },
  "syntax": { "css": "…Highlightr-style CSS…" }   // optional editor colouring
}
```

Rules: unknown keys are preserved on import; missing tokens inherit from the active built-in theme;
validation runs before storage and produces a per-file report (mirrors the observed import
validation flow — our format, our errors). Migrations are a chain of pure functions
`v(n) → v(n+1)`, applied on load; the current version is 1 and the chain is designed to grow
(the evidence's V4→V5→V6 chain is the mechanism, not a format to copy).

## 8. Interaction patterns

1. **Default action + options**: click = default verb (quick), caret = dialogue with options. Used on
   branch rows (checkout / checkout-with-options), toolbar pull/push, stash apply, tag publish,
   remote fetch.
2. **Confirmation before loss**: with an itemised list of what is lost (files, commits, refs), a
   danger primary, and a "don't ask again" only where the evidence shows a remembered setting
   (`AlwaysDiscardDiffContent`, `SkipAutoStash`, …). Default focus is on Cancel.
3. **Policy asks**: auto-stash offer, submodule update offer, apply-stash-after-op offer — rendered
   as dialogues with three buttons (Yes / No / Always) and an explicit "remember" checkbox bound to
   the observed settings semantics.
4. **Progress everywhere**: any operation that can exceed ~300 ms runs as an activity; the dialogue
   gets a **Background** action at 500 ms so the user is never trapped.
5. **Undo toast**: after any undoable operation, a toast offers Undo for 8 s (commit, reset, stash
   ops, checkout-files, branch delete when unmerged-but-forced). This is the UI face of the undo
   registry.
6. **Keyboard map (ours)** — defaults, all remappable: ⌘K quick open · ⌘⇧A quick actions · ⌘⇧F
   fetch · ⌘⇧P push · ⌘P pull · ⌘⇧S sync · ⌘⏎ commit · ⌘⇧⏎ commit-options · ⌘Z undo · ⌘⇧Z redo
   (history-pane local) · ⌘⌫ delete selection · ⌘R refresh · ⌘⌥F reveal in Finder · ⌘⌥T terminal ·
   ⌘⇧D discard selection · ⌘⇧C copy hash · ⌘⌥↑/↓ previous/next commit. Menu bar mirrors these; the
   menu is generated from the same command registry as the palette and the toolbar (single source
   of truth).
7. **Drag and drop**: repositories onto the window (open/bookmark), files onto the working copy
   (stage), patches onto the working copy (apply), stash rows onto branches (apply there, gated by
   a confirm) — the last is an affordance we add; only open/bookmark drop is evidenced.

## 9. Accessibility

- Contrast: every text token ≥ 4.5:1 against its background in both themes (verified in CI by a
  contrast test over the token tables).
- Focus: a 2 px `--md-accent` ring with 1 px offset on every interactive element; focus order
  follows the visual reading order; dialogues trap focus and restore it.
- Status is never colour-only: file status uses glyph + colour; badges use icon + number.
- Motion: all transitions ≤ 150 ms and suppressed under `prefers-reduced-motion`.
- Screen readers: lists use `role="list"`/`listitem`, the sidebar uses `tree`/`treeitem` with
  `aria-expanded`, the diff uses per-row labels ("line 42, added: …"), progress uses
  `role="progressbar"` with live phase text.
- Full keyboard operation: quick-open/quick-actions, sidebar navigation (⌥↑/↓ between sections),
  diff hunk navigation (⌥[ / ⌥]), dialogue submit (⏎) and cancel (Esc).

## 10. Localisation and copy

- All strings live in a message catalogue keyed by `ErrorCode`/component id; no inline literals in
  components (lint rule).
- The evidence's UI copy is **not** used — Meridian writes its own text, including the error
  catalogue (docs/02 §14). Pseudo-localisation tests run on every list to keep long-content layouts
  honest (the observed long-placeholder discipline is the mechanism; the text is ours).

## 11. Explicitly unspecified (no evidence, our decisions pending)

| Item | Why |
|---|---|
| Pixel geometry of anything | IB archives do not carry geometry (docs/00 §5.2) |
| Animation/motion details | not observed |
| Exact icon artwork | asset bytes never decoded; we draw/pick our own |
| Density of the services pane | provider frameworks out of scope; our layout is a proposal |
| Menu bar layout specifics | the observed main-menu nib lists controllers, not items; our menu is generated from the registry |
