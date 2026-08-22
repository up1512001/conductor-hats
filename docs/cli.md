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

