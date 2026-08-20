#!/bin/bash
# routing tests for conductor-hats.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Resolving which account a workspace gets, and the order the layers win in.

test_use_routes_a_single_workspace() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    "$ACCT" use personal claude "$SANDBOX/ws-b" >/dev/null

    is "ws-a gets work" \
        "$(route_claude "$SANDBOX/ws-a")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    is "ws-b gets personal" \
        "$(route_claude "$SANDBOX/ws-b")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
}

test_use_is_idempotent() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    "$ACCT" use personal claude "$SANDBOX/ws-a" >/dev/null
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    is "one route per workspace" \
        "$(grep -c "^$SANDBOX/ws-a	" "$CONDUCTOR_ACCOUNTS_ROOT/routes")" "1"
    is "last write wins" \
        "$(route_claude "$SANDBOX/ws-a")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_route_prefix_matches_children_not_siblings() {
    fake_profile claude work
    mkdir -p "$SANDBOX/ws-a/nested" "$SANDBOX/ws-abc"
    "$ACCT" assign work "$SANDBOX/ws-a" >/dev/null

    is "child inherits" \
        "$(route_claude "$SANDBOX/ws-a/nested")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    is "name-prefix sibling does not" \
        "$(route_claude "$SANDBOX/ws-abc")" ""
}

test_route_longest_prefix_wins() {
    fake_profile claude work
    fake_profile claude personal
    mkdir -p "$SANDBOX/ws-a/inner"
    "$ACCT" assign work "$SANDBOX/ws-a" >/dev/null
    "$ACCT" assign personal "$SANDBOX/ws-a/inner" >/dev/null

    is "deeper route wins" \
        "$(route_claude "$SANDBOX/ws-a/inner")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
}

test_route_default_is_the_fallback() {
    fake_profile claude personal
    "$ACCT" assign default personal >/dev/null

    is "unrouted workspace falls back" \
        "$(route_claude "$SANDBOX/ws-b")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
}

test_repo_binding_is_honoured_by_the_router() {
    fake_profile claude work
    local bound="$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CLAUDE_CONFIG_DIR="$bound" "$PROJECT_DIR/target/release/claude-router" 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "router leaves an injected binding alone" "$got" "$bound"
}

test_workspace_route_beats_repo_binding() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use personal claude "$SANDBOX/ws-a" >/dev/null
    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CLAUDE_CONFIG_DIR="$CONDUCTOR_ACCOUNTS_ROOT/claude/work" \
        "$PROJECT_DIR/target/release/claude-router" 2>/dev/null | sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "the more specific route wins" "$got" "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
}

test_parent_route_does_not_beat_repo_binding() {
    fake_profile claude work
    fake_profile claude personal
    mkdir -p "$SANDBOX/ws-a/inner"
    "$ACCT" assign personal "$SANDBOX/ws-a" >/dev/null
    local got
    got=$(cd "$SANDBOX/ws-a/inner" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a/inner" \
        CLAUDE_CONFIG_DIR="$CONDUCTOR_ACCOUNTS_ROOT/claude/work" \
        "$PROJECT_DIR/target/release/claude-router" 2>/dev/null | sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "an inherited route yields to an explicit binding" \
        "$got" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_env_override_wins_over_everything() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use personal claude "$SANDBOX/ws-a" >/dev/null
    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CONDUCTOR_ACCOUNT=work "$PROJECT_DIR/target/release/claude-router" 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "CONDUCTOR_ACCOUNT forces a profile" "$got" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_router_forwards_argv_untouched() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    local argv
    argv=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" --output-format stream-json \
        --session-id=abc123 --model opus 2>/dev/null | sed -n 's/^ARGV=//p')
    is "argv survives" "$argv" "--output-format stream-json --session-id=abc123 --model opus"
}

test_router_passes_through_with_nothing_configured() {
    is "no profiles, no change" "$(route_claude "$SANDBOX/ws-a")" ""
}

# Fail open is the whole safety story: a broken install costs the routing, never
# the agent. There is no separate library to corrupt now that the router is one
# binary, so the state it has to survive is unreadable or nonsense on disk.
test_router_fails_open_on_an_unreadable_routes_file() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    chmod 000 "$CONDUCTOR_ACCOUNTS_ROOT/routes"

    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" --model opus 2>/dev/null)
    chmod 644 "$CONDUCTOR_ACCOUNTS_ROOT/routes"

    contains "the agent still starts" "$out" "ARGV=--model opus"
    is "and gets no config dir rather than a wrong one" \
        "$(printf '%s\n' "$out" | sed -n 's/^CLAUDE_CONFIG_DIR=//p')" ""
}

test_router_fails_open_on_a_nonsense_routes_file() {
    fake_profile claude work
    printf 'this is not ( a route\n\x00\x01 garbage\n' > "$CONDUCTOR_ACCOUNTS_ROOT/routes"

    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
}

test_router_fails_open_with_no_accounts_root_at_all() {
    rm -rf "${CONDUCTOR_ACCOUNTS_ROOT:?}"
    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
}

test_router_refuses_to_route_into_itself() {
    local status=0
    (cd "$SANDBOX/ws-a" && CONDUCTOR_ACCOUNTS_DEPTH=2 \
        "$PROJECT_DIR/target/release/claude-router" >/dev/null 2>&1) || status=$?
    is "a real loop exits 70" "$status" "70"
}

test_router_tolerates_one_inherited_generation() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CONDUCTOR_ACCOUNTS_ROUTING=claude CONDUCTOR_ACCOUNTS_DEPTH=1 \
        "$PROJECT_DIR/target/release/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
    is "and is still routed" \
        "$(printf '%s\n' "$out" | sed -n 's/^CLAUDE_CONFIG_DIR=//p')" \
        "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_router_ignores_a_profile_with_no_directory() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    rm -rf "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"

    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
    is "on the default account" \
        "$(printf '%s\n' "$out" | sed -n 's/^CLAUDE_CONFIG_DIR=//p')" ""
}

test_session_pin_survives_a_route_change() {
    fake_profile claude work
    fake_profile claude personal
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/sessions/claude"
    printf 'personal\n' > "$CONDUCTOR_ACCOUNTS_ROOT/sessions/claude/sess-1"
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    is "a running session keeps its account" \
        "$(route_claude "$SANDBOX/ws-a" --session-id=sess-1)" \
        "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
    is "and a new session gets the new route" \
        "$(route_claude "$SANDBOX/ws-a" --session-id=sess-2)" \
        "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_resume_reuses_the_session_pin() {
    fake_profile claude personal
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/sessions/claude"
    printf 'personal\n' > "$CONDUCTOR_ACCOUNTS_ROOT/sessions/claude/sess-1"

    is "--resume finds the pin" \
        "$(route_claude "$SANDBOX/ws-a" --resume=sess-1)" \
        "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
}
