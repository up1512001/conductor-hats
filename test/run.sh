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
    # shellcheck disable=SC2016  # $schema is a literal TOML key, not a variable
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
    for sel in ".cma-btn,.cma-chip" ".cma-card" ".cma-signout" ".cma-back" ".cma-add" ".cma-go" ".cma-act"; do
        contains "$sel exists" "$css" "$sel"
    done
    # cursor:default is only ever right on something that cannot be clicked: a
    # disabled control, or a loading placeholder.
    is "the arrow cursor is only on unclickable things" \
        "$(printf '%s\n' "$css" | tr ',' '\n' | grep 'cursor:default' |
           grep -vc -e ':disabled' -e 'cma-ghost')" "0"
}

MASK_CASES="someone.long@example.com joe@mail.example.com ab@x.test a@b.test someone.else@example.org
first.last@example.com noatsign x@y"

test_masking_never_reveals_a_whole_part() {
    local addr out part head leaked=0
    for addr in $MASK_CASES; do
        out=$("$ACCT" mask "$addr")
        case "$out" in
            *'**'*) ;;
            *) not_ok "$addr is masked at all" "something with **" "$out"; return ;;
        esac
        # No local part or host may survive intact.
        for part in ${addr//@/ }; do
            head=${part%%.*}
            [ ${#head} -gt 2 ] || continue
            case "$out" in
                *"$head"*) echo "        leaked '$head' in $out"; leaked=1 ;;
            esac
        done
    done
    is "no part survives intact" "$leaked" "0"
}

# The panel cannot shell out once per row, so the rule exists twice. A test is
# cheaper than a refactor and catches the only thing that actually matters.
test_the_shell_and_the_panel_mask_identically() {
    command -v node >/dev/null || { skip "node is not installed"; return; }
    local addr from_sh from_js differed=0
    for addr in $MASK_CASES; do
        from_sh=$("$ACCT" mask "$addr")
        from_js=$(node -e '
            var fs = require("fs");
            var src = fs.readFileSync(process.argv[1], "utf8");
            var fns = src.match(/function maskPart[\s\S]*?\n  }\n/)[0] +
                      src.match(/function maskEmail[\s\S]*?\n  }\n/)[0];
            eval(fns.replace(/^  /gm, ""));
            process.stdout.write(maskEmail(process.argv[2]));
        ' "$PROJECT_DIR/tools/ui-patch/account-ui.js" "$addr")
        if [ "$from_sh" != "$from_js" ]; then
            echo "        $addr: shell '$from_sh' vs panel '$from_js'"
            differed=1
        fi
    done
    is "both maskers agree on every case" "$differed" "0"
}

test_mask_is_opt_in_for_the_terminal() {
    fake_profile claude work "person@example.com"
    contains "list shows the real address" "$("$ACCT" list)" "person@example.com"
    contains "list --mask does not" "$("$ACCT" list --mask)" "pe**n@ex**e.com"
    is "and the real one is absent when masked" \
        "$("$ACCT" list --mask | grep -c 'person@example.com')" "0"
}

# Nothing under the pointer may move once the panel is open. Four things make
# that true, and each one was a visible jump before it was there.
test_the_panel_cannot_shift_once_it_is_open() {
    local body css
    body=$(cat "$UI_JS")
    css=$(sed -n '/^  var CSS = \[/,/\].join("");/p' "$UI_JS")
    contains "the corner is placed once and reused" "$body" "if (open && open.pos)"
    contains "width is fixed, not content driven" "$css" "width:300px;box-sizing:border-box"
    contains "a long list scrolls instead of growing" "$css" "overflow-y:auto"
    contains "the card flexes beside its delete control" "$css" ".cma-row2 .cma-card{flex:1"
    contains "the tick has a slot of its own" "$css" ".cma-tickslot"
    contains "and the slot is always in the flow" "$body" 'el("div", "cma-tickslot")'
    contains "triggers stay hidden until labelled" "$body" "btn.hidden = true"
}

