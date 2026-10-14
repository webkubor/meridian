# 02 — Architecture

The Tauri architecture for Meridian, written to mirror the mechanisms found in the evidence. Three
structural mirrors are load-bearing and everything else hangs off them:

| Evidence mechanism | Meridian mechanism |
|---|---|
| `GTGit*Action` → `GTGit*ActionValidator` → `GTGit*Operation` triad (194/168/105 types, `EV:analysis/git-operations#F1`) | **Command layer in three parts**: a command descriptor (name, args, sender tags), a pure `validate()` precondition function, and an `execute()` on the engine |
| `FNGit*` domain layer vs `GTGit*` engine layer bridged by 21 explicit transformers (`EV:analysis/git-semantics#F1`) | **Domain types vs git2-backed engine**, with one pure transformer module between them; no `git2` type ever crosses into the command surface or the web UI |
| Seven linked provider frameworks behind one service layer (`EV:analysis/services#F1`) | **`ServiceAdapter` trait** with one implementation per host, plus a capability descriptor per provider |

Secondary mirrors, each justified by a named finding:

| Evidence | Meridian |
|---|---|
| Per-repository serialized queues (`GTGitQueue`, `GTGitRepositoryQueueManager`, `EV:analysis/git-operations#F7`) | One `RepoQueue` actor per repository key; all mutating commands go through it |
| Activities with progress, cancellation, stall watchdog (`EV:analysis/git-operations#F5`) | `ActivityRegistry` + `CancellationToken` + `Watchdog`; streamed to the UI |
| Progress as protocol (`EV:analysis/git-operations#F17`) | `ProgressEvent { phase, done, total, item }` over Tauri `Channel<T>` and stage events |
| Undo family + working-tree snapshots (`EV:analysis/git-operations#F3`) | `SnapshotStore` + `UndoRegistry`; every destructive command declares its inverse |
| Auto-fetch / auto-stash / auto-submodule policies (`EV:analysis/git-operations#F6`) | `policy/` module: policy objects consulted by command preconditions, not baked into commands |
| Unified merge-process state machine (`EV:analysis/git-semantics#F5`) | `IntegrationState` enum with payload + one loader, feeding the shared Continue/Skip/Abort surface |
| Shell-out family `FNGit*Command` (`EV:analysis/git-operations#F8`) | `engine::child` — supervised `git` child processes for the operations libgit2 cannot do |
| Versioned credential serialization V1→V3 (`EV:analysis/services#F3`) | Versioned record enums + migration functions from day one |
| `GTGitBinaryManager/Finder/Validator/ReportCreator` (`EV:analysis/git-operations#F9`) | `engine::gitbin` — discover, version-gate, report |
| Nine `GTGitOperationGroup*` (`EV:analysis/git-operations#F15`) | Nine engine modules + nine command modules, same boundaries |
| Sender-scoped action validation (`EV:analysis/git-operations#F2`) | `SenderTag` on every command invocation; the validator branches on it |

**Honesty note.** The evidence proves these *names and structures* existed. It does not prove how
they were sequenced internally (no pseudocode — docs/00 §5.1). Everything below is our design that
uses the same shape; where a detail is our invention it is marked. Conversely, nothing here depends
on a behavior we could not observe.

---

## 1. Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 (Rust host + WKWebView) | Small binary, native windowing, a command/event bridge that maps 1:1 onto the Action/Validator/Operation triad |
| Engine | `git2` (libgit2 bindings) | Direct counterpart of the `GTGit*` layer (docs/00 §4.1.1); libgit2's API is where every operation name in `EV:types/GTGit*Operation` lands |
| Async | Tokio | Network and filesystem work is async and cancellable, mirroring Activities |
| Frontend | Vue 3 + TypeScript + Vite | Component-per-pane maps onto the evidence's one-nib-per-pane structure (`EV:analysis/ui-structure#F1`) |
| Frontend state | Pinia | One store per domain module; stores hold *domain* types only |
| Persistence | SQLite (`rusqlite`) for metadata; macOS Keychain (`keyring` crate) for secrets | Mirrors "credentials in the keychain, account metadata in a versioned store" (`EV:analysis/services#F3`) |
| IPC typing | `tauri-specta` (generate TS bindings from Rust commands) | One source of truth for the command surface in docs/03 |
| Syntax highlighting | Shiki with CSS themes | Evidence shows CSS-theme-driven highlighting (`EV:analysis/visual-design#F8`); Shiki is CSS-theme-native |
| Virtual lists | TanStack Virtual | Commit lists, file lists, PR timelines are all virtualized |
| File watching | `notify` | External git usage must trigger a refresh; Tower has explicit reload actions and a debug auto-reload default (`EV:strings/GTUserDefaultsDebugAutoReloadRepository`) |
| Logging | `tracing` + rolling file appender | A "detailed logging" toggle exists in the evidence (`EV:strings/GTUserDefaultsDetailedLoggingEnabled`) |

