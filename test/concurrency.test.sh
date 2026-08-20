#!/bin/bash
# Every workspace shares one routes file, so changing account in two of them at
# once used to be a race: each process read the file, edited its copy and wrote
# the whole thing back, and the slower write erased the faster one.
#
# These are deliberately larger than any real workload. A lost update is rare
# enough that a handful of writers would pass by luck.

test_a_hundred_concurrent_route_writes_all_survive() {
    sandbox
    fake_profile claude work
    local i
    for i in $(seq 1 100); do
        mkdir -p "$SANDBOX/w$i"
    done
    for i in $(seq 1 100); do
        "$ACCT" use work claude "$SANDBOX/w$i" >/dev/null 2>&1 &
    done
    wait

    local routes="$CONDUCTOR_ACCOUNTS_ROOT/routes"
    local found
    found=$(grep -c "	work$" "$routes")
    is "every route was written" "$found" "100"

    local unique
    unique=$(grep "	work$" "$routes" | cut -f1 | sort -u | wc -l | tr -d ' ')
    is "and none was written twice" "$unique" "100"
    teardown
}

test_concurrent_writes_leave_no_partial_lines() {
    sandbox
    fake_profile claude work
    local i
    for i in $(seq 1 60); do
        mkdir -p "$SANDBOX/w$i"
    done
    for i in $(seq 1 60); do
        "$ACCT" use work claude "$SANDBOX/w$i" >/dev/null 2>&1 &
    done
    wait

    local malformed
    malformed=$(grep -v '^#' "$CONDUCTOR_ACCOUNTS_ROOT/routes" | grep -v '^$' |
        grep -cv '^/.*	work$' || true)
    is "no truncated or interleaved line" "$malformed" "0"
    teardown
}

test_an_unrelated_route_is_not_lost_by_a_concurrent_write() {
    sandbox
    fake_profile claude work
    fake_profile claude personal
    mkdir -p "$SANDBOX/keep"
    "$ACCT" use personal claude "$SANDBOX/keep" >/dev/null

    local i
    for i in $(seq 1 50); do
        mkdir -p "$SANDBOX/w$i"
    done
    for i in $(seq 1 50); do
        "$ACCT" use work claude "$SANDBOX/w$i" >/dev/null 2>&1 &
    done
    wait

    local kept
    kept=$(grep -c "^$SANDBOX/keep	personal$" "$CONDUCTOR_ACCOUNTS_ROOT/routes")
    is "the route nobody touched is still there" "$kept" "1"
    teardown
}

test_concurrent_session_pins_do_not_collide() {
    sandbox
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    local i
    for i in $(seq 1 50); do
        route_claude "$SANDBOX/ws-a" "--session-id=s$i" >/dev/null 2>&1 &
    done
    wait

    local pins
    pins=$(find "$CONDUCTOR_ACCOUNTS_ROOT/sessions/claude" -type f | wc -l | tr -d ' ')
    is "every session got its own pin" "$pins" "50"

    local wrong
    wrong=$(cat "$CONDUCTOR_ACCOUNTS_ROOT"/sessions/claude/* | grep -cv '^work$' || true)
    is "and every pin names the routed account" "$wrong" "0"
    teardown
}

test_a_route_change_racing_a_pin_leaves_both_readable() {
    sandbox
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    local i
    for i in $(seq 1 25); do
        route_claude "$SANDBOX/ws-a" "--session-id=p$i" >/dev/null 2>&1 &
        "$ACCT" use personal claude "$SANDBOX/ws-b" >/dev/null 2>&1 &
    done
    wait

    local out status=0
    out=$("$ACCT" list 2>&1) || status=$?
    is "the state still reads back" "$status" "0"
    contains "with the racing route intact" "$out" "$SANDBOX/ws-b	personal"
    teardown
}

test_removing_an_account_takes_its_routes_with_it() {
    sandbox
    fake_profile claude work
    fake_profile claude personal
    local i
    for i in $(seq 1 30); do
        mkdir -p "$SANDBOX/w$i"
        "$ACCT" use work claude "$SANDBOX/w$i" >/dev/null
    done
    mkdir -p "$SANDBOX/other"
    "$ACCT" use personal claude "$SANDBOX/other" >/dev/null

    "$ACCT" remove work claude >/dev/null 2>&1

    local dangling
    dangling=$(grep -c "	work$" "$CONDUCTOR_ACCOUNTS_ROOT/routes" || true)
    is "no route points at the removed account" "$dangling" "0"
    local kept
    kept=$(grep -c "	personal$" "$CONDUCTOR_ACCOUNTS_ROOT/routes")
    is "the other account keeps its route" "$kept" "1"
    teardown
}
