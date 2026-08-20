# The account panel inside Conductor

`hats patch` injects the built panel into a Conductor
copy's compiled frontend. It adds two controls to the app itself, no separate
window anywhere:

- a **toolbar button** next to "Open in", showing the account this workspace
  runs agents on
- a **chip** in the New Workspace composer footer, next to the model picker,
  showing the account the workspace you are about to create will start on

Both open the same panel. Patch a copy, never your real install:

```sh
hats dev-app      # build "Conductor Dev.app"
hats patch        # inject the panel
hats revert       # undo the UI only
open "/Applications/Conductor Dev.app"
```

## The panel

Two levels, because a flat list looked tidy with two accounts and would not
with ten.

**Level one** lists providers only:

```
Workspace: greenfield

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

  som**lse@ex**e.org                    | sign out
  Personal

  fir**ast@ex**e.com            tick  | sign out
  Work

  +  Add new account
```

Profile names are lower case on disk, because they are typed at a CLI and used
as directory names. They are capitalised for display through one `cap()` helper
and never fed back to `hats` in that form.

### Provider icons

Both marks are Conductor's own, lifted verbatim out of its frontend rather than
approximated. Hand-drawn stand-ins read as the wrong glyph sitting two rows from
the real ones in the model picker: an eight-line asterisk is not the Anthropic
sunburst, and a ringed dot is not the OpenAI knot. Both are filled paths on a
24-unit grid using `currentColor`, so they inherit the panel's text colour.

If a release ever changes them, re-extract:

```sh
hats assets                               # then search the renderApp chunk
```

The Claude mark is the path inside the component rendered beside a Claude
session, next to the Codex one in the same chunk.

### Addresses are masked

Every address the UI renders is masked, so a recorded session or a shared
screenshot cannot hand one out:

```
someone.long@example.com  ->  som**ong@ex**e.com
joe@mail.example.com            ->  j**@m**.example.com
```

Local part and host are masked separately and the suffix is kept, so the string
still reads as an email. How much is revealed scales with length, so a short
local part is not handed over in full for want of characters to hide, and
anything under three characters reveals nothing. The profile name underneath is
the identifier worth reading anyway.

The full address is not in a `title` attribute either: a tooltip is as visible on
video as the text is. To read one, use a terminal:

```sh
hats list          # real addresses
hats list --mask   # the masked form, as the UI shows it
hats mask <email>  # one address
```

The rule exists twice, because the panel cannot shell out once per row: in
`mask_email` in the CLI, which `list --mask`, `status --mask` and the `/account`
chat card use, and in `maskEmail` in `account-ui.js`. A test runs both over the
same cases and fails if they disagree.

### Signed in, signed out, and address unknown

Signed-in state is read from where the credentials are, not from a cached
address. Claude Code resolves them as `$CLAUDE_CONFIG_DIR/.credentials.json`, then
a keychain item whose service name carries the first 8 hex of `sha256` of the
config directory; `profile_signed_in` checks both, in that order.

That distinction matters because a profile can hold working credentials before its
address is readable anywhere. `.label` is written from
`oauthAccount.emailAddress` in `.claude.json`, and that is not always populated
the moment a sign-in finishes. Inferring "signed in" from `.label` meant such a
profile read as signed out for ever, so the panel offered to sign in an account
that already was, and `login-status` reported a completed sign-in as an error. So
there are three states, not two:

| Row shows | Means |
|---|---|
| masked address, profile name under it | signed in, address known |
| profile name, "Signed in" | signed in, address not cached yet |
| profile name, "Not signed in" | no credentials |

`hats json` reports `signedIn` alongside `email` for exactly this
reason. A route can point at a signed-out profile, which is legitimate: routes and
credentials are separate, and the tick shows the route.

### One account, one profile

A provider keeps a single live token per account. Two profiles signed in to the
same address are therefore not two accounts: whichever signed in last holds the
token and the other is silently signed out, so the pair take turns logging each
other out. The symptom makes no sense until you know this. An account you signed
in minutes ago asks again.

Three places say so:

- the panel, during a sign-in that lands on an address another profile already has
- `hats login`, which warns on stderr after the fact
- `hats doctor`, which reports any pair sharing an address

It is a warning rather than a refusal because the address is only knowable *after*
the OAuth round trip; refusing then would leave the profile in a state the message
denies. Resolve it by dropping one:

```sh
hats remove <profile>
```

### Signing out, and what the panel will not do

**The panel never deletes anything.** Signing out drops that account's
credentials and nothing else: the profile stays, so do its routes, its session
pins and its transcripts. The account reappears in the list as "Not signed in",
ready to sign back in from the same place.

Deleting a profile outright is `hats remove` in a terminal, on purpose.
It is the one irreversible operation here, and a popover you can open by accident
is the wrong place for it.

The sign-out control sits **inside** the row's border, divided from the
selectable area by a rule and running the full height of the row: deliberate to
hit, hard to hit by accident. It used to float in the gutter outside the border,
which read as unrelated to the row and put its target a few pixels from "switch
account" with nothing between them. It is only rendered for an account that is
signed in, because there is otherwise nothing to sign out of.

Clicking it opens a dialog with a scrim, not an inline confirmation and not a
control that arms on first click. Signing back in costs a browser round trip, so
it is worth one deliberate confirmation. The dialog names the account, says what
happens and what stays, and focuses Cancel. Escape cancels, clicking the scrim
cancels, clicking the dialog itself does not.

The dialog is a sibling of the panel rather than a descendant, so the panel
ignores outside clicks and Escape for as long as it is open. Without that, the
first interaction with the dialog would close the panel underneath it.

The email leads, because that is what identifies an account; the profile name
sits under it, because that is what you type at the CLI.

- **Clicking a row** switches to that account and the tick moves in place. The
  panel deliberately stays open, so the change is visible rather than inferred
  from the panel disappearing.
- **The sign-out control** asks first, naming the account. It signs that account
  out and changes nothing else.
- **The sign-in control** takes its place on a signed-out row, so the row is never
  a dead end. Same flow as "Add new account" without the name field, since the
  profile is already known.
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

## How it is built, and how it survives an update

Split out to keep this page about behaviour:
[panel-internals.md](panel-internals.md) covers how the panel finds its anchors,
why it opens on press, why nothing shifts once it is open, what the patch costs,
and the one command that re-applies everything after a Conductor release.