Dependency rules:

- `domain` must not depend on `git2`, `tauri`, or `rusqlite`.
- `engine` may depend on `git2` but must not depend on `tauri`.
- `commands` may depend on everything below it; nothing depends on `commands`.
- The web UI may only see types defined in `domain` (serialized) and the command/event surface.

## 2. Repository layout

```
meridian/
├── docs/                          # this specification, evidence extracts
├── src/                           # web UI (Vue 3 + TS)
│   ├── app/                       # shell: window, routing, menus, command palette wiring
│   ├── bridges/                   # generated bindings + typed invoke/Channel helpers
│   ├── stores/                    # Pinia stores (repos, workingCopy, history, branches, …)
│   ├── components/                # pane components + the shared dialogue shell
│   ├── design/                    # tokens.css, theme runtime, icon registry
│   └── diff/                      # diff renderer, hunk model, graph renderer
└── src-tauri/
    ├── src/
    │   ├── main.rs                # app bootstrap, deep-link/CLI intake, single-instance
    │   ├── commands/              # Action surface (docs/03) — one module per group
    │   │   ├── repository.rs   working_copy.rs   history.rs    branches.rs
    │   │   ├── remotes.rs      remote_branches.rs stashes.rs   submodules.rs
    │   │   ├── tags.rs         worktrees.rs       integrate.rs services.rs
    │   │   └── system.rs       workflows.rs       settings.rs
    │   ├── domain/                # FNGit* counterpart: plain data, serde, no git2
    │   │   ├── ids.rs         status.rs     refs.rs      revision.rs
    │   │   ├── diff.rs        merge.rs      progress.rs  activity.rs
    │   │   ├── activity.rs    settings.rs   error.rs
    │   ├── engine/                # GTGit* counterpart: git2-backed, one module per operation group
    │   │   ├── repository.rs  working_tree.rs history.rs   branches.rs
    │   │   ├── remotes.rs     remote_branches.rs stashes.rs submodules.rs
    │   │   ├── tags.rs        worktrees.rs    integrate.rs reflog.rs
    │   │   ├── blame.rs       blob.rs         signature.rs config.rs
    │   │   ├── lfs.rs         gitbin.rs       child.rs
    │   ├── transform/             # the 21 transformer counterparts (pure functions)
    │   │   ├── file_status.rs  merge_mode.rs  reset_mode.rs
    │   │   ├── submodule.rs    object_type.rs diff_mode.rs
    │   ├── queue/                 # RepoQueue, QueueManager, Enqueueable, cancellation
    │   ├── activity/              # ActivityRegistry, Watchdog, progress fan-out
    │   ├── policy/                # auto_fetch, auto_stash, auto_submodules, auto_apply_stash, finder
    │   ├── snapshot/              # SnapshotStore, UndoRegistry, inverse-op dispatch
    │   ├── store/                 # sqlite (settings/bookmarks/accounts/snapshots), keychain, migrations
    │   ├── services/              # ServiceAdapter trait + adapters + oauth + poller
    │   ├── workflows/             # git_flow, git_flow_next, custom, graphite, gerrit, svn
    │   ├── events.rs              # app-wide notification bus (Tauri emit)
    │   └── state.rs               # AppState: registries, pools, config
    ├── tauri.conf.json
    └── capabilities/              # Tauri permission scopes (fs, shell, deep-link)
```

## 3. The command layer (mirror of Action → Validator → Operation)

Every user-invocable verb is declared once:

```rust
// src-tauri/src/commands/branches.rs
pub struct CreateBranch {
    pub repo: RepoId,
    pub name: String,
    pub start_point: Revision,
    pub checkout: bool,          // EV:strings/GTUserDefaultsDialogueOptionCheckoutCreatedBranch
    pub sender: SenderTag,       // EV:analysis/git-operations#F2 (sender-scoped validation)
}

impl Command for CreateBranch {
    type Output = BranchInfo;

    fn validate(&self, cx: &ValidateCx) -> Result<(), Violation>;   // pure, no I/O beyond the repo
    fn risk(&self) -> Risk;                                         // None | Destructive | Rewrite
    fn execute(self, cx: &ExecCx) -> Result<Self::Output, GitError>;
}

#[tauri::command]
pub async fn branch_create(state: State<'_, AppState>, args: CreateBranch)
    -> Result<BranchInfo, ErrorPayload>
{
    state.run(args).await     // enqueue per repo -> validate -> policy hooks -> execute
}
```

