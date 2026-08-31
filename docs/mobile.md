# Conductor from a phone

Implementation constraints, data flow, failure recovery, and lessons from the
build are recorded in [mobile-internals.md](mobile-internals.md).

`hats serve` is a compact Conductor screen for a phone. It shows every open
chat in a project → workspace → chat hierarchy, and carries the details that matter while work is
running: status, unread and pending counts, provider, model, effort, permission
mode, fast mode, context use, active account, transcript, thinking, tool calls,
tool results and errors.

It synchronizes in both directions:

- messages and state written by Conductor stream from the Mac to the phone over
  one authenticated WebSocket
- a phone reply is saved to a private durable queue, then the injected panel
  puts it through Conductor's own composer for that exact chat
- the phone keeps showing the reply as waiting or sending until Conductor's
  database confirms that it received the user message
- **New chat** asks Conductor to open another chat in the same workspace, then
  opens the exact session Conductor recorded. A retry checks for that receipt
  before acting again, so a slow first attempt cannot create a duplicate.

The pairing belongs to the Conductor app whose open workspace created it. Its
listener reads only that app's database, labels the phone screen as Conductor or
Conductor Dev, and refuses ambiguous or foreign workspace/session ids. Creating
a pairing from another Conductor copy rotates the browser session and restarts
the loopback listener for that copy, so an open-source installation always binds
to the user's own selected app and never to the developer's machine.

That binding holds however the listener is started. The panel starts one with
the app's database already selected; `hats serve` run by hand adopts the same
recorded binding and prints which copy it is showing. Before any app is paired,
the listener refuses to start. A public mobile endpoint is never allowed to
guess between Conductor copies or expose their combined data.

When the target chat is not visible, the injected panel opens its exact project,
workspace and session route, then submits through the real composer. It refuses
to switch away while the laptop composer contains a draft. Nothing writes a
private Conductor database table. Account, model, thinking, fast-mode and
permission controls sit beside the phone's message composer. They live there
and nowhere else: a chat row carries status, agent and account, because a model
is a setting the next message is sent with rather than a fact about the row.
Account changes are pinned for the next agent start; the other changes travel
through Conductor's visible controls before the queued message is submitted. A
setting that Conductor refuses twice becomes an explicit failure on the phone
instead of disappearing, naming the setting and the value it could not apply.

## Security model

The listener stays on `127.0.0.1`. A named Cloudflare Tunnel makes an outbound
connection from the Mac and maps a stable HTTPS hostname to that loopback
listener, so no router port, public listener or shared Wi-Fi is involved.

The tunnel is transport, not the only login. hats also requires a pairing flow:

1. `hats serve` prints a one-use link with a fresh 64-hex path and a token that
   expires after ten minutes.
2. The authentication token is after `#`, so browsers do not send it to a proxy, server log or
   referrer.
3. The page exchanges it in a request header for a different 256-bit session
   cookie marked `Secure`, `HttpOnly` and `SameSite=Strict`.
4. `hats serve --revoke` invalidates every paired browser immediately.

Responses are private, non-cacheable and `no-transform`, so an edge proxy must
not inject analytics JavaScript into the authenticated chat page. Keep the
self-only script policy. If Cloudflare Web Analytics was enabled manually for
this hostname, disable its automatic setup instead of allowing its beacon in
the page's content policy.

The pairing controls live in their own phone button beside the account control
in Conductor's toolbar. On first use, enter the stable public
HTTPS address supplied by the named tunnel. Press **Create pairing code** and
scan the white QR with the phone camera, or use **Copy link**. The panel shows
the ten-minute expiry and masks both the random path and secret in its visible
URL. **Revoke paired
phones** signs every browser out after confirmation and displays a replacement
one-use QR. **Stop mobile access** disconnects every phone, revokes its browser
session and stops only hats' loopback listener.

Opening this view never starts a listener, tunnel, or browser. Pressing
**Create pairing code** is the explicit action that starts the loopback listener
when needed; it reuses an already-running listener and shows its local address
and status in the panel. The named tunnel remains independently managed, and no
browser is opened.

