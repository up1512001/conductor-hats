#!/bin/bash
# Test suite for conductor-multi-account.
#
# Everything runs against a sandbox under $TMPDIR with a stub agent binary, so
# no real Conductor install, ~/.claude directory or keychain item is touched.
#
#   test/run.sh            run everything
#   test/run.sh route      run tests whose name contains "route"
set -uo pipefail

SUITE_DIR=$(cd "$(dirname "$0")" && pwd)
PROJECT_DIR=$(dirname "$SUITE_DIR")
ACCT="$PROJECT_DIR/bin/conductor-acct"
FILTER="${1:-}"

PASS=0
FAIL=0


# --------------------------------------------------------------- harness ---

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

# ----------------------------------------------------------------- tests ---

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
    # Conductor injects this before the router runs.
    local bound="$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CLAUDE_CONFIG_DIR="$bound" "$PROJECT_DIR/bin/claude-router" 2>/dev/null |
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
        "$PROJECT_DIR/bin/claude-router" 2>/dev/null | sed -n 's/^CLAUDE_CONFIG_DIR=//p')
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
        "$PROJECT_DIR/bin/claude-router" 2>/dev/null | sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "an inherited route yields to an explicit binding" \
        "$got" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_env_override_wins_over_everything() {
    fake_profile claude work
    fake_profile claude personal
    "$ACCT" use personal claude "$SANDBOX/ws-a" >/dev/null
    local got
    got=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CONDUCTOR_ACCOUNT=work "$PROJECT_DIR/bin/claude-router" 2>/dev/null |
        sed -n 's/^CLAUDE_CONFIG_DIR=//p')
    is "CONDUCTOR_ACCOUNT forces a profile" "$got" "$CONDUCTOR_ACCOUNTS_ROOT/claude/work"
}

test_router_forwards_argv_untouched() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    local argv
    argv=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$PROJECT_DIR/bin/claude-router" --output-format stream-json \
        --session-id=abc123 --model opus 2>/dev/null | sed -n 's/^ARGV=//p')
    is "argv survives" "$argv" "--output-format stream-json --session-id=abc123 --model opus"
}

test_router_passes_through_with_nothing_configured() {
    is "no profiles, no change" "$(route_claude "$SANDBOX/ws-a")" ""
}

test_router_fails_open_when_the_library_is_broken() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null

    # A copy of the shipped router next to a deliberately broken library.
    mkdir -p "$SANDBOX/broken"
    cp "$PROJECT_DIR/bin/claude-router" "$SANDBOX/broken/"
    printf 'this is not ( valid shell\n' > "$SANDBOX/broken/_resolve.sh"

    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        "$SANDBOX/broken/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
    is "and gets no config dir rather than a wrong one" \
        "$(printf '%s\n' "$out" | sed -n 's/^CLAUDE_CONFIG_DIR=//p')" ""
}

test_router_fails_open_when_the_library_is_missing() {
    mkdir -p "$SANDBOX/lonely"
    cp "$PROJECT_DIR/bin/claude-router" "$SANDBOX/lonely/"
    local out
    out=$(cd "$SANDBOX/ws-a" && "$SANDBOX/lonely/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
}

test_router_refuses_to_route_into_itself() {
    local status=0
    (cd "$SANDBOX/ws-a" && CONDUCTOR_ACCOUNTS_DEPTH=2 \
        "$PROJECT_DIR/bin/claude-router" >/dev/null 2>&1) || status=$?
    is "a real loop exits 70" "$status" "70"
}

test_router_tolerates_one_inherited_generation() {
    fake_profile claude work
    "$ACCT" use work claude "$SANDBOX/ws-a" >/dev/null
    # Launching Conductor from a shell inside a routed session leaks these into
    # the app, and from there into every agent it starts.
    local out
    out=$(cd "$SANDBOX/ws-a" && CONDUCTOR_WORKSPACE_PATH="$SANDBOX/ws-a" \
        CONDUCTOR_ACCOUNTS_ROUTING=claude CONDUCTOR_ACCOUNTS_DEPTH=1 \
        "$PROJECT_DIR/bin/claude-router" --model opus 2>/dev/null)
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
        "$PROJECT_DIR/bin/claude-router" --model opus 2>/dev/null)
    contains "the agent still starts" "$out" "ARGV=--model opus"
    is "on the default account" \
        "$(printf '%s\n' "$out" | sed -n 's/^CLAUDE_CONFIG_DIR=//p')" ""
}

