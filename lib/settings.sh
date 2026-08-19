#!/bin/bash
# Writing Conductor's TOML: repository bindings and the router path.
#
# Sourced by bin/conductor-acct. Not executable on its own.

toml_set_env() {
    local file="$1" key="$2" value="$3" tmp
    mkdir -p "$(dirname "$file")"
    touch "$file"
    tmp=$(mktemp)
    awk -v key="$key" -v val="$value" '
        BEGIN { inenv = 0; written = 0 }
        /^[[:space:]]*\[environment_variables\][[:space:]]*$/ {
            print; inenv = 1; print key " = \"" val "\""; written = 1; next
        }
        /^[[:space:]]*\[/ { inenv = 0 }
        inenv && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { next }
        { print }
        END {
            if (!written) {
                print ""
                print "[environment_variables]"
                print key " = \"" val "\""
            }
        }
    ' "$file" > "$tmp"
    mv "$tmp" "$file"
}

toml_unset_env() {
    local file="$1" key="$2" tmp
    [ -f "$file" ] || return 0
    tmp=$(mktemp)
    awk -v key="$key" '
        BEGIN { inenv = 0 }
        /^[[:space:]]*\[environment_variables\][[:space:]]*$/ { inenv = 1; print; next }
        /^[[:space:]]*\[/ { inenv = 0 }
        inenv && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { next }
        { print }
    ' "$file" > "$tmp"
    mv "$tmp" "$file"
}

cmd_bind() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct bind <profile> [claude|codex] [repo-path]"
    require_profile "$agent" "$profile"
    local repo
    repo=$(repo_root_for "${3:-}")

    local dir="$ACCOUNTS_ROOT/$agent/$profile"
    local file="$repo/.conductor/settings.local.toml"
    toml_set_env "$file" "$(agent_home_var "$agent")" "$dir"
    echo "Bound $repo to the $agent profile '$profile'"
    echo "  $file"
    echo "  [environment_variables] $(agent_home_var "$agent") = \"$dir\""
    echo
    echo "Conductor reads this per repository, so other repos keep their own account."
    echo "Open a new chat in this repo's workspaces for it to take effect."
}

cmd_unbind() {
    local agent
    agent=$(agent_of "${1:-claude}")
    local repo file
    repo=$(repo_root_for "${2:-}")
    file="$repo/.conductor/settings.local.toml"
    toml_unset_env "$file" "$(agent_home_var "$agent")"
    echo "Removed the $agent binding from $file"
}

toml_set() {
    # Sets a top-level key in ~/.conductor/settings.toml, preserving the rest.
    # Top-level keys must be written above the first [table] header.
    local key="$1" value="$2" tmp
    mkdir -p "$(dirname "$CONDUCTOR_SETTINGS")"
    touch "$CONDUCTOR_SETTINGS"
    tmp=$(mktemp)
    awk -v key="$key" -v val="$value" '
        BEGIN { done = 0 }
        !done && /^[[:space:]]*\[/ { print key " = \"" val "\""; done = 1 }
        $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { next }
        { print }
        END { if (!done) print key " = \"" val "\"" }
    ' "$CONDUCTOR_SETTINGS" > "$tmp"
    mv "$tmp" "$CONDUCTOR_SETTINGS"
}

toml_unset() {
    local key="$1" tmp
    [ -f "$CONDUCTOR_SETTINGS" ] || return 0
    tmp=$(mktemp)
    grep -v "^[[:space:]]*${key}[[:space:]]*=" "$CONDUCTOR_SETTINGS" > "$tmp" || true
    mv "$tmp" "$CONDUCTOR_SETTINGS"
}
