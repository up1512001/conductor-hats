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

# Private repositories serve nothing to an anonymous curl, so fall back to the
# GitHub CLI when it is present and signed in. Both assets are fetched at once,
# because gh matches by pattern and the checksum shares the tarball's name.
gh_download() {
    dir="$1"
    want="$2"
    command -v gh >/dev/null || return 1
    if [ "$VERSION" = latest ]; then
        tag=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null) || return 1
    else
        tag="$VERSION"
    fi
    gh release download "$tag" --repo "$REPO" --pattern "hats-$TARGET.tar.gz*" \
        --dir "$dir" --clobber >/dev/null 2>&1 || return 1
    [ -f "$dir/$want" ]
}

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
    if ! curl -fsSL "$BASE/$TARBALL" -o "$TMP/$TARBALL" 2>/dev/null; then
        gh_download "$TMP" "$TARBALL" ||
            die "could not download $TARBALL
Public releases download with curl. A private one needs the GitHub CLI, signed
in as someone who can read $REPO:

  gh auth login

Or fetch the tarball yourself, extract it, and run ./install.sh from inside."
    fi
    if [ ! -f "$TMP/$TARBALL.sha256" ]; then
        curl -fsSL "$BASE/$TARBALL.sha256" -o "$TMP/$TARBALL.sha256" 2>/dev/null ||
            gh_download "$TMP" "$TARBALL.sha256" ||
            die "could not download the checksum for $TARBALL"
    fi

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

[ -x "$PROJECT/hats" ] || chmod +x "$PROJECT/hats" 2>/dev/null || true

"$PROJECT/hats" install

mkdir -p "$BINDIR"
cp "$PROJECT/hats" "$BINDIR/hats"
chmod +x "$BINDIR/hats"
ln -sf "$BINDIR/hats" "$BINDIR/conductor-acct"
say ""
say "hats -> $BINDIR/hats"
case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *) say "  $BINDIR is not on your PATH. Add it:"
       say "    echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.zshrc" ;;
esac

say ""
say "Next, add your accounts. The panel signs in without a terminal:"
say ""
say "  hats dev-app     # an isolated copy of Conductor, safe to modify"
say "  hats patch       # inject the account panel into it"
say ""
say "Then open it and use Add new account. From a terminal instead:"
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
say "Undo everything:  conductor-acct uninstall"
