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

pin <profile> [agent] [session]  point one chat at a profile
unpin [agent] [session]          let that chat follow the workspace
session [path] [agent]           the chat currently open in a workspace
chats [--mask] [--json]          every open chat and the account it is on
next <profile> [agent]           the account for the workspaces created next

bind <profile> [agent] [repo]    bind a whole repository to a profile
unbind [agent] [repo]            drop a repository binding
assign default <profile>         account for workspaces with no route
unassign [path|default]          drop a route

install                          turn the router on, add /account
uninstall                        turn it off again
sessions [clear]                 show or reset per-session pins
json [path] [session]            machine-readable, for the panel
debug [on|off|status|read|clear] record what the panel resolved
doctor                           check the setup end to end
```

`bind` is the blunt one and it is worth knowing why. A binding is a single
`CLAUDE_CONFIG_DIR` written into the repository's `.conductor` settings, which
Conductor exports into every workspace under it, so it moves all of them at
once. The panel never writes one. Reach for `use` for a workspace, `pin` for a
chat, and `next` for the workspace you are about to create.


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

## `hats json [path] [session]`

What the injected panel reads. One object per agent, and the two fields worth
naming:

- `chat` is the account the **next** process for that chat will take
- `started` is the account the process **already running** took when it spawned

They differ whenever a chat has been pointed somewhere new and not yet
restarted, and conflating them is what made the toolbar say Work while the agent
answering was on Personal. Anything showing a label wants `started`, falling
back to `chat` when the chat has not started yet.

The `session` argument names the chat to answer about. The panel reads that out
of the window, which is exact; without it the chat is worked out from
Conductor's database, which is the fallback for callers with no window to read.

A path that does not exist yet is not an error here. Conductor records a
workspace before it finishes making its working tree, and the panel asks about
one in that state every time a workspace is created.

## `hats chats [--mask] [--json]`

Every chat Conductor has open, and the account each one is on. The panel answers
that for the chat in front of you; this answers it for all of them, which is the
question that gets asked once several agents are running.

```
WORKSPACE            AGENT    STATUS      CTX  ON         NEXT       TITLE
atlanta              claude   working     62%  work       personal   Update Conductor-hats
lagos                claude   idle        17%  work       work       Create rtcamp2 PR
```

Two accounts per chat, and the difference is the point. **ON** is what the
process already running took when it spawned, which cannot be changed. **NEXT**
is what the process after it will take. They differ exactly when a chat has been
pointed somewhere new and not yet restarted, and the listing says how many are
in that state.

`-` in either column means there is nothing recorded: a chat that has not
started yet, or one that started before hats began keeping this.

`--json` prints the same list as an array, which is what anything drawing a
screen should read. Archived workspaces and hidden chats are left out of both.
