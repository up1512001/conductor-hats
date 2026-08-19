#!/bin/bash
# Reading an account's address, and whether it holds credentials.
#
# Sourced by bin/conductor-acct. Not executable on its own.

account_email() {
    local f="$1"
    [ -f "$f" ] || return 1
    sed -n 's/.*"emailAddress"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$f" | head -1
}

codex_email() {
    local f="$1"
    [ -f "$f" ] || return 1
    sed -n 's/.*"email"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$f" | head -1
}

refresh_label() {
    local agent="$1" profile="$2" dir email=""
    dir="$ACCOUNTS_ROOT/$agent/$profile"
    case "$agent" in
        claude) email=$(account_email "$dir/.claude.json" || true) ;;
        codex) email=$(codex_email "$dir/auth.json" || true) ;;
    esac
    if [ -n "$email" ]; then
        printf '%s\n' "$email" > "$dir/.label"
    fi
    printf '%s\n' "$email"
}

label_of() {
    read_line_from "$ACCOUNTS_ROOT/$1/$2/.label" 2>/dev/null || true
}

# Claude Code resolves credentials as $CLAUDE_CONFIG_DIR/.credentials.json, then a
# keychain item whose service name carries the first 8 hex of sha256 of the config
# directory. Both are checked, in that order.
#
# Not inferred from .label: that is a cached address, only written once
# oauthAccount.emailAddress is readable, so a profile with working credentials read
# as signed out for ever.
keychain_service() {
    local dir="$1" hash
    hash=$(printf '%s' "$dir" | shasum -a 256 | cut -c1-8)
    printf 'Claude Code-credentials-%s\n' "$hash"
}

profile_signed_in() {
    local agent="$1" profile="$2" dir
    dir="$ACCOUNTS_ROOT/$agent/$profile"
    [ -d "$dir" ] || return 1
    case "$agent" in
        claude)
            [ -s "$dir/.credentials.json" ] && return 0
            security find-generic-password -s "$(keychain_service "$dir")" \
                >/dev/null 2>&1 && return 0
            return 1
            ;;
        codex)
            [ -s "$dir/auth.json" ] && return 0
            return 1
            ;;
    esac
    return 1
}

# Which other profile already holds this address. One live token per account, so a
# pair sharing one address sign each other out.
profile_with_email() {
    local agent="$1" email="$2" skip="${3:-}" profile
    [ -n "$email" ] || return 1
    for profile in $(list_profiles "$agent"); do
        [ "$profile" != "$skip" ] || continue
        if [ "$(label_of "$agent" "$profile")" = "$email" ]; then
            printf '%s\n' "$profile"
            return 0
        fi
    done
    return 1
}

# The directory Conductor would run an agent in. Inside a Conductor session the
# app tells us; from a terminal, the shell's own working directory is right.
