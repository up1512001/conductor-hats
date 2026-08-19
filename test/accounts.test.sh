#!/bin/bash
# accounts tests for conductor-hats.
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

# This is published, so an address or a home directory left in a file is a leak
# rather than an untidiness. Both rules are stated positively so the test itself
# carries no personal data: every example address must sit on a domain RFC 2606
# reserves for documentation, and no path may name a real account.

# The README says any number of accounts, so that claim gets a test rather than a
# hope. Nothing in the design caps it: profiles are directories, routes are lines
# in a file, and each config directory gets its own keychain item. This checks the
# whole chain at five, across five workspaces, which is more than anyone has asked
# for and enough to catch a two-account assumption.
test_any_number_of_accounts_route_independently() {
    local names="alpha bravo charlie delta echo" n ws wrong=0
    for n in $names; do
        fake_profile claude "$n" "$n@example.com"
        ws="$SANDBOX/ws-$n"
        mkdir -p "$ws"
        "$ACCT" use "$n" claude "$ws" >/dev/null
    done

    for n in $names; do
        ws="$SANDBOX/ws-$n"
        if [ "$(route_claude "$ws")" != "$CONDUCTOR_ACCOUNTS_ROOT/claude/$n" ]; then
            echo "        ws-$n resolved to $(route_claude "$ws")"
            wrong=$((wrong + 1))
        fi
    done
    is "all five workspaces route to their own account" "$wrong" "0"

    local listed
    listed=$("$ACCT" json "$SANDBOX/ws-alpha")
    for n in $names; do
        case "$listed" in
            *"\"name\":\"$n\""*) ;;
            *) not_ok "json lists $n" "an entry for $n" "$listed"; return ;;
        esac
    done
    ok "json lists every one of them"

    is "and the routes file has one line each" \
        "$(grep -c "$SANDBOX/ws-" "$CONDUCTOR_ACCOUNTS_ROOT/routes")" "5"
}