Secrets and queued messages live under `~/.conductor-accounts` with owner-only
permissions. Requests have fixed header and body limits, and every data, send
and WebSocket route checks the current session secret. The unauthenticated
HTML, CSS and JavaScript contain no Conductor data.

Cloudflare terminates TLS at its edge, so it can technically see traffic. Add a
Cloudflare Access policy for the hostname as a useful second gate. If that
trust model is unacceptable, use an end-to-end VPN instead; it must preserve a
stable HTTPS origin and WebSockets.

## Public internet setup

Use a named tunnel. Quick Tunnels have random hostnames and cannot supply the
stable origin used by the secure browser cookie and permanent QR entry point.

```sh
brew install cloudflared
cloudflared tunnel login
cloudflared tunnel create conductor
cloudflared tunnel route dns conductor conductor.example.com
```

Put the generated tunnel UUID and credentials path in
`~/.cloudflared/config.yml`:

```yaml
tunnel: 00000000-0000-0000-0000-000000000000
credentials-file: /Users/you/.cloudflared/00000000-0000-0000-0000-000000000000.json
ingress:
  - hostname: conductor.example.com
    service: http://127.0.0.1:8787
  - service: http_status:404
```

In Cloudflare Zero Trust, create a self-hosted Access application for the same
hostname and allow only your identity. Start the tunnel, then open **Mobile
access** in the Conductor copy you intend to use, save the hostname and create
the first pairing code. That explicit action binds the source and starts hats:

```sh
cloudflared tunnel run conductor
```

Open the pairing link printed by `hats serve` on the phone. It works on mobile
data and any other internet connection; the phone does not need to share the
Mac's Wi-Fi.

Every pairing URL has the form
`https://conductor.example.com/<64-hex-path>#token=<one-use-secret>`. The named
tunnel forwards every path on the hostname to the same loopback listener, so a
new QR does not need a new DNS record or tunnel rule.

Alternatively, create and scan the link from **Mobile access** in Conductor.
Entering the address there saves it owner-only, so later `hats serve`,
`hats serve --pair`, and `hats serve --revoke` can reuse it without another
`--origin` argument. The panel starts `hats serve` on `127.0.0.1:8787` when the
pairing code is created, so no separate Terminal command is needed for that
flow.

To pair another browser while the server is already running:

```sh
hats serve --pair --origin https://conductor.example.com
```

To sign every paired browser out and create a replacement link:

```sh
hats serve --revoke --origin https://conductor.example.com
```

`HATS_SERVE_ORIGIN=https://conductor.example.com` can replace repeated
`--origin` arguments.

## Verification checklist

Before relying on it:

- open the hostname in a private window without a pairing link; no chat data
  should load
- with both Conductor and Conductor Dev installed, confirm the phone lists only
  the copy the pairing was made from
- pair on the phone using mobile data, not the Mac's Wi-Fi
- send from a chat that is not visible on the Mac and confirm Conductor opens
  that exact chat and receives it
- send from Conductor and confirm the phone updates without a manual refresh
- leave a laptop draft in the composer; a queued phone reply must wait
- change model or effort on the phone and confirm the Conductor composer control
  reflects it
- create a new chat from a workspace and confirm the exact new Conductor chat
  opens on the phone
- disconnect and reconnect while a chat is open; its transcript and pending
  actions should resume without navigating away and back
- press **Stop mobile access**, then confirm the phone disconnects and its old
  session cannot reconnect
- confirm nothing answers on the Mac's LAN address and no router port exists

After one Conductor copy has been paired, `hats serve` can be started from a
Terminal and adopts that recorded source; it refuses to start before then.
Terminal-started `hats serve` and `cloudflared tunnel run conductor` are
foreground processes. The Conductor pairing button instead detaches its
loopback listener after confirming that it is ready. Keep a manually managed
tunnel alive with its installed service or another process manager you already
use.
