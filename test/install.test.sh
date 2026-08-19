#!/bin/bash
# install tests for conductor-hats.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Turning routing on and off, and the CLI's own reports.

test_install_preserves_existing_conductor_settings() {
    cat > "$CONDUCTOR_ACCT_SETTINGS_FILE" <<'EOF'
"$schema" = "https://conductor.build/schemas/settings.schema.json"

[git]
branch_prefix = "feat/"
EOF
    "$ACCT" install >/dev/null
    local body
    body=$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")
    # shellcheck disable=SC2016  # $schema is a literal TOML key, not a variable
    contains "schema line survives" "$body" '$schema'
    contains "git table survives" "$body" 'branch_prefix = "feat/"'
    contains "router is wired" "$body" "claude_code_executable_path"
    local key_line table_line
    key_line=$(grep -n 'claude_code_executable_path' "$CONDUCTOR_ACCT_SETTINGS_FILE" | cut -d: -f1)
    table_line=$(grep -n '^\[git\]' "$CONDUCTOR_ACCT_SETTINGS_FILE" | cut -d: -f1)
    if [ "$key_line" -lt "$table_line" ]; then
        ok "written above the first table"
    else
        not_ok "written above the first table" "line < $table_line" "line $key_line"
    fi
}

test_install_replaces_a_stale_deployment() {
    fake_profile claude work
    "$ACCT" install >/dev/null
    rm -rf "${CONDUCTOR_ACCOUNTS_ROOT:?}/bin"
    ln -s "$PROJECT_DIR/bin" "$CONDUCTOR_ACCOUNTS_ROOT/bin"
    "$ACCT" install >/dev/null

    is "the deployment is a real directory" \
        "$([ -d "$CONDUCTOR_ACCOUNTS_ROOT/bin" ] && [ ! -L "$CONDUCTOR_ACCOUNTS_ROOT/bin" ] && echo yes)" "yes"
    is "and the deployed CLI is the current one" \
        "$(cmp -s "$PROJECT_DIR/bin/conductor-acct" "$CONDUCTOR_ACCOUNTS_ROOT/bin/conductor-acct" && echo same)" "same"
}

test_install_redeploys_after_the_checkout_changes() {
    fake_profile claude work
    "$ACCT" install >/dev/null
    printf '\n# drift\n' >> "$CONDUCTOR_ACCOUNTS_ROOT/bin/conductor-acct"
    "$ACCT" install >/dev/null
    is "a stale copy is overwritten" \
        "$(cmp -s "$PROJECT_DIR/bin/conductor-acct" "$CONDUCTOR_ACCOUNTS_ROOT/bin/conductor-acct" && echo same)" "same"
}

test_uninstall_reverses_install() {
    "$ACCT" install >/dev/null
    "$ACCT" uninstall >/dev/null
    is "no router path left" \
        "$(grep -c 'claude_code_executable_path' "$CONDUCTOR_ACCT_SETTINGS_FILE")" "0"
}

test_remove_drops_routes_pointing_at_the_profile() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    "$ACCT" use personal claude "$SANDBOX/ws-b" >/dev/null
    rm -rf "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    printf '%s\n' "$(grep -v "	work$" "$CONDUCTOR_ACCOUNTS_ROOT/routes")" > "$CONDUCTOR_ACCOUNTS_ROOT/routes"

    is "the other route is intact" \
        "$(route_claude "$SANDBOX/ws-b")" "$CONDUCTOR_ACCOUNTS_ROOT/claude/personal"
    is "the removed one resolves to nothing" "$(route_claude "$SANDBOX/ws-a")" ""
}

test_codex_router_swaps_codex_home() {
    fake_profile codex work
    "$ACCT" use work codex "$SANDBOX/ws-a" >/dev/null
    is "CODEX_HOME is set" \
        "$(route_codex "$SANDBOX/ws-a")" "$CONDUCTOR_ACCOUNTS_ROOT/codex/work"
}

test_which_reports_the_effective_account() {
    fake_profile claude work
    "$ACCT" install >/dev/null
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    local out
    out=$("$ACCT" which "$SANDBOX/ws-a")
    contains "names the route" "$out" "route:      work   (this workspace)"
    contains "names the effective account" "$out" "effective:  work"
}

test_status_is_two_lines() {
    fake_profile claude work
    "$ACCT" install >/dev/null
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    is "one line per agent plus the location" \
        "$("$ACCT" status "$SANDBOX/ws-a" | wc -l | tr -d ' ')" "2"
}

test_doctor_flags_a_dangling_route() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    rm -rf "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    contains "warns about the missing profile" "$("$ACCT" doctor 2>&1)" \
        "points at missing profile 'work'"
}

test_profile_names_are_validated() {
    local out status=0
    out=$("$ACCT" add "../escape" 2>&1) || status=$?
    is "rejected" "$status" "1"
    contains "with a reason" "$out" "may only contain letters"
}
