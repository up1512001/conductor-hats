#!/bin/bash
# hygiene tests for conductor-multi-account.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Rules about the repository itself: no personal data, no oversized files, no
# version skew between the CLI and the panel.

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
# asserted. Nothing is exempt except build output, the lockfile, the licence and
# documentation, and there is no allowlist any more: every source file is under
# the limit, so the next one over it is a failure.
LINE_LIMIT=300

test_no_file_exceeds_the_line_limit() {
    local files f n over=0
    files=$(cd "$PROJECT_DIR" && git ls-files 2>/dev/null)
    if [ -z "$files" ]; then skip "not a git checkout"; return; fi

    for f in $files; do
        case "$f" in dist/*|pnpm-lock.yaml|LICENSE|*.md) continue ;; esac
        [ -f "$PROJECT_DIR/$f" ] || continue
        n=$(wc -l < "$PROJECT_DIR/$f" | tr -d ' ')
        [ "$n" -gt "$LINE_LIMIT" ] || continue
        echo "        $f is $n lines, limit is $LINE_LIMIT"
        over=$((over + 1))
    done

    is "every source file is under the limit" "$over" "0"
}
