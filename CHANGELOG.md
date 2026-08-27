# Changelog

Notable changes, newest first. Individual commits carry the detail and the
reasoning; this is the version-level view.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- **The toolbar shows the provider beside the account**, Claude's mark or
  Codex's, so it says whose account it is naming rather than only which one.
  Inside the panel the heading already says which provider, so nothing is
  repeated on the rows.

### Fixed

- **The toolbar moved every chat in the workspace, not the one it was pressed
  in.** Choosing an account wrote the workspace route and then cleared the open
  chat's pin, so two chats set to two accounts converged on one. A workspace
  holds several chats and each runs its own agent process; pressing a control
  inside one chat now means that chat, and writes a pin. The workspace route is
  left alone, so the others stay where they are.

  Setting every chat at once is still there, as one button under the accounts
  reading `Use <account> for every chat here`. It appears only when the chat's
  account and the workspace's disagree, which is the only time it would do
  anything. It writes the route and clears the pin, because a pin beats a route
  and the route alone would leave the open chat behind.

  Neither moves the conversation already on screen. That has not changed and
  cannot: the agent took its config directory when it spawned.

- **The panel often could not tell which chat was open, and fell back to the
  workspace.** Which chat is live was inferred from transcript timestamps, which
  fails in three ordinary situations: a workspace nobody has typed in for five
  minutes read as idle, two chats answering within two seconds of each other were
  refused as ambiguous, and a conversation resumed after a compaction is filed
  under a different id. Every one of those left the toolbar with no chat to act
  on, which is what made a choice hit the whole workspace.

  Conductor records the open chat itself, in `workspaces.active_session_id`.
  That is read directly now, read-only, filtered to the agent whose accounts are
  on screen so a Codex chat is never offered as the Claude one. Measured against
  the running app, three workspaces that all had a chat open reported "no chat
  active here recently" before this and resolve exactly now.

  The two ids are different namespaces: `sessions.claude_session_id` is what
  reaches the agent's command line, and it wins where it differs from Conductor's
  own, because a pin filed under the other is a file nothing looks up.

  The timestamp scan stays as the fallback, for a Conductor with no database, no
  `sessions` table, or no `sqlite3` to read it with.

- **The panel could not tell which workspace was on screen.** It counted
  workspace ids across React's whole tree and took the commonest. Every sidebar
  row is handed the id of the workspace it links to, so a real window offered 38
  distinct ids and the count picked one that was not open, or gave up and fell
  back to matching names, where the only name it could find was the
  repository's. Measured, not guessed: `hats debug` records it.

  The button is mounted in the open chat's own toolbar, so the components
  enclosing it belong to that chat and the sidebar is nowhere among them. Those
  are read instead. The chat's id is taken from the same place, sweeping each
  enclosing level for the status indicator that reports on it, so a choice can be
  pinned to the chat with no workspace to resolve at all.

- **An account chosen while creating a workspace bound the whole repository.**
  The composer is pressed before the workspace exists, and a binding is one value
  for every workspace under that repository. Creating one workspace on Work and
  the next on Personal therefore left both on Personal, and moved every other
  workspace in the repository with them.

  It records a one-shot instead, spent by the next agent to start in a workspace
  with no account of its own, which then writes itself an ordinary route.
  Verified end to end against the router: two workspaces created one after the
  other on two accounts each keep theirs, and later chats in each inherit the
  right one.

### Security

- **`repatch` quit the real Conductor.** It asked LaunchServices to quit the copy
  by bundle identifier, `quit app id "com.conductor.dev"`. The copy is Conductor
  with one string rewritten, so LaunchServices resolved that identifier back to
  the original and quit that: the real application, and every agent running
  inside it, including whichever one had asked for the rebuild. It also fell back
  to `pkill -f`, whose pattern is a regular expression, so every `.` in an
  application path is a wildcard and the two paths differ by very little.

  Nothing is resolved by name or identifier now. The process list is read and
  only processes whose executable path sits inside the target bundle are
  signalled, compared as a plain prefix. Two refusals sit in front of that:
  rebuilding a bundle onto itself, and quitting an application this process is
  running inside. The second holds whatever else goes wrong, so an agent cannot
  be made to close the window it is working in.

- **Pressing an account before creating a workspace looked inert.** The choice
  was recorded, but the tick stayed where it was, so it read as a dead control
  and the account ended up being set afterwards from inside the chat instead. The
  panel is looking at the repository there, and the check that reports a pending
  choice only answered for workspaces. It answers for anywhere that is not a
  workspace too, since that is the New Workspace view and the choice is precisely
  what the workspace made there will use. A workspace that already exists still
  reports its own account, which a test pins down with a default that matches
  neither choice, so nothing passes by coincidence.

