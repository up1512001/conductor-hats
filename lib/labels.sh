#!/bin/bash
# Reading an account's address, and whether it holds credentials.
#
# Sourced by bin/conductor-acct. Not executable on its own.

account_email() {
    # Reads oauthAccount.emailAddress out of a profile's .claude.json without
    # requiring jq.
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

# Whether a profile actually holds credentials, asked of the place that holds
# them rather than inferred from a cached label.
#
# This used to be "does .label exist", which is a cached email address written
# after a sign-in and deleted on sign-out. It lies in the case that matters:
# .label is only written when oauthAccount.emailAddress can be read out of
# .claude.json, and that is not always populated the moment a sign-in finishes.
# A profile with perfectly good credentials then read as "not signed in" for
# ever, so the panel offered to sign an account in that was already signed in.
#
# Claude Code resolves credentials as $CLAUDE_CONFIG_DIR/.credentials.json, then
# a keychain item whose service name carries the first 8 hex of sha256 of the
# config directory. Both are checked here, in that order.
keychain_service() {
    # Claude normalises the path to NFC first. Every path under ACCOUNTS_ROOT is
    # ASCII, where NFC is a no-op, so plain bytes are the same answer.
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

# Which other profile of this agent already holds an address, if any. Two
# profiles on one account is not a second account: the provider's OAuth hands
# out one live token per account, so signing the second one in silently revokes
# the first, and the pair then take turns logging each other out.
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