`AppState::run` performs, in order:

1. **Resolve** `RepoId` → repo handle (canonical path; worktrees resolve to their own working
   directory but share the gitdir key, docs/02 §5).
2. **Enqueue** on the repository's `RepoQueue` (serialized — evidence F7).
3. **Validate** with `validate()`; a `Violation` becomes a typed error (`ErrorPayload`).
4. **Policy hooks**: `policy::*` may inject a confirmation request (auto-stash offer, submodule
   update offer) — these are *asks* to the caller, returned as `Outcome::NeedsDecision`, never
   silent behaviours (evidence F6: every policy has its own dialogue).
5. **Snapshot** if `risk()` says so (docs/02 §7).
6. **Execute** on a blocking thread with a `CancellationToken`.
7. **Emit** domain events (`RepoChanged`, `RefsChanged`, `WorkingCopyChanged`, …).

`SenderTag` mirrors the observed `*ActionSenderValidator` split (menu, toolbar, sidebar context
menu, quick-action palette, dialogue): it selects *which preconditions apply* (e.g. "create branch"
from a commit row defaults the start point to that commit), never *whether the operation exists*.

Registered commands are grouped exactly like `GTGitOperationGroup*` (`EV:types/GTGitOperationGroupBranches`
… `WorkingTree`) so the docs/03 command list, the UI routes and the engine modules share one map.

## 4. Domain ↔ engine split and transformers

`domain` carries the value vocabulary the evidence documents; `transform` converts to/from git2
values; `engine` never returns a git2 type upward.

```rust
// domain/status.rs — EV:analysis/git-semantics#F2
pub enum FileStatus { Added, Copied, Deleted, Modified, Renamed, Unmerged, Untracked, Ignored }
pub struct EntryStatus { pub code: FileStatus, pub staged: bool, pub assume_unchanged: bool }

// domain/merge.rs — EV:analysis/git-semantics#F5
pub enum MergeMode { Merge, MergeSquash, Rebase, CherryPick, Revert }
pub enum ResetMode { Soft, Mixed, Hard }
pub enum ConflictKind { AddedByUs, AddedByThem, BothAdded, BothDeleted, BothModified,
                        DeletedByUs, DeletedByThem }          // 7 kinds, EV:analysis/git-semantics#F3
pub enum DiffPerspective { IndexToBase, MineToMergeBase, MineToTheirs,
                           TheirsToMergeBase, WorkingTreeToBase, WorkingTreeToIndex }
                                                              // 6 perspectives, EV:analysis/git-semantics#F4
// domain/submodule.rs — case sets INFERRED from class names + adjacent strings
pub enum SubmoduleIgnoreMode { None, Untracked, Dirty, All }
pub enum SubmoduleUpdateStrategy { Checkout, Rebase, Merge, None_ }

// domain/activity.rs — EV:analysis/git-operations#F5/#F17
pub struct Progress { pub phase: Phase, pub done: Option<u64>, pub total: Option<u64>,
                      pub item: Option<String> }
pub enum ActivityState { Queued, Running, Cancelling, Cancelled, Failed(FailureInfo), Finished }
```

`transform::file_status::from_git2(status: git2::Status) -> EntryStatus` is a pure function; the
mapping is 1:N because git2 packs staged/unstaged bits and we keep the display axis separate.
`UiStatusView { short: char, text: String, tone: Tone }` is derived from `EntryStatus` — the
"one enum, three renderings" rule (evidence F2) enforced by the type system: the UI never computes
status strings itself.

## 5. Repository identity, queues and concurrency

- **Identity**: `RepoId` wraps the canonicalised *gitdir* path. Two worktrees of one repository
  therefore share a queue key but have distinct `WorkdirId`s; the UI addresses the pair.
- **Queue**: `RepoQueueManager` holds `HashMap<RepoId, RepoQueue>`. A `RepoQueue` is an
  `mpsc::Sender<Job>` plus a worker task; each job runs to completion (or cancellation) before the
  next starts. Read-only loads (`history_load`, `status_load`) bypass the queue and run on a
  bounded `spawn_blocking` pool — they are idempotent and cheap to coalesce; all *mutating*
  commands are queued. Coalescing key: `(RepoId, JobKind, args_hash)` for loads only.