# A dot next to a label is decoration standing in for a word. The label says it.
test_no_status_dots_anywhere() {
    is "no dot element" "$(grep -c 'cma-dot' "$UI_JS")" "0"
}

# Profile names are lower case on disk and capitalised only for display.
test_display_names_are_capitalised_without_touching_the_cli() {
    local body
    body=$(cat "$UI_JS")
    contains "a display-only helper" "$body" "function cap(s)"
    contains "account rows use it" "$body" "cap(account.name)"
    contains "the provider badge uses it" "$body" "cap(provider.current)"
    contains "the trigger label uses it" "$body" "cap(cur)"
    is "and writes still send the raw name" \
        "$(printf '%s' "$body" | grep -c 'applyAccount(state, provider.agent, account.name)')" "1"
    is "as does sign-out" \
        "$(printf '%s' "$body" | grep -c 'acct("logout " + account.name')" "1"
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
    contains "a named sign-out confirmation" "$body" "function confirmSignOut("
    contains "escape steps back before it closes" "$body" 'open.view.level === "provider"'
}

# The panel signs an account out and touches nothing else. Deleting a profile
# outright stays in the terminal, where an accidental click cannot reach it.
test_the_panel_signs_out_and_deletes_nothing() {
    local body
    body=$(cat "$UI_JS")
    contains "it calls logout" "$body" 'acct("logout " + account.name'
    is "and never remove" "$(printf '%s' "$body" | grep -c 'acct("remove ')" "0"
    contains "the copy says nothing else changes" "$body" "Nothing else changes"
    contains "it names what survives" "$body" "routes, sessions and transcripts are untouched"
    contains "the icon is a sign-out, not a bin" "$body" 'icon("signout"'
    is "no bin glyph is left" "$(printf '%s' "$body" | grep -c '^    trash: \[')" "0"
    # Nothing to sign out of when the profile has no credentials, and a
    # signed-out row gets a way back in instead of a dead end.
    contains "offered only when signed in" "$body" "if (account.signedIn) {"
    contains "and signed-out rows offer sign-in" "$body" 'icon("signin"'
}

# Sign-out still costs a browser round trip to undo, so it asks in a dialog with
# a scrim, not a control that arms on a first click.
test_sign_out_asks_in_a_dialog() {
    local body css
    body=$(cat "$UI_JS")
    css=$(sed -n '/^  var CSS = \[/,/\].join("");/p' "$UI_JS")
    contains "a reusable dialog" "$body" "function dialog(opts)"
    contains "with a scrim" "$css" ".cma-scrim{position:fixed;inset:0"
    contains "announced as a modal alert" "$body" '"alertdialog"'
    contains "escape cancels it" "$body" 'if (e.key === "Escape") { e.stopPropagation(); shut(); }'
    contains "the scrim cancels, the box does not" "$body" "if (e.target === scrim) shut()"
    contains "and it says what will happen" "$body" "Signs " 
    # A dialog is a sibling of the panel, so clicking it must not read as
    # clicking away from the panel.
    contains "the panel ignores clicks while it is open" "$body" "if (!open || openDialog) return"
}

# The sign-out control lives inside the row's border, divided from the selectable
# area, rather than floating in the gutter beside it.
test_sign_out_sits_inside_the_row() {
    local css
    css=$(sed -n '/^  var CSS = \[/,/\].join("");/p' "$UI_JS")
    contains "the row carries the border" "$css" ".cma-row2{display:flex"
    contains "the card inside it does not" "$css" ".cma-row2 .cma-card{flex:1;min-width:0;width:auto;margin:0;border:0"
    contains "a divider before the control" "$css" "border-left:1px solid var(--border)"
    contains "full height of the row" "$css" "align-self:stretch"
}

test_the_panel_never_renders_a_full_address() {
    local body
    body=$(cat "$UI_JS")
    contains "rows mask" "$body" "maskEmail(account.email)"
    contains "the sign-out dialog masks" "$body" "account.email ? maskEmail(account.email) : cap(account.name)"
    contains "sign-in confirmation masks" "$body" '"Signed in as " + maskEmail(email)'
    # A tooltip is as visible on video as the text is.
    is "no address in a title attribute" \
        "$(printf '%s' "$body" | grep -c 'title = .*account\.email')" "0"
}

