# The account panel inside Conductor

`tools/patch-ui.py` injects `tools/ui-patch/account-ui.js` into a Conductor
copy's compiled frontend. It adds two controls to the app itself, no separate
window anywhere:

- a **toolbar button** next to "Open in", showing the account this workspace
  runs agents on
- a **chip** in the New Workspace composer footer, next to the model picker,
  showing the account the workspace you are about to create will start on

Both open the same panel. Patch a copy, never your real install:

```sh
tools/make-dev-conductor.sh     # build "Conductor Dev.app"
tools/patch-ui.py               # inject the panel
open "/Applications/Conductor Dev.app"
tools/patch-ui.py --revert      # undo the UI only
```

## The panel

Two levels, because a flat list looked tidy with two accounts and would not
with ten.

**Level one** lists providers only:

```
Workspace: antananarivo

  Claude Code       2 Accounts         Work    >
  Codex             0 Accounts         None    >
  ───────────────────────────────────────────────
  Turn routing off
  agents go back to one account

Applies to the next chat here. A chat already running keeps
the account it started on.
```

**Level two**, after choosing a provider:

```
  <  Back
  Claude Code

  up1**001@g**.com                      | delete
  Personal

  uts**tel@rt**p.com              tick  | delete
  Work

  +  Add new account
```

Profile names are lower case on disk, because they are typed at a CLI and used
as directory names. They are capitalised for display through one `cap()` helper
and never fed back to `conductor-acct` in that form.

### Addresses are masked

Every address the UI renders is masked, so a recorded session or a shared
screenshot cannot hand one out:

```
someone.long@example.com  ->  som**ong@exa**le.com
joe@mail.co.uk            ->  j**@m**.co.uk
```

Local part and host are masked separately and the suffix is kept, so the string
still reads as an email. How much is revealed scales with length, so a short
local part is not handed over in full for want of characters to hide, and
anything under three characters reveals nothing. The profile name underneath is
the identifier worth reading anyway.

The full address is not in a `title` attribute either: a tooltip is as visible on
video as the text is. To read one, use a terminal:

```sh
conductor-acct list          # real addresses
conductor-acct list --mask   # the masked form, as the UI shows it
conductor-acct mask <email>  # one address
```

The rule exists twice, because the panel cannot shell out once per row: in
`mask_email` in the CLI, which `list --mask`, `status --mask` and the `/account`
chat card use, and in `maskEmail` in `account-ui.js`. A test runs both over the
same cases and fails if they disagree.

### Deleting an account

The delete control sits **inside** the row's border, divided from the selectable
area by a rule and running the full height of the row: deliberate to hit, hard to
hit by accident. It used to float in the gutter outside the border, which read as
unrelated to the row and put a destructive target a few pixels from "switch
account" with nothing between them.

Clicking it opens a dialog with a scrim, not an inline confirmation and not a
control that arms on first click, because an armed control is still one stray
click from destruction. The dialog names the account, says what will be lost and
that it cannot be undone, and focuses Cancel. Escape cancels, clicking the scrim
cancels, clicking the dialog itself does not.

The dialog is a sibling of the panel rather than a descendant, so the panel
ignores outside clicks and Escape for as long as it is open. Without that, the
first interaction with the dialog would close the panel underneath it.

The email leads, because that is what identifies an account; the profile name
sits under it, because that is what you type at the CLI.

- **Clicking a row** switches to that account and the tick moves in place. The
  panel deliberately stays open, so the change is visible rather than inferred
  from the panel disappearing.
- **The delete control** asks first, naming the account: signing out, deleting
  the profile directory and dropping every route that pointed at it is not
  undoable.
- **Add new account** signs in without a terminal. Type a profile name, your
  browser opens at the OAuth URL, paste the code back into the panel. `claude
  auth login` prints the URL then blocks reading a code from stdin, so
  `login-start` runs it with stdin on a FIFO and `login-code` feeds the answer
  in.
- **Escape** steps back to level one before it closes the panel.

## What the panel changes, depending on where it is opened

| Opened from | Scope line reads | What choosing an account writes |
|---|---|---|
| a workspace toolbar | `Workspace: <name>` | a `use` route for that workspace |
| the New Workspace modal | `New workspaces in <repo>` | a `bind` on the repository |
| neither identified | `No workspace in view` | nothing; rows are disabled |

## Why it is built the way it is

**No state of its own.** Every read is `conductor-acct json`, every write is
`use`, `bind`, `remove` or `login-*`, run through Conductor's own
`execute_shell_command` Tauri command. The panel, the CLI and `/account` cannot
disagree, because only one of them owns anything.

**It finds anchors by product copy, not by class.** Class names are hashed per
build. "The control whose tooltip says Open in" and "the field whose
placeholder says What do you want to work on" survive releases that rename
every identifier. When an anchor does move, the control fails to appear rather
than taking the app down, and the toolbar button falls back to floating at the
top right so a missing anchor is visible rather than silent.

**It knows where it is by matching the chrome against Conductor's database.**
The webview runs an in-memory router, so `location` never carries a workspace
id. `conductor-acct workspaces` and `repos` list every name and path from every
`com.conductor*` database on the machine, and the panel matches the app chrome
against them, longest name first so `belo-horizonte` cannot be beaten by a repo
called `belo`. Globbing the databases matters: a patched copy keeps its own, and
asking the real app about a workspace it has never heard of returns nothing.

## Nothing moves once it is open

A popover that reflows while you are aiming at it is worse than one that is
plain, so four things are pinned deliberately. Changing any of them brings the
jumping back, and the test suite guards each one.

| Pinned | Why it moved before |
|---|---|
| the top left corner, measured once on open | re-measuring on every render moved the panel when the provider view came in at a different height, and right-edge clamping moved it sideways |
| the width, in CSS, with a capped scrolling height | a card at `width:100%` with the delete control as a flex sibling overflowed 300px, so the provider view was wider than the root view |
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
tools/repersonalize.sh
```

It quits the copy, rebuilds it from the freshly updated Conductor, injects the
panel, redeploys the CLI, runs `doctor` and relaunches. `--no-launch` stops
before the last step; `--keep-app` patches the existing copy instead of
rebuilding it.

Two steps in there exist because doing this by hand goes wrong in the same two
ways every time.

**The stale backup.** `patch-ui.py` keeps a pristine copy of the binary and
always patches from it, so patching twice is not a stack. That backup is keyed by
app name, not by version, so after an update it holds the *previous* Conductor's
binary — and patching a freshly rebuilt copy against it silently reinstates the
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
