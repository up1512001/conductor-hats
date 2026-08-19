#!/usr/bin/env python3
"""Inject the account UI into a Conductor binary's embedded frontend.

    tools/patch-ui.py                       patch "Conductor Dev.app"
    tools/patch-ui.py --app /path/to.app
    tools/patch-ui.py --revert

Refuses to touch /Applications/Conductor.app unless --i-know is passed: patching
your working Conductor is a bad trade, and tools/make-dev-conductor.sh exists to
give you a copy that is free to break.

How it works, and why it holds together:

Tauri stores the frontend in the executable as an asset map in __DATA_CONST,
32-byte entries of (key_ptr, key_len, value_ptr, value_len), keys plaintext and
values brotli. To change a file we decompress its value, append our script,
recompress and write it back.

The catch is that the value has to fit where it already is, because moving it
would mean relocating a pointer into a segment with no spare room. It does fit:
Conductor's bundle was compressed at a lower quality than brotli's maximum, so
recompressing at quality 11 wins back about 200 KB, far more than this script
costs. The patcher checks anyway and refuses rather than corrupt the binary.

Only value_len changes in the map. value_ptr, every other entry, and every
offset in the file stay exactly where they were.
"""

import argparse
import pathlib
import shutil
import struct
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from importlib import import_module

extract = import_module("extract-assets")

DEV_APP = "/Applications/Conductor Dev.app"
REAL_APP = "/Applications/Conductor.app"
MARKER = b"__conductorMultiAccount"
TARGET_HINT = "renderApp"


def brotli_compress(data, quality=11):
    proc = subprocess.run(
        ["node", "-e",
         "const z=require('zlib'),c=[];process.stdin.on('data',d=>c.push(d)).on('end',()=>{"
         "const b=Buffer.concat(c);process.stdout.write(z.brotliCompressSync(b,{params:{"
         "[z.constants.BROTLI_PARAM_QUALITY]:%d,[z.constants.BROTLI_PARAM_SIZE_HINT]:b.length}}))})"
         % quality],
        input=data, capture_output=True,
    )
    if proc.returncode != 0:
        sys.exit("brotli compression failed: " + proc.stderr.decode()[:300])
    return proc.stdout


