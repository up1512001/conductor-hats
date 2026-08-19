#!/bin/sh
# conductor-hats installer.
#
#   ./install.sh                 from a checkout
#   curl -fsSL <raw-url>/install.sh | sh
#
# Clones to ~/.conductor-accounts/src when run outside a checkout, wires
# Conductor up, and stops. Signing in needs a browser, so it is left to you.
set -eu

REPO_URL="${CONDUCTOR_ACCT_REPO:-https://github.com/up1512001/conductor-hats}"
ROOT="${CONDUCTOR_ACCOUNTS_ROOT:-$HOME/.conductor-accounts}"
SRC="$ROOT/src"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "Conductor is macOS only"


# Running from a checkout, or piped from curl?
SELF_DIR=""
if [ -n "${0##*/}" ] && [ -f "$0" ]; then
    SELF_DIR=$(cd "$(dirname "$0")" && pwd)
fi

if [ -n "$SELF_DIR" ] && [ -x "$SELF_DIR/bin/conductor-acct" ]; then
    PROJECT="$SELF_DIR"
    say "Installing from $PROJECT"
else
    command -v git >/dev/null || die "git not found"
    mkdir -p "$ROOT"
    if [ -d "$SRC/.git" ]; then
        say "Updating $SRC"
        git -C "$SRC" pull --ff-only
    else
        say "Cloning into $SRC"
        git clone --depth 1 "$REPO_URL" "$SRC"
    fi
    PROJECT="$SRC"
fi

[ -x "$PROJECT/bin/conductor-acct" ] || chmod +x "$PROJECT/bin/conductor-acct" \
    "$PROJECT/bin/claude-router" "$PROJECT/bin/codex-router"

"$PROJECT/bin/conductor-acct" install

say ""
say "Next, in this terminal, sign in to each account:"
say ""
say "  $ROOT/bin/conductor-acct add personal"
say "  $ROOT/bin/conductor-acct add work"
say ""
say "Then restart Conductor, open a workspace and run /account in the chat."
say ""
say "Put conductor-acct on your PATH if you want it there:"
say "  ln -sf $ROOT/bin/conductor-acct /usr/local/bin/conductor-acct"
