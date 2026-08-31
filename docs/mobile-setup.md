# Setting up Conductor from a phone

Start to finish, from nothing to a working phone. What it is and why it is built
this way is [mobile.md](mobile.md); the internals and the failure modes are
[mobile-internals.md](mobile-internals.md).

Budget twenty minutes the first time. Most of it is Cloudflare.

## What you end up with

A private URL on your own hostname that shows the Conductor copy you paired it
with: every open chat, its transcript as it streams, and the run settings the
next message will be sent with. You can reply, change model or effort, and open
a new chat, all of which go through Conductor's own controls on the Mac.

No router port is opened. The listener stays on `127.0.0.1`, and an outbound
tunnel supplies the public hostname.

## Before you start

- **macOS**, with Conductor installed
- **A domain on Cloudflare.** Free tier is enough. A named tunnel needs one; a
  Quick Tunnel gets a random hostname each run, which cannot supply the stable
  origin the browser cookie and the QR entry point are pinned to
- **hats installed and on `PATH`**

```sh
git clone https://github.com/up1512001/conductor-hats
cd conductor-hats
pnpm install && pnpm build
cargo build --release
./target/release/hats install
```

`install` copies the binary to `~/.conductor-accounts/bin` and points
Conductor's `claude_code_executable_path` at the router. Re-run it after every
pull: the phone client and the server both live in this binary, and a stale copy
is the single most common cause of confusing behaviour.

## 1. Patch a Conductor copy

Pairing is driven from a panel injected into Conductor, so you need a patched
copy. **Never patch your real Conductor**; `hats patch` refuses it, and a
re-signed copy loses the keychain items your real install depends on.

```sh
hats dev-app          # builds Conductor Dev.app, a re-signed copy
hats patch --app "/Applications/Conductor Dev.app"
hats verify --app "/Applications/Conductor Dev.app"
```

`verify` should print ten `ok` lines. Patching clears that copy's keychain
items, so **it starts signed out**. Open it and sign in before going further.

Re-apply the patch after every Conductor release, and after every `pnpm build`
that changes `src/panel/`.

## 2. Create the tunnel

```sh
brew install cloudflared
cloudflared tunnel login
cloudflared tunnel create conductor
cloudflared tunnel route dns conductor conductor.example.com
```

Write `~/.cloudflared/config.yml`:

```yaml
tunnel: 00000000-0000-0000-0000-000000000000
credentials-file: /Users/you/.cloudflared/00000000-0000-0000-0000-000000000000.json
ingress:
  - hostname: conductor.example.com
    service: http://127.0.0.1:8787
  - service: http_status:404
```

Run it:

```sh
cloudflared tunnel run conductor
```

That is a foreground process. `cloudflared service install` will keep it up
across reboots, but note it installs a **root** launchd daemon, which hats
cannot start or stop. If you want the tunnel to exist only while mobile access
is on, keep it as a user process you start yourself.

## 3. Put Access in front of it, if you can

In Cloudflare Zero Trust, create a self-hosted Access application for the same
hostname and allow only your own identity. hats authenticates every request on
its own, so this is defence in depth rather than the lock itself, but it means
an unauthenticated request never reaches your Mac at all.

## 4. Save the address and pair

In the patched Conductor copy, open a workspace, then the phone button beside
the account control in the toolbar.

1. Enter `https://conductor.example.com` and save it. It is stored owner-only,
   so later commands do not need `--origin`
2. Press **Create pairing code**

That one press does three things: it records which Conductor copy the phone may
mirror, starts the loopback listener, and mints a one-use link that expires in
ten minutes.

3. Scan the QR with the phone

The equivalent from a terminal, once a copy has been paired at least once:

```sh
hats serve --pair --origin https://conductor.example.com
```

## 5. Check it actually works

From the Mac, with a bogus token, so nothing real is spent:

```sh
curl -s -o /dev/null -w "%{http_code}\n" \
  -X POST -H "X-Hats-Token: bogus" -H "Transfer-Encoding: chunked" \
  https://conductor.example.com/api/pair
```

`401` is correct: the request reached hats and hats refused the token.

Then, on the phone, walk the list in [mobile.md](mobile.md#verification-checklist).
The two worth doing every time are pairing over **mobile data** rather than the
Mac's Wi-Fi, and confirming the hostname shows nothing in a private window
without a pairing link.

## Day to day

| you want to | do this |
|---|---|
| pair another browser | **Create pairing code** again, or `hats serve --pair` |
| sign every phone out | **Revoke paired phones**, or `hats serve --revoke` |
| turn it off | **Stop mobile access** |
| turn it back on | **Create pairing code**. See the warning below |
| move to another Conductor copy | pair from that copy; it rebinds and rotates the session |

**Stop mobile access unpairs the app.** It does not merely stop the listener: it
also clears the record of which Conductor copy the phone may mirror. Until you
create a pairing code again, `hats serve` exits immediately with `mobile access
is not paired with a Conductor app`, and because the panel only polls status
rather than starting anything, the phone shows nothing but `Reconnecting`. If
you only want to pause, quit the Conductor copy instead.

## When something is wrong

| symptom | cause | fix |
|---|---|---|
| `502` from the hostname | nothing listening on `127.0.0.1:8787` | **Create pairing code** |
| `400 {"error":"invalid or oversized request"}` | a hats build older than the chunked-body fix. Cloudflare forwards a body-less POST as `Transfer-Encoding: chunked` | pull, `pnpm build`, `cargo build --release`, `hats install`, restart the listener |
| `This pairing link expired or was already used` on a link you just made | same as above, or the link really was used | check the `curl` above returns `401`, not `400` |
| phone stuck on `Reconnecting`, nothing in the logs | `serve-source` cleared by **Stop mobile access** | **Create pairing code** |
| listener dies seconds after starting | it exits before reporting ready, and `start()` kills it at the two second mark | read `error.log` **before** retrying, see below |
| panel button says `off` while a listener is clearly running | the recorded pid does not match a live process | **Stop mobile access**, then pair again |
| phone paired but shows the wrong Conductor copy | the pairing binds one copy | pair again from the copy you want |

`~/.conductor-accounts/serve-runtime/error.log` holds the listener's stderr and
is the first thing to read. It is **truncated on every start attempt**, so copy
it before you retry anything, or the reason is gone.

To see the failure directly, run the listener in the foreground; it prints the
real reason rather than dying silently under the panel:

```sh
hats serve --host 127.0.0.1 --port 8787
```

## What is exposed, and what is not

- The listener binds loopback only. Nothing answers on your LAN address
- Every request is authenticated by hats itself, independently of Cloudflare
- The unauthenticated page contains no Conductor data
- **The QR encodes a working credential.** The visible text is masked, the QR is
  not. Do not put one in a screenshot or a video without running
  `hats serve --revoke` afterwards, which rotates the path and the session
- Snapshots are compressed on the way out only. Nothing a client sends is ever
  decompressed
