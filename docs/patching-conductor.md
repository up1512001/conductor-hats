# Can an extension add UI to Conductor?

Short answer: it can add UI to the **chat**, and nothing else. The toolbar, the
New Workspace modal and every other part of Conductor's window are closed to
outside code. This page records the evidence, because it is the first question
everyone asks.

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
Authority=Developer ID Application: Charlie Holtz (27XN666UJ7)
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
  stored credentials stop being readable by the re-signed binary, so you get
  signed out of Conductor itself.
- Notarization is void, so the app is untrusted on any other machine.
- Conductor auto-updates, roughly weekly. Every update replaces the binary and
  undoes the patch.

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

Conductor's Tauri commands (`sync_agent_path_overrides`, `open_settings_page`
and the rest) are also gated by `__TAURI_INVOKE_KEY__` and reachable only from
its own webview, so there is nothing to call from outside even if you got code
running.

## Does writing the extension in Rust or Tauri change any of this

No. The host application's language is not an access control boundary. A Tauri
app you write is a separate process with its own window, exactly like a Swift or
Electron app you write. Tauri has no third-party plugin API that lets one app
render into another app's webview, and nothing about matching Conductor's stack
gets you past the code signature, the hardened runtime or the invoke key.

The only thing that would change this is Conductor shipping an extension point.

## What is actually open

| Surface | Open to an extension? |
|---|---|
| Chat messages and cards, via `mcp__conductor__AskUserQuestion` | Yes |
| Slash commands, via `~/.claude/commands` | Yes |
| Agent process environment, via `environment_variables` | Yes |
| Agent executable, via `claude_code_executable_path` | Yes |
| Workspace setup, via `scripts.setup` | Yes |
| Toolbar, title bar, New Workspace modal, Settings pane | No |

This project uses every row in the first group. `/account` draws its picker as a
native card in the Conductor conversation, which is the closest thing to in-app
UI that exists for third-party code.

## Asking for the real thing

A per-workspace account selector belongs in Conductor. The way to get it is
Help, then Send Feedback, asking for per-workspace agent account selection.
Everything in this repository becomes unnecessary the day they ship it, which
is the correct outcome.
