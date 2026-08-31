# Mobile access internals and learnings

This document records the constraints and decisions behind mobile access. The
user-facing setup is in [mobile.md](mobile.md); this is the engineering reference
for changing the feature without rediscovering its failure modes.

## Product boundary

The phone mirrors the core Conductor loop: find a chat, read its transcript,
change next-message settings, send a reply, and create another chat. It does not
try to reproduce terminals, diffs, checkpoints, or the full desktop shell.

The boundary between hats and Conductor is deliberate:

- reads come from Conductor's databases through the existing read-only SQLite
  command path
- hats writes only to private state under `~/.conductor-accounts`
- messages, settings, navigation, and new-chat actions enter Conductor through
  its visible UI, driven by the injected panel
- hats never inserts, updates, or deletes a private Conductor database row

Conductor has no supported local control API for these actions. Writing its
undocumented queue tables would couple hats to private schema and invariants and
could corrupt real work. Driving the real controls is less convenient, but it
keeps Conductor responsible for its own state transitions.

## One Conductor copy, never a merged view

The release app and Conductor Dev can run together and maintain different
databases. The hats queues, however, are shared under one accounts root. An
unscoped command could therefore read a session from one copy and hand it to the
panel injected into the other.

Pairing records the database belonging to the workspace that created the code.
Every listener and queue command adopts that source before its first lookup:

- an explicit `CONDUCTOR_DB` must name a readable database
- otherwise a valid recorded mobile source must exist
- a missing, stale, foreign, or ambiguous source is an error
- no mobile path falls back to combining every installed Conductor database

This is intentionally fail-closed even though the agent router is fail-open.
They protect different outcomes: a router failure must not prevent an agent from
starting, while a mobile scope failure must not disclose another app's chats or
deliver an action to the wrong window.

## Read model

Conductor stores user message bodies as plain text and assistant traffic as JSON
SDK envelopes in the same column. JSON extraction must therefore be guarded by
`json_valid`. Text, thinking, tool calls, tool results, errors, and lifecycle
events also occupy different envelope shapes; flattening everything to prose
loses the shape users see in Conductor.

Other lessons from the database path:

- `conductor.db` is the authoritative transcript store; cache databases can be
  incomplete
- a WAL database can reject `mode=ro` when its shared-memory file is absent, so
  reads retry with `immutable=1`
- SQLite emits transcript JSON; delimiter-separated output is unsafe because
  messages routinely contain pipes, tables, commands, and code
- each chat belongs to one database; arrays from different copies must never be
  concatenated into invalid or cross-app JSON
- bounded transcript reads fetch the newest envelopes, then reverse envelopes
  rather than the flattened rows so blocks inside one response keep their order
- chat lists use Conductor's workspace/chat order instead of last activity, which
  would reshuffle the phone under the reader whenever an agent writes

There is no linked SQLite runtime. Change detection combines database and WAL
metadata with the highest message row and hats queue stamps. Queue metadata uses
nanosecond modification times so two quick operations cannot collapse into one
snapshot version.

## Network and authentication

The listener binds `127.0.0.1` by default. A named outbound HTTPS tunnel maps a
stable public origin to it; no router port or LAN listener is required. LAN-only
access was rejected because VPNs, client isolation, guest Wi-Fi, and changing
networks make it unreliable and because cleartext session traffic is not an
acceptable launch default.

Pairing has two separate secrets:

1. A short-lived, one-use secret appears after `#` in the QR URL. Fragments are
   not sent to the tunnel, origin server, referrer, or ordinary access log.
2. The page exchanges that secret in a header for a different random session in
   a `Secure`, `HttpOnly`, `SameSite=Strict` cookie.

The cookie authenticates protected HTTP routes and the same-origin WebSocket
upgrade, so no reusable credential appears in a socket URL. Revocation rotates
both pairing and browser-session secrets, and a live socket rechecks the session
so it disconnects after revocation.

Requests have fixed header and body bounds, transfer encoding is refused, and
all responses are private, non-cacheable, non-transformable, frame-denied, and
covered by a self-only content policy. Cloudflare terminates TLS and can
technically see the plaintext; deployments that reject that trust boundary must
use an end-to-end VPN while preserving stable HTTPS and WebSockets.

## Snapshot and reconnect protocol

One authenticated WebSocket carries snapshots and commands in both directions.
The state is the chat hierarchy, active transcript, accounts, Conductor's
current model catalog, queue receipts, new-chat receipts, and a source label.

Snapshots are **partial**. Each section carries its own stamp and only the
sections that moved are sent; a section absent from a message means the phone
keeps what it holds. `active` is the trap: it is legitimately `null` when no
chat is open, so absent and null must stay distinguishable end to end. Two tests
pin that, because collapsing them either strands the phone on a chat it closed
or throws away a transcript still being read.

A single stamp over everything was the original design and it was wrong in a way
that only shows up under load. It included `places::revision()`, the size and
mtime of `conductor.db` and its write-ahead log, and that changes on any
Conductor write anywhere on the machine. An agent streaming in an unrelated
workspace resent the entire chat list and the entire open transcript, measured
at 257 KB, roughly three times a second, over a tunnel, to a phone.

