#!/bin/bash
# Sets the version everywhere it appears, in one place.
#
#   tools/set-version.sh 0.3.0     set it
#   tools/set-version.sh --check   print each file's version, fail if they differ
#
# The version lives in five files. They ship together, so a skew between them is a
# bug, and a test asserts they match. This is what keeps that true without anyone
# having to remember all five.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

die() { echo "set-version: $*" >&2; exit 1; }

read_cargo() { sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1; }
read_package() { sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1; }
read_cli() { sed -n 's/^CONDUCTOR_ACCT_VERSION=\(.*\)$/\1/p' bin/conductor-acct | head -1; }
read_panel() { sed -n 's/^const VERSION = "\(.*\)";$/\1/p' src/panel/index.ts | head -1; }
read_changelog() { sed -n 's/^## \([0-9][0-9.]*\).*$/\1/p' CHANGELOG.md | head -1; }

report() {
    printf '%-16s %s\n' "Cargo.toml" "$(read_cargo)"
    printf '%-16s %s\n' "package.json" "$(read_package)"
    printf '%-16s %s\n' "bin/conductor-acct" "$(read_cli)"
    printf '%-16s %s\n' "src/panel/index.ts" "$(read_panel)"
    printf '%-16s %s\n' "CHANGELOG.md" "$(read_changelog)"
}

check() {
    local versions
    versions=$(report | awk '{print $NF}' | sort -u | wc -l | tr -d ' ')
    report
    [ "$versions" -eq 1 ] || die "versions disagree"
    echo "all agree on $(read_cargo)"
}

if [ "${1:-}" = "--check" ]; then
    check
    exit 0
fi

NEW="${1:-}"
[ -n "$NEW" ] || die "usage: tools/set-version.sh <version> | --check"
case "$NEW" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) die "expected a semantic version like 0.3.0, got '$NEW'" ;;
esac

OLD=$(read_cargo)
if [ "$(report | awk '{print $NF}' | sort -u | tr '\n' ' ')" = "$NEW " ]; then
    die "every file already says $NEW"
fi

tmp=$(mktemp)
awk -v v="$NEW" 'NR==1,/^version = / { sub(/^version = ".*"$/, "version = \"" v "\"") } 1' \
    Cargo.toml > "$tmp" && mv "$tmp" Cargo.toml

tmp=$(mktemp)
awk -v v="$NEW" '{ sub(/^  "version": ".*",$/, "  \"version\": \"" v "\",") } 1' \
    package.json > "$tmp" && mv "$tmp" package.json

tmp=$(mktemp)
awk -v v="$NEW" '{ sub(/^CONDUCTOR_ACCT_VERSION=.*$/, "CONDUCTOR_ACCT_VERSION=" v) } 1' \
    bin/conductor-acct > "$tmp" && mv "$tmp" bin/conductor-acct
chmod +x bin/conductor-acct

tmp=$(mktemp)
awk -v v="$NEW" '{ sub(/^const VERSION = ".*";$/, "const VERSION = \"" v "\";") } 1' \
    src/panel/index.ts > "$tmp" && mv "$tmp" src/panel/index.ts

if grep -q '^## Unreleased' CHANGELOG.md; then
    tmp=$(mktemp)
    awk -v v="$NEW" '{ sub(/^## Unreleased$/, "## " v) } 1' CHANGELOG.md > "$tmp"
    mv "$tmp" CHANGELOG.md
    echo "CHANGELOG: '## Unreleased' is now '## $NEW'"
else
    echo "CHANGELOG: no '## Unreleased' heading, add one for the next release" >&2
fi

# Cargo.lock records the version of this crate too, so it goes stale otherwise.
if command -v cargo >/dev/null; then
    cargo update --workspace --quiet 2>/dev/null || true
fi

echo
echo "$OLD -> $NEW"
check