test_session_pin_survives_a_route_change() {
    fake_profile claude work
    fake_profile claude personal
    # What the router recorded when this session first started.
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

test_install_preserves_existing_conductor_settings() {
    cat > "$CONDUCTOR_ACCT_SETTINGS_FILE" <<'EOF'
"$schema" = "https://conductor.build/schemas/settings.schema.json"

[git]
branch_prefix = "feat/"
EOF
    "$ACCT" install >/dev/null
    local body
    body=$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")
    contains "schema line survives" "$body" '$schema'
    contains "git table survives" "$body" 'branch_prefix = "feat/"'
    contains "router is wired" "$body" "claude_code_executable_path"
    # Top-level keys must land above the first table or the TOML is invalid.
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
    # Simulate an older layout, where the deployed path was a symlink.
    rm -rf "$CONDUCTOR_ACCOUNTS_ROOT/bin"
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
    # No sign-out round trip: the stub agent has nothing to sign out of.
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

# ------------------------------------------------------- the injected UI ---
#
# The panel cannot be driven from a shell, but its two worst failures were both
# visible in the source, so they are guarded there. A broken bundle is expensive
# to find out about: it means patching a Conductor, launching it and clicking.

UI_JS="$PROJECT_DIR/tools/ui-patch/account-ui.js"

test_the_injected_ui_parses() {
    command -v node >/dev/null || { skip "node is not installed"; return; }
    local out status=0
    out=$(node --check "$UI_JS" 2>&1) || status=$?
    is "node --check is happy" "$status" "0"
    [ "$status" -eq 0 ] || printf '        %s\n' "$out"
}

# Sealing pointer events on the capture phase stopped the click before it ever
# reached the row that was clicked: every account row went inert and the panel
# stopped opening at all. The seal has to be on the bubble phase, after the
# panel's own handlers have run.
test_the_panel_seals_pointer_events_on_the_bubble_phase() {
    local body
    body=$(sed -n '/^  function seal(/,/^  }/p' "$UI_JS")
    contains "listener is registered non-capturing" "$body" "}, false);"
    is "and nothing in seal captures" "$(printf '%s' "$body" | grep -c 'true)')" "0"
}

test_every_clickable_thing_says_it_is_clickable() {
    local css
    css=$(sed -n '/^  var CSS = \[/,/\].join("");/p' "$UI_JS")
    local sel
    for sel in ".cma-btn,.cma-chip" ".cma-card" ".cma-trash" ".cma-back" ".cma-add" ".cma-go" ".cma-act"; do
        contains "$sel exists" "$css" "$sel"
    done
    is "no interactive rule left on the arrow cursor" \
        "$(printf '%s' "$css" | grep -c 'cursor:default')" "1"
}

# The wireframe is a drill-down: providers first, then that provider's accounts
# with a delete each and one "Add new account" at the foot.
test_the_panel_is_a_two_level_drill_down() {
    local body
    body=$(cat "$UI_JS")
    contains "a root view" "$body" "function rootView("
    contains "a provider view" "$body" "function providerView("
    contains "a back control" "$body" 'el("button", "cma-back")'
    contains "add at the foot of the provider view" "$body" '"Add new account"'
    contains "a named delete confirmation" "$body" "function confirmDelete("
    contains "escape steps back before it closes" "$body" 'open.view.level === "provider"'
}

# ------------------------------------------------------------------ main ---

for t in $(declare -F | sed -n 's/^declare -f \(test_.*\)$/\1/p'); do
    run_test "$t"
done

echo
if [ "$FAIL" -eq 0 ]; then
    echo "$PASS passed"
else
    echo "$PASS passed, $FAIL failed"
    exit 1
fi
