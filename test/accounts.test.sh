#!/bin/bash
# accounts tests for conductor-multi-account.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Signed-in state, and the one-account-one-profile rule.

test_signed_in_is_read_from_the_credentials_not_the_label() {
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/claude/quiet"
    printf '{"claudeAiOauth":{}}\n' > "$CONDUCTOR_ACCOUNTS_ROOT/claude/quiet/.credentials.json"
    contains "signed in with no address cached" "$("$ACCT" json "$SANDBOX/ws-a")" \
        '"name":"quiet","email":"","active":false,"signedIn":true'
    contains "list says so rather than calling it signed out" "$("$ACCT" list)" \
        "(signed in, address not cached yet)"

    fake_profile claude loud "loud@example.com"
    contains "and a labelled profile still reports its address" \
        "$("$ACCT" json "$SANDBOX/ws-a")" '"email":"loud@example.com"'
}

test_a_profile_with_nothing_is_not_signed_in() {
    mkdir -p "$CONDUCTOR_ACCOUNTS_ROOT/claude/empty"
    contains "reported as signed out" "$("$ACCT" json "$SANDBOX/ws-a")" \
        '"name":"empty","email":"","active":false,"signedIn":false'
    contains "doctor warns" "$("$ACCT" doctor 2>&1)" "profile 'empty' is not signed in"
}

# Two profiles on one account is not two accounts: the provider keeps one live
# token per account, so the second sign-in revokes the first and the pair take
# turns logging each other out. The symptom is an account asking to sign in
# minutes after it did.
test_two_profiles_on_one_address_are_flagged() {
    fake_profile claude hello "same@example.com"
    fake_profile claude personal "same@example.com"
    fake_profile claude work "other@example.com"
    local out
    out=$("$ACCT" doctor 2>&1)
    contains "doctor names the shared address" "$out" "share the address same@example.com"
    contains "and says why it matters" "$out" "sign each other out"
    is "the unshared one is not flagged" "$(printf '%s' "$out" | grep -c 'other@example.com')" "0"
}

# The CLI and the injected panel ship together, so a version skew between them is
# a bug rather than a variation. Cheap to assert, and it has already drifted once.
test_the_cli_and_the_panel_agree_on_the_version() {
    local cli panel
    cli=$("$ACCT" version | awk '{print $2}')
    panel=$(sed -n 's/^const VERSION = "\([^"]*\)";/\1/p' "$UI_SRC_DIR/index.ts")
    is "same version" "$cli" "$panel"
    contains "and the changelog has an entry for it" "$(cat "$PROJECT_DIR/CHANGELOG.md")" "## $cli"
}

# This is published, so an address or a home directory left in a file is a leak
# rather than an untidiness. Both rules are stated positively so the test itself
# carries no personal data: every example address must sit on a domain RFC 2606
# reserves for documentation, and no path may name a real account.