- **The New Workspace view refused every choice.** With no chat and no workspace
  to name, the panel matched the repository, and the toolbar is not allowed to
  bind one, so it said "no chat open here, and this workspace could not be
  identified" and did nothing. There is something to mean there: the workspace
  about to be created. It goes through the same one-shot the composer chip uses,
  never a binding, and the panel says so, `No chat here yet, so this applies to
  the workspace you create next.` The panel now carries no repository binding at
  all, which a test asserts against the built bundle.

- **The panel and the toolbar disagreed in plain sight.** Level one showed
  `provider.current`, the workspace route; the toolbar showed what the chat would
  run on. Where a chat carried a pin the two differed, so the panel said Personal
  while the toolbar beside it said Work, and both were right about different
  questions. Level one reads the same value the toolbar does.

- **The open panel described the place it was opened from, not the one on
  screen.** Moving to another workspace with it open left `This chat in macau`
  above a window showing amman. The panel is redrawn along with the toolbar when
  the workspace or chat changes, and the change is detected on the workspace as
  well as the chat.

- **The toolbar named the account a chat would take next, not the one it is
  running on.** A conversation cannot change account once its agent has spawned,
  so pinning one made the panel report the new account immediately while the
  agent answering carried on under the old: `CLAUDE_CONFIG_DIR` said personal
  while the toolbar said Work. The account an agent takes is recorded as it
  starts, separately from the pin, and that is what the label names. Where the
  two differ the panel says so: which account the conversation is on, and which
  one it will come up on next time it is opened.

- **The toolbar showed the previous workspace's account.** Caching the fiber the
  toolbar climbs from, added to make watching for a change of chat cheap, cached
  the wrong thing. React replaces a fiber on every render and leaves the old one
  holding the props it had at the time, so the panel went on reporting whichever
  workspace was open when the cache was filled: measured against Conductor's own
  database, it answered `pangyo` while `albany` was on screen, every time. The
  element is kept now, never the fiber. React hangs the live fiber off the
  element, so reading it back costs nothing and cannot be out of date.

- **Switching chats felt slow.** Two process spawns stood between the switch and
  the new label: one to turn the workspace id into a path, then one to read the
  account. The first is the same answer every time, since switching chats does
  not change the workspace, so it is now asked once and kept. The chat itself is
  watched on a short interval as well as on DOM mutation, because a switch need
  not touch the toolbar's own subtree and waiting for the observer left the wrong
  account on screen while the window was still. Reading the chat costs a walk up
  a kept fiber and a string compare.

- **The toolbar kept the last chat's account after switching chats.** Each chat
  can be on its own account, so a label left over from the previous one does not
  read as stale, it names the wrong account. The button survives a switch, so
  nothing prompted a re-read and the old label stood until the panel was opened
  by hand. The chat behind the button is checked as the window changes, and the
  label is re-read the moment it differs. The fiber that check starts from is kept
  between calls so it costs nothing, and validated before reuse: React leaves a
  replaced fiber holding the props it had at the time, and reusing one would have
  frozen the label on whichever chat was open when it was cached.

- **`no such directory` where an account should have been.** Conductor records a
  workspace before it finishes making its working tree, so the panel can ask
  about one that does not exist on disk yet, and `json` refused. Nothing in that
  answer needs the directory: routes are matched as paths.

- **A workspace did not keep the account it was created with.** Choosing one in
  the New Workspace composer bound the repository, so the second creation
  overwrote the first and both moved, along with every other workspace under it.
  The choice is recorded instead, and used by the workspaces created after it,
  until another is chosen. Each writes itself an ordinary route as its first agent
  starts, so a later choice cannot move it.

  Getting "created after it" right took three goes, each one measured rather than
  guessed. Any agent could take it, and Conductor starts one with the working
  directory set to `/` before the workspace's own, which swallowed it. Limiting it
  to real workspaces was not enough, because a dozen are open at any time and each
  respawns an agent on a resume, a model switch or a generator restart. So the
  workspaces that already exist are written down beside the choice, and only one
  absent from that list may use it.

  The toolbar reports it before anything has run, too. A workspace created a
  moment ago has no route yet, and showing the default until the first message
  would name one account while the next spawn used another.

