#!/bin/sh
# Installs conductor-hats.
#
#   ./install.sh                       from an extracted release, or a checkout
#   curl -fsSL <raw-url>/install.sh | sh    fetches the latest release first
#
# Downloads are verified against the .sha256 published beside them. Nothing is
# installed outside $HOME.
#
# Signing in needs a browser, so it is left to you afterwards.
set -eu

REPO="${CONDUCTOR_HATS_REPO:-up1512001/conductor-hats}"
ROOT="${CONDUCTOR_ACCOUNTS_ROOT:-$HOME/.conductor-accounts}"
SRC="$ROOT/src"
BINDIR="${CONDUCTOR_HATS_BINDIR:-$HOME/.local/bin}"
VERSION="${CONDUCTOR_HATS_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "Conductor is macOS only"

case "$(uname -m)" in
    arm64) TARGET=aarch64-apple-darwin ;;
    x86_64) TARGET=x86_64-apple-darwin ;;
    *) die "unsupported architecture: $(uname -m)" ;;
esac

SELF_DIR=""
if [ -f "$0" ]; then
    SELF_DIR=$(cd "$(dirname "$0")" && pwd)
fi

fetch_release() {
    command -v curl >/dev/null || die "curl not found"
    mkdir -p "$SRC"
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT

    if [ "$VERSION" = latest ]; then
        BASE="https://github.com/$REPO/releases/latest/download"
    else
        BASE="https://github.com/$REPO/releases/download/$VERSION"
    fi
    TARBALL="hats-$TARGET.tar.gz"

    say "Downloading $TARBALL"
    curl -fsSL "$BASE/$TARBALL" -o "$TMP/$TARBALL" ||
        die "could not download $BASE/$TARBALL
A private repository needs an authenticated download. Fetch the tarball
yourself, extract it and run ./install.sh from inside it."
    curl -fsSL "$BASE/$TARBALL.sha256" -o "$TMP/$TARBALL.sha256" ||
        die "could not download the checksum for $TARBALL"

    say "Verifying"
    ( cd "$TMP" && shasum -a 256 -c "$TARBALL.sha256" >/dev/null ) ||
        die "checksum mismatch: refusing to install $TARBALL"

    tar xzf "$TMP/$TARBALL" -C "$TMP"
    rm -rf "$SRC"
    mkdir -p "$(dirname "$SRC")"
    mv "$TMP/hats-$TARGET" "$SRC"
    PROJECT="$SRC"
}

if [ -n "$SELF_DIR" ] && [ -x "$SELF_DIR/bin/conductor-acct" ]; then
    PROJECT="$SELF_DIR"
    say "Installing from $PROJECT"
else
    fetch_release
fi

chmod +x "$PROJECT/bin/conductor-acct" "$PROJECT/bin/claude-router" \
    "$PROJECT/bin/codex-router" 2>/dev/null || true

"$PROJECT/bin/conductor-acct" install

if [ -x "$PROJECT/hats" ]; then
    mkdir -p "$BINDIR"
    cp "$PROJECT/hats" "$BINDIR/hats"
    chmod +x "$BINDIR/hats"
    say ""
    say "hats -> $BINDIR/hats"
    case ":$PATH:" in
        *":$BINDIR:"*) ;;
        *) say "  $BINDIR is not on your PATH. Add it:"
           say "    echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.zshrc" ;;
    esac
fi

ln -sf "$ROOT/bin/conductor-acct" "$BINDIR/conductor-acct" 2>/dev/null || true

say ""
say "Next, sign in to each account. A browser opens, so this needs a terminal:"
say ""
say "  conductor-acct add personal"
say "  conductor-acct add work"
say ""
say "Then pick an account per workspace, from Conductor's toolbar once patched,"
say "with /account in any chat, or from here:"
say ""
say "  conductor-acct use work"
say "  conductor-acct status"
say ""
if [ -x "$PROJECT/hats" ]; then
    say "For the account panel inside Conductor, on a copy of the app:"
    say ""
    say "  hats dev-app     # an isolated copy, safe to modify"
    say "  hats patch       # inject the panel into it"
    say ""
fi
say "Undo everything:  conductor-acct uninstall"