Two things fell out of splitting it. The database probe became one `sqlite3`
invocation covering both sections rather than one per tick, and it is skipped
entirely while the filesystem revision is unchanged, so an idle phone costs no
query at all rather than one every 350 ms.

A section is recorded as delivered only after the frame is on the wire. Recording
first and sending second means a dropped frame is a section the phone never sees
again.

Frames over 4 KB are gzipped and sent as binary. Compression runs outward only.
Decompressing anything a client sends would let a paired but hostile phone hand
over a small frame that expands into an enormous one, and the incoming message
cap would then be measuring the compressed size rather than the real one.

The client treats the socket as replaceable:

- reconnect uses bounded backoff
- the active session is subscribed again after reconnect
- a pending new-chat target is also restored
- an incoming snapshot preserves the reader's scroll position and expanded tool
  rows, whether it carried every section or only one
- the first active-chat snapshot says it is loading instead of claiming the
  conversation is empty
- a removed or foreign chat disables and hides the composer

Static mobile assets use content fingerprints in their paths. `no-store` is a
privacy rule, not a complete cache-busting strategy; a rebuilt script needs a
new address so a browser or proxy cannot reuse parsed bytes from an older build.

## Durable message delivery

A phone reply first becomes an owner-only JSON item below the hats accounts
root. The panel claims the oldest eligible work for the chat visible in its
Conductor copy. Items use leases rather than destructive reads so a panel reload
or crash does not lose them.

Delivery follows this sequence:

1. Resolve the exact project, workspace, and session inside the paired database.
2. If that chat is not visible, navigate Conductor to its exact route.
3. Refuse to replace a laptop draft; leave the phone item queued.
4. Apply any queued run control through Conductor's visible control first.
5. Set the real composer through its native value setter and input event, then
   use Conductor's send action.
6. Confirm only when the read-only database reports one more delivered user
   message with the same text than existed before the lease.

The occurrence baseline makes identical consecutive messages independently
acknowledgeable. Client request IDs make optimistic echoes and rejected-message
restoration exact even when message text is duplicated. A send error restores
the rejected text to the composer and exposes a status notice instead of losing
the draft.

## Run controls and new chats

Account, model, thinking, permission, and fast mode are next-message controls and
belong beside the composer. The account selector must remain directly reachable;
having an action in the protocol without a rendered control is not a feature.

The model picker has one owner: Conductor. Its provider APIs already determine
which built-in model IDs are visible and in what order, then pass that result to
the mounted picker as `visibleBuiltInModelIds`. The injected panel reads that
live React value without opening the menu and publishes it with the visible
workspace ID through a confined `hats remote catalog` command. That workspace
must resolve inside the paired database, and the private record also names the
exact Conductor database that supplied it, so another installed app copy cannot
replace or reuse its choices. Mobile appends a chat's current model only when
Conductor's live catalog does not contain it; hats carries no fallback model
inventory of its own. Repeating an unchanged publication does not rewrite the
record or wake every connected phone.

That live publication also carries non-placeholder session-title overrides.
Conductor can keep `New Chat` in the read-only session row while its sidebar
already presents the workspace's custom name or branch. The snapshot keeps a
real generated session title, otherwise it derives the readable workspace label
Conductor presents. Session objects in the mounted component tree may supply a
newer real title, but placeholder catalog entries are ignored so they cannot
replace the workspace presentation value.

Thinking values are a capability of the model, not of the provider. Sonnet 4.6
stops at Max with no Extra high, Opus carries Ultracode above Max, Haiku has no
effort control at all, and gpt-5.6 calls its lowest level Light. The phone
mirrors Conductor's own registry per model, hides the control on a model with no
levels, and draws Conductor's meter filled to the level in force. Offering one
list per agent showed levels Conductor would never accept. Wire values stay
unchanged so the Mac receives exactly what was chosen on the phone.

Effort does not go through the control beside the composer. That control is a
bar meter bound to `chat.toggleThinking`: a press advances one level rather than
opening a list, so pressing it applied whatever came next, the check never saw
the level that was asked for, and the phone reported the setting refused after
two attempts while the level on the Mac moved on its own. Effort calls the
picker's own `onApplyConfiguration` with the configuration the chat is already
holding and only the level changed, which is the function a click on the Mac
reaches. The picker's Effort submenu is the compatibility fallback. A test keeps
the composer control out of the opener.

Conductor keeps a Claude effort column and a Codex thinking column on every chat
and writes only the one its agent owns, so the other holds whatever it last
held. Both the applied check and the chat list pick the column by the chat's
agent; reading the first non-empty of the two compared a Codex chat with a stale
Claude value and made every acknowledgement fail.

Model changes call the exact mounted picker `onSelect` handler whose enclosing
component belongs to the selected session. This is Conductor's own model
mutation path, including its cross-provider new-chat behavior. Opening the
visible picker and choosing its exact menu item remains a compatibility
fallback; neither path writes Conductor's database, and both require database
readback before the setting is acknowledged.

