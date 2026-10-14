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
release tag and carries the app version only. Nothing else in the app opens a
network connection of its own: the window runs under a content-security policy
whose only network destination is the app's own IPC bridge, and a test fails if
the frontend ever grows a `fetch`, an `XMLHttpRequest` or a `WebSocket`.

When you choose to report a bug, Meridian builds a URL and shows you exactly
what will be in it before anything is opened. The query string reaches
github.com as soon as that page loads — before you press Submit — so the
confirmation is the review, and **取消 / Cancel** sends nothing at all. Meridian
never submits the issue; you read the form and submit it yourself.

### Debugging and reporting

Open **Diagnostics** from the toolbar (or **About Meridian → 诊断**) to turn on
debug mode, inspect recent failures and command calls, read the log, or
copy/export a report. To start the installed app in debug mode from Terminal,
run `MERIDIAN_DEBUG=1 /Applications/Meridian.app/Contents/MacOS/meridian`. The
log is `~/Library/Logs/dev.meridian.app/meridian.log`; it rotates at 2 MB keeping
the live file plus `.1` and `.2`, and logs, rotated files, the crash note and the
debug-mode setting are readable by their owner only (`0600` in a `0700` folder).
If the folder cannot be created, Meridian keeps logging in memory and says so in
the log itself rather than failing to start.

Logs record command names, outcomes, durations, the error code, domain, message
and detail, and argument *key names*. Argument values are included only in debug
mode, only for a small allowlist of harmless scalar fields, and only for
Meridian's own log targets — third-party crates stay at warnings. File contents,
diffs, patch text, commit messages, remote URLs with credentials and
credential-helper output are never logged. Reports mask home directories, URL
logins and token-shaped strings on both sides of the app — the backend and the
webview run the same redaction rules, checked against one shared test fixture —
but automatic redaction is not perfect: read a report before sharing it. Copy the
report, or export it to Downloads and attach the `.txt` file to a bug report you
review and submit yourself in GitHub. If the report does not fit in the issue
URL, the form marks it as truncated and asks you to attach the exported file
instead.

**Every failure is visible.** A command that fails, a pane that cannot render
its error, an exception inside the app's own code and a crash from the previous
run all end up in the same place: the recorder, the local log, and something you
can see — a toast with the reason, the inline error arm, or a crash banner at
launch. The rule behind it: a failure is quiet only while something on screen is
actually showing it.

工具栏上的「诊断」（或「关于 Meridian → 诊断」）可开启调试模式、查看最近失败和命令调用、
阅读日志，或复制/导出报告。也可以在终端运行
`MERIDIAN_DEBUG=1 /Applications/Meridian.app/Contents/MacOS/meridian` 以调试模式启动应用。
日志位于 `~/Library/Logs/dev.meridian.app/meridian.log`；每到 2 MB 轮换一次，保留当前文件
以及 `.1`、`.2` 两份。日志、轮换文件、崩溃记录和调试模式设置仅本人可读写（`0700` 目录中的
`0600` 文件）。若日志目录无法创建，Meridian 会退回到仅在内存中记录，并在日志里说明原因，
而不是拒绝启动。

日志记录命令名称、结果、耗时、错误码与分类、错误信息与详情，以及参数**键名**。只有开启
调试模式时，才会把少量无害的标量参数值写入日志，且只限 Meridian 自身的日志目标——第三方
库最高只记录警告。文件内容、差异、补丁文本、提交说明、含凭据的远程地址和凭据助手输出绝
不会记录。报告在前后端都会遮盖主目录、URL 登录信息和类似令牌的字符串——两侧使用同一套
脱敏规则，并由同一份测试清单校验——但自动脱敏并不完美：分享前请先阅读报告。你可以复制
报告，或将报告导出到「下载」文件夹，再把 `.txt` 文件附加到你自行检查并提交的 GitHub 问
题报告中。若报告无法完整放入问题链接，表单会标明内容已截断，并提示改为附上导出的文件。

**失败一定会被看见。** 命令失败、面板无法渲染错误、Meridian 自身代码抛出的异常，以及上次
运行崩溃的记录，最终都会出现在同一处：记录器、本地日志，以及你能看到的界面——带原因
的提示、内联错误提示，或启动时的崩溃横幅。背后的规则是：只有当确实有界面正在显示它时，
失败才算安静。

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
  src/commands/       the Tauri command surface (docs/03, §21 for diagnostics)
  src/queue.rs        one serialized queue per repository
  src/events.rs       app-wide event bus (Tauri emit)
  src/logging.rs      local log, redaction, debug mode, crash notes
  src/state.rs        open-repository registry + the run sequence
src/                  Vue 3 + TypeScript frontend
  bridges/            typed command wrappers + event subscription
  diagnostics/        failure net, recorder, report, redaction, log sink
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
- `src-tauri/tests/flows/` — headless command flows against real temporary
  repositories: the wire JSON in, the repository state and emitted events out.
- `src-tauri/tests/fixtures/redaction-contract.json` — the redaction rules both
  sides of the app must agree on (the backend test and a vitest suite both read
  it).
- `src/flows/` — the same idea for the UI: the real `App.vue`, stores and
  catalogues over a programmable Tauri backend, asserting what the user can see
  after a failure.
- `src/networkGuard.test.ts` — the web layer reaches no network, and the window
  it runs in does not allow it to.
- `src/diagnostics/claim.test.ts` — a failure is quiet only while something on
  screen is showing it.

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
