#!/bin/bash
# Rebuild the personalized Conductor after a Conductor update, in one command.
#
#   tools/repersonalize.sh                 rebuild, patch, redeploy, relaunch
#   tools/repersonalize.sh --no-launch     do everything except relaunch
#   tools/repersonalize.sh --keep-app      patch the existing copy, do not rebuild
#
# Conductor auto-updates roughly weekly and ships a new frontend bundle each
# time, which drops the injected account panel. This script is the whole recovery
# path. It is deliberately one file, because the order matters and two of the
# steps are easy to get wrong from memory.
#
# What it does, in order:
#
#   1. quits the copy, so nothing is patched while it is running
#   2. drops the stale UI backup  <- the step that is easy to miss, see below
#   3. rebuilds "Conductor Dev.app" from the current real Conductor
#   4. injects the account panel and re-signs
#   5. redeploys the CLI to ~/.conductor-accounts/bin so the router matches
#   6. relaunches with a scrubbed environment  <- the other easy one
#
# Step 2, the trap: tools/patch-ui.py keeps a pristine backup of the binary and
# always patches from it, so patching twice is not a stack. That backup is keyed
# by app name, not by version, so after an update it holds the PREVIOUS
# Conductor's binary. Patching a freshly rebuilt copy against it would silently
# reinstate the old version. The backup has to go when the copy is rebuilt.
#
# Step 6, the other trap: launching with `open` from inside a routed agent
# session leaks CONDUCTOR_ACCOUNTS_ROUTING into the app, which hands it to every
# agent it spawns, and the router's loop guard then refuses them with exit code
# 70. The environment is scrubbed before launching.
set -euo pipefail

TOOLS=$(cd "$(dirname "$0")" && pwd)
PROJECT=$(dirname "$TOOLS")
APP="${CONDUCTOR_DEV_APP:-/Applications/Conductor Dev.app}"
BACKUPS="$HOME/.conductor-accounts/ui-patch-backups"

LAUNCH=1
REBUILD=1
for arg in "$@"; do
    case "$arg" in
        --no-launch) LAUNCH=0 ;;
        --keep-app) REBUILD=0 ;;
        -h|--help) sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "repersonalize: unknown option '$arg'" >&2; exit 1 ;;
    esac
done

step() { printf '\n==> %s\n' "$*"; }

step "Quitting $(basename "$APP")"
ID=$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$APP/Contents/Info.plist" \
    2>/dev/null || echo com.conductor.dev)
osascript -e "quit app id \"$ID\"" 2>/dev/null || true
for _ in 1 2 3 4 5 6 7 8 9 10; do
    pgrep -f "$APP/Contents/MacOS/" >/dev/null 2>&1 || break
    sleep 1
done
if pgrep -f "$APP/Contents/MacOS/" >/dev/null 2>&1; then
    echo "    still running, asking harder"
    pkill -f "$APP/Contents/MacOS/" || true
    sleep 2
fi
echo "    quit"

if [ "$REBUILD" -eq 1 ]; then
    # See the header: a backup from the previous version would be restored over
    # the new bundle on the next patch.
    step "Dropping the stale UI backup"
    rm -f "$BACKUPS/$(basename "$APP" | tr ' ' '-').conductor.orig"
    echo "    gone, the next patch takes a fresh one"

    step "Rebuilding the copy from the current Conductor"
    "$TOOLS/make-dev-conductor.sh" --force
else
    step "Keeping the existing copy"
fi

step "Injecting the account panel"
"$TOOLS/patch-ui.py" --app "$APP"

step "Redeploying the CLI"
"$PROJECT/bin/conductor-acct" install

step "Checking the setup"
"$PROJECT/bin/conductor-acct" doctor || true

if [ "$LAUNCH" -eq 1 ]; then
    step "Relaunching with a scrubbed environment"
    env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH \
        -u CONDUCTOR_WORKSPACE_PATH -u CONDUCTOR_ROOT_PATH \
        -u CONDUCTOR_ACCOUNT -u CLAUDE_CONFIG_DIR -u CODEX_HOME \
        open -a "$APP"
    echo "    launched"
else
    step "Not relaunching"
    echo "    do it yourself with a scrubbed environment, or agents will get exit code 70:"
    echo "    env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH open -a '$APP'"
fi

cat <<EOF

Done. Look for:
  the account button next to "Open in", top right of a workspace
  the account chip in the New Workspace composer footer

If the panel is missing, the anchors moved in this Conductor release. The button
falls back to floating at the top right when it cannot find the toolbar, so
nothing there at all means the script did not run: check
tools/ui-patch/account-ui.js against the new bundle.

Undo just the UI:   tools/patch-ui.py --revert --app "$APP"
Undo everything:    rm -rf "$APP"
EOF
