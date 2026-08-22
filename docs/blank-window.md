# When the copy paints nothing

Conductor 0.82 shipped a change that makes a patched copy launch, sign clean and
then show an empty window. This is what it is, what it is not, and how to find
out for yourself when the next release does something similar.

## What it is

0.82 puts the whole UI behind one request:

```
GET https://api.conductor.build/minimum-client-version
```

The component holding that query renders `null` while the query is neither
settled nor failed. In a re-signed copy it never settles, so the window stays
empty for ever and nothing is logged anywhere.

Conductor already handles the check *failing*: it carries on and renders. So
`hats patch` injects a small script, the boot guard, that answers that one
request with an error. Nothing else is intercepted.

The cost, stated rather than buried: a patched copy does not enforce Conductor's
minimum client version. The endpoint currently answers `0.0.0`, so nothing is
being skipped today, and this should be removed the day a release settles the
query. `hats verify` reports whether the guard is present.

## What it is not

Each of these was measured in the copy before the guard was written, because
each looks like the obvious culprit and none of them is.

| Suspect | Evidence it is innocent |
|---|---|
| The account panel | A copy with the panel left out, only a probe in the entry chunk, is equally blank |
| The identifier rewrite | A copy built with the identifier left at `com.conductor.app`, run with an isolated `HOME`, is equally blank |
| The profile | Blank with a warm profile, blank with a fresh one, blank after a reload |
| Brotli or the injection | The panel evaluates: `window.__conductorHats` is set, and the bundle passes `node --check` as a module |
| The IPC bridge | Every Tauri command answers, including `is_running_under_rosetta` |
| The network | The identical request issued by hand in the same window returns 200 and streams its body in full |

React does mount. The tree stops at the component holding that query: its other
two queries succeed, that one stays `pending`/`fetching` for as long as the
window is open.

## Finding this out again

Three commands do the work. All of them act on a copy.

**Inject a probe instead of the panel.** `patch` takes an arbitrary script, an
arbitrary asset, and a side to put it on:

```sh
hats patch --asset '/assets/index-<hash>.js' --prepend --script probe.js
```

`--script` may be repeated; `--asset` and `--prepend` apply to the next one. So
one command can carry a probe *and* the real panel, which is how the two were
told apart.

**Read the frontend.** `hats assets` lists the embedded assets and
`--dump` prints one decompressed, which is how the gate above was read:

```sh
hats assets --dump '/index.html'
hats assets --dump '/assets/renderApp-<hash>.js' > render.js
```

**Get the probe's output back.** The copy has no console you can reach, so the
probe should send what it sees somewhere you can read. An image request to a
local port is enough and is not subject to CORS:

```js
new Image().src = "http://127.0.0.1:8899/?m=" + encodeURIComponent(message);
```

Useful things for a probe to report, in the order they narrow it down:

- `document.getElementById("root").childElementCount` — zero means React
  rendered nothing, which is different from React never running.
- The keys on `#root`: a `__reactContainer$…` key means React did mount.
- Walk that fiber's `child`/`sibling` chain and report each `type.name`. The
  last name is the component that returned null.
- Walk that fiber's `memoizedState` hook chain and report anything carrying
  `status`/`fetchStatus`. That names the query that never settles.
- Wrap `window.fetch`: Tauri commands go through it as `ipc://localhost/<command>`,
  so this logs every IPC call, its arguments and how long it took.

**Check the copy afterwards.** `hats verify` asserts the things that are
statically checkable in one command: both scripts present, the bundle
decompresses and closes cleanly, the signature is valid, `allow-jit` survived
re-signing, and no trace of the real app's identifier is left.

## One thing to expect after it renders

The copy has its own database and its own keychain items, so it is signed out.
0.82 opens on **Sign in to continue**. Sign in there and the account button
appears in the workspace toolbar as usual.
