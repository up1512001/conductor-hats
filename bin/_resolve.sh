#!/bin/sh
# Shared logic for the Conductor multi-account routers.
# Sourced, never executed. POSIX sh / bash 3.2 compatible, and fork-free on the
# path where a decision is already known: this runs on every agent spawn.

ACCOUNTS_ROOT="${CONDUCTOR_ACCOUNTS_ROOT:-$HOME/.conductor-accounts}"
ROUTES_FILE="$ACCOUNTS_ROOT/routes"
CONFIG_FILE="$ACCOUNTS_ROOT/config"
SESSION_DIR="$ACCOUNTS_ROOT/sessions"

# This router never opens a dialog. An agent spawn is not a moment to block on
# a window, and a system popup is not part of Conductor's UI. Accounts are
# chosen ahead of time, from inside Conductor with /account or from a terminal
# with `conductor-acct use`, and recorded as routes that resolve silently here.

# Set by resolve_route. ROUTE_EXACT is 1 when the winning route named the
# workspace itself rather than a parent path or the default entry.
ROUTE_PROFILE=
ROUTE_EXACT=0

# The config file has no keys yet, and the one it used to parse was dead: the
# `default` fallback lives in the routes file, where resolve_route can see it
# alongside every other route. Kept as the hook for the first real setting, and
# because callers treat a missing accounts root as "not set up".
load_config() {
    [ -f "$CONFIG_FILE" ] || return 0
}

read_line_from() {
    [ -f "$1" ] || return 1
    IFS= read -r _rl < "$1" || return 1
    printf '%s\n' "$_rl"
}

# normalize_dir <path>
# Routes are compared as strings, so both sides have to agree on symlinks.
# On macOS /var is a symlink to /private/var, and a route written from a shell
# would never match a path Conductor handed us verbatim without this.
normalize_dir() {
    (cd "$1" 2>/dev/null && pwd) || printf '%s\n' "$1"
}

# workspace_dir
# Conductor exports the workspace path to every agent process. Fall back to the
# working directory, which Conductor also sets to the workspace.
workspace_dir() {
    if [ -n "${CONDUCTOR_WORKSPACE_PATH:-}" ]; then
        normalize_dir "$CONDUCTOR_WORKSPACE_PATH"
    else
        normalize_dir "$PWD"
    fi
}

# session_id_from_args "$@"
# Conductor passes --session-id=<uuid> for new sessions and --resume=<uuid> when
# restarting a generator, both in the --flag=value form.
session_id_from_args() {
    for _a in "$@"; do
        case "$_a" in
            --session-id=*) printf '%s\n' "${_a#--session-id=}"; return 0 ;;
        esac
    done
    for _a in "$@"; do
        case "$_a" in
            --resume=*) printf '%s\n' "${_a#--resume=}"; return 0 ;;
        esac
    done
    return 0
}

# resolve_route <dir>
# Longest matching path prefix from the routes file, else the `default` entry.
# Sets ROUTE_PROFILE (empty when nothing matches) and ROUTE_EXACT. It assigns
# rather than prints because a command substitution would run it in a subshell
# and throw ROUTE_EXACT away.
resolve_route() {
    _dir="$1"
    ROUTE_PROFILE=
    ROUTE_EXACT=0
    [ -f "$ROUTES_FILE" ] || return 0

    _best_len=-1
    _best=""
    _best_exact=0
    _default=""

    while IFS= read -r _line || [ -n "$_line" ]; do
        case "$_line" in
            ''|'#'*) continue ;;
        esac

        _path=${_line%%[	 ]*}
        _profile=${_line#"$_path"}
        _profile=${_profile#"${_profile%%[![:space:]]*}"}
        _profile=${_profile%"${_profile##*[![:space:]]}"}
        [ -n "$_profile" ] || continue

        if [ "$_path" = "default" ]; then
            _default="$_profile"
            continue
        fi

        # The directory itself or anything beneath it, never a sibling sharing a
        # name prefix (/a/b must not match /a/bc).
        case "$_dir" in
            "$_path") _exact=1 ;;
            "$_path"/*) _exact=0 ;;
            *) continue ;;
        esac

        _len=${#_path}
        if [ "$_len" -gt "$_best_len" ]; then
            _best_len=$_len
            _best="$_profile"
            _best_exact=$_exact
        fi
    done < "$ROUTES_FILE"

    if [ -n "$_best" ]; then
        ROUTE_PROFILE="$_best"
        ROUTE_EXACT=$_best_exact
    elif [ -n "$_default" ]; then
        ROUTE_PROFILE="$_default"
    fi
}

# list_profiles <agent>
list_profiles() {
    [ -d "$ACCOUNTS_ROOT/$1" ] || return 0
    for _p in "$ACCOUNTS_ROOT/$1"/*; do
        [ -d "$_p" ] || continue
        printf '%s\n' "${_p##*/}"
    done
}