- **Backpressure**: queues report depth to the UI (`QueueDepthChanged`), which renders the
  "queued work" affordance the evidence's queue types imply.
- **Cancellation**: every job carries a `CancellationToken`. Cancellation of a git2 call is
  cooperative: libgit2 progress callbacks check the token and return an error (matching
  `FNGitCommandCancelling`’s role); child processes get SIGTERM then SIGKILL after a grace period.
- **Watchdog**: activities that produce no progress for `settings.remote_operation_timeout_threshold`
  (evidence key `GTUserDefaultsRemoteOperationTimeoutThreshold`) are marked *stalled* and surfaced
  in the activity window with a cancel action (`GTGitCancelStalledRemoteActivitiesAction`).
- **Refresh triggers**: `notify` watches `.git` (HEAD, refs, index, packed-refs) and the working
  tree top level; a debounced `RepoChanged` event fans out to open windows. This is our mechanism
  for the external-change behaviour Tower solves with `ReloadRepositoryAction` and
  `GTUserDefaultsDebugAutoReloadRepository`.

## 6. Activities and the progress protocol

```rust
pub enum ProgressEvent {                       // streamed over Channel<ProgressEvent>
    Started   { activity: ActivityId, kind: ActivityKind, total: Option<u64> },
    Phase     { activity: ActivityId, phase: Phase },            // "Receiving objects" …
    Tick      { activity: ActivityId, progress: Progress },
    RequiresInput { activity: ActivityId, prompt: Prompt },       // credentials, SSH passphrase
    Finished  { activity: ActivityId, summary: Summary },
    Failed    { activity: ActivityId, error: ErrorPayload },
}

pub enum AppEvent {                            // app-wide, Tauri emit
    ReposChanged, RefsChanged { repo: RepoId }, WorkingCopyChanged { repo: RepoId },
    QueueDepthChanged { repo: RepoId, depth: usize },
    ActivityStarted/Updated/Finished { activity: ActivityId },
    AccountsChanged, PullRequestsChanged { account: AccountId },
    SettingsChanged { key: String },
}
```

`ActivityKind` values follow the observed manager split: `Clone`, `Remote`, `WorkingCopy`,
`Submodule`, `Lfs`, `Service`, `Indexer` (`EV:types/GTGit{Remote,Clone,WorkingCopy}ActivityManager`).

git2 callbacks are bridged into this stream by adapters in `activity/`: transport progress
(`TransferProgress`) → `Phase`/`Tick`; checkout progress; child-process stderr parsing → same
events. Credential prompting is a *prompt round-trip*: the engine pauses on a channel, the UI shows
the askpass-style dialogue (our equivalent of `TowerAskpass`), the answer returns through the
channel. Secrets are never logged and never leave the host.

## 7. Snapshots and undo

```rust
pub struct Snapshot { pub id: SnapshotId, pub repo: RepoId, pub created_at: i64,
                      pub trigger: SnapshotTrigger,          // BeforeReset, BeforeStash, Manual, AutoStash
                      pub head: Oid, pub index_tree: Oid, pub workdir_tree: Option<Oid>,
                      pub patch_blob: Option<Vec<u8>>, pub label: String }

pub trait Undoable { fn inverse(&self) -> UndoOp; fn snapshot_policy(&self) -> SnapshotPolicy; }
```

- Every command with `Risk::Destructive | Rewrite` declares a snapshot policy (evidence:
  `performResetActionWithRevision:headRevision:mode:snapshot:`, `GTGit*WorkingTreeSnapshot*Operation`).
- `UndoRegistry` keeps the last N undoable actions per repo (N configurable; default 20) in SQLite;
  the six observed undo families map to `UndoOp::{Commit, Reset, SaveStash, ApplyStash,
  CheckoutFiles, AbortMerge}` (`EV:types/GTGitUndo*Action`).
- Restores construct a diff from the stored trees/patch and apply it via
  `git2::Repository::apply(diff, ApplyLocation, opts)` (verified against the git2 docs at build
  time), refusing when the working tree has diverged unless the user confirms a merge of the
  restore.
- Snapshots are pruned by age/size policy; a snapshot never blocks a normal command.

## 8. Policies (automatic behaviours)

