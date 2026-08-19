#!/bin/bash
# Per-workspace routes: use, assign, unassign.
#
# Sourced by bin/conductor-acct. Not executable on its own.

cmd_use() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct use <profile> [claude|codex] [workspace-path]"
    require_profile "$agent" "$profile"
    ensure_root

    local ws
    ws=$(current_workspace "${3:-}")
    write_route "$ws" "$profile"

    local label
    label=$(label_of "$agent" "$profile")
    echo "$(basename "$ws") now uses '$profile'${label:+ ($label)}"
    echo "  $ws"

    if ! router_installed; then
        echo
        echo "The router is off, so this route is recorded but not applied."
        echo "Turn it on with: conductor-acct install   (then restart Conductor)"
        return 0
    fi

    echo
    echo "Open a new chat in this workspace for it to take effect. A chat that is"
    echo "already running keeps the account its agent process started on."
}

write_route() {
    local key="$1" profile="$2" tmp
    tmp=$(mktemp)
    grep -v "^$(printf '%s' "$key" | sed 's/[[\.*^$/]/\\&/g')[[:space:]]" "$ROUTES_FILE" > "$tmp" 2>/dev/null || true
    printf '%s\t%s\n' "$key" "$profile" >> "$tmp"
    mv "$tmp" "$ROUTES_FILE"
}

drop_route() {
    local key="$1" tmp
    tmp=$(mktemp)
    grep -v "^$(printf '%s' "$key" | sed 's/[[\.*^$/]/\\&/g')[[:space:]]" "$ROUTES_FILE" > "$tmp" 2>/dev/null || true
    mv "$tmp" "$ROUTES_FILE"
}

cmd_assign() {
    [ $# -ge 1 ] || die "usage: conductor-acct assign <profile> [path] | assign default <profile>"
    ensure_root

    local key profile
    if [ "$1" = "default" ]; then
        # `assign default <profile>` reads more naturally for the fallback.
        key=default
        profile="${2:-}"
        [ -n "$profile" ] || die "usage: conductor-acct assign default <profile>"
    else
        profile="$1"
        key=$(current_workspace "${2:-$PWD}")
    fi

    if [ ! -d "$ACCOUNTS_ROOT/claude/$profile" ] && [ ! -d "$ACCOUNTS_ROOT/codex/$profile" ]; then
        die "no such profile '$profile'"
    fi

    write_route "$key" "$profile"
    echo "$key -> $profile"
}

cmd_unassign() {
    local path="${1:-$PWD}" key
    ensure_root
    if [ "$path" = "default" ]; then
        key=default
    else
        key=$(current_workspace "$path")
    fi
    drop_route "$key"
    echo "removed route for $key"
}
