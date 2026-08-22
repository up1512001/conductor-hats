# When the copy paints nothing

Conductor 0.82 shows an empty window in a patched copy. The app launches, the
signature is valid, nothing is logged.

## Cause

0.82 renders its UI only once a client version check has settled. In a re-signed
copy that check never settles, so the gate renders nothing, permanently.

It is not the panel: a copy with the panel left out is equally blank. Nor the
identifier rewrite, the profile, the injection, the IPC bridge or the network,
each ruled out against a control before the fix was written.

## What hats does about it

`hats patch` injects a small boot guard ahead of Conductor's entry module which
answers that one request with an error. Conductor already handles the check
failing: it logs and carries on.

The cost, stated rather than buried: a patched copy does not enforce Conductor's
minimum client version. Remove the guard the day a release settles the query.

`hats verify` reports whether the guard is present.

## If a future release breaks it again

```sh
hats verify                                   # what is statically checkable
hats assets                                   # the embedded frontend assets
hats assets --dump '/index.html'              # one asset, decompressed
hats patch --asset KEY --prepend --script F   # inject a probe instead of the panel
```

`--script` may be repeated, and `--asset`/`--prepend` apply to the next one, so a
single patch can carry a probe alongside the real panel.

## One thing to expect

The copy has its own database and its own keychain items, so it signs in
separately from your real Conductor.