```rust
pub trait Policy { fn applies(&self, cmd: &dyn Any, repo: &RepoHandle) -> Option<PolicyAsk>; }
pub struct PolicyAsk { pub kind: PolicyKind, pub default: Decision, pub remember_key: SettingsKey }
pub enum PolicyKind { AutoStash, ApplyStashAfterOp, UpdateSubmodules, FetchBeforeOp }
```

Mirrors the four observed controllers (`GTGitAuto{Fetch,Stash,UpdateSubmodules,ApplyStash}Controller`,
evidence F6). The rule: **a policy never acts silently on a destructive path** — it returns
`NeedsDecision`, the UI renders the corresponding dialogue (auto-stash offer, auto-update-submodules
offer), and the user's answer can be remembered under the observed settings keys
(`GTUserDefaultsAlwaysSkipAutoStash`, `...AutoStashAlwaysReapplyChanges`, `...AlwaysAutoUpdateSubmodules`,
`...AlwaysFetchAllRemotes`, `...AutoFetchTimeInterval`).

The repository finder (`policy::finder`) is an incremental indexer with the observed lifecycle
phases: `Gathering → Processing → Searching` (`GTGitRepositoryFinderDid{StartSearching,UpdateResults,
StopGathering}Notification`), walking configured search roots, skipping ignored paths, emitting
`FinderResultsUpdated` batches. It runs only when Quick Open is enabled
(`GTUserDefaultsQuickOpenEnabled`).

## 9. Storage

| Data | Where | Versioning |
|---|---|---|
| Bookmarks, recents, window state, settings | SQLite `meridian.db` (tables: `bookmarks`, `recent_repos`, `settings`, `snapshots`, `undo_log`, `accounts`, `pr_cache`) | Schema migrations via `user_version`; settings have typed accessors, one Rust field per observed key that we adopt |
| Secrets: HTTPS tokens/passwords, SSH passphrases, PATs, OAuth refresh tokens | macOS Keychain via `keyring` (service = bundle id, account = stable credential id) | Credential records serialized as a **versioned enum** (`CredRecordV1/V2/V3`) with `migrate()` — mirrors `GTServiceAccountCredentialsToSerializationV{1,2,3}FormatTransformer` |
| Themes | files in `~/Library/Application Support/Meridian/themes/*.json` (our own format, docs/04 §7) | `ThemeFileVersion` + migration chain, mirroring `ThemeV4toV5Migration` / `ThemeV5toV6Migration` |
| Repo-scoped metadata (pinning, archiving review flags, stack parents) | SQLite keyed by `RepoId`, **never written into the repository working tree** | Not a git-visible store: nothing in the user's repo changes |
| Git config | the repo's own `.git/config` and global config, via `git2::Config` (levels: System/Global/XDG/Local/Worktree) | — |

Rule enforced in review: **Meridian writes git-native data only through git mechanisms** (refs,
config, index, objects). Side metadata (pin/archive/stack flags in our UI) lives in SQLite and
degrades gracefully — if the file is deleted, the repo is untouched.

## 10. Services layer

```rust
pub trait ServiceAdapter: Send + Sync {
    fn flavor(&self) -> HostFlavor;                    // 11 observed flavors
    fn capabilities(&self) -> &'static ServiceCapabilities;
    fn probe_host(&self, base: &Url) -> BoxFuture<'_, Result<HostInfo>>;   // EV: GTServiceProbeServerAPIVersionOperation
    fn validate_credentials(&self, acct: &Account) -> BoxFuture<'_, Result<Identity>>;
    fn repositories(&self, q: RepoQuery) -> BoxFuture<'_, Result<Vec<ServiceRepo>>>;
    fn org_units(&self, q: OrgQuery) -> BoxFuture<'_, Result<Vec<OrgUnit>>>;   // orgs | groups | teams | projects
    fn public_keys(&self, acct: &Account) -> BoxFuture<'_, Result<Vec<PublicKey>>>;
    fn pull_requests(&self, repo: &ServiceRepo) -> BoxFuture<'_, Result<Vec<PullRequest>>>;
    fn pull_request(&self, id: PrId) -> BoxFuture<'_, Result<PullRequestDetail>>;
    fn create_pull_request(&self, draft: PrDraft) -> BoxFuture<'_, Result<PullRequest>>;
    fn edit_pull_request(&self, id: PrId, patch: PrPatch) -> BoxFuture<'_, Result<()>>;
    fn merge_pull_request(&self, id: PrId, opt: MergeOption) -> BoxFuture<'_, Result<()>>;
    fn close_pull_request(&self, id: PrId) -> BoxFuture<'_, Result<()>>;
    fn comments(&self, id: PrId) -> BoxFuture<'_, Result<Vec<Comment>>>;
    fn comment_create(&self, id: PrId, c: CommentDraft) -> BoxFuture<'_, Result<Comment>>;
    fn comment_edit(&self, cid: CommentId, body: String) -> BoxFuture<'_, Result<()>>;
    fn comment_delete(&self, cid: CommentId) -> BoxFuture<'_, Result<()>>;
    fn search_issues(&self, q: IssueQuery) -> BoxFuture<'_, Result<Vec<Issue>>>;
}

pub struct ServiceCapabilities {
    pub org_unit_noun: OrgUnitNoun,          // Organizations | Groups | Teams | Projects
    pub supports_public_keys: bool,
    pub supports_issue_search: bool,
    pub supports_draft_pr: bool,
    pub merge_options: &'static [MergeOption],   // provider-shaped; EV:analysis/services#F5
    pub auth_methods: &'static [AuthMethod],
    pub base_url_shape: HostShape,           // Cloud | EnterprisePath | EnterpriseHost
}
```