- **The panel could not tell which workspace was on screen.** It counted
  workspace ids across React's tree and took the commonest, but every sidebar row
  is handed the id of the workspace it links to: measured on a real window, 4296
  fibers held 38 distinct ids and the winner was a workspace that was not on
  screen at all. The toolbar button is mounted inside the open chat, so the
  components enclosing it are the answer. Its DOM ancestors carry no React key
  here, so the tree is walked from the root for the innermost fiber whose element
  contains the button, and the component chain is climbed from there. The chat's
  own session id comes back with it, and is handed to `hats json`, so the CLI
  answers about the chat in the window rather than inferring one from transcript
  timestamps.

- **A toolbar press could bind the whole repository.** Which command a choice
  ran was decided from `target.kind`, and the target came from matching names
  against the visible chrome. When the repository's name was on screen and the
  workspace's was not, the toolbar wrote a repository binding: one
  `CLAUDE_CONFIG_DIR` in the repository's `.conductor` settings, inherited by
  every workspace in it. Two chats set to two accounts therefore both ended up on
  whichever was chosen last, in every workspace of that repository, which is what
  "I set one to Work and one to Personal and both say Personal" was.

  The choice now follows the control that was pressed rather than the name that
  was found. The composer chip asks for repository scope and is the only thing
  that may bind. The toolbar pins the open chat, falls back to the workspace
  route when no chat is open, and can no longer bind anything.

- **`hats debug` records what the panel resolved.** Off unless turned on, and it
  logs decisions rather than anything typed: the scope asked for, the id found in
  the fiber tree and how much of the tree was walked to find it, the target that
  came back, and what a choice wrote. Diagnosing the panel used to mean injecting
  a probe, and every injection re-signs the copy, which signs it out of
  Conductor.

- **Every lookup in Conductor's database could fail silently.** It is in WAL
  mode, and a WAL database opened read-only fails outright unless its `-shm`
  file already exists, which it does not while Conductor is closed or between a
  quit and the next launch. `sqlite3` exited non-zero with an empty result, and
  an empty result is indistinguishable from an answer: `workspaces`, `repos`,
  `resolve` and the open chat all read as "Conductor knows of none". The panel
  then could not identify the workspace or the chat, and fell back to the
  workspace route, which is the whole-workspace switching this release exists to
  end. A refusal is retried as `immutable=1`, which reads the file without the
  shared index; it is only reached when there is no live index to share.

- **The panel said "Workspace" while acting on a chat.** The scope line reads
  `This chat in <name>` when a chat is open, and the note under the accounts says
  the choice applies to that chat alone and that the running conversation keeps
  what it started with.

## 0.4.0

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

- **`patch` can inject something other than the panel**, for working out what a
  release broke: `--script FILE`, `--asset KEY`, `--prepend`, repeatable, so one
  patch can carry a probe alongside the real panel. `hats guard` prints the boot
  guard as `hats panel` prints the panel.
- **`verify` checks the boot guard**, and finds Conductor's entry module by
  reading index.html rather than guessing at a hash-versioned name.
- **Both injected scripts are checked as ES modules** by the tests. They are
  spliced into modules, where strict mode applies; `node --check` parses its
  input as a script and allows what a module forbids.

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

- **The copy no longer asks for the login keychain password.** A re-signed copy
  looks like a different application to the keychain, so macOS either blocks it
  from reading what the previous build stored, with a password dialog on launch,
  or the items are removed and the copy simply starts signed out. Removing them
  is the choice: a dialog asking for the login password is not something to
  answer by habit. `hats reset-keychain` does it on demand, and a signing
  identity that stays the same between patches would remove both, which is
  written up rather than built.
- **The panel could not see which workspace was on screen.** The port to Rust
  dropped `workspaces` and `repos`, which the panel matches the visible chrome
  against, so every account row disabled itself under "Open a workspace to
  choose its account". Restored, reading Conductor's database read-only.
- **The toolbar button bound the whole repository rather than the workspace.**
  Both names are on screen and the longer one won, which was the repository. The
  target now follows the control pressed: the toolbar button means this
  workspace, the composer chip means workspaces created from here.
- **A patched copy came up blank on Conductor 0.82.** 0.82 renders the whole UI
  only once `GET /minimum-client-version` has settled, and in a re-signed copy
  that query never settles: the window stays empty with nothing logged. `patch`
  now also injects a boot guard, ahead of Conductor's entry module, which answers
  that one request with an error. Conductor already handles a failed check by
  carrying on. The copy therefore does not enforce the minimum client version,
  which is stated in the docs rather than hidden, and should be removed when a
  release settles the query. `verify` reports whether the guard is present.
  The panel, the identifier rewrite, the profile and the network were each ruled
  out against a control first. See [docs/blank-window.md](docs/blank-window.md).

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
- **The test suite is `cargo test`.** 1550 lines of shell became 120 integration
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