def pick_bundle(assets):
    """The main application chunk, the one holding the toolbar and the composer."""
    candidates = [a for a in assets if TARGET_HINT in a[0] and a[0].endswith(".js")]
    if not candidates:
        candidates = [a for a in assets if a[0].endswith(".js")]
    if not candidates:
        sys.exit("no JavaScript assets found")
    return max(candidates, key=lambda a: a[2])


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--app", default=DEV_APP)
    ap.add_argument("--script",
                    default=str(pathlib.Path(__file__).parent.parent / "dist" / "account-ui.js"),
                    help="the built panel; run 'pnpm build' if it is missing")
    ap.add_argument("--revert", action="store_true", help="restore the backup taken on first patch")
    ap.add_argument("--i-know", action="store_true", help="allow patching the real Conductor")
    args = ap.parse_args()

    app = pathlib.Path(args.app)
    binary = app / "Contents" / "MacOS" / "conductor"
    # Outside the bundle: every file under Contents/ is covered by the code
    # signature seal, so a backup left in MacOS/ is enough to invalidate it.
    backup_dir = pathlib.Path.home() / ".conductor-accounts" / "ui-patch-backups"
    backup = backup_dir / (app.name.replace(" ", "-") + ".conductor.orig")

    if not binary.is_file():
        sys.exit(f"no Conductor binary at {binary}")
    if str(app.resolve()) == str(pathlib.Path(REAL_APP).resolve()) and not args.i_know:
        sys.exit("refusing to patch your real Conductor.\n"
                 "Build a copy first:  tools/make-dev-conductor.sh\n"
                 "Then:                tools/patch-ui.py\n"
                 "Override with --i-know if you really mean it.")

    if args.revert:
        if not backup.is_file():
            sys.exit(f"no backup at {backup}")
        shutil.copy2(backup, binary)
        print(f"restored {binary} from {backup.name}")
        resign(app)
        return

    if not backup.is_file():
        backup_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, backup)
        print(f"backup   {backup}")

    # Always patch from the pristine copy, so patching twice is not a stack.
    shutil.copy2(backup, binary)

    script_path = pathlib.Path(args.script)
    if not script_path.is_file():
        sys.exit(f"no built panel at {script_path}\n"
                 "dist/ is generated, not committed. Build it first:\n"
                 "  pnpm install && pnpm build")
    script = script_path.read_bytes()
    macho = extract.MachO(binary)
    assets = extract.find_assets(macho)
    key, off, length, entry = pick_bundle(assets)
    print(f"target   {key}")

    original = macho.data[off : off + length]
    plain = extract.brotli_decompress(original)
    if MARKER in plain:
        sys.exit("this binary already contains the account UI")
    print(f"         {length:,} compressed -> {len(plain):,} bytes")

    merged = plain + b"\n;" + script
    packed = brotli_compress(merged)
    print(f"         + {len(script):,} bytes of UI -> {len(packed):,} compressed")

    if len(packed) > length:
        sys.exit(f"the patched bundle does not fit: {len(packed):,} > {length:,} available.\n"
                 "Relocating the asset is not implemented; trim the injected script.")

    data = bytearray(macho.data)
    data[off : off + len(packed)] = packed
    # Brotli stops at its own end-of-stream marker, so the slack after it is
    # never read. Zero it so the tail of the old bundle is not left lying around.
    data[off + len(packed) : off + length] = b"\0" * (length - len(packed))
    # The only pointer that moves is the length, 24 bytes into the map entry.
    struct.pack_into("<Q", data, entry + 24, len(packed))
    binary.write_bytes(bytes(data))
    print(f"         {length - len(packed):,} bytes of headroom left over")

    resign(app)
    print(f"\nPatched. Launch it:\n  open '{app}'\n\nUndo:\n  tools/patch-ui.py --revert")


def drop_stale_keychain_items(app):
    """Re-signing changes the code hash, which orphans the app's keychain items.

    macOS then asks for the login password to release them. Removing this app's
    own items means it starts clean instead of prompting. Scoped to the copy's
    identifier so the real Conductor's credentials are never in scope.
    """
    plist = app / "Contents" / "Info.plist"
    ident = subprocess.run(
        ["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleIdentifier", str(plist)],
        capture_output=True, text=True,
    ).stdout.strip()
    if not ident or ident == "com.conductor.app":
        return
    service = f"{ident}.production.settings"
    removed = 0
    while removed < 20:
        r = subprocess.run(["security", "delete-generic-password", "-s", service],
                           capture_output=True)
        if r.returncode != 0:
            break
        removed += 1
    if removed:
        print(f"keychain  cleared {removed} stale item(s) for {service}")


def resign(app):
    ent = subprocess.run(["codesign", "-d", "--entitlements", "-", "--xml", str(app)],
                         capture_output=True)
    entitlements = None
    if ent.returncode == 0 and b"allow-jit" in ent.stdout:
        entitlements = pathlib.Path("/tmp/conductor-ui-patch-ent.plist")
        entitlements.write_bytes(ent.stdout)
    cmd = ["codesign", "-f", "-s", "-", "--options", "runtime"]
    if entitlements:
        cmd += ["--entitlements", str(entitlements)]
    cmd.append(str(app))
    proc = subprocess.run(cmd, capture_output=True)
    if proc.returncode != 0:
        sys.exit("re-signing failed: " + proc.stderr.decode()[:300])
    subprocess.run(["xattr", "-cr", str(app)], capture_output=True)
    drop_stale_keychain_items(app)
    verify = subprocess.run(["codesign", "--verify", "--strict", str(app)], capture_output=True)
    print("signature", "valid" if verify.returncode == 0 else "INVALID")


if __name__ == "__main__":
    main()
