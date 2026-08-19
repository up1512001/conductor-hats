#!/bin/bash
# Build an isolated, modifiable copy of Conductor to experiment on.
#
#   tools/make-dev-conductor.sh [--force]
#
# The copy gets a different bundle identifier, so it uses its own database,
# its own agent binaries and its own keychain items. Your real Conductor is
# never touched, and deleting the copy undoes everything.
#
# Why the identifier has to change: `com.conductor.app` is compiled into both
# binaries, not just Info.plist, and it is what builds the
# ~/Library/Application Support path. Two apps sharing that path would share
# one SQLite database. `com.conductor.dev` is the same 17 bytes, so it is a
# straight byte-for-byte substitution with no offsets to repair.
#
# What you lose in the copy: Developer ID and notarization. Gatekeeper will
# refuse to launch it from quarantine, so the script clears the quarantine
# attribute. The entitlements are copied across, without which the webview
# cannot JIT and the app dies on launch.
set -euo pipefail

SRC="${CONDUCTOR_APP:-/Applications/Conductor.app}"
DST="${CONDUCTOR_DEV_APP:-/Applications/Conductor Dev.app}"
OLD_ID="com.conductor.app"
NEW_ID="${CONDUCTOR_DEV_ID:-com.conductor.dev}"

die() { echo "make-dev-conductor: $*" >&2; exit 1; }
step() { printf '\n==> %s\n' "$*"; }

