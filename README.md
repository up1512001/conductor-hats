# conductor-hats

https://github.com/user-attachments/assets/9b8a35f4-1f67-453c-930b-93992a531f02

Run as many Claude Code or Codex accounts as you like in
[Conductor](https://conductor.build) **at the same time**, one per chat, with no
signing in and out.

**With a real account picker in Conductor's own toolbar.** A button beside
"Open in" and a chip in the New Workspace composer, drawn in Conductor's own
theme, opening a panel that switches accounts, signs in and signs out. Inside the
app, not beside it.

Conductor is a signed, notarized, closed-source application with no plugin API.
Its entire frontend is compiled into a 66 MB Mach-O and brotli-compressed inside
`__DATA_CONST`. `hats` reads that asset map, injects the panel and re-signs the
copy. [docs/patching-conductor.md](docs/patching-conductor.md) has the evidence
for every part of that, including the things that genuinely are impossible.

```sh
hats dev-app     # an isolated copy of Conductor, safe to modify
hats patch       # inject the account panel into it
hats repatch     # do both again after a Conductor update
```

Agents then run concurrently, each on its own subscription, each with its own
transcripts. Codex works the same way through `CODEX_HOME`.

## Why this exists

Claude Code keeps one login per config directory. Conductor has one global
setting for agent environment variables. Point that setting at a config
directory and *every* workspace moves to that account, which is the same churn
as signing out and back in.

This routes per chat instead, so every account you add stays live at once. Two
chats in one workspace can be on two accounts, answering at the same time.

## Requirements

macOS, Conductor 0.81 or newer, and Claude Code or Codex already working inside
it. No other dependencies: everything here is POSIX shell.

## Getting started

**1. Install.**

```sh
curl -fsSL https://raw.githubusercontent.com/up1512001/conductor-hats/main/install.sh | sh
```

That picks the build for your Mac, verifies it against its published `.sha256`,
deploys the router under `~/.conductor-accounts`, and puts `hats` on
`~/.local/bin`. Add that to your `PATH` if it is not already there; the script
prints the line.

Prefer to look before you run it? Download the tarball for your Mac from the
[latest release](https://github.com/up1512001/conductor-hats/releases/latest) and
use the copy inside:

```sh
tar xzf hats-aarch64-apple-darwin.tar.gz     # or x86_64 on an Intel Mac
cd hats-aarch64-apple-darwin
./install.sh
```

**2. Build a Conductor copy to patch.** The panel is injected into the app, and
that costs the copy its notarization, so it never touches the Conductor you rely
on:

```sh
hats dev-app
```

This produces `/Applications/Conductor Dev.app` with its own bundle identifier,
database and keychain items. Both apps run at the same time.

**3. Inject the panel:**

```sh
hats patch
open "/Applications/Conductor Dev.app"
```

**4. Add your accounts, in the panel.** Click the account button next to "Open
in", pick a provider, then **Add new account**. Type a name, your browser opens
for approval, and you paste the code back into the panel. No terminal.

Each account gets its own config directory and therefore its own keychain item.
Your skills, plugins, commands and transcripts stay shared.

If you would rather not patch anything, `hats add work` does the same
from a terminal, and `/account` in any chat covers the rest.

**5. Pick an account per chat.** Open a chat, click the account button, choose
one. That chat alone moves; the others in the workspace stay where they are. The
same button sets the workspace when no chat is open, and one line under the
accounts sets every chat in it at once.

Choosing in the New Workspace composer sets the workspace you are about to
create, and the ones after it until you choose again.

A conversation that is already running cannot change account: its agent took one
when it started and never reads another. The panel says which account it is on
and which it will come up on next time you open it.

**6. After a Conductor update**, which replaces the bundle and removes the panel:

```sh
hats repatch
```

Check any of it with `hats doctor`, which reports what every layer
resolves to.

**7. Use Conductor from a phone.** `hats serve` mirrors projects, workspaces,
chats, transcripts, tools, live status and run settings in a mobile layout.
One authenticated WebSocket keeps both screens current. Replies typed on the
phone are durably queued, open their exact chat on the Mac when safe, and are
submitted through Conductor's own composer; replies made on the Mac stream back
to the phone. New chats are created through Conductor's own action and open on
the phone only after its database reports the exact new session. A fresh 64-hex path plus a one-use pairing
secret protects the browser,
and a named outbound HTTPS tunnel makes it reachable away from Wi-Fi without
opening a router port. See [docs/mobile.md](docs/mobile.md) for setup and the
security model.

Pairing has its own phone button beside the account control in the toolbar.
Enter the stable HTTPS tunnel address once,
create a pairing code, and scan the high-contrast QR with the phone. Each QR
gets its own random path under that address. The same
view copies the link, shows its ten-minute expiry and connected-phone status,
changes the address, revokes browsers, and stops access behind confirmations. Creating the code also
starts the protected loopback service automatically; opening the view alone is
still read-only.

Prefer to skip the patching entirely? `/account` in any Conductor chat works on an
unpatched install, and `hats use work` works from a terminal. You lose
the toolbar button, not the feature.

## What the panel does

The CLI list below is the terminal half. This is the half that matters, and it is
all inside Conductor:

| In the panel | What it does |
|---|---|
| Click a provider | Drills into that provider's accounts. Claude Code and Codex are listed separately, each showing its current account. |
| Click an account | Routes this workspace to it. The tick moves in place and the panel stays open, so the change is visible rather than inferred. |
| From the New Workspace chip | Binds the repository instead, so the workspace you are about to create starts on the account you picked. |
| Add new account | Signs in without a terminal. Type a name, your browser opens at the OAuth URL, paste the code back into the panel. |
| Sign out | Drops that account's credentials and nothing else, after a dialog that names the account and says what stays. |
| Sign in, on a signed-out row | The same flow, with the profile already known. A signed-out row is never a dead end. |
| Turn routing off | Puts every workspace back on one account without uninstalling anything. |
| Escape | Steps back from a provider to the list, then closes. |

Everything it shows is masked: addresses render as `fir**ast@ex**e.com`, so a
recorded session or a shared screenshot cannot hand one out. Signed in, signed in
with no address cached yet, and signed out are three distinct states, because
credentials can exist before an address is readable anywhere.

If two profiles end up on one address, the panel says so during sign-in: one live
token per account means the pair would otherwise take turns signing each other
out.

The panel is drawn with Conductor's own theme tokens, so it follows the app's
palette, radii and light or dark mode rather than approximating them. Nothing in
it moves once it is open. [docs/account-panel.md](docs/account-panel.md) covers
the behaviour and [docs/panel-internals.md](docs/panel-internals.md) how it
attaches.

## Everything else

| Page | What it covers |
|---|---|
| [docs/usage.md](docs/usage.md) | picking an account, several at once, binding a repository |
| [docs/cli.md](docs/cli.md) | every `hats` command, the layout on disk, turning it off |
| [docs/how-it-works.md](docs/how-it-works.md) | how credentials are namespaced, the router, precedence |
| [docs/account-panel.md](docs/account-panel.md) | the panel: layout, masking, signing in and out |
| [docs/panel-internals.md](docs/panel-internals.md) | how the panel attaches, and the update path |
| [docs/mobile.md](docs/mobile.md) | private, bidirectional access from a phone over the public internet |
| [docs/mobile-internals.md](docs/mobile-internals.md) | mobile architecture, security invariants and implementation learnings |
| [docs/patching-conductor.md](docs/patching-conductor.md) | what the app bundle allows, with the evidence |
| [docs/dev-conductor.md](docs/dev-conductor.md) | building a Conductor copy that is safe to modify |
| [docs/blank-window.md](docs/blank-window.md) | the copy launches and paints nothing: cause, guard, how to diagnose the next one |
| [CONTRIBUTING.md](CONTRIBUTING.md) | tests, linting, releases |
| [AGENTS.md](AGENTS.md) | rules for changing this code, human or agent |
| [CHANGELOG.md](CHANGELOG.md) | what changed, by version |

## What patching costs

The panel never deletes anything: signing out drops credentials and leaves the
profile, its routes, its session pins and its transcripts alone.
`hats remove` in a terminal is the only way to delete a profile,
deliberately. Addresses are masked wherever they render, as `fir**ast@ex**e.com`;
`hats list` in a terminal is where you read the real thing. See
[docs/account-panel.md](docs/account-panel.md).

It is not free. **Nothing outside the app can add UI to it.** Conductor's UI is
compiled into a single Developer ID signed Mach-O with the hardened runtime, and
every file in the bundle is covered by the code signature seal, so adding one
file is enough to make macOS reject the app. The panel is therefore *injected
into the compiled frontend* and the app is ad-hoc re-signed, which means:

- patch a copy, not your install: `hats dev-app`, then `hats patch`.
  The patcher refuses `/Applications/Conductor.app` unless you pass `--i-know`.
- every Conductor release ships a new bundle, so the patch has to be re-applied
  after each update.

`/account` in the chat survives updates and needs no patching, which is why it
exists alongside. It is not the interesting part: anyone can write a slash
command. Details and the tests behind both
claims are in [docs/patching-conductor.md](docs/patching-conductor.md).

If you would rather have this natively, ask Conductor for it: Help, then Send
Feedback, asking for per-workspace agent account selection.

## A note on accounts

Using several subscriptions to work around rate limits is against Anthropic's
usage policy. Choosing between a personal account and a work account that you
or your employer separately pay for is not. This project is built for the
second case.

## For the Conductor team

This project modifies Conductor, so you should know exactly how, and it is all
documented rather than implied.

- The patch is applied to a **copy** built by `hats dev-app`, which carries its own
  bundle identifier, database and keychain items. `hats patch` refuses
  `/Applications/Conductor.app` unless someone passes `--i-know`.
- **Nothing of yours is redistributed.** The release ships our binary and our
  scripts. The panel is injected on the user's own machine, against the Conductor
  they already installed.
- The copy is ad-hoc re-signed, which costs it notarization and its keychain
  access. That cost is stated plainly in the README and in
  [docs/patching-conductor.md](docs/patching-conductor.md), along with the fact
  that every Conductor release undoes the patch.
- Routing itself uses only your documented settings,
  `claude_code_executable_path` and `environment_variables`, and needs no patching
  at all.

**What we would rather have.** A per-workspace agent account selector in
Conductor. Everything here becomes unnecessary the day you ship one, which is the
right outcome and one we would be glad to see.

If any of this is a problem, or you want it changed or taken down, say so and it
will be: **utsav@up1512001.com**.

## Licence

MIT. See [LICENSE](LICENSE).