Adapters: `github`, `github_enterprise`, `gitlab`, `gitlab_ceee`, `gitswarm`, `bitbucket`, `stash`,
`beanstalk`, `azure_devops`, `azure_devops_server`, `custom` (generic HTTP service with templated
endpoints). `custom` mirrors the observed CustomServiceAPI framework: the user supplies base URL and
endpoint templates; the adapter renders them.

**Auth.** We do not use Tower's OAuth broker. Per provider: OAuth device flow where the provider
offers it with a public client (GitHub, GitLab); Personal Access Token as the universal fallback;
Basic auth only for hosts that require it (Stash/Beanstalk-era services) and only over HTTPS.
Credentials are stored exactly as §9 describes. A shared `oauth::TokenStore` refreshes tokens ahead
of expiry and surfaces `AccessTokenExpired` as a re-auth prompt (the observed
`GTServiceAccessTokenExpiredError` state).

**Errors.** One enum mirrors the observed service taxonomy: `RateLimited{retry_after}`,
`TwoFactorRequired`, `LoginAttemptLimitExceeded`, `Authentication`, `TokenExpired`, `Permission`,
`NotFound`, `InvalidRequest`, `InvalidResponse`, `ServerError`, `Transport`, plus a
`diagnostic` struct carrying request URL / request body / status / response body / headers as
structured fields (`GTServiceAPIErrorMapper` §F7). The UI maps error → affordance: rate limit →
back-off banner; 2FA → OTP prompt; token expired → re-auth; permission → scope explanation.

**Polling.** `services::poller` refreshes open pull requests and account data on
`settings.automatic_pull_requests_update_interval`, with jitter and exponential back-off on
rate-limit errors, deduplicated per (account, repo). All refreshes run as `ActivityKind::Service`
activities so they are visible and cancellable.

**PR checkout** uses the git layer through a thin interface: fetch `<provider refspec>` into a local
ref, resolve the PR head and base, and when the PR comes from a fork, ensure the fork remote exists
(create a remote for the fork URL; the observed behaviour around
`GTGitCleanUpPullRequestForkRefsAction` implies cleanup of such refs after checkout). The
provider-agnostic part lives in `engine::remotes`; the provider-specific ref names live in each
adapter (`refs/pull/N/head`, `refs/merge-requests/N/head`, …).

## 11. Workflows

`workflows/` hosts the pluggable layer the evidence shows (`EV:analysis/git-operations#F12`):
git-flow classic (`configure`, `feature|hotfix|release` × `start|finish` with the observed finish
options), GitFlow Next (parse/update its config file format — **format not in evidence**: our parser
is written from the upstream git-flow-next documentation, marked as an external dependency), custom
workflows (a small declarative model: branch prefixes, base branch, actions on start/finish),
Graphite (shell-out: locate binary, version-gate, run `gt` verbs, read its repo config, mirror stack
state), Gerrit (a push path that rewrites the refspec to `refs/for/<branch>` behind a setting), and
SVN interop (clone via `git svn`, and the observed add-remote / fetch / pull / push / publish
verbs). Each workflow is a module exposing commands; none of them can bypass the command layer's
validation or the repo queue.

## 12. Engine internals: libgit2 vs child processes

