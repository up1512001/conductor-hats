#!/bin/bash
# bindings tests for conductor-hats.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Repository bindings, which Conductor applies itself and the router never sees.

test_bind_writes_repo_settings() {
    fake_profile claude work
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    local f="$SANDBOX/repo/.conductor/settings.local.toml"
    contains "table header" "$(cat "$f")" "[environment_variables]"
    contains "the config dir" "$(cat "$f")" \
        "CLAUDE_CONFIG_DIR = \"$CONDUCTOR_ACCOUNTS_ROOT/claude/work\""
}

test_bind_preserves_other_settings() {
    fake_profile claude work
    cat > "$SANDBOX/repo/.conductor/settings.local.toml" <<'EOF'
[scripts]
setup = "pnpm install"

[environment_variables]
DATABASE_URL = "postgres://localhost/dev"
EOF
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    local body
    body=$(cat "$SANDBOX/repo/.conductor/settings.local.toml")
    contains "scripts survive" "$body" 'setup = "pnpm install"'
    contains "other env vars survive" "$body" 'DATABASE_URL = "postgres://localhost/dev"'
    contains "the binding is added" "$body" "CLAUDE_CONFIG_DIR"
    is "no duplicate table" "$(grep -c '^\[environment_variables\]' "$SANDBOX/repo/.conductor/settings.local.toml")" "1"
}

test_bind_replaces_rather_than_appends() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    "$ACCT" bind personal claude "$SANDBOX/repo" >/dev/null
    is "one key" \
        "$(grep -c 'CLAUDE_CONFIG_DIR' "$SANDBOX/repo/.conductor/settings.local.toml")" "1"
    contains "the newer profile" "$(cat "$SANDBOX/repo/.conductor/settings.local.toml")" \
        "claude/personal"
}

# The router never exports a repository binding, because Conductor applies the
# repo's [environment_variables] itself. Reporting only the dry run therefore
# hid the binding the moment the router was installed, and the New Workspace
# chip read "default account" for a repository that was firmly bound.
test_a_repo_binding_is_reported_while_the_router_is_on() {
    fake_profile claude work
    "$ACCT" install >/dev/null
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    contains "json names the bound account" "$("$ACCT" json "$SANDBOX/repo")" '"current":"work"'
    contains "status names it too" "$("$ACCT" status "$SANDBOX/repo")" "work"
    contains "which calls it effective" "$("$ACCT" which "$SANDBOX/repo")" "effective:  work"
    contains "check answers with it" "$("$ACCT" check "$SANDBOX/repo")" "ACCOUNT claude work"
}

test_a_workspace_route_still_wins_in_json() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" install >/dev/null
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    mkdir -p "$SANDBOX/repo/ws"
    "$ACCT" use personal claude "$SANDBOX/repo/ws" >/dev/null
    contains "the route, not the binding" "$("$ACCT" json "$SANDBOX/repo/ws")" '"current":"personal"'
}

test_unbind_leaves_the_rest_of_the_file() {
    fake_profile claude work
    cat > "$SANDBOX/repo/.conductor/settings.local.toml" <<'EOF'
[environment_variables]
DATABASE_URL = "postgres://localhost/dev"
EOF
    "$ACCT" bind work claude "$SANDBOX/repo" >/dev/null
    "$ACCT" unbind claude "$SANDBOX/repo" >/dev/null
    local body
    body=$(cat "$SANDBOX/repo/.conductor/settings.local.toml")
    contains "unrelated var survives" "$body" "DATABASE_URL"
    is "binding is gone" "$(grep -c 'CLAUDE_CONFIG_DIR' "$SANDBOX/repo/.conductor/settings.local.toml")" "0"
}
