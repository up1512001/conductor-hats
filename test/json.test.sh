#!/bin/bash
# The panel reads `hats json`, and the address shown in it comes out of the
# provider's own state file. Both used to be handled by looking for quotes and
# colons in a string, which is right until a value contains one.

test_an_address_is_read_from_the_field_not_the_first_match() {
    sandbox
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    cat > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.claude.json" <<'EOF'
{
  "note": "the key \"emailAddress\" is documented elsewhere",
  "oauthAccount": { "emailAddress": "real@example.test" }
}
EOF
    printf '{"x":1}' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.credentials.json"
    "$ACCT" login work claude >/dev/null 2>&1

    contains "the real address is cached" \
        "$(cat "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.label")" "real@example.test"
    teardown
}

test_a_corrupt_state_file_yields_no_address_rather_than_a_wrong_one() {
    sandbox
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    printf '{"oauthAccount": {"emailAddress": "trunc' \
        > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.claude.json"

    local out status=0
    out=$("$ACCT" login work claude 2>&1) || status=$?
    is "login still reports what it did" "$status" "0"
    is "and nothing was cached" "$([ -f "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.label" ] && echo yes)" ""
    teardown
}

test_json_stays_parseable_when_a_path_contains_a_quote() {
    sandbox
    fake_profile claude work
    local odd="$SANDBOX/say \"hi\""
    mkdir -p "$odd"

    local out
    out=$("$ACCT" json "$odd" 2>/dev/null)
    ok_if "the output parses as JSON" "printf '%s' '$out' | python3 -c 'import json,sys; json.load(sys.stdin)'"
    teardown
}

test_json_keeps_the_field_names_the_panel_expects() {
    sandbox
    fake_profile claude work

    local out
    out=$("$ACCT" json "$SANDBOX/ws-a" 2>/dev/null)
    local keys
    keys=$(printf '%s' "$out" | python3 -c '
import json, sys
state = json.load(sys.stdin)
top = sorted(state)
provider = sorted(state["providers"][0])
account = sorted(state["providers"][0]["accounts"][0])
print(",".join(top + provider + account))
')
    is "the contract is unchanged" "$keys" \
        "enabled,providers,repo,workspace,accounts,agent,current,active,email,name,signedIn"
    teardown
}

test_an_address_with_a_backslash_survives_the_round_trip() {
    sandbox
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    printf 'back\\slash@example.test\n' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/work/.label"

    local out
    out=$("$ACCT" json "$SANDBOX/ws-a" 2>/dev/null)
    ok_if "the output still parses" "printf '%s' '$out' | python3 -c 'import json,sys; json.load(sys.stdin)'"
    teardown
}
