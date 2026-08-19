#!/bin/bash
# Driving a sign-in from the panel, with stdin on a FIFO.
#
# Sourced by bin/conductor-acct. Not executable on its own.

# ------------------------------------------------------------ in-app login ---
#
# `claude auth login` prints an authorisation URL, opens a browser and then
# blocks reading the code from stdin. That is scriptable if stdin is a pipe that
# stays open: start the process with its stdin on a FIFO, hand the URL to
# whoever asked, and write the code into the FIFO when it arrives. Which is what
# lets the account panel run a sign-in without a terminal.

login_dir() { printf '%s\n' "$ACCOUNTS_ROOT/login/$1"; }

cmd_login_start() {
    local profile="${1:-}" agent
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct login-start <profile> [agent]"
    case "$profile" in
        *[!a-zA-Z0-9_-]*) die "profile names may only contain letters, digits, - and _" ;;
    esac
    ensure_root

    local dir state bin
    dir="$ACCOUNTS_ROOT/$agent/$profile"
    state=$(login_dir "$profile")
    bin=$(resolve_agent_binary "$agent" "$SELF") || die "could not locate the real $agent binary"

    cmd_login_cancel "$profile" >/dev/null 2>&1 || true
    rm -rf "$state"
    mkdir -p "$state" "$dir"

    if [ "$agent" = "claude" ]; then
        local item
        for item in $SHARED_LINKS; do
            [ -e "$HOME/.claude/$item" ] || continue
            [ -e "$dir/$item" ] && continue
            ln -s "$HOME/.claude/$item" "$dir/$item"
        done
    fi

    mkfifo "$state/stdin"
    # Held open by a sleeper so the CLI does not see EOF before the code lands.
    ( exec 9<>"$state/stdin"; sleep 300 ) >/dev/null 2>&1 &
    printf '%s\n' "$!" > "$state/holder.pid"

    case "$agent" in
        claude) env CLAUDE_CONFIG_DIR="$dir" "$bin" auth login ;;
        codex) env CODEX_HOME="$dir" "$bin" login ;;
    esac < "$state/stdin" > "$state/out" 2>&1 &
    printf '%s\n' "$!" > "$state/pid"

    # Wait for the URL rather than guessing how long the CLI takes to print it.
    local i=0 url=""
    while [ "$i" -lt 100 ]; do
        url=$(sed -n 's|.*\(https://[^ ]*oauth[^ ]*\).*|\1|p' "$state/out" 2>/dev/null | head -1)
        [ -n "$url" ] && break
        kill -0 "$(cat "$state/pid")" 2>/dev/null || break
        sleep 0.1
        i=$((i + 1))
    done

    if [ -z "$url" ]; then
        cmd_login_cancel "$profile" >/dev/null 2>&1 || true
        die "sign-in did not produce a URL; run 'conductor-acct add $profile' in a terminal"
    fi
    printf '%s\n' "$url"
}

cmd_login_code() {
    local profile="${1:-}" code="${2:-}" state
    if [ -z "$profile" ] || [ -z "$code" ]; then
        die "usage: conductor-acct login-code <profile> <code>"
    fi
    state=$(login_dir "$profile")
    [ -p "$state/stdin" ] || die "no sign-in in progress for '$profile'"
    printf '%s\n' "$code" > "$state/stdin"
    echo "submitted"
}

cmd_login_status() {
    local profile="${1:-}" agent state pid email
    agent=$(agent_of "${2:-claude}")
    [ -n "$profile" ] || die "usage: conductor-acct login-status <profile> [agent]"
    state=$(login_dir "$profile")
    [ -d "$state" ] || { echo "idle"; return 0; }

    pid=$(read_line_from "$state/pid" 2>/dev/null || true)
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        echo "pending"
        return 0
    fi

    email=$(refresh_label "$agent" "$profile" || true)
    cmd_login_cancel "$profile" >/dev/null 2>&1 || true
    if [ -n "$email" ]; then
        printf 'ok %s\n' "$email"
    elif profile_signed_in "$agent" "$profile"; then
        # Credentials landed, the address has not been written to .claude.json
        # yet. Reporting this as a failure was wrong, and it is what made a
        # completed sign-in look like one that had not happened.
        printf 'ok\n'
    else
        printf 'error %s\n' "$(tail -3 "$state/out" 2>/dev/null | tr '\n' ' ' | sed 's/  */ /g')"
    fi
}

cmd_login_cancel() {
    local profile="${1:-}" state pid
    [ -n "$profile" ] || die "usage: conductor-acct login-cancel <profile>"
    state=$(login_dir "$profile")
    while read -r pid; do
        [ -n "$pid" ] || continue
        kill "$pid" 2>/dev/null || true
    done < <(cat "$state/pid" "$state/holder.pid" 2>/dev/null)
    rm -rf "$state"
    echo "cancelled"
}

cmd_sessions() {
    ensure_root
    if [ "${1:-}" = "clear" ]; then
        rm -rf "${SESSION_DIR:?}"/claude "${SESSION_DIR:?}"/codex
        echo "cleared session pins"
        return
    fi
    local agent f
    for agent in claude codex; do
        [ -d "$SESSION_DIR/$agent" ] || continue
        for f in "$SESSION_DIR/$agent"/*; do
            [ -f "$f" ] || continue
            printf '%-7s %s -> %s\n' "$agent" "${f##*/}" "$(cat "$f")"
        done
    done
}

# Sets one key inside the [environment_variables] table of a repo settings file,
# creating the table when it is missing and leaving every other table alone.