[ ${#OLD_ID} -eq ${#NEW_ID} ] || die "identifiers must be the same length"
[ -d "$SRC" ] || die "not found: $SRC"

if [ -e "$DST" ]; then
    [ "${1:-}" = "--force" ] || die "$DST exists (pass --force to rebuild)"
    step "Removing the previous copy"
    rm -rf "$DST"
fi

step "Copying $SRC"
cp -R "$SRC" "$DST"

step "Saving the original entitlements"
# Without allow-jit and allow-unsigned-executable-memory the WebView cannot run
# and the app crashes immediately after launch.
ENT=$(mktemp -t conductor-ent).plist
codesign -d --entitlements - --xml "$SRC" 2>/dev/null > "$ENT" ||
    die "could not read the entitlements from $SRC"
grep -q allow-jit "$ENT" || die "entitlements look wrong, refusing to continue"

step "Rewriting Info.plist"
PB=/usr/libexec/PlistBuddy
"$PB" -c "Set :CFBundleIdentifier $NEW_ID" "$DST/Contents/Info.plist"
"$PB" -c "Set :CFBundleName Conductor Dev" "$DST/Contents/Info.plist"
"$PB" -c "Set :CFBundleDisplayName Conductor Dev" "$DST/Contents/Info.plist" 2>/dev/null || true

step "Patching the identifier inside the binaries"
# Targeted, not a blind search and replace. The identifier appears four times in
# the Tauri binary and only one of them is the Tauri config string that builds
# the Application Support path. Two of the others are inside the code signature,
# which codesign rewrites anyway, and one sits in an encoded string table where
# editing raw bytes corrupts what comes back out: a blind replace of "app" with
# "dev" there produced a data directory called com.conductor.dep.
python3 - "$DST" "$OLD_ID" "$NEW_ID" <<'PY'
import sys, pathlib, re
root, old, new = sys.argv[1], sys.argv[2].encode(), sys.argv[3].encode()
assert len(old) == len(new)

def patch(rel, pick, why):
    p = pathlib.Path(root) / rel
    if not p.is_file():
        print(f"    skip    {rel} (absent)")
        return 0
    data = bytearray(p.read_bytes())
    done = 0
    for m in list(re.finditer(re.escape(old), bytes(data))):
        s = m.start()
        if not pick(bytes(data), s):
            continue
        data[s:s + len(old)] = new
        done += 1
    if done:
        p.write_bytes(bytes(data))
    print(f"    {rel}: {done} patched  ({why})")
    return done

# The Tauri config blob stores the identifier immediately before the dev-server
# URL. That adjacency is what makes this occurrence identifiable.
n = patch("Contents/MacOS/conductor",
          lambda d, s: d[s + len(old):s + len(old) + 21] == b"http://localhost:1420",
          "tauri config identifier")
if n != 1:
    sys.exit(f"expected exactly 1 tauri config identifier, found {n}")

# The keychain service name is assembled from a separate, length-prefixed entry
# in an encoded string table: 0x12 "com.conductor.app." <backref> ".settings",
# which resolves to com.conductor.app.production.settings. Miss this one and the
# copy prompts for the real Conductor's keychain password on first launch.
# Substituting in place keeps the prefix at 18 bytes, so the length byte holds.
n = patch("Contents/MacOS/conductor",
          lambda d, s: d[s - 1] == 0x12 and d[s + len(old):s + len(old) + 1] == b".",
          "keychain service prefix")
if n != 1:
    sys.exit(f"expected exactly 1 keychain service prefix, found {n}")

# In the runtime this is a JavaScript comparison against __CFBundleIdentifier,
# a plain source string, so the copy has to agree or the check silently fails.
patch("Contents/Resources/bin/.internal/conductor-runtime",
      lambda d, s: b'__CFBundleIdentifier === "' in d[max(0, s - 40):s],
      "runtime bundle check")
PY

step "Re-signing ad-hoc"
# Inner Mach-O files first, then the bundle, so the outer seal covers the final
# bytes of everything it contains.
#
# Each inner binary is re-signed with its OWN original entitlements, read back
# from the pristine app. Signing them bare looks harmless and is not:
# conductor-runtime is a Bun executable that JIT-compiles JavaScript, so without
# com.apple.security.cs.allow-jit it starts, fails the moment it needs to
# compile, and Conductor reports "Sidecar terminated unexpectedly, code 1".
find "$DST/Contents/Resources/bin" -type f -perm -u+x -print0 2>/dev/null |
    while IFS= read -r -d '' f; do
        file "$f" | grep -q 'Mach-O' || continue
        rel=${f#"$DST/"}
        inner_ent=$(mktemp -t conductor-inner-ent).plist
        if codesign -d --entitlements - --xml "$SRC/$rel" 2>/dev/null > "$inner_ent" &&
           [ -s "$inner_ent" ]; then
            codesign -f -s - --options runtime --entitlements "$inner_ent" "$f" 2>/dev/null ||
                codesign -f -s - --options runtime "$f" 2>/dev/null || true
        else
            codesign -f -s - --options runtime "$f" 2>/dev/null || true
        fi
        rm -f "$inner_ent"
    done
codesign -f -s - --options runtime --entitlements "$ENT" "$DST"
rm -f "$ENT"

step "Clearing this copy's stale keychain items"
# An ad-hoc signature carries no stable identity, so every rebuild looks like a
# different application to the keychain. Items the previous build created are
# still there, ACL'd to a code hash that no longer exists, and macOS asks for
# the login password to hand them over. Dropping them means the copy starts
# clean and is never in a position to ask. Only ever this copy's own service
# name; the real Conductor's items are left alone.
KC_SERVICE="$NEW_ID.production.settings"
kc_removed=0
while security delete-generic-password -s "$KC_SERVICE" >/dev/null 2>&1; do
    kc_removed=$((kc_removed + 1))
    [ "$kc_removed" -gt 20 ] && break
done
echo "    removed $kc_removed stale item(s) for $KC_SERVICE"
echo "    left alone: com.conductor.app.production.settings"

step "Clearing quarantine"
xattr -cr "$DST" 2>/dev/null || true

step "Verifying"
codesign --verify --strict "$DST" && echo "    signature: valid (ad-hoc)"
echo "    identifier: $(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$DST/Contents/Info.plist")"
echo "    data dir:   ~/Library/Application Support/$NEW_ID"

cat <<EOF

Done. "$DST"

It starts empty: its own database, its own login, its own agent binaries. Your
real Conductor is untouched and both can run at the same time.

  open "$DST"

Note that ~/.conductor/settings.toml is user-scoped rather than bundle-scoped,
so both apps read the same settings file, including the account router.

To remove:  rm -rf "$DST"
EOF
