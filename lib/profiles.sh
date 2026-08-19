#!/bin/bash
# Creating, signing in and out of, and deleting profiles.
#
# Sourced by bin/conductor-acct. Not executable on its own.

cmd_init() {
    ensure_root
    echo "Initialised $ACCOUNTS_ROOT"
    echo "Next: conductor-acct add <profile>"
}

cmd_add() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct add <profile> [claude|codex]"
    case "$profile" in
        *[!a-zA-Z0-9_-]*) die "profile names may only contain letters, digits, - and _" ;;
    esac

    ensure_root
    local dir="$ACCOUNTS_ROOT/$agent/$profile"
    local src
    src=$(source_home "$agent")

    if [ -d "$dir" ]; then
        echo "Profile '$profile' already exists at $dir"
    else
        mkdir -p "$dir"
        echo "Created $dir"
    fi

    if [ "$agent" = "claude" ]; then
        local item
        for item in $SHARED_LINKS; do
            [ -e "$src/$item" ] || continue
            [ -e "$dir/$item" ] && continue
            ln -s "$src/$item" "$dir/$item"
            echo "  linked $item -> $src/$item"
        done
    fi

    cmd_login "$profile" "$agent"
}

cmd_login() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct login <profile> [claude|codex]"
    require_profile "$agent" "$profile"

    local dir="$ACCOUNTS_ROOT/$agent/$profile" bin
    bin=$(resolve_agent_binary "$agent" "$SELF") || die "could not locate the real $agent binary"

    echo "Signing in to '$profile' using $bin"
    echo "  $(agent_home_var "$agent")=$dir"
    echo

    case "$agent" in
        claude) env CLAUDE_CONFIG_DIR="$dir" "$bin" auth login ;;
        codex) env CODEX_HOME="$dir" "$bin" login ;;
    esac

    local email clash
    email=$(refresh_label "$agent" "$profile" || true)
    echo
    if [ -n "$email" ]; then
        echo "Profile '$profile' is now $email"
        if clash=$(profile_with_email "$agent" "$email" "$profile"); then
            warn_duplicate_email "$agent" "$profile" "$clash" "$email"
        fi
    else
        echo "Signed in. Could not read the account email; the picker will show the profile name only."
    fi
}

# Said as a warning rather than an error: the sign-in already happened, and
# refusing after the fact would leave the profile in a state the message denies.
warn_duplicate_email() {
    local agent="$1" profile="$2" clash="$3" email="$4"
    cat >&2 <<EOF

Warning: '$clash' is already signed in to $email on $agent.

One account cannot be two accounts. The provider keeps a single live token per
account, so whichever of '$profile' and '$clash' signed in last holds it and the
other is now signed out. They will keep logging each other out.

Keep one and drop the other:
  conductor-acct remove $clash $agent
EOF
}

cmd_logout() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct logout <profile> [claude|codex]"
    require_profile "$agent" "$profile"
    local dir="$ACCOUNTS_ROOT/$agent/$profile" bin
    bin=$(resolve_agent_binary "$agent" "$SELF") || die "could not locate the real $agent binary"
    case "$agent" in
        claude) env CLAUDE_CONFIG_DIR="$dir" "$bin" auth logout ;;
        codex) env CODEX_HOME="$dir" "$bin" logout ;;
    esac
    rm -f "$dir/.label"
    echo "Signed out of '$profile'. The profile directory is still there; conductor-acct remove $profile deletes it."
}

cmd_remove() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct remove <profile> [claude|codex]"
    require_profile "$agent" "$profile"
    ensure_root

    cmd_logout "$profile" "$agent" || true
    rm -rf "${ACCOUNTS_ROOT:?}/$agent/$profile"

    local tmp
    tmp=$(mktemp)
    awk -v p="$profile" '
        /^[[:space:]]*(#|$)/ { print; next }
        { line = $0; sub(/^[^\t ]*[\t ]+/, "", line) }
        line != p { print }
    ' "$ROUTES_FILE" > "$tmp"
    mv "$tmp" "$ROUTES_FILE"

    echo "Removed the $agent profile '$profile' and any routes pointing at it."
}

cmd_list() {
    ensure_root
    local agent profile label masked=0
    [ "${1:-}" = "--mask" ] && masked=1
    for agent in claude codex; do
        local found=0
        for profile in $(list_profiles "$agent"); do
            [ "$found" -eq 0 ] && { echo "$agent profiles:"; found=1; }
            label=$(label_for_display "$agent" "$profile" "$masked")
            if [ -n "$label" ]; then
                printf '  %-14s %s\n' "$profile" "$label"
            elif profile_signed_in "$agent" "$profile"; then
                printf '  %-14s %s\n' "$profile" "(signed in, address not cached yet)"
            else
                printf '  %-14s %s\n' "$profile" "(not signed in)"
            fi
        done
        [ "$found" -eq 1 ] && echo
    done

    echo "routes:"
    if grep -qv '^[[:space:]]*\(#.*\)\?$' "$ROUTES_FILE" 2>/dev/null; then
        grep -v '^[[:space:]]*\(#.*\)\?$' "$ROUTES_FILE" | sed 's/^/  /'
    else
        echo "  (none)"
    fi
    echo
    if router_installed; then
        echo "router: on"
    else
        echo "router: off   (repository bindings still work; conductor-acct install turns it on)"
    fi
}

# The headline command: point this workspace at an account.
