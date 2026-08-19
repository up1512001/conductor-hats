# conductor-hats

Run as many Claude Code or Codex accounts as you like in
[Conductor](https://conductor.build) **at the same time**, one per workspace,
with no signing in and out.

Pick the account per workspace from inside Conductor with `/account`, or from a
terminal with `conductor-acct use`. Work and personal agents then run
concurrently, each on its own subscription, each with its own transcripts.

Codex works the same way through `CODEX_HOME`.

```
/account                      pick this workspace's account, as a card in the chat
conductor-acct use work       the same thing from a terminal
conductor-acct status         what this workspace resolves to
```

## Why this exists

Claude Code keeps one login per config directory. Conductor has one global
setting for agent environment variables. Point that setting at a config
directory and *every* workspace moves to that account, which is the same churn
as signing out and back in.

This routes per workspace instead, so every account you add stays live at once.

## Requirements

macOS, Conductor 0.81 or newer, and Claude Code or Codex already working inside
it. No other dependencies: everything here is POSIX shell.

## Install

```sh
git clone <this repository>
cd conductor-playground/conductor-hats
bin/conductor-acct setup
```

Then sign in to each account, in a real terminal, because the browser has to
open:

```sh
bin/conductor-acct add personal
bin/conductor-acct add work
```

`add` creates a config directory per account, symlinks your skills, plugins,
commands and transcripts back to `~/.claude` so every account shares them, and
runs the sign in flow.

Restart Conductor once, and check it:

```sh
bin/conductor-acct doctor
```

## Use it

Inside a Conductor chat:

```
/account
```

You get a card in the conversation listing your accounts. Pick one and that
workspace is routed to it. No windows, no system dialogs, no menu bar item.

From a terminal, in any workspace directory:

```sh
conductor-acct use work           # this workspace runs on the work account
conductor-acct status             # claude  work  you@example.com
conductor-acct which              # the same, with every layer that fed into it
```

Open a **new** chat for a switch to take effect. A chat that is already running
keeps the account its agent process started on, because the account is fixed
when that process spawns.

### Several accounts at once

```sh
cd ~/conductor/workspaces/company-app/one  && conductor-acct use work
cd ~/conductor/workspaces/side-project/two && conductor-acct use personal
```

Open a chat in each. Conductor keeps a separate agent host per workspace, so
both run at the same time without interfering.

### A whole repository on one account

For a repository where every workspace should use the same account, bind the
repository instead of each workspace:

```sh
cd ~/conductor/repos/company-app
conductor-acct bind work
```

That writes `.conductor/settings.local.toml`, which Conductor reads per
repository. It needs no router at all, so it keeps working even with this
extension turned off. Add the file to `.gitignore`; it is machine local.

A `use` route on a specific workspace overrides its repository binding.

## How it works

Claude Code namespaces credentials by config directory. With `CLAUDE_CONFIG_DIR`
set, the macOS keychain item becomes `Claude Code-credentials-<sha256(dir)[0:8]>`
instead of `Claude Code-credentials`, so each directory is a separate login.
That is Anthropic's own mechanism for several accounts, not a workaround.

Conductor exposes two documented settings that let you aim it:

| Setting | Scope | What this project does with it |
|---|---|---|
| `claude_code_executable_path` | user or repository | points at `bin/claude-router` |
| `environment_variables` | repository | holds a `CLAUDE_CONFIG_DIR` binding |

`claude-router` is a small shell script. Conductor spawns it instead of
`claude`, it works out which account this workspace should use, exports
`CLAUDE_CONFIG_DIR`, and `exec`s the real binary with the argv it was given.

Precedence, highest first:

1. `CONDUCTOR_ACCOUNT` in the environment, for one spawn
2. the session pin, so a running conversation never changes account mid flight
3. a route naming this exact workspace, from `use` or `/account`
4. a repository binding from `bind`
5. a route on a parent directory, then the `default` route

If none of those match, nothing is exported and you get your normal account.

[docs/how-it-works.md](docs/how-it-works.md) has the full picture, including
what was learned by reading Conductor's runtime.

## The in-app panel, and what it costs

A toolbar button next to "Open in" and an account chip in the New Workspace
composer both exist, and both open a two-level panel: providers first, then that
provider's accounts with a sign-out each and one "Add new account" at the foot.
Signing in happens in the panel, no terminal. The panel never deletes anything:
signing out drops credentials and leaves the profile, its routes, its session pins
and its transcripts alone. `conductor-acct remove` in a terminal is the only way
to delete a profile, deliberately.

Addresses are masked wherever they render, as `fir**ast@ex**e.com`, so a recorded
session or a shared screenshot cannot hand one out. `conductor-acct list` in a
terminal is where you read the real thing. See
[docs/account-panel.md](docs/account-panel.md).

They are not free. **Nothing outside the app can add them.** Conductor's UI is
compiled into a single Developer ID signed Mach-O with the hardened runtime, and
every file in the bundle is covered by the code signature seal, so adding one
file is enough to make macOS reject the app. The panel is therefore *injected
into the compiled frontend* and the app is ad-hoc re-signed, which means:

- patch a copy, not your install: `tools/make-dev-conductor.sh`, then
  `tools/patch-ui.py`. The patcher refuses `/Applications/Conductor.app` unless
  you pass `--i-know`.
- every Conductor release ships a new bundle, so the patch has to be re-applied
  after each update.

`/account` in the chat is the version that survives updates and needs no
patching, which is why it exists alongside. Details and the tests behind both
claims are in [docs/patching-conductor.md](docs/patching-conductor.md).

If you would rather have this natively, ask Conductor for it: Help, then Send
Feedback, asking for per-workspace agent account selection.

## Commands

```
setup                            guided first run
add <profile> [claude|codex]     create a profile and sign in to it
use <profile> [agent] [path]     point this workspace at a profile
status [path] [--mask]           what this workspace resolves to, in two lines
which [path] [agent]             the same, with every layer that fed into it
list [--mask]                    profiles, accounts and routes
mask <email>                     the masked form the UI shows on screen

login <profile> [agent]          re-run sign in for a profile
logout <profile> [agent]         sign out, keep the profile
remove <profile> [agent]         sign out, delete the profile and its routes

bind <profile> [agent] [repo]    bind a whole repository to a profile
unbind [agent] [repo]            drop a repository binding
assign default <profile>         account for workspaces with no route
unassign [path|default]          drop a route

install                          turn the router on, add /account
uninstall                        turn it off again
sessions [clear]                 show or reset per-session pins
doctor                           check the setup end to end
```

## Layout

```
~/.conductor-accounts/
  routes                  workspace path -> profile
  config                  reserved
  claude/<profile>/       CLAUDE_CONFIG_DIR, one per account
  codex/<profile>/        CODEX_HOME, one per account
  sessions/claude/<id>    which account a session started on
  bin -> <checkout>/bin   stable path for /account
```

Each Claude profile symlinks `projects`, `skills`, `plugins`, `commands`,
`agents`, `settings.json` and `CLAUDE.md` back to `~/.claude`, so every account
shares your skills, hooks and transcripts. Only credentials and `.claude.json`
are per account. `doctor` warns if a symlink is replaced by a real file.

## Turning it off

```sh
conductor-acct uninstall     # router off, /account removed
conductor-acct unbind        # per repository
rm -rf ~/.conductor-accounts # removes both logins
```

Restart Conductor. That removes the routing side completely: no modified
`~/.claude`, no changes to Conductor's database, nothing left in
`~/.conductor/settings.toml`.

If you also patched a copy for the in-app panel, that copy is separate and stays
until you remove it:

```sh
tools/patch-ui.py --revert                 # the injected UI only
rm -rf "/Applications/Conductor Dev.app"   # the whole copy
```

Your real Conductor was never touched by either.

## Documentation

| Page | What it covers |
|---|---|
| [docs/how-it-works.md](docs/how-it-works.md) | how credentials are namespaced, the router, precedence |
| [docs/account-panel.md](docs/account-panel.md) | the in-app panel: layout, masking, sign-out |
| [docs/panel-internals.md](docs/panel-internals.md) | how the panel attaches, and the update path |
| [docs/patching-conductor.md](docs/patching-conductor.md) | what the app bundle allows, with the evidence |
| [docs/dev-conductor.md](docs/dev-conductor.md) | building a Conductor copy that is safe to modify |
| [AGENTS.md](AGENTS.md) | rules for changing this code, human or agent |
| [CONTRIBUTING.md](CONTRIBUTING.md) | tests, linting, changelog, versions |
| [CHANGELOG.md](CHANGELOG.md) | what changed, by version |

## Tests

```sh
test/run.sh              # everything
test/run.sh route        # tests matching "route"
```

They run against a sandbox under `$TMPDIR` with a stub agent binary, so no real
Conductor install, `~/.claude` directory or keychain item is involved.

## A note on accounts

Using several subscriptions to work around rate limits is against Anthropic's
usage policy. Choosing between a personal account and a work account that you
or your employer separately pay for is not. This project is built for the
second case.

## Licence

MIT. See [LICENSE](LICENSE).
