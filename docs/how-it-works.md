# How it works

Everything here was established by reading Conductor 0.81.0 and Claude Code
2.1.220 on macOS, and by testing against them. Conductor is closed source, so
treat version-specific details as observations rather than contract. The two
settings this project depends on are in Conductor's published schemas, and those
are contract.

## How credentials are namespaced

Claude Code derives its keychain service name from the config directory:

```js
function keychainService(suffix = "") {
  let r = process.env.CLAUDE_SECURESTORAGE_CONFIG_DIR,
      isDefault = r !== undefined ? !r : !process.env.CLAUDE_CONFIG_DIR,
      dir = r !== undefined ? r.normalize("NFC") : homedir(),
      tag = isDefault ? "" : "-" + sha256(dir).hex().slice(0, 8);
  return `Claude Code${OAUTH_FILE_SUFFIX}${suffix}${tag}`;
}
```

So `CLAUDE_CONFIG_DIR=~/.conductor-accounts/claude/work` gets its own keychain
item, `Claude Code-credentials-<hash>`, and its own login. Resolution order is
`$CLAUDE_CONFIG_DIR/.credentials.json`, then that keychain item, then
`ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN`.

This is Anthropic's own mechanism for multiple accounts. Codex has the
equivalent in `CODEX_HOME` and `<home>/auth.json`.

`CLAUDE_CODE_OAUTH_TOKEN` looks like a lighter switch, and is a trap: on macOS
the claude binary deletes it from the environment it hands to its background
spare host, so warm starts silently fall back to the default account. Config
directories do not have that problem.

## What Conductor gives us to aim it

Two keys, both in
`https://conductor.build/schemas/settings.repo.schema.json`:

```toml
# .conductor/settings.local.toml, per repository
claude_code_executable_path = "/path/to/claude-router"

[environment_variables]
CLAUDE_CONFIG_DIR = "/Users/you/.conductor-accounts/claude/work"
```

`environment_variables` is documented as "Environment variables passed to agents
running in this repository", with optional `.local` and `.cloud` subtables.
Layering across the four settings files is managed, then repository local, then
repository shared, then user, then defaults.

Inside the agent process Conductor also exports `CONDUCTOR_WORKSPACE_PATH`,
`CONDUCTOR_ROOT_PATH`, `CONDUCTOR_WORKSPACE_ID`, `CONDUCTOR_SESSION_ID` and
`CONDUCTOR_IS_LOCAL`. The router routes on `CONDUCTOR_WORKSPACE_PATH`.

## How the agent is spawned

Conductor drives `@anthropic-ai/claude-agent-sdk`, which spawns the binary:

```js
let isNative = ![".js",".mjs",".tsx",".ts",".jsx"].some(r => path.endsWith(r));
spawn(isNative ? path : "node", …, { cwd: workspaceDir, env, signal });
```

Any executable whose path does not end in a JavaScript extension is exec'd
directly with claude's argv, so a `#!/bin/sh` script qualifies. That is the
whole trick behind `claude-router`: Conductor thinks it is running claude.

The router must `exec` rather than fork or daemonize. Conductor's stdio pipes
assume a direct child, and the claude binary's background spare host has the
same expectation.

Argv Conductor passes, which the router forwards untouched:
`--output-format stream-json --verbose --input-format stream-json`, plus
`--model`, `--permission-mode`, `--session-id=`, `--resume=`,
`--resume-session-at=`, `--add-dir`, `--mcp-config`, `--settings`,
`--managed-settings`.

## Why the account is per session, not per message

```js
ClaudeAgentRunner.startGenerator(sessionId, cfg, { resume, resumeSessionAt })
```

One SDK `query()` per session, with user messages fed into a long-lived async
generator. The `claude` process stays alive across turns, so its environment is
fixed when the generator starts. A switch therefore applies to the next chat,
not the current one, and the CLI and `/account` both say so rather than
pretending otherwise.

Conductor keys each agent host on
`configKey = sha256(JSON.stringify({claudeExecutablePath, claudeEnvVars, …}))`
and tears the host down when that changes, which is the machinery that makes
per-repository bindings take effect without restarting the app.

## Precedence, and why

```
1. CONDUCTOR_ACCOUNT             an explicit override for one spawn
2. session pin                   the account this session started on
3. exact workspace route         `use` or `/account` named this directory
4. an inherited CLAUDE_CONFIG_DIR a repository binding is already in effect
5. parent-directory route, then the `default` route
```

The session pin sits above routes because Conductor respawns the agent on
resume, on model switches and on generator restarts. Without the pin, changing a
route would move a live conversation to a different account, and `--resume`
would fail to find its transcript.

An exact route sits above a repository binding because naming one workspace is
more specific than naming a repository. An inherited binding sits above a parent
route for the same reason in reverse.

Paths are normalized with `cd … && pwd` on both sides before comparison. On
macOS `$TMPDIR` lives under `/var`, which is a symlink to `/private/var`, and
without normalization a route written from a shell would never match the path
Conductor hands to the agent.

## Failing open

The router is in front of every agent Conductor starts, so a bug in it is an
outage. All decision making happens inside a single command substitution:

```sh
DECISION=$(
    [ -r "$LIB" ] || exit 0
    . "$LIB"
    …
) 2>/dev/null
```

A syntax error or a missing library kills that subshell and leaves `DECISION`
empty. The router then falls back through `CONDUCTOR_ACCOUNTS_CLAUDE_BIN`,
Conductor's bundled binary path, and `command -v claude`, and execs whatever it
finds. You lose the account routing and keep your agent. Two tests cover this.

The router also refuses to run inside itself, exiting 70 if
`CONDUCTOR_ACCOUNTS_ROUTING` is already set, so a misconfigured
`CONDUCTOR_ACCOUNTS_CLAUDE_BIN` cannot fork-bomb.

## Sharing state between accounts

Each profile symlinks back to `~/.claude`:

```
projects  skills  plugins  commands  agents  settings.json  CLAUDE.md
```

Transcripts live in `projects`, so `--resume` keeps working when a workspace
changes account, and your skills and hooks are the same on every account. Only
credentials and `.claude.json` are per account. `conductor-acct doctor` warns
when one of those symlinks has been replaced by a real file, which is what
happens if a tool writes through it.

## Things that were tried and rejected

**Patching the app bundle.** Breaks the code signature. See
[patching-conductor.md](patching-conductor.md).

**Interposing on the sidecar socket.** Conductor's Tauri process talks to
`conductor-runtime` over a unix socket at
`$TMPDIR/conductor-sidecar-v2-<pid>.sock`, NDJSON, zod-validated, around 25
message types. Total control, entirely undocumented, and it would break on
every release.

**Writing `claude_env_vars` in Conductor's SQLite database.** It works, and it
is global: the `settings` table is `(key, value)` with no scope column, so one
value applies to all of Conductor. That is the churn this project exists to
avoid, and writing to another app's database is not something to ship.

**A local proxy on `ANTHROPIC_BASE_URL`,** rewriting the `Authorization` header
per HTTP request. This is the only design that gives genuinely per-request
account switching, and it is a supported environment variable. It also means
holding long-lived credentials in a process of your own and terminating requests
to Anthropic through it. Out of scope here; per-workspace is enough when
workspaces are the unit of work.
