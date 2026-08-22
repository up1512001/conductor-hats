# Changelog

Notable changes, newest first. Individual commits carry the detail and the
reasoning; this is the version-level view.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Security

- **A profile name could escape the accounts root.** Validation existed but was
  called from one place, so `hats remove ../../foo` resolved outside the root and
  recursively deleted whatever was there. A crafted `--session-id` wrote a pin
  file to any path the user could write to. Every identifier that becomes a path
  component is now validated in one module, at each of the five boundaries it can
  arrive through: argv, `CONDUCTOR_ACCOUNT`, the session id Conductor passes, the
  pin file, and the routes file. `remove` checks containment before deleting.
- **A bad signature no longer reports success.** `patch`, `revert` and `dev-app`
  printed `INVALID` beside the signature and exited 0, so an application macOS
  would refuse to launch looked installed.
- **Signing no longer falls back to dropping entitlements.** The retry produced
  the worst outcome available: a bundle that verifies and dies the moment the
  WebView needs to JIT.
- Entitlements were written to a fixed path in a world-writable temporary
  directory. They now go in a directory created with `create_dir`, which fails
  rather than follows if the name is taken.
- Every offset read out of a Mach-O is bounds-checked. A malformed binary
  produces an error rather than a panic or a read past the buffer.

### Added

- **An account per chat, not just per workspace.** A workspace holds many chats
  and each runs its own agent process, but the panel only ever read the
  workspace route, so every chat in a workspace claimed the same account and
  switching from the toolbar appeared to move all of them while the running ones
  stayed exactly where they were. The panel now reports what the chat on screen
  actually resolves to, and a scope switch says which layer a selection writes
  to. `hats pin`, `hats unpin` and `hats session` expose the same thing in the
  terminal, and `hats json` carries `session`, `chat` and `pinned`.

  A pin cannot move a conversation that is already running: its agent took a
  config directory when it spawned and never reads one again. It decides the
  next process Conductor starts for that chat, and every message that mentions
  it says so.

  Which chat is live is read from the newest transcript in the workspace's
  project directory. Two written within two seconds of each other are both
  plausibly on screen, so that case is refused rather than guessed at.

### Fixed

- **Concurrent account changes lost each other.** Every workspace shares one
  routes file and each write read it, edited a copy and wrote it back. With the
  lock removed, 100 concurrent writers lost 9 routes. Writes now take a lock and
  land by atomic rename.
- **A failed sign-out no longer deletes the account anyway.** `login`, `logout`
  and `remove` discarded the agent's exit status. `remove` now refuses when
  sign-out fails, and `--force` deletes with a warning that the provider may
  still consider the account signed in.
- **Uninstall no longer edits somebody else's TOML table.** The settings keys were
  matched by name anywhere in the file. Install followed by uninstall now returns
  Conductor's settings byte for byte.
- **The account address is parsed rather than scanned.** Looking for the literal
  `"emailAddress"` returned the wrong value on a state file that mentions the key
  before the field.
- **Patching is transactional.** The live binary was overwritten before the
  patched image existed. It is now built from the pristine copy and renamed into
  place in one step, so a refusal leaves the previous installation byte for byte.
- **The bundle is identified rather than guessed.** The patcher fell back to the
  largest JavaScript asset, which in a future build would produce an application
  that launches with a rewritten bundle and no panel. It now requires exactly one
  `renderApp` asset carrying the expected anchors, and refuses otherwise.

### Changed

- **The CLI and the routers are Rust.** The shell implementation is gone,
  replaced by modules in `src/rust`. One binary answers to three names:
  `install.sh` symlinks `claude-router` and `codex-router` at `hats`, and it
  reports itself as whichever name invoked it.
- **The test suite is `cargo test`.** 1550 lines of shell became 96 integration
  tests sharing one sandbox. `tools/set-version.sh` is now
  `examples/set-version.rs`. `install.sh` is the only shell left in the
  repository, because it runs before there is a binary to run.
- CI runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test --all`.
- A release tarball is the binary, `install.sh` and `commands/`. No shell
  implementation travels with it.
- The router fails open through `catch_unwind` rather than a subshell, and
  `panic = "abort"` is out of the release profile: an abort there would mean no
  agent starts at all, which is the failure fail-open exists to prevent.
- The README leads with the one-line install now that the repository is public
  and it works.

## 0.3.4

### Fixed

- **A symlinked `conductor-acct` could not find its libraries.** `install.sh` puts
  one on `$PATH`, and the CLI resolved `$0` without following it, so it looked for
  `lib/` beside the symlink and every command died on a missing `_resolve.sh`. The
  CLI and both routers follow symlinks now. Found by installing the release for
  real: the sandbox checks called the target directly and never saw it.

### Added

- `install.sh` falls back to the GitHub CLI when an anonymous download fails, so
  it works against a private repository for anyone signed in with `gh`. The
  public `curl` path is unchanged and still tried first.

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
