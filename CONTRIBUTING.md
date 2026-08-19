# Contributing

## Ground rules

**The UI lives inside Conductor.** The panel is injected into Conductor's own
toolbar and composer and drawn in its theme; the chat card is drawn by
`mcp__conductor__AskUserQuestion`. Both are inside the app, which is the point of
the project.

Outside it, nothing may prompt: no `osascript`, no system dialogs, no windows of
our own. The router runs on every agent spawn, so anything that blocks there
blocks Conductor.

**The router must fail open.** A broken or missing `_resolve.sh` costs you the
routing, never the agent. All decision making happens inside one subshell whose
failure leaves `DECISION` empty, and the router still `exec`s the real binary.
There are tests for this; keep them passing.

**Only documented Conductor settings.** `claude_code_executable_path`,
`environment_variables` and `scripts.*` are in Conductor's published schemas.
Reading Conductor's SQLite database, patching its bundle or talking to its
sidecar socket are all out of scope. See
[docs/patching-conductor.md](docs/patching-conductor.md).

**POSIX shell, no dependencies.** `bin/_resolve.sh` and both routers are `/bin/sh`
and run on every spawn, so no bashisms and no forks on the fast path.
`bin/conductor-acct` may use bash 3.2, which is what macOS ships.

## Running the tests

```sh
test/run.sh              # everything
test/run.sh route        # only tests whose name contains "route"
```

Each test gets a fresh sandbox under `$TMPDIR`, with `CONDUCTOR_ACCOUNTS_ROOT`,
`CONDUCTOR_ACCT_SETTINGS_FILE` and stub agent binaries pointed at it. No test
may touch a real Conductor install, `~/.claude`, `~/.conductor` or the keychain.
If you need a new escape hatch to keep that true, add it as an environment
variable rather than a code path that only runs under test.

A test is a shell function named `test_*`. The harness discovers them, so there
is no list to update.

```sh
test_your_thing() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    is "what you are asserting" "$(route_claude "$SANDBOX/ws-a")" \
        "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}
```

Helpers: `is`, `contains`, `fake_profile`, `route_claude`, `route_codex`, `skip`.

## Changelog and versions

Commits carry the detail; `CHANGELOG.md` carries the version-level view. Add an
entry under the current heading when a change is worth knowing about at release
level, which is most of them.

`CONDUCTOR_ACCT_VERSION` in `bin/conductor-acct` and the version in
`tools/ui-patch/account-ui.js` must match: the CLI and the injected panel ship
together, so a skew between them is a bug rather than a variation. A test asserts
both, and that the changelog has a heading for that version.

## Linting

```sh
shellcheck -x --source-path=SCRIPTDIR \
  bin/conductor-acct bin/_resolve.sh bin/claude-router bin/codex-router \
  lib/*.sh test/run.sh test/harness.sh test/*.test.sh \
  install.sh
pnpm typecheck
```

`-x` follows the sourced libraries rather than guessing at them, and
`--source-path=SCRIPTDIR` resolves each `# shellcheck source=` directive relative
to the script carrying it, so the command passes from any working directory.
Zero findings is the bar: shellcheck exits non-zero on info-level notes too.

CI runs these and the test suite on macOS and Linux. Its workflow lives at
`.github/workflows/conductor-hats.yml`, at the **repository root**,
because GitHub only runs workflows from there; a copy inside this directory
would be ignored silently.

## Cutting a release

```sh
tools/set-version.sh 0.3.2      # five files, plus the changelog heading
git commit -am "chore: 0.3.2"
git tag -a v0.3.2 -m "hats 0.3.2"
git push && git push origin v0.3.2
```

The tag is what publishes. `.github/workflows/release.yml` builds both macOS
targets, packages the binary with `bin/`, `lib/`, `commands/` and `install.sh`,
and attaches a tarball, a `.sha256` per target and a combined `sha256.sum`.

Then check the artifact rather than the workflow, because v0.3.0 shipped without
an installer and the workflow looked fine:

```sh
gh release download v0.3.2 --pattern '*aarch64*'
shasum -a 256 -c hats-aarch64-apple-darwin.tar.gz.sha256
tar xzf hats-aarch64-apple-darwin.tar.gz && cd hats-aarch64-apple-darwin
S=$(mktemp -d)
CONDUCTOR_ACCOUNTS_ROOT="$S/accounts" CONDUCTOR_HATS_BINDIR="$S/bin" \
  CONDUCTOR_ACCT_SETTINGS_FILE="$S/settings.toml" \
  CONDUCTOR_ACCT_COMMANDS_DIR="$S/commands" ./install.sh
"$S/bin/hats" version
```

The sandbox variables matter: without them the check installs over your own
setup.

## Manual checks before a release

Routing is exercised by the tests, but sign in and concurrency need a real
machine and two real accounts:

1. `conductor-acct add personal` and `conductor-acct add work` produce two
   different emails in `conductor-acct list`. Same email twice means the config
   directory isolation broke.
2. `conductor-acct use` in two workspaces, then a chat open in each at once,
   both answering, each reporting its own `CLAUDE_CONFIG_DIR`.
3. `conductor-acct uninstall` leaves `~/.conductor/settings.toml` valid TOML
   with your other settings intact.

## Commit messages

Conventional Commits. Keep the subject under 50 characters and explain why in
the body when it is not obvious from the diff.
