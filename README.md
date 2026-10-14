# Meridian

A Git client for macOS. Tauri + Vue 3 on a `git2` (libgit2) engine.

**Free and open source** — MIT, no paid tier, no telemetry, no account required.

Built from a reverse-engineering study of Git Tower 18 (see `docs/00-evidence-summary.md`).
Mechanism was studied; no assets, code, or text were copied.

## Download

Grab the latest `.dmg` from [Releases](../../releases/latest). One universal
build covers Apple silicon and Intel, and macOS 13 (Ventura) or later is
required.

Meridian updates itself: shortly after launch it asks GitHub for this
repository's latest release tag, and nothing else. The switch that turns that
off lives in **About Meridian → Updates**, beside a full description of what the
request carries. Building from source works too:

```sh
pnpm install
pnpm tauri build            # → src-tauri/target/release/bundle/
```

## What it is

A working Git client, not a preview. The capabilities below are driven by a
`git2`/libgit2 engine through a typed command layer and are covered by the test
suite that gates every change.

- **Repositories** — open and initialise, with a near-by repository scan, recent
  list, and per-repository state and health reporting.
- **Working copy** — status, staging and unstaging by path, staging everything,
  discarding changes, per-entry diff, ignore and untrack, the assume-unchanged
  flag, and image diffs.
- **Commit** — create, amend, and undo the last commit.
- **History** — the commit list with its graph, single-commit detail, change
  sets, contributor counts, file history, revision ranges, search, sorting,
  first-parent filtering, and blame.
- **Branches** — list, create, checkout, rename, delete, update, fast-forward,
  stacked-branch creation and restacking, upstream tracking, and checking out a
  remote branch or an arbitrary revision.
- **Remotes** — add, rename, delete, change URL, edit refspecs, fetch, pull,
  push, force-push, prune, and publish a branch. HTTPS and SSH transports are
  both built in, with `credential.helper` and `useHttpPath` honoured, including
  a helper's `quit=1` veto.
- **Tags** — list, detail, create, delete, publish, and delete on the remote.
- **Stashes** — save, list, apply, pop, drop, and show.
- **Submodules and worktrees** — list, initialise, update, sync, per-submodule
  settings; list, add, remove and lock worktrees.
- **Integration** — merge, rebase including interactive plans, cherry-pick,
  revert, reset to a revision, and resolve/continue/skip/abort flows.
- **History rewriting** — squash, fixup, move and delete revisions, with an
  impact report before anything is changed.
- **Conflicts** — a conflict list, per-hunk resolution, whole-file resolution,
  and launching an external diff or merge tool.
- **The application itself** — version and build provenance, a one-click
  self-update that shows download progress, the full release history rendered
  from the bundled `CHANGELOG.md`, and bug/feature buttons that open a
  pre-filled GitHub issue after showing you exactly which details were filled in.

### Languages

Simplified Chinese and English, Chinese by default, switchable in
**Settings → Appearance → Language** or from **About Meridian**. The choice is
remembered on the machine. Both changelogs are compiled into the binary, so the
release notes read offline and in either language.

### Privacy

No telemetry, no account. The optional update check asks GitHub for the latest
release tag and carries the app version only. When you choose to report a bug,
Meridian builds a URL and opens it in your browser; if you include a diagnostics
report, that report is in the URL. Meridian never submits the issue. Review the
form and its URL before you choose to submit it in GitHub.

### Debugging and reporting

Open **About Meridian → Diagnostics** to turn on debug mode, inspect recent
failures and command calls, read the log, or copy/export a report. To start the
installed app in debug mode from Terminal, run
`MERIDIAN_DEBUG=1 /Applications/Meridian.app/Contents/MacOS/meridian`. The log is
`~/Library/Logs/dev.meridian.app/meridian.log`; it rotates automatically, and
logs and rotated files stay on your Mac.

Logs record command names, outcomes, durations, error details and argument key
names. Argument values are included only in debug mode for a small allowlist of
harmless scalar fields. File contents, diffs, commit messages, credentials and
credential-helper output are never logged. Reports mask your home folder, URL
logins and token-shaped strings, but automatic redaction is not perfect: read a
report before sharing it. Copy the report, or export it to Downloads and attach
the `.txt` file to a bug report you review and submit yourself in GitHub. If the
report does not fit in the issue URL, the form marks it as truncated and asks you
to attach the exported file instead.

打开「关于 Meridian → 诊断」可开启调试模式、查看最近失败和命令调用、阅读日志，或
复制/导出报告。也可以在终端运行
`MERIDIAN_DEBUG=1 /Applications/Meridian.app/Contents/MacOS/meridian` 以调试模式启动应用。
日志位于 `~/Library/Logs/dev.meridian.app/meridian.log`，会自动轮换；日志及轮换文件只保存在
你的 Mac 上。

日志记录命令名称、结果、耗时、错误详情和参数键名。只有开启调试模式时，少量无害的标量
参数值才会写入日志。文件内容、差异、提交说明、凭据和凭据助手输出绝不会记录。报告会遮盖
主目录、URL 登录信息和类似令牌的字符串，但自动脱敏并不完美：分享前请先阅读报告。你可以
复制报告，或将报告导出到「下载」文件夹，再把 `.txt` 文件附加到你自行检查并提交的 GitHub
问题报告中。若报告无法完整放入问题链接，表单会标明内容已截断，并提示改为附上导出的文件。

### Not built yet

Pull requests, provider accounts and hosting-service integration; undo and the
snapshot store; the activity registry; running Git hooks on commit. The build
order is `docs/02-architecture.md` §17.

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
  components/         presentational panes and screens
  i18n/               the zh-CN and en catalogues
  stores/             Pinia stores (one per domain)
  styles/             design tokens (docs/04 §2)
.github/workflows/    CI and the release pipeline
```

`CLAUDE.md` documents the layering rules and how to add a command.

## Working on it

Requirements: macOS, Rust (stable), pnpm, Node.

```sh
pnpm install                 # frontend dependencies
pnpm tauri dev               # run the app (starts Vite + the Tauri host)

pnpm typecheck               # vue-tsc --noEmit
pnpm test                    # frontend unit tests (vitest)
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Some of those tests exist to catch drift no compiler can see, and each has
already caught a real omission:

- `src-tauri/tests/handler_completeness.rs` — every `#[tauri::command]` is
  registered in `lib.rs`'s `generate_handler!`.
- `src-tauri/tests/changelog_parity.rs` — the two changelogs list the same
  releases, and the crate version is documented.
- `src-tauri/tests/github_metadata.rs` — the field ids the feedback form
  pre-fills still exist in the issue templates.

## Releasing

Push a tag; the release workflow runs the checks, builds a universal `.dmg`,
signs it, and drafts the release.

```sh
git tag -a v0.1.0 -m "Meridian 0.1.0"
git push origin v0.1.0
```

The workflow publishes a **draft** on purpose: `releases/latest` ignores drafts,
so nothing reaches an installed copy until a person publishes it. That is the
one gate between a bad build and every user's machine, and it is why the app can
check for updates on launch without that being reckless. The release body is
extracted from `CHANGELOG.md`, so the GitHub page and the in-app update notice
cannot disagree.

Signing needs two repository secrets: `TAURI_SIGNING_PRIVATE_KEY` (the contents
of the minisign private key) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. An update
signed with a different key is rejected by every installed copy, so the key must
not be regenerated once a release is public.

## License

MIT — see [LICENSE](LICENSE).

Contributions welcome: open an issue describing the capability, or a PR against
the build order in `docs/02-architecture.md` §17. Bug reports can also be filed
from inside the app, under **About Meridian → Feedback**.
