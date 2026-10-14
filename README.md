# Meridian

A Git client for macOS. Tauri + Vue 3 on a `git2` (libgit2) engine.

**Free and open source** — MIT, no paid tier, no telemetry, no account required.

Built from a reverse-engineering study of Git Tower 18 (see `docs/00-evidence-summary.md`).
Mechanism was studied; no assets, code, or text were copied.

## Status

P0 skeleton (docs/02 §17). What exists today, end to end:

- The command layer in three parts — command descriptor → pure `validate()` →
  `execute()` on the git2 engine — with the orchestrator sequence
  (resolve → enqueue → validate → execute → emit) in `src-tauri/src/state.rs`.
- Commands: `repository_open`, `repository_init`, `wc_status_load`,
  `wc_stage_paths`, `wc_unstage_paths`, `history_commits_load`, `branch_list`,
  `branch_create`, `branch_checkout` (docs/03 is the full contract; this is the
  first slice of it).
- Per-repository serialized queues, the app-wide event bus, and the Vue shell
  (repositories / working copy / history / branches panes).

Not yet built: policies, snapshots and undo, the activity registry, remote
transport, services/PRs, themes. The build order is docs/02 §17.

## Layout

```
docs/                 the specification (start at docs/02-architecture.md)
src-tauri/            Rust: Cargo workspace root
  src/domain/         plain data — no git2, no tauri
  src/engine/         git2-backed operations
  src/transform/      pure git2 ↔ domain mappings
  src/validation/     pure precondition validators
  src/commands/       the Tauri command surface (docs/03)
  src/queue.rs        one serialized queue per repository
  src/events.rs       app-wide event bus (Tauri emit)
  src/state.rs        open-repository registry + the run sequence
src/                  Vue 3 + TypeScript frontend
  bridges/            typed command wrappers + event subscription
  composables/        the data layer (one per domain)
  stores/             Pinia stores (one per domain)
  styles/             design tokens (docs/04 §2)
```

`CLAUDE.md` documents the layering rules and how to add a command.

## Getting started

Requirements: macOS, Rust (stable), pnpm, Node. 

```sh
pnpm install          # frontend dependencies
pnpm tauri dev        # run the app (starts Vite + the Tauri host)

pnpm typecheck        # vue-tsc --noEmit
pnpm build            # frontend → dist/ (Tauri reads it from there)
cargo check --manifest-path src-tauri/Cargo.toml
```

## License

MIT — see [LICENSE](LICENSE).

Contributions welcome: open an issue describing the capability, or a PR against the
build order in `docs/02-architecture.md` §17.