# decide_profile <agent> <workspace_dir> <session_id> <env_already_bound>
#
# Precedence, highest first:
#   1. CONDUCTOR_ACCOUNT      explicit override for one spawn
#   2. session pin            this session already started on an account
#   3. exact workspace route  `conductor-acct use` or /account on this workspace
#   4. env_already_bound      a repository binding is in effect, honour it
#   5. prefix route, then the default route
#
# Prints the profile name, or nothing to mean "change nothing". Never blocks,
# never prompts: if none of the above answers, the default account is right.
decide_profile() {
    _agent="$1"
    _dir="$2"
    _sid="$3"
    _env_bound="${4:-0}"

    if [ -n "${CONDUCTOR_ACCOUNT:-}" ]; then
        printf '%s\n' "$CONDUCTOR_ACCOUNT"
        return 0
    fi

    # A session keeps the account it started on. Conductor respawns the agent
    # on resume, model switches and generator restarts, and an account change
    # underneath a running conversation would break --resume.
    _pin="$SESSION_DIR/$_agent/$_sid"
    if [ -n "$_sid" ] && read_line_from "$_pin"; then
        return 0
    fi

    resolve_route "$_dir"
    _routed="$ROUTE_PROFILE"

    # A route naming this exact workspace is the most specific thing anyone can
    # express, so it outranks a repository-wide binding.
    if [ -n "$_routed" ] && [ "$ROUTE_EXACT" = 1 ]; then
        remember_session "$_agent" "$_sid" "$_routed"
        printf '%s\n' "$_routed"
        return 0
    fi

    # Otherwise a repository binding already answered the question, and
    # overriding it here would make two configured things disagree.
    if [ "$_env_bound" = 1 ]; then
        return 0
    fi

    [ -n "$_routed" ] && remember_session "$_agent" "$_sid" "$_routed"
    printf '%s\n' "$_routed"
}

# remember_session <agent> <session_id> <profile>
# Best effort. Losing a pin costs stability on resume, never correctness.
remember_session() {
    [ -n "$2" ] || return 0
    mkdir -p "$SESSION_DIR/$1" 2>/dev/null || return 0
    printf '%s\n' "$3" > "$SESSION_DIR/$1/$2" 2>/dev/null || true
}

# resolve_agent_binary <agent> <self_path>
# Finds the real agent binary, refusing to return the router itself.
resolve_agent_binary() {
    _agent="$1"
    _self="$2"

    case "$_agent" in
        claude) eval "_override=\${CONDUCTOR_ACCOUNTS_CLAUDE_BIN:-}" ;;
        codex) eval "_override=\${CONDUCTOR_ACCOUNTS_CODEX_BIN:-}" ;;
        *) _override="" ;;
    esac
    if [ -n "$_override" ] && [ -x "$_override" ]; then
        printf '%s\n' "$_override"
        return 0
    fi

    if _pinned_path=$(read_line_from "$ACCOUNTS_ROOT/$_agent-bin"); then
        if [ -x "$_pinned_path" ]; then
            printf '%s\n' "$_pinned_path"
            return 0
        fi
    fi

    # Conductor manages these symlinks and repoints them on every agent update,
    # which is exactly why the router must never patch them in place.
    _bundled="${CONDUCTOR_AGENT_BIN_DIR:-$HOME/Library/Application Support/com.conductor.app/bin}/$_agent"
    if [ -x "$_bundled" ]; then
        printf '%s\n' "$_bundled"
        return 0
    fi

    _found=$(command -v "$_agent" 2>/dev/null)
    if [ -n "$_found" ] && [ "$_found" != "$_self" ]; then
        printf '%s\n' "$_found"
        return 0
    fi

    return 1
}
