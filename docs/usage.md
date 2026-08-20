# Using it day to day

Picking an account, running several at once, and binding a whole repository.
[cli.md](cli.md) is the command reference.

## Picking an account

**From the toolbar.** The button next to "Open in" shows the account this
workspace runs on. Click it for the panel: providers first, then that provider's
accounts, each with a sign-in or sign-out control and one "Add new account" at
the foot. Signing in happens in the panel, no terminal. Addresses are masked, so a
recorded session cannot hand one out.

**From the New Workspace composer.** The chip beside the model picker binds the
repository, so the workspace you are about to create starts on the account you
meant.

**From the chat**, with `/account`, which needs no patching and survives every
Conductor update. This is the fallback rather than the point: a slash command is
something anyone can write, and it is here so the feature still works on a
Conductor you have not patched.

**From a terminal**, in any workspace directory:

```sh
hats use work           # this workspace runs on the work account
hats status             # claude  work  you@example.com
hats which              # the same, with every layer that fed into it
```

Open a **new** chat for a switch to take effect. A chat that is already running
keeps the account its agent process started on, because the account is fixed
when that process spawns.

### Several accounts at once

```sh
cd ~/conductor/workspaces/company-app/one  && hats use work
cd ~/conductor/workspaces/side-project/two && hats use personal
```

Open a chat in each. Conductor keeps a separate agent host per workspace, so
both run at the same time without interfering.

### A whole repository on one account

For a repository where every workspace should use the same account, bind the
repository instead of each workspace:

```sh
cd ~/conductor/repos/company-app
hats bind work
```

That writes `.conductor/settings.local.toml`, which Conductor reads per
repository. It needs no router at all, so it keeps working even with this
extension turned off. Add the file to `.gitignore`; it is machine local.

A `use` route on a specific workspace overrides its repository binding.


## How it decides

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

[how-it-works.md](how-it-works.md) has the full picture, including
what was learned by reading Conductor's runtime.

