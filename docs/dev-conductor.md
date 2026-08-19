# A Conductor you can safely break

`tools/make-dev-conductor.sh` builds `Conductor Dev.app`: a copy of Conductor
with a different bundle identifier, ad-hoc re-signed, isolated from your real
install. Use it to try modifications without risking the Conductor you work in.

```sh
tools/make-dev-conductor.sh          # build it
open "/Applications/Conductor Dev.app"
rm -rf "/Applications/Conductor Dev.app"   # undo, completely
```

Takes about five seconds. Nothing outside `/Applications/Conductor Dev.app` and
its own data directory is written.

## What isolation actually requires

The bundle identifier is not only in `Info.plist`. It is compiled into the
binaries, and it is what builds both the Application Support path and the
keychain service name. Change `Info.plist` alone and the copy happily writes to
the real Conductor's SQLite database.

`com.conductor.dev` is the same 17 bytes as `com.conductor.app`, so every
substitution is byte for byte with no offsets or length fields to repair.

The identifier appears four times in `Contents/MacOS/conductor`. They are not
interchangeable, and a blind search and replace gets two of them wrong:

| Where | Recognised by | Patch it? |
|---|---|---|
| Tauri config blob | followed by `http://localhost:1420` | **yes**, drives the data directory |
| Keychain service prefix | preceded by a `0x12` length byte, followed by `.` | **yes**, see below |
| CodeDirectory | next to the team identifier `27XN666UJ7` | no, `codesign` rewrites it |
| Signature requirements blob | inside the `0xfade0c00` structure | no, same |

Plus one in `conductor-runtime`, a JavaScript comparison against
`__CFBundleIdentifier`, which has to agree or the check silently fails.

### The keychain one is not optional

Skip it and the copy launches with:

> "Conductor Dev" wants to use your confidential information stored in
> "com.conductor.app.production.settings" in your keychain.

That is the copy reaching for the **real** Conductor's credentials. Deny it. The
string is a length-prefixed entry in an encoded table:

```
0x12 "com.conductor.app." <backref> ".settings"   ->  com.conductor.app.production.settings
```

`0x12` is 18, the length of `com.conductor.app.` including the trailing dot, so
substituting 17 bytes in place keeps the length byte correct. With it patched
the copy creates `com.conductor.dev.production.settings` for itself and never
prompts.

### Entitlements have to be carried over

The original has exactly three:

```
com.apple.security.cs.allow-jit
com.apple.security.cs.allow-unsigned-executable-memory
com.apple.security.device.audio-input
```

Re-sign without `allow-jit` and the WebView cannot compile JavaScript, so the
app launches and dies. The script extracts them from the original and passes
them to `codesign`.

## What you give up

Ad-hoc signing loses the Developer ID and the notarization ticket:

```console
$ codesign -dvv "/Applications/Conductor Dev.app"
CodeDirectory flags=0x10002(adhoc,runtime)
Signature=adhoc
TeamIdentifier=not set

$ spctl -a -t exec "/Applications/Conductor Dev.app"
rejected
```

`spctl` rejecting only matters for quarantined files, and the script clears the
quarantine attribute, so the copy launches locally. It will not run on anyone
else's machine, which for a test rig is the right outcome.

The copy also starts empty: its own database, its own Conductor login, its own
downloaded agent binaries.

## A quirk worth knowing

The data directory does not come out as the identifier verbatim. The last
character is always `p`:

| Identifier patched in | Directory created |
|---|---|
| `com.conductor.app` | `com.conductor.app` |
| `com.conductor.dev` | `com.conductor.dep` |
| `com.conductor.xyz` | `com.conductor.xyp` |

So Conductor builds the path from a 16-byte prefix of the identifier plus a
literal `p` held somewhere else, rather than from the whole string. Cosmetic:
the directory is still distinct from the real one, which is the property that
matters. Verified by `lsof` on the running copy, which shows zero handles under
`com.conductor.app`.

## What this does and does not unlock

It gives you a Conductor you can modify and relaunch without consequences, and
it is the prerequisite for changing the UI rather than the thing that changes it.

The frontend is still minified JavaScript compiled into the binary and brotli
compressed, with no asset files on disk, but it is reachable:
[extract-assets.py](../tools/extract-assets.py) reads it and
[patch-ui.py](../tools/patch-ui.py) writes it back. That is how the account panel
gets in, and it only happens to a copy, because the re-signing it needs would cost
your real install its notarization and its keychain access. See
[account-panel.md](account-panel.md) for the panel and
[patching-conductor.md](patching-conductor.md) for what the bundle does and does
not allow.

Note that `~/.conductor/settings.toml` is user-scoped rather than bundle-scoped,
so both apps read the same settings, including the account router. That is
usually convenient, and worth remembering when a change appears in the copy that
you did not expect.
