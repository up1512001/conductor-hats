# Changelog

Notable changes, newest first. Individual commits carry the detail and the
reasoning; this is the version-level view.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.3

### Changed

- CI runs on macOS only. Conductor is macOS software, `install.sh` refuses any
  other system, and the tests shell out to `security`, which does not exist on
  Linux, so a Linux run either failed as noise or passed for the wrong reason. It
  was also the slow half: on the last run macOS had finished while Ubuntu was
  still installing shellcheck through apt.
- The README was 351 lines. It is the pitch, a numbered walkthrough from download
  to a working panel, what the panel does, and links. Day to day usage moved to
  `docs/usage.md` and the command reference to `docs/cli.md`.

### Fixed

- `AGENTS.md` and `CLAUDE.md` linked to a parent repository that stopped existing
  when this became its own repo.

## 0.3.2

### Added

- A section for the Conductor team: how this modifies their app, that a copy is
  patched and their binary never redistributed, that routing itself uses only
  documented settings, and where to write if they want something changed.
- CONTRIBUTING documents how to cut a release and how to verify the artifact
  afterwards, since v0.3.0 shipped without an installer while the workflow looked
  correct.

### Changed

- The README gave the CLI a full command reference and left the panel described in
  a paragraph. The panel now has a section of its own, listing what each control
  does, ahead of the section on what patching costs rather than after it.

## 0.3.1

### Fixed

- **The release tarball had no `install.sh`.** It shipped the binary, the router
  and the libraries, so anyone who downloaded it had to read the README and wire
  things up by hand. It is packaged now.
- **`install.sh` installed from a git clone**, which is not how anyone installs a
  release. It detects the architecture, downloads the matching tarball, verifies
  it against the published `.sha256`, deploys the router and puts `hats` on
  `~/.local/bin`. Run from an extracted tarball or a checkout it uses the files
  beside it and downloads nothing.
- README install instructions described the clone rather than the release.

## 0.3.0

### Changed

- **The panel is TypeScript and SCSS now, built with pnpm and esbuild.** It was
  one 1,294 line hand-written script. The artifact still has to be a single
  self-contained file, because it is appended to a compiled bundle with no module
  loader, so the source is many small files and the build joins them. Styles moved
  from a JavaScript array of string fragments to real SCSS partials, compiled and
  inlined at build time; the compiled CSS is byte-identical to what the array
  produced. `dist/` is generated and gitignored, and the build rewrites module
  paths to repo-relative so two builds of the same source produce identical bytes
  on any machine. `pnpm verify` checks both.
- **`bin/conductor-acct` is dispatch only.** 1,360 lines became 13 files under
  `lib/`, one concern each. `install` deploys them beside the CLI and `doctor`
  reports drift in them, which it previously only did for `bin/`.
- **The test suite is one file per area** plus `test/harness.sh`, collected by
  `test/run.sh`.
- No source file exceeds 300 lines, and a test enforces it with no allowlist.
- CI pins every action to a verified commit SHA, runs on Node 24, and uses
  pnpm 11 with its settings in `pnpm-workspace.yaml`.

## 0.2.0, the account panel inside Conductor

Routing worked in 0.1.0 but had to be driven from the chat or a terminal. This
release puts it in the app: a toolbar button next to "Open in" and a chip in the
New Workspace composer, both opening the same panel.

### Added

- **In-app account panel**, injected into a Conductor copy's compiled frontend by
  `hats patch`. Two levels: providers, then that provider's accounts.
  Switch, sign in, sign out and turn routing on or off, without leaving the app.
- **Sign-in without a terminal.** Type a name, the browser opens at the OAuth URL,
  paste the code back into the panel. `claude auth login` prints the URL then
  blocks on stdin, so `login-start` runs it with stdin on a FIFO and `login-code`
  feeds the answer in.
- **`hats dev-app`**, which builds an isolated `Conductor Dev.app`
  with its own bundle identifier, database and keychain items, so nothing here has
  to be tried on a Conductor you rely on.
- **`hats assets`**, which walks the Tauri asset map in `__DATA_CONST`
  to read the compiled frontend out of the binary.
- **`hats repatch`**: one command to re-apply everything after a
  Conductor update, including the two steps that fail quietly if done by hand.
- **Address masking.** Every address the UI renders is masked
  (`fir**ast@ex**e.com`), so a recorded session or shared screenshot cannot hand
  one out. `conductor-acct mask`, `list --mask` and `status --mask` expose the same
  rule to the `/account` chat card; a test asserts the shell and panel
  implementations agree on every case.
