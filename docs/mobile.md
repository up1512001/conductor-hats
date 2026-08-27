# Conductor from a phone

`hats serve` puts a read-only screen in front of Conductor's own state: every
chat, which account each is on, and what was said in any of them. It binds
loopback, and the way out is a tunnel that connects outward and authenticates
at the edge.

```sh
hats serve
```

```
hats serve, read-only, on http://127.0.0.1:8787

  /            the screen
  /api/chats   every open chat and its account
  /api/chat    one conversation, ?session=<id>&limit=<n>
  /api/events  a stream that fires when Conductor's state moves
```

Open it on the machine itself first. Everything below is about reaching it from
somewhere else.

## What it can and cannot do

Read-only, and that is the point of the first version. Nothing here changes an
account, sends a message, or writes to Conductor. A `POST` is refused before it
is parsed. That is what makes it safe enough to leave running while the
authentication story is still a tunnel and a policy rather than code.

It reads Conductor's databases the way the rest of hats does: read-only, with
the `immutable=1` fallback for the WAL case where Conductor is closed. It never
holds a connection open, so it cannot block Conductor's own writes.

## Why not just bind the LAN

`--host 0.0.0.0` exists and prints a warning, because it is fine for a
five-minute test and wrong as a setup:

- there is no password on it, so anything on that network can read your work
- a laptop moves between networks, and the ones it visits are not yours
- a VPN client on either device silently breaks it. Measured: with Cloudflare
  WARP running, every address on the subnet was unreachable and the failure
  looked exactly like a router problem

## The tunnel

`cloudflared` connects outward from this machine to Cloudflare and serves a
hostname you own. Nothing is exposed inbound, and Cloudflare Access does the
authenticating, which is a great deal better than a token this repository would
have had to invent.

```sh
brew install cloudflared
cloudflared tunnel login
cloudflared tunnel create conductor
cloudflared tunnel route dns conductor conductor.example.com
```

Point it at the loopback server, in `~/.cloudflared/config.yml`:

```yaml
tunnel: conductor
credentials-file: /Users/you/.cloudflared/<tunnel-id>.json
ingress:
  - hostname: conductor.example.com
    service: http://127.0.0.1:8787
  - service: http_status:404
```

Then, in the Cloudflare dashboard, under Zero Trust → Access → Applications, add
a self-hosted application for that hostname with one policy: allow, and your
email address only.

```sh
hats serve            # loopback, in one terminal
cloudflared tunnel run conductor
```

### Test that the policy is on

A tunnel with a policy that is not enforcing is worse than no tunnel, because it
feels safe. Before trusting it:

- open the hostname in a private window and confirm you are challenged
- do it on mobile data, off your own network
- confirm `http://127.0.0.1:8787` still works locally and that nothing answers
  on your LAN address

## What Cloudflare sees

Cloudflare terminates TLS, so your chat transcripts and code pass through their
edge in plaintext. For most private dashboards that is an accepted trade. It is
your work and your clients' code, so make it deliberately.

If it is not acceptable, Tailscale is the alternative: `tailscale serve` gives
HTTPS with no third party in the middle, at the cost of the app on every device
and a `ts.net` hostname instead of your own.

## Keeping it running

`hats serve` is a foreground process and stays one. Wrap it in whatever you
already use for background work rather than having hats learn to daemonise:

```sh
brew services   # if you package it
launchd         # a plist in ~/Library/LaunchAgents
tmux            # for the version you are still deciding about
```

## What it costs Conductor

Nothing measurable. The screen polls a single query about twice a second, which
is one `sqlite3` invocation of roughly 5 ms. The chat list is about 80 ms across
160 chats; a transcript of 60 messages is about 50 ms.
