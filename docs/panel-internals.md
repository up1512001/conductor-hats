# How the account panel is built

Behaviour is [account-panel.md](account-panel.md). This page is the mechanism:
where the source lives, how the panel attaches to an app it does not control, the
two traps that cost the most time, and the update path.

## Source and build

```
src/panel/*.ts          the panel, one file per concern
src/panel/styles/*.scss partials, one per group of elements
src/panel/styles.scss   pulls the partials together
src/panel/compat.ts     the boot guard, built separately
dist/account-ui.js      the panel, generated and gitignored
dist/boot-guard.js      the boot guard, same
tools/build-panel.mjs   esbuild plus a sass plugin
```

```sh
pnpm install
pnpm build      # dist/account-ui.js and dist/boot-guard.js
pnpm watch      # rebuild on change
pnpm verify     # fail unless two builds produce identical bytes
pnpm typecheck
```

Each artifact is spliced into Conductor's compiled frontend, where there is no
module loader and no second file to load, so each has to be one self-contained
script. Bundling is what allows the source to be many small files instead of one
unreadable one.

There are two because they go to different places. The panel is appended to the
chunk that draws the toolbar. The boot guard is prepended to the module
index.html loads first, which is the only place early enough: injected into the
panel's chunk it installs after Conductor has taken its own reference to `fetch`
and never sees the request it exists to answer. See
[blank-window.md](blank-window.md).

It is **not** minified. There is roughly 199 KB of headroom in the asset slot, so
minifying saves nothing that matters, and readable output can be inspected in the
app's devtools when an anchor breaks after a Conductor release.

`dist/` is generated, never committed. Two reasons, and the second one was found
rather than predicted: build output in git goes stale and muddies every diff, and
esbuild labels each bundled module with its path, which for the SCSS plugin was
absolute. The committed artifact therefore carried the home directory of whoever
built it, and the repository's own no-personal-data test caught it.

The build rewrites those paths to repo-relative, so the output is identical on any
machine. `pnpm verify` builds twice, fails if the two differ, and fails if any
absolute path survives. That matters because a release attaches this artifact and
anyone should be able to rebuild exactly what was published.

## Why it is built the way it is

**No state of its own.** Every read is `hats json`, every write is
`use`, `bind`, `logout` or `login-*`, run through Conductor's own
`execute_shell_command` Tauri command. The panel, the CLI and `/account` cannot
disagree, because only one of them owns anything.

**It opens on press, and reads state sparingly.** Both were bugs first: the panel
sometimes needed two or three clicks to appear. Two independent causes, both
fixed, and both easy to reintroduce.

*The trigger was rebuilt underneath the pointer.* Conductor re-renders its toolbar
constantly, and when React replaces the container the injected button goes with
it. A rebuild landing between `mousedown` and `mouseup` means the browser fires no
`click` at all, so the press did nothing. The trigger now opens on `pointerdown`,
a single event that cannot be split that way, which is also how native menus
behave. `click` is still handled for keyboard activation, guarded against
double-toggling.

*Every render pass cost a process spawn.* The label refresh ran from the mutation
observer, and each refresh shells out to `hats json`, which runs the
router twice internally to answer. During a streaming chat that was several spawns
a second, and a press's own read then queued behind the backlog. Now: one
in-flight read shared by all callers, a four-second cache after it, invalidated by
every write, and the labels kept current by one slow timer rather than by the
observer. The observer only re-attaches controls, and both attach paths return
immediately when the control is already in place.

The panel also opens **before** the read finishes, showing a placeholder row per
provider. A control that does nothing for half a second reads as broken, and the
placeholder is sized so the corner is pinned at roughly the right height.

**It finds anchors by product copy, not by class.** Class names are hashed per
build. "The control whose tooltip says Open in" and "the field whose
placeholder says What do you want to work on" survive releases that rename
every identifier. When an anchor does move, the control fails to appear rather
than taking the app down, and the toolbar button falls back to floating at the
top right so a missing anchor is visible rather than silent.

