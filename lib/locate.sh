#!/bin/bash
# Working out which workspace and repository we are talking about.
#
# Sourced by bin/conductor-acct. Not executable on its own.

current_workspace() {
    local path="${1:-}"
    if [ -n "$path" ]; then
        [ -d "$path" ] || die "no such directory: $path"
        normalize_dir "$path"
        return 0
    fi
    workspace_dir
}

repo_root_for() {
    local start="${1:-$PWD}"
    # Conductor exports the repo root; fall back to the git common dir so this
    # also works from a plain checkout.
    if [ -z "${1:-}" ] && [ -n "${CONDUCTOR_ROOT_PATH:-}" ]; then
        printf '%s\n' "$CONDUCTOR_ROOT_PATH"
        return 0
    fi
    local common
    common=$(git -C "$start" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || {
        printf '%s\n' "$start"; return 0
    }
    printf '%s\n' "$(dirname "$common")"
}

router_installed() {
    grep -q "claude-router" "$CONDUCTOR_SETTINGS" 2>/dev/null
}

# Runs the real router against a workspace with /usr/bin/env standing in for the
# agent binary, and reports the config dir it would have exported. This is the
# routing decision itself rather than a re-implementation of it.
router_dry_run() {
    local agent="$1" ws="$2" var out
    var=$(agent_home_var "$agent")
    # -u CONDUCTOR_ACCOUNTS_ROUTING because this may itself be running inside a
    # routed agent session, where the router's loop guard would refuse to start.
    case "$agent" in
        claude) out=$(cd "$ws" && env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH -u CLAUDE_CONFIG_DIR \
            CONDUCTOR_WORKSPACE_PATH="$ws" CONDUCTOR_ACCOUNTS_CLAUDE_BIN=/usr/bin/env \
            "$BIN_DIR/claude-router" 2>/dev/null) || return 1 ;;
        codex) out=$(cd "$ws" && env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH -u CODEX_HOME \
            CONDUCTOR_WORKSPACE_PATH="$ws" CONDUCTOR_ACCOUNTS_CODEX_BIN=/usr/bin/env \
            "$BIN_DIR/codex-router" 2>/dev/null) || return 1 ;;
    esac
    printf '%s\n' "$out" | sed -n "s/^$var=//p" | head -1
}

# Whatever a repository settings file pins for this agent, if anything.
repo_binding() {
    local agent="$1" repo="$2" var f
    var=$(agent_home_var "$agent")
    for f in "$repo/.conductor/settings.local.toml" "$repo/.conductor/settings.toml"; do
        [ -f "$f" ] || continue
        awk -v key="$var" '
            /^[[:space:]]*\[environment_variables(\.local)?\][[:space:]]*$/ { inenv = 1; next }
            /^[[:space:]]*\[/ { inenv = 0 }
            inenv && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
                sub(/^[^=]*=[[:space:]]*/, ""); gsub(/^"|"$/, ""); print; exit
            }
        ' "$f" && return 0
    done
}

# What this path actually ends up on, whichever mechanism gets there first.
#
# The router only ever exports a route, because a repository binding needs no
# router: Conductor applies `[environment_variables]` from the repo's settings
# itself when it spawns the agent. Reporting the dry run alone therefore told
# you "default account" for a repository that was firmly bound, which is what
# made the New Workspace chip read "default account" while the binding said
# work. Ask the router, then fall back to the binding it never sees.
effective_dir() {
    local agent="$1" ws="$2" repo="${3:-}" out
    [ -n "$repo" ] || repo=$(repo_root_for "$ws")
    if router_installed; then
        out=$(router_dry_run "$agent" "$ws" || true)
        [ -z "$out" ] || { printf '%s\n' "$out"; return 0; }
    fi
    repo_binding "$agent" "$repo" || true
}

profile_from_dir() {
    local dir="$1"
    case "$dir" in
        "$ACCOUNTS_ROOT"/*/*) printf '%s\n' "${dir##*/}" ;;
        *) printf '\n' ;;
    esac
}

# Mask an address for anything that renders on screen: a recorded session or a
# shared screenshot should not hand one out. Local part and host are masked
# separately and the suffix is kept, so the result still reads as an email:
#
#   someone.long@example.com  ->  som**ong@ex**e.com
#   joe@mail.example.com      ->  j**@m**.example.com
#
# How much is revealed scales with length, so a short part is not handed over in
# full for want of characters to hide. This rule is duplicated in
# tools/ui-patch/account-ui.js, which cannot shell out per row, and a test
# asserts the two agree on every case.
