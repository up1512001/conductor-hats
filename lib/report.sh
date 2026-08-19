#!/bin/bash
# What this workspace resolves to: which, status, check.
#
# Sourced by bin/conductor-acct. Not executable on its own.

cmd_which() {
    local ws agent
    ws=$(current_workspace "${1:-}")
    agent=$(agent_of "${2:-claude}")
    ensure_root
    load_config

    local repo var binding resolved profile
    repo=$(repo_root_for "$ws")
    var=$(agent_home_var "$agent")
    binding=$(repo_binding "$agent" "$repo" || true)

    echo "workspace:  $ws"
    echo "repository: $repo"

    if [ -n "$binding" ]; then
        profile=$(profile_from_dir "$binding")
        echo "binding:    ${profile:-$binding}   (.conductor settings, applies to the whole repository)"
    else
        echo "binding:    (none)"
    fi

    resolve_route "$ws"
    if [ -n "$ROUTE_PROFILE" ]; then
        if [ "$ROUTE_EXACT" = 1 ]; then
            echo "route:      $ROUTE_PROFILE   (this workspace)"
        else
            echo "route:      $ROUTE_PROFILE   (inherited from a parent path or the default)"
        fi
    else
        echo "route:      (none)"
    fi

    if ! router_installed; then
        echo
        echo "router:     off, so routes are recorded but not applied"
        if [ -n "$binding" ]; then
            echo "effective:  ${profile:-$binding}   (from the repository binding)"
        else
            echo "effective:  your default account"
        fi
        return 0
    fi

    resolved=$(router_dry_run "$agent" "$ws" || true)
    echo
    if [ -n "$resolved" ]; then
        profile=$(profile_from_dir "$resolved")
        echo "effective:  ${profile:-$resolved}"
        echo "            $var=$resolved"
    elif [ -n "$binding" ]; then
        echo "effective:  ${profile:-$binding}   (from the repository binding, applied by Conductor)"
        echo "            $var=$binding"
    else
        echo "effective:  your default account (no $var would be set)"
    fi
}

cmd_status() {
    ensure_root
    local ws agent resolved profile label masked=0 arg
    local -a rest=()
    for arg in "$@"; do
        if [ "$arg" = "--mask" ]; then masked=1; else rest+=("$arg"); fi
    done
    ws=$(current_workspace "${rest[0]:-}")
    for agent in claude codex; do
        [ -n "$(list_profiles "$agent")" ] || continue
        resolved=$(effective_dir "$agent" "$ws")
        if [ -n "$resolved" ]; then
            profile=$(profile_from_dir "$resolved")
            label=$(label_for_display "$agent" "$profile" "$masked")
            printf '%-7s %s%s\n' "$agent" "${profile:-$resolved}" "${label:+  $label}"
        else
            printf '%-7s %s\n' "$agent" "(default account)"
        fi
    done
    printf 'in      %s\n' "$ws"
}

# Machine-readable, for the prompt snippet `ask` installs. Deliberately terse:
# an agent reads this at the top of every session.
cmd_check() {
    ensure_root
    local ws agent resolved
    ws=$(current_workspace "${1:-}")
    for agent in claude codex; do
        [ -n "$(list_profiles "$agent")" ] || continue
        resolved=$(effective_dir "$agent" "$ws")
        if [ -n "$resolved" ]; then
            printf 'ACCOUNT %s %s\n' "$agent" "$(profile_from_dir "$resolved")"
            return 0
        fi
    done
    [ -n "$(list_profiles claude)" ] || { echo "NO_PROFILES"; return 0; }
    echo "NEEDS_ACCOUNT"
}