**It knows where it is by matching the chrome against Conductor's database.**
The webview runs an in-memory router, so `location` never carries a workspace
id. `hats workspaces` and `repos` list every name and path from every
`com.conductor*` database on the machine, and the panel matches the app chrome
against them, longest name first so `rio-branch` cannot be beaten by a repo
called `rio`. Globbing the databases matters: a patched copy keeps its own, and
asking the real app about a workspace it has never heard of returns nothing.

## Nothing moves once it is open

A popover that reflows while you are aiming at it is worse than one that is
plain, so four things are pinned deliberately. Changing any of them brings the
jumping back, and the test suite guards each one.

| Pinned | Why it moved before |
|---|---|
| the top left corner, measured once on open | re-measuring on every render moved the panel when the provider view came in at a different height, and right-edge clamping moved it sideways |
| the width, in CSS, with a capped scrolling height | a card at `width:100%` with the sign-out control as a flex sibling overflowed 300px, so the provider view was wider than the root view |
| the tick's slot, always in the flow | adding and removing the tick reflowed both the row it left and the row it landed in |
| the trigger labels, hidden until known | both rendered "Account" and replaced it with the real name a moment later, shifting the toolbar on every workspace open |

There is no status dot next to either trigger label. The label already reads
`Work`, `Default` or `Off`; a dot beside it is decoration standing in for a word
that is right there.

## Two traps worth knowing about

**Pointer events must be sealed on the bubble phase, not capture.** Conductor's
New Workspace modal dismisses on pointer events it considers outside itself, and
a panel on `document.body` counts as outside, so choosing an account used to
dismiss the modal and lose the typed prompt. Mounting the panel inside the
dialog fixes containment; stopping propagation at the panel's edge covers
listeners bound higher up. Doing that on the **capture** phase stopped the event
before it reached the row that was clicked: every row went inert and the panel
stopped opening at all. Bubble phase gives both, because the panel's own
handlers have already run by then.

**`position: fixed` is not always relative to the viewport.** Conductor animates
its dialog with a transform, which establishes a containing block, so a fixed
panel mounted inside it is positioned against the dialog. Rather than guess
which ancestor wins, the panel sets its coordinates, measures where it actually
landed, and corrects by the difference.

## Cost of the patch

```
/assets/renderApp-CIIBeY95.js   2,148,637 compressed -> 8,597,308 bytes
+ 37 KB of UI                   -> 1,948,688 compressed
                                   ~200 KB of headroom left
```

Conductor ships that bundle compressed below brotli's maximum, so recompressing
at quality 11 pays for the injection several times over. Only `value_len` in the
asset map changes, so nothing is relocated.
[docs/patching-conductor.md](patching-conductor.md) has the mechanism, and what
the signature does and does not allow.

Each Conductor release ships a new bundle, so the patch has to be re-applied
after every update. That is the standing cost of this route, and the reason
`/account` in the chat exists as the version that never breaks.

## After a Conductor update

One command:

```sh
hats repatch
```

It quits the copy, rebuilds it from the freshly updated Conductor, injects the
panel, redeploys the CLI, runs `doctor` and relaunches. `--no-launch` stops
before the last step; `--keep-app` patches the existing copy instead of
rebuilding it.

Two steps in there exist because doing this by hand goes wrong in the same two
ways every time.

**The stale backup.** `hats` keeps a pristine copy of the binary and
always patches from it, so patching twice is not a stack. That backup is keyed by
app name, not by version, so after an update it holds the *previous* Conductor's
binary, and patching a freshly rebuilt copy against it silently reinstates the
old version. The script deletes it whenever it rebuilds.

**The leaked routing variable.** Launching the app with `open` from inside a
routed agent session hands `CONDUCTOR_ACCOUNTS_ROUTING` to the app, which hands
it to every agent it spawns, and the router's loop guard refuses all of them with
`exited with code 70, refusing to route into itself`. The script scrubs the
environment before launching. If you launch by hand, do the same:

```sh
env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH \
    open -a "/Applications/Conductor Dev.app"
```

If the panel does not appear afterwards, the anchors moved in that release. The
toolbar button falls back to floating at the top right when it cannot find the
toolbar, so a button in the wrong place means the script ran and one selector
needs updating; nothing at all means the injection itself did not take.
