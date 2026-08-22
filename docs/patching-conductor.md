# Can an extension add UI to Conductor?

Two answers, and the distinction is the whole point of this page.

**To a Conductor you rely on, from outside: no.** Nothing a third party can do
adds a control to the toolbar or the New Workspace modal of an installed,
notarized Conductor. The evidence is below, and it has not changed.

**To a copy you re-sign yourself: yes.** The frontend is compiled into the
binary, but it is readable, writable and re-signable, so the panel this project
ships is *injected into a copy* rather than added to your install. That works, it
is what [account-panel.md](account-panel.md) documents, and it costs real things:
Gatekeeper trust, notarization, and re-application after every Conductor release.

An earlier version of this page stopped at the first answer and concluded the
toolbar was closed full stop. That was wrong, and the section
[What changed the answer](#what-changed-the-answer) says how.

Measured against Conductor 0.81.0 (macOS arm64, arm64 Mach-O), 2026-08-19.

## Where Conductor's UI actually lives

```
Conductor.app/Contents/
  MacOS/conductor                     66 MB Mach-O, Tauri (Rust + WebView)
  Resources/icon.icns
  Resources/bin/…                     gh, git helpers, watchexec, conductor CLI
  Resources/bin/.internal/…           conductor-runtime (Bun exe), sidecar, logger
  Resources/conductor-skill/…         a Claude plugin with one SKILL.md
```

There is no `.html`, `.js`, `.css` or `.wasm` file anywhere in the bundle:

```console
$ find /Applications/Conductor.app -type f \( -name '*.js' -o -name '*.html' -o -name '*.css' \)
$
```

Tauri compiles the frontend into the executable. So "edit the UI" means "patch a
66 MB signed binary", not "edit a file".

## The signature

```console
$ codesign -dvvv /Applications/Conductor.app
Identifier=com.conductor.app
CodeDirectory v=20500 flags=0x10000(runtime)      ← hardened runtime
Authority=Developer ID Application: <the vendor> (27XN666UJ7)
Notarization Ticket=stapled
Sealed Resources version=2 rules=13 files=14
TeamIdentifier=27XN666UJ7
```

The seal covers everything. From `Contents/_CodeSignature/CodeResources`:

```
'^.*': True
'^Resources/': {'weight': 20.0}
```

Only `Info.plist`, `PkgInfo` and `.DS_Store` are omitted. `conductor-skill/`,
the one place that looks like a plugin directory, is sealed file by file:

```
Resources/conductor-skill/.claude-plugin/plugin.json
Resources/conductor-skill/skills/conductor/SKILL.md
```

That directory is writable, because the user owns `/Applications`. Writable and
sealed are different things.

## What happens if you modify it

Copy the app, add one file, verify:

```console
$ cp -R /Applications/Conductor.app /tmp/Conductor.app
$ codesign --verify --strict /tmp/Conductor.app
$                                                     # valid

$ echo '{}' > /tmp/Conductor.app/Contents/Resources/conductor-skill/.claude-plugin/extra.json
$ codesign --verify --strict /tmp/Conductor.app
/tmp/Conductor.app: a sealed resource is missing or invalid

$ spctl -a -vvv -t exec /tmp/Conductor.app
/tmp/Conductor.app: a sealed resource is missing or invalid
```

One extra file, in the most plugin-shaped directory in the bundle, and macOS
rejects the app.

## What happens if you re-sign it

Ad-hoc re-signing makes `codesign` happy again, and Gatekeeper still says no:

```console
$ codesign -f -s - --deep --options runtime /tmp/Conductor.app
$ codesign --verify --strict /tmp/Conductor.app
$                                                     # valid again

$ spctl -a -vvv -t exec /tmp/Conductor.app
/tmp/Conductor.app: rejected

$ codesign -dvv /tmp/Conductor.app
CodeDirectory flags=0x10002(adhoc,runtime)
Signature=adhoc
TeamIdentifier=not set
```

Losing `TeamIdentifier=27XN666UJ7` costs more than Gatekeeper:

- Keychain items are access-controlled by signing identity. Conductor's own
  stored credentials stop being readable by the re-signed binary, so a re-signed
  copy of your install signs you out of Conductor.
- Notarization is void, so the app is untrusted on any other machine.
- Conductor auto-updates, roughly weekly. Every update replaces the binary and
  undoes the patch.

**This is the reason the panel goes into a copy and never into your install.**
`hats dev-app` gives the copy its own
bundle identifier, database and keychain items, so none of the above touches the
Conductor you work in. `hats patch` refuses `/Applications/Conductor.app`
unless passed `--i-know`.

## What about injecting a library instead

Blocked by the entitlements. The full set is three keys:

```
com.apple.security.cs.allow-jit
com.apple.security.cs.allow-unsigned-executable-memory
com.apple.security.device.audio-input
```

Absent, and both required:

- `com.apple.security.cs.allow-dyld-environment-variables`, without which the
  hardened runtime ignores `DYLD_INSERT_LIBRARIES` entirely.
- `com.apple.security.cs.disable-library-validation`, without which any loaded
  library must be signed by team `27XN666UJ7`.

So there is no way *into a running Conductor from another process*. Patching the
bundle before it launches is a different mechanism, and the one that works.

## Does writing the extension in Rust or Tauri change any of this

No, and this is still true. The host application's language is not an access
control boundary. A Tauri app you write is a separate process with its own
window, exactly like a Swift or Electron app you write. Tauri has no third-party
plugin API that lets one app render into another app's webview, and nothing about
matching Conductor's stack gets you past the code signature, the hardened runtime
or the invoke key.

Matching the host's stack buys nothing. Patching the host's assets buys
everything. Those are unrelated facts, and conflating them is what made the
earlier version of this page wrong.

## What changed the answer

The first pass concluded "no `.js` files in the bundle, therefore the frontend is
out of reach". The first half is true and the second does not follow. Looking
properly:

```
2878 assets, 36 MB of frontend, extracted from __DATA_CONST
assets/renderApp-CIIBeY95.js   8.6 MB   <- the New Workspace modal and the toolbar
```

Tauri stores the frontend as an asset map of 32-byte entries,
`(key_ptr, key_len, value_ptr, value_len)`, with plaintext keys and brotli
values. `hats assets` walks it.

Writing back is viable because Conductor ships that bundle compressed below
brotli's maximum:

```
original compressed   2,148,637 bytes
recompressed at q11   1,939,578 bytes   ->  ~209 KB of headroom
```

So a modified bundle fits where the original was. Only `value_len` changes in the
map, so no pointer is relocated and no segment is resized. Then ad-hoc re-sign,
which the copy already proves works.

Two further pieces made a panel rather than a demo:

- `execute_shell_command` is one of 48 Tauri commands the webview can invoke, and
  it is reachable through `window.__TAURI_INTERNALS__.invoke` without the key. So
  injected UI can call `hats` directly, and the panel needs no state of
  its own.
- Conductor flushes `localStorage` to `local-storage.subsystem.*.json` under
  Application Support, so injected code has a persistence path if it ever needs
  one. This project does not: `hats` owns all state.

## What is open, and to what

| What you want to change | Third party, unmodified install | Injected into a re-signed copy |
|---|---|---|
| Chat messages and cards, via `mcp__conductor__AskUserQuestion` | Yes | Yes |
| Slash commands, via `~/.claude/commands` | Yes | Yes |
| Agent process environment, via `environment_variables` | Yes | Yes |
| Agent executable, via `claude_code_executable_path` | Yes | Yes |
| Workspace setup, via `scripts.setup` | Yes | Yes |
| Toolbar, New Workspace modal, any part of the window | **No** | **Yes** |
| A running Conductor's process, via dyld or the invoke key | No | No |

This project uses every row. `/account` covers the first group and survives
updates; the injected panel covers the toolbar row and does not.

## Asking for the real thing

A per-workspace account selector belongs in Conductor. The way to get it is
Help, then Send Feedback, asking for per-workspace agent account selection. That
is the only version that needs no copy, no re-signing and no re-patching after
each release. Everything in this repository becomes unnecessary the day they
ship it, which is the correct outcome.
