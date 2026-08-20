#!/bin/bash
# Containment: no identifier may become a path component that escapes its root.
#
# Every case here was reachable before validation was centralised. `remove` with
# a traversal name deleted an arbitrary directory, and a crafted --session-id
# wrote a pin file anywhere the user could write.

test_remove_refuses_a_traversal_name() {
    sandbox
    mkdir -p "$SANDBOX/victim"
    echo keep > "$SANDBOX/victim/important.txt"

    local out status=0
    out=$("$ACCT" remove "../../victim" claude 2>&1) || status=$?
    is "rejected" "$status" "1"
    contains "naming the offending value" "$out" "invalid profile name"
    is "the directory survives" "$([ -f "$SANDBOX/victim/important.txt" ] && echo yes)" "yes"
    teardown
}

test_remove_refuses_an_absolute_name() {
    sandbox
    mkdir -p "$SANDBOX/victim"

    local status=0
    "$ACCT" remove "$SANDBOX/victim" claude >/dev/null 2>&1 || status=$?
    is "rejected" "$status" "1"
    is "the directory survives" "$([ -d "$SANDBOX/victim" ] && echo yes)" "yes"
    teardown
}

test_traversal_names_are_refused_everywhere() {
    sandbox
    fake_profile claude work
    local command status
    for command in use bind login logout remove; do
        status=0
        "$ACCT" "$command" "../escape" claude >/dev/null 2>&1 || status=$?
        is "$command rejects a traversal name" "$status" "1"
    done
    teardown
}

test_a_crafted_session_id_writes_no_pin_outside_the_sessions_dir() {
    sandbox
    fake_profile claude work
    route_claude "$SANDBOX/ws-a" >/dev/null
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    route_claude "$SANDBOX/ws-a" "--session-id=../../../../pwned" >/dev/null
    is "no pin escaped the sessions directory" "$([ -e "$SANDBOX/pwned" ] && echo escaped)" ""
    teardown
}

test_a_valid_session_id_still_pins() {
    sandbox
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    route_claude "$SANDBOX/ws-a" "--session-id=abc123" >/dev/null

    "$ACCT" use personal claude "$SANDBOX/ws-a" >/dev/null
    local got
    got=$(route_claude "$SANDBOX/ws-a" "--session-id=abc123")
    contains "a running session keeps its account" "$got" "/claude/work"
    teardown
}

test_a_corrupt_route_is_skipped_rather_than_followed() {
    sandbox
    fake_profile claude work
    printf '%s\t%s\n' "$SANDBOX/ws-a" "../../../../etc" > "$CONDUCTOR_ACCOUNTS_ROOT/routes"

    local got
    got=$(route_claude "$SANDBOX/ws-a")
    is "the agent is left on its default account" "$got" ""
    teardown
}

test_conductor_account_override_is_validated() {
    sandbox
    fake_profile claude work

    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_ACCOUNT="../../../../etc" \
        CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "a traversal override is ignored" "$got" ""
    teardown
}

test_conductor_account_override_still_works() {
    sandbox
    fake_profile claude work

    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_ACCOUNT=work \
        CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/target/release/claude-router" 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    contains "a valid override is honoured" "$got" "/claude/work"
    teardown
}
