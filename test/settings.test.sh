#!/bin/bash
# Conductor's settings file belongs to Conductor. Install and uninstall add and
# remove two top-level keys and must leave everything else byte for byte, which
# means understanding which table a key sits in rather than matching its name
# anywhere in the file.

settings_with_a_lookalike_key() {
    cat > "$CONDUCTOR_ACCT_SETTINGS_FILE" <<'EOF'
# Conductor settings, hand edited.
theme = "dark"

[editor]
claude_code_executable_path = "/somebody/elses/claude"
font = "Berkeley Mono"

[telemetry]
enabled = false
EOF
}

test_uninstall_leaves_a_lookalike_key_in_another_table_alone() {
    sandbox
    settings_with_a_lookalike_key
    "$ACCT" uninstall >/dev/null 2>&1

    local kept
    kept=$(grep -c 'claude_code_executable_path = "/somebody/elses/claude"' \
        "$CONDUCTOR_ACCT_SETTINGS_FILE")
    is "the editor table keeps its key" "$kept" "1"
    contains "and the rest of the file survives" "$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")" 'font = "Berkeley Mono"'
    teardown
}

test_install_then_uninstall_restores_the_file() {
    sandbox
    settings_with_a_lookalike_key
    local before
    before=$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")

    "$ACCT" install >/dev/null 2>&1
    "$ACCT" uninstall >/dev/null 2>&1

    is "the file is back as it was" "$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")" "$before"
    teardown
}

test_install_writes_its_keys_above_the_first_table() {
    sandbox
    settings_with_a_lookalike_key
    "$ACCT" install >/dev/null 2>&1

    local key_line table_line
    key_line=$(grep -n '^claude_code_executable_path' "$CONDUCTOR_ACCT_SETTINGS_FILE" | head -1 | cut -d: -f1)
    table_line=$(grep -n '^\[' "$CONDUCTOR_ACCT_SETTINGS_FILE" | head -1 | cut -d: -f1)
    ok_if "the key lands above the first table" "[ $key_line -lt $table_line ]"
    teardown
}

test_a_commented_key_is_not_mistaken_for_a_real_one() {
    sandbox
    cat > "$CONDUCTOR_ACCT_SETTINGS_FILE" <<'EOF'
# claude_code_executable_path = "/commented/out"
theme = "dark"
EOF
    "$ACCT" uninstall >/dev/null 2>&1
    contains "the comment survives" "$(cat "$CONDUCTOR_ACCT_SETTINGS_FILE")" '# claude_code_executable_path'
    teardown
}

test_reinstalling_does_not_duplicate_the_key() {
    sandbox
    "$ACCT" install >/dev/null 2>&1
    "$ACCT" install >/dev/null 2>&1
    local n
    n=$(grep -c '^claude_code_executable_path' "$CONDUCTOR_ACCT_SETTINGS_FILE")
    is "the key appears once" "$n" "1"
    teardown
}

test_a_path_with_spaces_round_trips() {
    sandbox
    local dir="$SANDBOX/App Support/bin"
    mkdir -p "$dir"
    printf 'claude_code_executable_path = "%s/claude-router"\n' "$dir" > "$CONDUCTOR_ACCT_SETTINGS_FILE"
    contains "doctor reads the quoted path back whole" "$("$ACCT" doctor "$SANDBOX/ws-a" 2>&1)" "App Support"
    teardown
}
