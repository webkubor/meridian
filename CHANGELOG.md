# Changelog

Notable changes to Meridian. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The application ships this file and renders it in Settings → Release notes. A
Chinese translation lives beside it in `CHANGELOG.zh-CN.md`, and
`src-tauri/tests/changelog_parity.rs` asserts the two files list the same
versions — so a release cannot be documented in one language and not the other.

No repository URLs appear in this file on purpose: the project slug is derived
from the git remote by `src-tauri/build.rs` and reaches the UI from there, so
there is exactly one place that knows where the project is hosted.

## [Unreleased]

### Added

- **Diagnostics** — an in-app Diagnostics screen with a persistent debug-mode
  switch, recent failures and command calls, and a readable log tail. Logs stay
  on the Mac at `~/Library/Logs/dev.meridian.app/meridian.log` and rotate
  automatically. Debug mode can also be enabled at launch with
  `MERIDIAN_DEBUG=1 /Applications/Meridian.app/Contents/MacOS/meridian`.
- **Failure visibility** — the shared command instrumentation records failures
  locally, and a fallback notification surfaces an unhandled failure instead of
  leaving routine operations to fail silently.
- **Diagnostic reports** — copy a redacted report, export it to Downloads, or
  prefill it into a bug report for review in GitHub. Meridian never submits the
  report or sends diagnostics elsewhere.

## [0.1.0] - 2026-10-14

The first release. It is a working Git client, not a preview: the capabilities
below are driven by a `git2`/libgit2 engine through a typed command layer, and
are covered by the test suite that gates every change.

### Added

- **Repositories** — clone and open, with a near-by repository scan, recent
  list, and per-repository state and health reporting.
- **Working copy** — status, staging and unstaging by path, staging everything,
  discarding changes, per-entry diff, ignore and untrack, and the
  assume-unchanged flag.
- **Commit** — create, amend, and undo the last commit.
- **History** — the commit list with its graph, single-commit detail, change
  sets, contributor counts, file history, revision ranges, search, sorting and
  first-parent filtering.
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
- **Bilingual interface** — Simplified Chinese and English, Chinese by default,
  switchable in Settings and remembered on the machine. Every user-facing
  sentence, including error text, is rendered from a catalogue rather than
  assembled in the backend.
- **Self-update** — Meridian checks the project's GitHub releases shortly after
  launch and installs a newer version on request, showing download progress. The
  check carries the app version and nothing else, and the switch that disables
  it sits next to a full description of what the request sends.
- **Release history** — the complete changelog, in both languages, compiled into
  the binary and readable offline under About Meridian → Release notes.
- **Feedback** — bug and feature buttons that open a pre-filled GitHub issue
  after showing which version, macOS version and architecture details were
  filled in. The app submits nothing; the browser does, after you review it.

### Security

- Credentials are resolved through the repository's configured helpers. The
  engine refuses to authenticate with a bare `Cred::default()` negotiation, and
  a test asserts that no such call site exists in the remote code.

### Known limitations

- Pull requests, provider accounts and hosting-service integration are not
  built. The command surface for them does not exist yet.
- Undo and the snapshot store are not built. Destructive commands are already
  classified for it; the store itself is a later phase.
- The activity registry is not built, so long-running operations report through
  their own call rather than a shared activity list.
- Git hooks are not run on commit, and `skip_hooks` currently has no effect.
- Amend reflog entries carry the repository's configured identity rather than a
  caller-supplied one; `git2` 0.21 exposes no signature parameter on
  `Repository::reference`.
- Reset and branch-delete do not yet warn about unpushed commits.