| Need | Mechanism | Evidence |
|---|---|---|
| Everything libgit2 supports | `git2` | `GTGit*Operation` layer |
| Archive / format-patch / `git svn` clone | supervised `git` child process | `FNGit{Archive,FormatPatch,SVNClone}Command` |
| LFS transfer and pointer ops | `git lfs` child process + pointer parsing in Rust | `GTGitLFS*` family + bundled `git-lfs` |
| Git binary management | `engine::gitbin`: find system/bundled git, parse `git --version`, gate on a minimum, produce a diagnostic report | `GTGitBinaryManager/Finder/Validator/ReportCreator` |
| SSH transport | libgit2 with libssh2 (git2 `ssh` feature); passphrase prompting through the activity prompt round-trip | `GTGitRemoteConnectionPrivateKeyNotFoundError`, `...SSHAgentNotRunningError` |
| Authentication for HTTPS | git2 credential callbacks backed by the credential store and Keychain, with a "connection doctor" command that explains failures | `GTGitValidateRemoteConnectionOperation`, `GTGitRemoteConnectionDoctor` |

Child processes get: a progress parser, a cancellation token, a timeout, output capture into the
activity log, and exit-code → typed-error mapping. This is the `FNGitCommandDelegate` /
`FNGitCommandProgress` / `FNGitCommandCancelling` trio, re-created.

## 13. Frontend architecture

- **Shell** (`app/`): window chrome, sidebar, toolbar zones (global verbs / per-pane list header /
  sidebar footer — `EV:analysis/visual-design#F13`), the dialogue host, and the two utility windows
  (Activity, Quick Open / Quick Actions).
- **Stores** (`stores/`): `repos`, `repository` (selected), `workingCopy`, `history`, `refs`,
  `stashes`, `submodules`, `worktrees`, `activities`, `settings`, `services`, `themes`. Stores
  subscribe to `AppEvent`s and issue commands; they never call `invoke` directly — a thin
  `bridges/commands.ts` (generated) is the only call site, which keeps docs/03 authoritative.
- **Lists**: one virtualized list component per kind with the standard trio of states
  (loading / empty / data) plus the large-content warning state — the systematic convention found
  in the evidence (`EV:analysis/visual-design#F10`).
- **Diff view**: our own renderer over `DiffDocument` (domain type): rows are `HunkHeader | Context
  | Added | Removed | NoNewline`, with staged/unstaged duality resolved by the store; chunk
  selection emits `stageChunk`/`unstageChunk`/`discardChunk` commands with the hunk's
  `(old_start, old_lines, new_start, new_lines)` and the file path — the same shape the selectors
  imply (`EV:sel/stageChunk`).
- **Graph**: `engine::history::draw_graph` returns `Vec<GraphLaneSegment>` (`lane`, `from_row`,
  `to_row`, `edge_kind`) computed in Rust (evidence: lane indexes are computed engine-side,
  `EV:analysis/git-semantics#F7`); the web layer draws them; lane colour = `palette[lane % 9]`
  (nine hues in the evidence, docs/04 §2.5).
- **Menus/keyboard**: the IB selector inventory (`EV:sel/*`) is the checklist for the default-action
  pattern: every row/list has one default action plus an options dropdown, and the quick-action
  palette exposes the same action set contextually (`GTQuickActionObjectName*`).

## 14. Error model

```rust
pub struct ErrorPayload {
    pub code: ErrorCode,             // stable machine code, one per error class
    pub message: String,             // user-facing, from our own catalogue (no Tower copy)
    pub detail: Option<String>,      // technical detail for the log/copy-report action
    pub params: BTreeMap<String, String>,   // message interpolation (path, ref name, …)
    pub retryable: bool,
    pub domain: ErrorDomain,         // Git | Service | Fs | Internal | Cancelled
}
```

`ErrorCode` carries the 50 observed engine error classes as *classes* (not names): index-locked,
invalid-repository, no-working-copy, authentication, push-rejected, keychain-item-not-found,
private-key-not-found, ssh-agent-not-running, gpg-binary-not-found, gpg-signature-missing/untrusted/
failed, merge-conflict-not-unmerged-entry, unknown-revision, multiple-ref-names, delete-unmerged-branch,
rewrite-would-lose-{unpushed,mainline,merge}-commits, worktree-locked, worktree-has-changes,
binary-{path,version,minimum-version}, service-{rate-limit,two-factor,login-limit,expired,permission,
not-found,invalid-request,invalid-response,transport,server}. Every one of these classes has an
evidence citation in docs/00 §4.2.11 and §4.3.30. The UI renders `code` → icon/tone, `message`
verbatim.

## 15. Testing strategy

