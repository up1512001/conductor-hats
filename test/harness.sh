#!/bin/bash
# Shared harness: the sandbox, the assertions, the runner.
#
# Sourced by test/run.sh. Every test runs in a fresh $TMPDIR sandbox with stub
# agent binaries, so no real Conductor install, ~/.claude directory or keychain
# item is touched.

sandbox() {
    # Resolved, because $TMPDIR lives under the /var -> /private/var symlink and
    # routes are compared as strings.
    SANDBOX=$(cd "$(mktemp -d "${TMPDIR:-/tmp}/cma-test.XXXXXX")" && pwd)
    export CONDUCTOR_ACCOUNTS_ROOT="$SANDBOX/accounts"
    export CONDUCTOR_ACCT_SETTINGS_FILE="$SANDBOX/settings.toml"
    export CONDUCTOR_ACCT_COMMANDS_DIR="$SANDBOX/commands"
    export CONDUCTOR_ACCOUNTS_CLAUDE_BIN="$SANDBOX/stub-claude"
    export CONDUCTOR_ACCOUNTS_CODEX_BIN="$SANDBOX/stub-codex"
    unset CONDUCTOR_ACCOUNT CONDUCTOR_WORKSPACE_PATH CONDUCTOR_ROOT_PATH
    unset CLAUDE_CONFIG_DIR CODEX_HOME
    # The suite may itself be running inside a routed agent session, where this
    # is set and the router's loop guard would refuse every spawn.
    unset CONDUCTOR_ACCOUNTS_ROUTING CONDUCTOR_ACCOUNTS_DEPTH

    # Stubs stand in for the real agents: they report the two things the router
    # is responsible for, the config dir it exported and the argv it forwarded.
    cat > "$SANDBOX/stub-claude" <<'EOF'
#!/bin/sh
echo "CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_DIR:-}"
echo "ARGV=$*"
EOF
    cat > "$SANDBOX/stub-codex" <<'EOF'
#!/bin/sh
echo "CODEX_HOME=${CODEX_HOME:-}"
echo "ARGV=$*"
EOF
    chmod +x "$SANDBOX/stub-claude" "$SANDBOX/stub-codex"

    "$ACCT" init >/dev/null
    mkdir -p "$SANDBOX/ws-a" "$SANDBOX/ws-b" "$SANDBOX/repo/.conductor"
}

teardown() {
    [ -n "${SANDBOX:-}" ] && rm -rf "$SANDBOX"
}

# fake_profile <agent> <name> [email]
fake_profile() {
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/$1/$2"
    printf '%s\n' "${3:-$2@example.test}" > "$CONDUCTOR_ACCOUNTS_ROOT/$1/$2/.label"
}

# route_claude <workspace> -- runs the router as Conductor would and prints the
# config dir the agent actually received.
route_claude() {
    (cd "$1" && CONDUCTOR_WORKSPACE_PATH="$1" "$PROJECT_DIR/bin/claude-router" "${@:2}") 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p'
}

route_codex() {
    (cd "$1" && CONDUCTOR_WORKSPACE_PATH="$1" "$PROJECT_DIR/bin/codex-router" "${@:2}") 2>/dev/null |
        sed -n 's/^CODEX_HOME=//p'
}

ok() {
    PASS=$((PASS + 1))
    printf '  ok    %s\n' "$1"
}

# Counted as neither, and said out loud, so a missing tool cannot read as green.
skip() {
    printf '  skip  %s\n' "$1"
}

not_ok() {
    FAIL=$((FAIL + 1))
    printf '  FAIL  %s\n' "$1"
    printf '        expected: %s\n' "$2"
    printf '        actual:   %s\n' "$3"
}

is() {
    if [ "$2" = "$3" ]; then ok "$1"; else not_ok "$1" "$3" "$2"; fi
}

contains() {
    case "$2" in
        *"$3"*) ok "$1" ;;
        *) not_ok "$1" "something containing '$3'" "$2" ;;
    esac
}

run_test() {
    local name="$1"
    case "$name" in
        *"$FILTER"*) ;;
        *) return 0 ;;
    esac
    printf '%s\n' "$name"
    sandbox
    "$name"
    teardown
}