# Signed-in state has to come from where the credentials are, not from a cached
# address. The old check was "does .label exist", and .label is only written when
# an address can be read out of .claude.json, which does not always happen the
# moment a sign-in finishes. A profile with working credentials then read as
# signed out for ever, and the panel offered to sign in an account that already
# was.
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
    panel=$(sed -n 's/.*__conductorMultiAccount = { version: "\([^"]*\)".*/\1/p' "$UI_JS")
    is "same version" "$cli" "$panel"
    contains "and the changelog has an entry for it" "$(cat "$PROJECT_DIR/CHANGELOG.md")" "## $cli"
}

# This is published, so an address or a home directory left in a file is a leak
# rather than an untidiness. Both rules are stated positively so the test itself
# carries no personal data: every example address must sit on a domain RFC 2606
# reserves for documentation, and no path may name a real account.
test_no_personal_information_is_committed() {
    local files bad
    files=$(cd "$PROJECT_DIR" && git ls-files 2>/dev/null)
    if [ -z "$files" ]; then skip "not a git checkout"; return; fi

    # Addresses on any domain other than the reserved ones.
    bad=$(cd "$PROJECT_DIR" && printf '%s\n' "$files" | while read -r f; do
        [ -f "$f" ] || continue
        grep -HoE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "$f" 2>/dev/null |
            grep -vE '@(example\.(com|org|net)|[A-Za-z0-9.-]*\.(test|example|invalid|localhost))(\b|$)'
    done)
    if [ -n "$bad" ]; then
        not_ok "example addresses use reserved domains" "only example.com/org/net and .test" "$bad"
    else
        ok "example addresses use reserved domains"
    fi

    # Home directories of a real account, as opposed to ~ or /Users/you.
    bad=$(cd "$PROJECT_DIR" && printf '%s\n' "$files" | while read -r f; do
        [ -f "$f" ] || continue
        grep -HoE '/Users/[A-Za-z0-9._-]+' "$f" 2>/dev/null | grep -vE '/Users/(you|USER|username)\b'
    done)
    if [ -n "$bad" ]; then
        not_ok "no real home directories" "~ or /Users/you" "$bad"
    else
        ok "no real home directories"
    fi
}

# AGENTS.md says no file over 300 lines, so the rule is enforced rather than
# asserted. The three files that break it today are listed with the length they
# are at, which means the debt cannot quietly grow: adding a line to one of them
# fails this test until it is split.
LINE_LIMIT=300
KNOWN_LONG="bin/conductor-acct:1360 test/run.sh:834 tools/ui-patch/account-ui.js:1294"

test_no_file_exceeds_the_line_limit() {
    local files f n allowed entry over=0 grew=0
    files=$(cd "$PROJECT_DIR" && git ls-files 2>/dev/null)
    if [ -z "$files" ]; then skip "not a git checkout"; return; fi

    for f in $files; do
        case "$f" in dist/*|pnpm-lock.yaml|LICENSE) continue ;; esac
        [ -f "$PROJECT_DIR/$f" ] || continue
        n=$(wc -l < "$PROJECT_DIR/$f" | tr -d ' ')
        [ "$n" -gt "$LINE_LIMIT" ] || continue

        allowed=""
        for entry in $KNOWN_LONG; do
            [ "${entry%%:*}" = "$f" ] && allowed="${entry##*:}"
        done
        if [ -z "$allowed" ]; then
            echo "        $f is $n lines, limit is $LINE_LIMIT"
            over=1
        elif [ "$n" -gt "$allowed" ]; then
            echo "        $f grew to $n lines, was $allowed and already over the limit"
            grew=1
        fi
    done

    is "nothing new is over the limit" "$over" "0"
    is "the files already over it did not grow" "$grew" "0"
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
