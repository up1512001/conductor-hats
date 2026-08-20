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

# A command that signs must never report a bad signature and still exit 0: a
# broken application would read as installed.
test_patching_a_bogus_app_fails_loudly() {
    sandbox
    local app="$SANDBOX/Fake.app"
    mkdir -p "$app/Contents/MacOS"
    printf 'not a mach-o binary at all' > "$app/Contents/MacOS/conductor"

    local out status=0
    out=$("$ACCT" patch --app "$app" --i-know 2>&1) || status=$?
    not_zero "patch refuses and exits non-zero" "$status"
    is "no success line was printed" "$(printf '%s' "$out" | grep -c 'signature valid')" "0"
    teardown
}

test_reverting_without_a_backup_fails_loudly() {
    sandbox
    local app="$SANDBOX/Fake.app"
    mkdir -p "$app/Contents/MacOS"
    printf 'nothing' > "$app/Contents/MacOS/conductor"

    local status=0
    "$ACCT" revert --app "$app" >/dev/null 2>&1 || status=$?
    not_zero "revert refuses and exits non-zero" "$status"
    teardown
}

# A sign-out that failed means the provider still holds a live session. Deleting
# the local profile at that point throws away the only record of it.
failing_agent() {
    cat > "$SANDBOX/stub-claude" <<'STUB'
#!/bin/sh
echo "refusing" >&2
exit 3
STUB
    chmod +x "$SANDBOX/stub-claude"
}

test_remove_refuses_when_sign_out_fails() {
    sandbox
    fake_profile claude work
    printf '{"x":1}' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.credentials.json"
    failing_agent

    local out status=0
    out=$("$ACCT" remove work claude 2>&1) || status=$?
    not_zero "remove refuses" "$status"
    contains "and says the exit status" "$out" "exited with status 3"
    is "the profile is still there" "$([ -d "$CONDUCTOR_ACCOUNTS_ROOT/claude/work" ] && echo yes)" "yes"
    teardown
}

test_remove_force_deletes_and_warns() {
    sandbox
    fake_profile claude work
    printf '{"x":1}' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.credentials.json"
    failing_agent

    local out status=0
    out=$("$ACCT" remove work claude --force 2>&1) || status=$?
    is "remove succeeds" "$status" "0"
    contains "with a warning" "$out" "may still consider this account signed in"
    is "the profile is gone" "$([ -d "$CONDUCTOR_ACCOUNTS_ROOT/claude/work" ] && echo yes)" ""
    teardown
}

test_logout_reports_a_refusal_rather_than_claiming_success() {
    sandbox
    fake_profile claude work
    printf '{"x":1}' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.credentials.json"
    failing_agent

    local out status=0
    out=$("$ACCT" logout work claude 2>&1) || status=$?
    not_zero "logout fails" "$status"
    contains "naming what happened" "$out" "signing 'work' out failed"
    teardown
}

test_logout_of_a_signed_out_profile_is_not_an_error() {
    sandbox
    fake_profile claude work

    local out status=0
    out=$("$ACCT" logout work claude 2>&1) || status=$?
    is "it succeeds" "$status" "0"
    contains "and says why there was nothing to do" "$out" "nothing to sign out of"
    teardown
}

test_remove_of_a_signed_out_profile_still_works() {
    sandbox
    fake_profile claude work
    mkdir -p "$SANDBOX/ws-a"
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    local status=0
    "$ACCT" remove work claude >/dev/null 2>&1 || status=$?
    is "it succeeds" "$status" "0"
    is "the profile is gone" "$([ -d "$CONDUCTOR_ACCOUNTS_ROOT/claude/work" ] && echo yes)" ""
    teardown
}