- **`conductor-acct update`**, and `ask on` to have a fresh workspace's first chat
  ask which account to use.
- CI at the repository root, covering shellcheck, `node --check` on the injected
  script, and the test suite on macOS and Linux.

### Changed

- **Signed-in state is read from the credentials, not from a cached address.**
  See Fixed; this changes `json`'s shape, which now reports `signedIn` alongside
  `email`.
- **The panel signs out; it never deletes.** Signing out drops that account's
  credentials and leaves the profile, its routes, its session pins and its
  transcripts alone. Deleting a profile stays `conductor-acct remove` in a
  terminal, because it is the one irreversible operation here.
- Destructive confirmation is a dialog with a scrim, not a control that arms on a
  first click. An armed control is still one stray click from acting.
- Provider marks are Conductor's own, extracted from its frontend rather than
  approximated.
- The AppleScript account picker and the menu bar app from early prototyping are
  gone. No `osascript` anywhere, no separate window.

### Fixed

- **An account could ask to sign in again minutes after signing in.** Signed-in
  state was inferred from `.label`, a cached address only written when
  `oauthAccount.emailAddress` is readable from `.claude.json`, which is not always
  populated when a sign-in finishes. A profile with working credentials read as
  signed out for ever. It now checks `.credentials.json` and the keychain item
  keyed on `sha256(config_dir)[0:8]`, which is Claude Code's own resolution order.
  `login-status` had the mirror-image bug and reported a completed sign-in as an
  error.
- **Two profiles on one address take turns signing each other out**, because a
  provider keeps one live token per account. Now reported by the panel during
  sign-in, by `login` on stderr, and by `doctor` for any pair.
- **Nothing in the panel responded to clicks**, and eventually it stopped opening
  at all: pointer events were sealed on the capture phase, which stopped the event
  before it reached the row that was clicked. Sealing happens on the bubble phase
  now.
- **The panel sometimes needed several presses to open.** Two causes: the trigger
  was rebuilt between mousedown and mouseup by Conductor's re-render, so no click
  was ever fired, and the label refresh ran per render pass, spawning a process
  each time so the press's own read queued behind a backlog. Opens on pointerdown
  now, with one shared cached read and a slow refresh timer.
- **The panel jumped as it was used.** Position is measured once on open, the width
  is fixed, the tick has its own slot, and the triggers stay hidden until labelled.
- **Choosing an account dismissed the New Workspace modal**, taking the typed
  prompt with it. The panel mounts inside the dialog.
- **A repository binding was invisible while the router was on**, so a bound
  repository reported "default account". The router never exports a binding because
  Conductor applies it directly; `effective_dir` now falls back to it.
- Nothing was selectable in the panel, because the workspace was read from
  `location` and Conductor's webview runs an in-memory router. It matches the app
  chrome against every `com.conductor*` database instead.
- The loop guard tripped on an inherited `CONDUCTOR_ACCOUNTS_ROUTING`, refusing
  every agent with exit code 70. It counts depth now, so one inherited generation
  is tolerated.
- `install` pointed Conductor at the checkout, so archiving the workspace holding
  it would have stopped every agent starting. It deploys to
  `~/.conductor-accounts/bin` and `doctor` warns if that drifts.
- Every shellcheck finding, on its first run: `$key[` parsed as an array
  expansion, and a test ran `rm -rf` on a path that would have been `/bin` had a
  variable been empty.
- `cursor: pointer` on everything clickable. A popover injected over an app cannot
  borrow the app's affordances.

## 0.1.0, per-workspace routing

- `claude-router` and `codex-router`, spawned by Conductor in place of the real
  agent, resolve which account this workspace should use, export
  `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, and `exec` the real binary with the argv
  they were given. All routing logic runs in a subshell whose failure leaves the
  agent starting normally, so a broken install cannot stop work.
- Precedence: `CONDUCTOR_ACCOUNT`, the session pin, a route on this workspace, a
  repository binding, a parent-directory route, the default.
- `conductor-acct` with `setup`, `add`, `use`, `bind`, `status`, `which`, `list`,
  `login`, `logout`, `remove`, `install`, `uninstall`, `doctor`.
- `/account` in any Conductor chat, which needs no patching and survives updates.
- Test suite running against a `$TMPDIR` sandbox with stub agent binaries, so no
  real Conductor install, `~/.claude` or keychain item is touched.