Settings have durable leases and failure receipts. Before retrying a click, the
panel checks Conductor's database state. This prevents a slow successful toggle
from being toggled back on retry. After two failed attempts, the phone receives
an explicit error until it acknowledges the receipt or it expires. A short
retry pause prevents a refused setting from holding the queued message behind
it for tens of seconds.

Picker choices are matched to Conductor's human label exactly after normalizing
the wire ID. Substring matching is unsafe: `sonnet-4-6` matches both “Sonnet
4.6” and “Sonnet 4.6 1M”, so DOM order can silently click the wrong model and
make every database acknowledgement fail. A cross-provider choice follows
Conductor's own behavior: it opens a new chat, hats identifies the new session
by its model and pre-click workspace marker, and the phone moves to that chat
after acknowledging the durable result.

New-chat requests use the same principles:

- record the workspace and the newest session marker before acting
- use Conductor's visible New Chat action when it can be found
- fall back to Conductor's documented `nav.newTab` shortcut command
- identify the exact new session by database readback
- check for that session before retrying, so a slow first action cannot create a
  duplicate chat
- retain success or failure until the phone acknowledges it

## Mobile layout invariants

The page is an application shell: top bar, one scrolling content pane, and the
composer as a real footer. Overlaying or guessing space for the composer caused
the last message to be covered, especially after run controls were added.

A tool call and the result that answered it share one row, as they do on the
Mac, with the tool's own mark, what it was pointed at, and for a change to a
file what it added and removed. A read is named by what came back. Consecutive
rows collapse behind a count and the marks of the tools they used; expanding
shows the input and the output. Thinking is a row of the same shape.

The marks are Conductor's, not an interpretation of them: the lucide nodes it
draws, matched by the same tool names it matches, copied out of its frontend and
documented in `src/mobile/icons.ts` with the command that extracts them again.
A hand-made set answered the shape of a tool rather than the tool, which is how
the phone ended up showing a padlock where the Mac showed a terminal. Text
glyphs are not used as pretend icons, and open activity remains open across live
snapshot replacement.

The client preserves these UX rules:

- project → workspace → chat hierarchy matches Conductor's mental model
- rows carry status, provider, and account; next-message settings live in the
  composer
- narrow layouts may hide text labels, but icons and accessible names remain
- account/model labels truncate instead of expanding the composer beyond the
  viewport
- transient failures use an accessible status region
- opening a chat begins at the end, while subsequent snapshots preserve reading
  position and expanded details

## Build, install, and Dev patch discipline

The release binary embeds generated browser clients. The only safe order is:

```sh
pnpm typecheck
pnpm build
pnpm verify
cargo build --release
```

Building Rust before rebuilding the browser assets creates a valid binary with
stale UI—the source can be fixed while the injected app still runs old code.
After building, compare the exact release binary with both installed locations,
then patch only `/Applications/Conductor Dev.app` and run `hats verify`.

Never patch the real Conductor app. Repatch with `--no-launch` when the user owns
runtime and visual QA. Re-signing the Dev copy clears its keychain items, so a
repatch has a real sign-in cost and should not be repeated casually. Verification
must cover the injected marker, clean bundle ending, boot guard, signature,
`allow-jit`, rewritten identifier, and rewritten runtime.

Two integration tests deliberately bind loopback listeners. When a test session
forbids starting servers, compile them with the suite but skip their execution:

- `a_terminal_listener_shows_only_the_conductor_copy_the_phone_is_paired_with`
- `an_unpaired_listener_refuses_to_expose_every_copy`

All other static, queue, protocol, panel-source, authentication, and database
tests can run without opening Conductor or starting the listener.

## Two failures that hide themselves

Both cost most of an afternoon, and neither produced anything in a log.

**A tunnel forwards a body-less POST as chunked.** The HTTP parser refused any
request carrying `Transfer-Encoding`, which is a reasonable request-smuggling
guard until you notice that Cloudflare Tunnel forwards `POST /api/pair` to the
origin as `chunked` with no `Content-Length`. Every phone pairing through the
public hostname got `400`, while `curl` against `127.0.0.1` paired perfectly,
because a direct client sends `Content-Length: 0`. The browser only ever showed
`This pairing link expired or was already used`. What actually has to be refused
is the disagreement a smuggler needs: both framings on one request, or a
transfer coding this does not implement. Chunked bodies are decoded and held to
the same ceiling a declared length is.

**Stop mobile access unpairs the app.** It calls `auth::invalidate_all`, which
clears `serve-source` along with the pairing and the catalog. After that the
listener cannot start at all: `mobile_scope::adopt` fails in milliseconds, the
child exits before reporting ready, `start()` kills it at the two second mark,
and `private_log()` truncates `error.log` on the next attempt so the reason is
gone before anyone reads it. The panel only polls status and never tries to
start anything, so the phone shows `Reconnecting` forever with no message
anywhere. `mobile-pair` is the only caller of `mobile_scope::bind`, which makes
**Create pairing code** the sole recovery. Read `error.log` before retrying.

Related, and the reason a listener once died mid-session: `hats install` used
`std::fs::copy`, which truncates the target in place, and that target is the
running router as well as any `hats serve` the panel started. It stages and
renames now, so anything already running keeps the image it started with.
