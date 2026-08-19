#!/bin/bash
# Which agent, where its home lives, and the accounts root.
#
# Sourced by bin/conductor-acct. Not executable on its own.

die() { echo "conductor-acct: $*" >&2; exit 1; }

agent_of() {
    case "${1:-claude}" in
        claude|codex) printf '%s\n' "$1" ;;
        *) die "unknown agent '$1' (expected claude or codex)" ;;
    esac
}

agent_home_var() {
    case "$1" in
        claude) printf 'CLAUDE_CONFIG_DIR\n' ;;
        codex) printf 'CODEX_HOME\n' ;;
    esac
}

source_home() {
    case "$1" in
        claude) printf '%s\n' "$HOME/.claude" ;;
        codex) printf '%s\n' "$HOME/.codex" ;;
    esac
}

require_profile() {
    local agent="$1" profile="$2"
    [ -d "$ACCOUNTS_ROOT/$agent/$profile" ] ||
        die "no such $agent profile '$profile' (run: conductor-acct add $profile $agent)"
}

ensure_root() {
    mkdir -p "$ACCOUNTS_ROOT/claude" "$ACCOUNTS_ROOT/codex" "$SESSION_DIR"
    if [ ! -f "$CONFIG_FILE" ]; then
        cat > "$CONFIG_FILE" <<'EOF'
# Reserved for future settings. Accounts are chosen per workspace and live in
# the routes file next to this one.
EOF
    fi
    if [ ! -f "$ROUTES_FILE" ]; then
        cat > "$ROUTES_FILE" <<'EOF'
# <workspace-or-repo-path><TAB><profile>     longest matching prefix wins
# default<TAB><profile>                      fallback when nothing matches
EOF
    fi
}
