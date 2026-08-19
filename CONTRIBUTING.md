# Contributing

## Ground rules

**Nothing may prompt.** No `osascript`, no dialogs, no Terminal windows, no menu
bar items. The router runs on every agent spawn, so anything that blocks there
blocks Conductor. User-facing choices belong in the Conductor chat, through
`commands/account.md` and `mcp__conductor__AskUserQuestion`.

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

## Linting

```sh
shellcheck -x --source-path=SCRIPTDIR \
  bin/conductor-acct bin/_resolve.sh bin/claude-router bin/codex-router \
  test/run.sh install.sh tools/make-dev-conductor.sh tools/repersonalize.sh
node --check tools/ui-patch/account-ui.js
```

`-x` follows the sourced library rather than guessing at it, and
`--source-path=SCRIPTDIR` resolves each `# shellcheck source=` directive relative
to the script carrying it, so the command passes from any working directory.
Zero findings is the bar: shellcheck exits non-zero on info-level notes too.

CI runs both and the test suite on macOS and Linux. Its workflow lives at
`.github/workflows/conductor-multi-account.yml`, at the **repository root**,
because GitHub only runs workflows from there; a copy inside this directory
would be ignored silently. It is filtered to paths under
`conductor-multi-account/`.

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
