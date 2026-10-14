# CLAUDE.md — working in Meridian

Meridian is a macOS Git client: Tauri 2 host + Vue 3/TypeScript frontend, with
a `git2` (libgit2) engine behind a strict layering. The specification lives in
`docs/` — read `docs/02-architecture.md` before structural changes, `docs/03-command-surface.md`
before touching commands, `docs/04-ui-spec.md` before touching the UI. This file
is the short version: the rules that must hold in every change.

## Layering (do not bypass)

```
src-tauri/src/
  domain/      plain data. MUST NOT depend on git2, tauri, or rusqlite.
  engine/      git2-backed operations. MUST NOT depend on tauri.
  transform/   pure git2 ↔ domain mapping functions (the only place a git2
               value becomes a domain value).
  validation/  pure precondition checks; read the repo, never mutate it.
  commands/    the Tauri command surface. May depend on everything below;
               NOTHING below may depend on commands.
  queue.rs     one serialized queue per repository (mutating commands).
  events.rs    app-wide event bus (Tauri emit).
  state.rs     the open-repository registry + AppState::run.
```

Invariants:

- No `git2` type crosses upward out of `engine`/`transform`. Commands and the
  web layer only ever see `domain` types and `ErrorPayload`.
- The web UI may only see types defined in `domain` (serialized) and the
  command/event surface.
- Meridian writes git-native data only through git mechanisms (refs, config,
  index, objects). UI-only metadata (pins, archive flags) goes to SQLite in a
  later phase — never into the working tree (docs/02 §9).

## Adding a command (the tri-pattern)

Every user-invocable verb is declared once, mirroring the evidence's
Action → Validator → Operation triad. To add one:

1. **Domain types** — add the output/args vocabulary to `domain/` (serde types,
   no git2). If a new error class can occur, add its `ErrorCode` (codes are
   added, never renamed).
2. **Engine operation** — implement the git2 work in `engine/<module>.rs`,
   returning domain values via `Result<T, ErrorPayload>` (`?` on any
   `git2::Error` converts through `engine::error`).
3. **Validator** — add the preconditions to `validation/<module>.rs` as a pure
   function over `(cx, args)`, returning `Result<(), Violation>`. Sender-scoped
   preconditions branch on `cx.sender` (e.g. toolbar defaults vs. explicit
   dialogue arguments); a `Violation` converts to `ErrorPayload` at the command
   boundary.
4. **Command descriptor** — in `commands/<module>.rs`, implement the `Command`
   trait: `repo_id()`, optional `sender()`/`risk()`, `validate()`, `execute()`.
   Declare `Risk::Destructive | Rewrite` honestly — snapshot policy and undo
   will key off it. Emit domain events from `execute` via `events::emit`.
5. **Tauri wrapper** — a `#[tauri::command]` fn that just calls
   `state.run(&app, args).await`, plus its registration in
   `src-tauri/src/lib.rs` (`generate_handler!`) and the typed wrapper in
   `src/bridges/commands.ts`.
6. **IPC contract and flow test** — add the command and its input/output
   vocabulary to `docs/03-command-surface.md`. Add a flow test that follows the
   command from the wrapper through validation and execution to its result, so
   the contract cannot drift from the shipped behaviour.
7. **Diagnostics and visible failures** — route frontend calls through the
   shared instrumentation in `src/bridges/commands.ts`, which records argument
   key names by default and only safe, allowlisted scalar values in debug mode.
   Catch blocks use `reportFailure(error, source)` and keep what it returns;
   components that render an error call `useFailureClaim` (through `<ListShell>`'s
   `error` prop, or directly) so a failure is quiet only while something on screen
   is showing it. Logging an error alone is not handling it, and neither is
   marking it handled from the catch block: the pane that would render it may not
   be mounted. See `src/diagnostics/failures.ts` for the net, `claim.ts` for the
   renderer's side, and `logSink.ts` for the bridge to the local log.
   On the Rust side, every command writes an outcome line (`logging::instrument`),
   the outcome log and the toast vocabulary both use `errors.byCode` /
   `errors.byClass`, and the redaction rules are the ones in
   `src-tauri/src/logging.rs` — kept equal to `src/diagnostics/recorder.ts` by the
   shared fixture `src-tauri/tests/fixtures/redaction-contract.json`.

Orchestration lives only in `AppState::run` (docs/02 §3): resolve RepoId →
enqueue on the repo's queue → validate → (policy/snapshot hooks) → execute on a
blocking thread with the repo lock → result. Never do work directly in a Tauri
wrapper past the two lifecycle commands, and never lock the registry from a
command body.

## Rust conventions

- Command arguments: one struct per command, `#[serde(rename_all = "camelCase")]`,
  every field the frontend can omit carries `#[serde(default)]`. Error codes
  stay `snake_case` because they are stable machine identifiers.
- Errors: always `ErrorPayload { code, message, detail, params, retryable, domain }`.
  `message` is user-facing; `detail` is for the log; `params` carries
  interpolation values (paths, ref names).
- Identity: `RepoId` wraps the canonicalised **gitdir**. Worktrees of one repo
  share the queue key; the UI addresses repo+workdir.
- The queue serializes mutating commands per repository. Read-only loads
  (`wc_status_load`, `history_commits_load`, `branch_list`) are idempotent and
  may bypass it — do not add mutations to that path.
- `progress.rs` types are declared but not yet wired; the activity registry
  (docs/02 §6) lands with remote operations. Do not half-use them.

## Frontend conventions

- Call graph: components → `composables/` (behaviour) → `stores/` (state) →
  `bridges/commands.ts` (the only `invoke` call site). Nothing else may call
  `invoke` directly — `src/networkGuard.test.ts` fails the build if one appears;
  `bridges/bindings.ts` is the typed mirror of the Rust command surface
  (docs/02 §1 schedules `tauri-specta` to generate it — until then, keep it in
  sync by hand in the same change that adds a command).
- The webview may not reach the network: `src/networkGuard.test.ts` pins that
  too, alongside the CSP in `src-tauri/tauri.conf.json`. The one request the app
  makes (the update check) is made from Rust.
- Stores hold domain types only, one store per domain module. They react to
  `AppEvent`s from `bridges/events.ts`; they never poll.
- Every async list renders the four-state contract from `stores/loadState.ts`
  (`loading | empty | error | data`) — no list may skip a state.
- Styling uses the design tokens in `src/styles/tokens.css` only (docs/04 §2:
  light on `:root`, dark on `.theme-dark`, every light token has a dark
  counterpart). No raw hex values in components.
- Status strings are computed in Rust (`EntryStatus.view`, the "one enum, three
  renderings" rule) — the UI renders `view.short/text/tone` and never derives
  labels itself.

## Commands

```sh
pnpm install
pnpm tauri dev                            # run the app
pnpm typecheck                            # vue-tsc --noEmit
pnpm build                                # frontend → dist/ (needed before cargo check)
cargo check --manifest-path src-tauri/Cargo.toml
```

`dist/` must exist for `cargo check` to pass — `tauri::generate_context!()`
embeds it. Run `pnpm build` first on a fresh checkout.

## Scope note

This tree is the P0 skeleton (docs/02 §17). Policies, snapshots/undo, the
activity registry, storage (SQLite + Keychain), services, and workflows are
specified but not present; add them along the same module boundaries rather
than inventing new ones. `docs/03-command-surface.md` rows that P0 has not
wired must either be implemented properly or refuse loudly (as
`history_commits_load` does for `to`/`path`) — never silently ignore an
argument.
