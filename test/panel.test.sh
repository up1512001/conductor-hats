#!/bin/bash
# panel tests for conductor-hats.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# The injected panel. Behaviour is checked against the built artifact, because
# that is what runs; structure and style rules against the source.

# ------------------------------------------------------- the injected UI ---
#
# The panel cannot be driven from a shell, so these check the two things a shell
# can see. Behaviour is asserted against the built artifact, because that is what
# actually gets injected. Structure and style rules are asserted against the
# source. Finding a broken bundle the other way costs a patch, a launch and a
# click.

UI_SRC_DIR="$PROJECT_DIR/src/panel"
UI_DIST="$PROJECT_DIR/dist/account-ui.js"

# Structure and styles are each spread over several files now, so both are read
# whole rather than one file at a time.
ui_src() {
    cat "$UI_SRC_DIR"/*.ts "$UI_SRC_DIR"/views/*.ts 2>/dev/null
}

ui_css() {
    cat "$UI_SRC_DIR"/styles.scss "$UI_SRC_DIR"/styles/*.scss 2>/dev/null
}

test_the_built_panel_exists_and_parses() {
    command -v node >/dev/null || { skip "node is not installed"; return; }
    if [ ! -f "$UI_DIST" ]; then
        not_ok "dist/account-ui.js is built" "a built bundle" "missing, run 'pnpm build'"
        return
    fi
    local out status=0
    out=$(node --check "$UI_DIST" 2>&1) || status=$?
    is "node --check is happy" "$status" "0"
    [ "$status" -eq 0 ] || printf '        %s\n' "$out"
    contains "it is one self-contained IIFE" "$(head -c 400 "$UI_DIST")" "(() => {"
}

# The artifact is what runs, so the behaviour that matters is checked there rather
# than in the source it was built from.
test_the_built_panel_carries_the_behaviour() {
    if [ ! -f "$UI_DIST" ]; then skip "not built"; return; fi
    local dist
    dist=$(cat "$UI_DIST")
    contains "the guard against double injection" "$dist" "__conductorMultiAccount"
    contains "compiled styles are inlined" "$dist" ".cma-panel{position:fixed"
    contains "it signs out" "$dist" "logout "
    is "and never removes" "$(printf '%s' "$dist" | grep -c 'acct(`remove \|acct("remove ')" "0"
    contains "it opens on press" "$dist" "pointerdown"
    contains "Conductor's own Claude mark" "$dist" "m4.7144"
    contains "Conductor's own Codex mark" "$dist" "M22.2819"
    contains "addresses are masked" "$dist" "maskEmail"
}

# Sealing pointer events on the capture phase stopped the click before it ever
# reached the row that was clicked: every account row went inert and the panel
# stopped opening at all. The seal has to be on the bubble phase, after the
# panel's own handlers have run.
test_the_panel_seals_pointer_events_on_the_bubble_phase() {
    local body
    body=$(sed -n '/^export function seal(/,/^}/p' "$UI_SRC_DIR/attach.ts")
    contains "listener is registered non-capturing" "$body" "false);"
    is "and nothing in seal captures" "$(printf '%s' "$body" | grep -c 'true)')" "0"
}

test_every_clickable_thing_says_it_is_clickable() {
    local css
    css=$(ui_css)
    local sel
    for sel in ".cma-btn" ".cma-chip" ".cma-card" ".cma-signout" ".cma-back" ".cma-add" ".cma-go" ".cma-act"; do
        contains "$sel exists" "$css" "$sel"
    done
    # cursor: default is only ever right on something that cannot be clicked, so
    # every one of them sits in a :disabled block or on the loading placeholder.
    is "the arrow cursor is only on unclickable things" \
        "$(ui_css | grep -B 3 'cursor: default' | grep -c -e ':disabled' -e 'cma-ghost')" \
        "$(ui_css | grep -c 'cursor: default')"
}

MASK_CASES="someone.long@example.com joe@mail.example.com ab@x.test a@b.test
first.last@example.com someone.else@example.org noatsign x@y"

test_masking_never_reveals_a_whole_part() {
    local addr out part head leaked=0
    for addr in $MASK_CASES; do
        out=$("$ACCT" mask "$addr")
        case "$out" in
            *'**'*) ;;
            *) not_ok "$addr is masked at all" "something with **" "$out"; return ;;
        esac
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
    [ -f "$UI_DIST" ] || { skip "not built"; return; }
    local addr from_sh from_js differed=0
    for addr in $MASK_CASES; do
        from_sh=$("$ACCT" mask "$addr")
        from_js=$(node -e '
            var fs = require("fs");
            var src = fs.readFileSync(process.argv[1], "utf8");
            var fns = src.match(/function maskPart[\s\S]*?\n  }\n/)[0] +
                      src.match(/function maskEmail[\s\S]*?\n  }\n/)[0];
            eval(fns);
            process.stdout.write(maskEmail(process.argv[2]));
        ' "$UI_DIST" "$addr")
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
    local src css
    src=$(ui_src)
    css=$(ui_css)
    contains "the corner is placed once and reused" "$src" "if (panel && panel.pos)"
    contains "width is fixed, not content driven" "$css" "width: 300px"
    contains "a long list scrolls instead of growing" "$css" "overflow-y: auto"
    contains "the card flexes beside its sign-out control" "$css" "flex: 1"
    contains "the tick has a slot of its own" "$css" ".cma-tickslot"
    contains "and the slot is always in the flow" "$src" '"cma-tickslot"'
    contains "triggers stay hidden until labelled" "$src" "btn.hidden = true"
}

# A dot next to a label is decoration standing in for a word. The label says it.
test_no_status_dots_anywhere() {
    is "no dot element" "$(ui_src | grep -c 'cma-dot')" "0"
    is "and no dot rule" "$(ui_css | grep -c 'cma-dot')" "0"
}

# Profile names are lower case on disk and capitalised only for display.
test_display_names_are_capitalised_without_touching_the_cli() {
    local src
    src=$(ui_src)
    contains "a display-only helper" "$src" "export function cap("
    contains "account rows use it" "$src" "cap(account.name)"
    contains "the trigger label uses it" "$src" "cap(cur)"
    is "and writes still send the raw name" \
        "$(printf '%s' "$src" | grep -c 'applyAccount(state, provider.agent, account.name)')" "1"
    # shellcheck disable=SC2016  # a literal template string, not an expansion
    is "as does sign-out" \
        "$(printf '%s' "$src" | grep -c 'logout \${account.name}')" "1"
}

# The wireframe is a drill-down: providers first, then that provider's accounts
# with a sign-out each and one "Add new account" at the foot.
test_the_panel_is_a_two_level_drill_down() {
    local src
    src=$(ui_src)
    contains "a root view" "$src" "export function rootView("
    contains "a provider view" "$src" "export function providerView("
    contains "a back control" "$src" 'el("button", "cma-back")'
    contains "add at the foot of the provider view" "$src" '"Add new account"'
    contains "a named sign-out confirmation" "$src" "export function confirmSignOut("
    contains "escape steps back before it closes" "$src" 'panel.view.level === "provider"'
}

# The panel signs an account out and touches nothing else. Deleting a profile
# outright stays in the terminal, where an accidental click cannot reach it.
test_the_panel_signs_out_and_deletes_nothing() {
    local src
    src=$(ui_src)
    # shellcheck disable=SC2016  # a literal template string, not an expansion
    contains "it calls logout" "$src" 'acct(`logout ${account.name}'
    is "and never remove" "$(printf '%s' "$src" | grep -c 'acct(`remove ')" "0"
    contains "the copy says nothing else changes" "$src" "Nothing else changes"
    contains "it names what survives" "$src" "sessions and transcripts are untouched"
    contains "the icon is a sign-out, not a bin" "$src" 'icon("signout"'
    is "no bin glyph is left" "$(printf '%s' "$src" | grep -c 'trash:')" "0"
    contains "offered only when signed in" "$src" "if (account.signedIn) {"
    contains "and signed-out rows offer sign-in" "$src" 'icon("signin"'
}

# Sign-out still costs a browser round trip to undo, so it asks in a dialog with
# a scrim, not a control that arms on a first click.
test_sign_out_asks_in_a_dialog() {
    local src css
    src=$(ui_src)
    css=$(ui_css)
    contains "a reusable dialog" "$src" "export function dialog("
    contains "with a scrim" "$css" ".cma-scrim"
    contains "announced as a modal alert" "$src" '"alertdialog"'
    contains "escape cancels it" "$src" 'if (e.key === "Escape") {'
    contains "the scrim cancels, the box does not" "$src" "if (e.target === scrim) shut()"
    contains "and it says what will happen" "$src" '"Signs "'
    # A dialog is a sibling of the panel, so clicking it must not read as
    # clicking away from the panel.
    contains "the panel ignores clicks while it is open" "$src" "if (!panel || openDialog()) return"
}

# The sign-out control lives inside the row's border, divided from the selectable
# area, rather than floating in the gutter beside it.
test_sign_out_sits_inside_the_row() {
    local css
    css=$(ui_css)
    contains "the row carries the border" "$css" ".cma-row2"
    contains "a divider before the control" "$css" "border-left: 1px solid var(--border)"
    contains "full height of the row" "$css" "align-self: stretch"
}

test_the_panel_never_renders_a_full_address() {
    local src
    src=$(ui_src)
    contains "rows mask" "$src" "maskEmail(account.email)"
    contains "the sign-out dialog masks" "$src" "account.email ? maskEmail(account.email) : cap(account.name)"
    contains "sign-in confirmation masks" "$src" '"Signed in as " + maskEmail(email)'
    # A tooltip is as visible on video as the text is.
    is "no address in a title attribute" \
        "$(printf '%s' "$src" | grep -c 'title = .*account\.email')" "0"
}

# Signed-in state has to come from where the credentials are, not from a cached
# address. The old check was "does .label exist", and .label is only written when
# an address can be read out of .claude.json, which does not always happen the
# moment a sign-in finishes. A profile with working credentials then read as
# signed out for ever, and the panel offered to sign in an account that already
# was.