- **Fixture repositories**: generated by script into `tests/fixtures/` (deterministic bundles:
  linear, merged, conflicted, submoduled, worktree'd, LFS-marked, signed/unsigned commits). Every
  fixture is created by git itself in a sandbox, never committed as a binary blob except where an
  LFS pointer or signed object must be byte-exact.
- **Engine tests**: property tests for transformers (status mapping round-trips), golden tests for
  graph lane computation, diff document generation, and conflict-kind classification.
- **Command tests**: for each command in docs/03, a table-driven test asserting (a) validator
  rejects the documented preconditions, (b) execute on the right fixture produces the expected
  repository state (verified by re-reading through libgit2, not by inspecting our own output).
- **Queue/activity tests**: cancellation mid-fetch, watchdog firing, queue ordering under concurrent
  commands.
- **UI tests**: component tests for state trios; a screenshot harness for the pane inventory
  (visual review only — no pixel targets exist in the evidence).
- **Fuzz**: parse targets for patch application, theme files, custom-service templates, and deep
  links.

## 16. Security and privacy

- Secrets live only in Keychain; the Rust process never writes them to SQLite or logs. Log redaction
  is a compile-time-tested filter list (token-shaped strings, `https://user:pass@` in URLs).
- `shell` plugin scope is limited to the exact executables configured (git, git-lfs, diff/merge
  tools, terminal app, ssh programs); no arbitrary shell.
- Deep links are parsed by a closed verb table; anything unrecognised is refused and logged
  (mirrors the observed "unimplemented verb" resilience).
- No telemetry by default. If we ever add crash reporting it is opt-in with a visible switch, and
  the decision is ours, not inherited (docs/00 §7).
- File watching is read-only; we never write into a repository outside git mechanisms (§9).

## 17. Build order (recommended)

| Phase | Deliverable | Capabilities |
|---|---|---|
| P0 | Skeleton: Tauri app, command layer, queue, events, activity registry, SQLite+keychain store, generated bindings | infra |
| P1 | Repository open/discover/init; refs; status loads; config reads; git binary management | G1-01/02, G17-05 |
| P2 | Working copy pane: file list, status, diffs, stage/unstage/discard (file + chunk), ignore/assume-unchanged/untrack, open-with | G2 all |
| P3 | Commit composer: commit/amend/undo, sign-off, warnings, changeset view | G3-01..07/11 |
| P4 | History pane: commit list, graph lanes, detail, changeset, diff options, copy verbs | G4-01..06/10/11 |
| P5 | Remotes: CRUD, fetch/pull/push/force-push/sync, publish, activities window, auto-fetch | G6 |
| P6 | Branches: create/checkout/rename/delete, tracking, pin, sidebar grouping, fast-forward, update-branch | G5-01..08 |
| P7 | Stash and tags | G8, G9 |
| P8 | Integrate: merge/rebase/cherry-pick/revert/reset, conflict UI, integration state machine, validators, snapshots + undo | G7-01..09/11 |
| P9 | Forensics: file history, blame, blob, reflog; profiles; security (SSH/GPG/signing); preferences; quick actions; quick open | G14, G15, G16, G17-01..05/09, G18-01/02 |
| P10 | Submodules, worktrees, LFS ¹, git-flow classic, AI commit messages | G10, G11, G13-01, G3-10 |
| P11 | Services/PRs (one adapter first: GitHub; then GitLab/Bitbucket/Azure DevOps; then legacy hosts and Custom) | G12 |
| P12 | Themes, deep links + CLI, onboarding, system workflows (Graphite/Gerrit/SVN/GitFlow Next/custom), updates | G17-06..08, G18, G13-02..06 |

¹ LFS has no capability row of its own in docs/01: the evidence shows it as a transfer + tracking
surface (`EV:analysis/git-operations#F13`) that is folded into G2 (tracked patterns in file context
menus) and G6 (transfer during clone/fetch/pull/push). It is scheduled in P10 rather than earlier
because it needs the child-process supervisor from P5.

Each phase ends with its docs/03 commands and docs/04 screens complete — no partial commands in the
surface.

## 18. What would change for a future non-macOS port

Only §1 and §9 are platform-bound: WKWebView/webview choice, Keychain, and the "default terminal
app" integration. The domain/engine/queue/activity layers are portable as written; the services and
workflows layers are already HTTP/CLI-based. The design deliberately keeps all platform calls behind
`store::secrets`, `platform::terminal`, `platform::reveal`, and `platform::deeplink` modules so a
port is a swap of four modules, not a rewrite.
