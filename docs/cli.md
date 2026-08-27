# hats

The command reference. [usage.md](usage.md) is the day to day guide.

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


## Diagnosing a copy

```sh
hats verify                                  # check a patched copy end to end
hats assets                                  # list the frontend assets in a binary
hats assets --dump '/index.html'             # print one decompressed
hats panel                                   # print the panel this binary injects
hats guard                                   # print the boot guard it injects
hats patch --asset KEY --prepend --script F  # inject something else, for diagnosis
```

`--script` may be repeated, and `--asset`/`--prepend` apply to the next one, so a
single patch can carry a probe alongside the real panel. See
[blank-window.md](blank-window.md) for the method this exists for.


## Turning it off

```sh
hats uninstall     # router off, /account removed
hats unbind        # per repository
rm -rf ~/.conductor-accounts # removes both logins
```

Restart Conductor. That removes the routing side completely: no modified
`~/.claude`, no changes to Conductor's database, nothing left in
`~/.conductor/settings.toml`.

If you also patched a copy for the in-app panel, that copy is separate and stays
until you remove it:

```sh
hats revert                                # the injected UI only
rm -rf "/Applications/Conductor Dev.app"   # the whole copy
```

Your real Conductor was never touched by either.

## `hats debug [on|off|status|read|clear]`

An opt-in log of what the injected panel resolved, for when it picks the wrong
place. Off unless turned on, and it records decisions rather than anything
typed: the scope asked for, the workspace id found in Conductor's fiber tree and
how much of the tree was walked to find it, the target that came back, and what
a choice wrote.

```sh
hats debug on
# reproduce it in Conductor
hats debug read
hats debug off
```

The log lives at `~/.conductor-accounts/debug.log`. `hats debug clear` empties it.

It exists because the alternative is worse: injecting a probe means patching,
patching re-signs the copy, and a re-signed copy loses the keychain items it
stored, so every diagnosis cost a login.

## `hats next <profile> [agent]`

The account for the workspaces created from now on. What the account chip in the
New Workspace composer writes.

It applies to workspaces that appear after it and not to any that already exist,
which is checked against Conductor's own list rather than assumed: Conductor
starts an agent with the working directory set to `/` before a new workspace's
own, and every open workspace respawns agents on resumes and model switches, and
any of those would otherwise have taken it.

It stands until another is chosen, so one press of the chip covers a batch of
workspaces created together. Each one writes itself an ordinary route as its
first agent starts, so choosing again later does not move it.
