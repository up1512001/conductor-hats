#!/bin/bash
# hygiene tests for conductor-hats.
#
# Sourced by test/run.sh, which owns the harness. Not executable on its own.
#
# Rules about the repository itself: no personal data, no oversized files, no
# version skew between the CLI and the panel.

# Five files carry the version and they ship together, so a skew between any two
# is a bug. tools/set-version.sh keeps them in step; this notices when one is
# edited by hand.
test_every_file_agrees_on_the_version() {
    local out cli
    if ! out=$("$PROJECT_DIR/tools/set-version.sh" --check 2>&1); then
        printf '%s\n' "$out" | sed 's/^/        /'
        not_ok "every file agrees on the version" "one version" "a skew"
        return
    fi
    ok "Cargo.toml, package.json, the CLI, the panel and the changelog agree"

    cli=$("$ACCT" version | awk '{print $2}')
    contains "and the CLI reports the same one" "$out" "all agree on $cli"
    contains "which the changelog has an entry for" "$(cat "$PROJECT_DIR/CHANGELOG.md")" "## $cli"
}

# This is published, so an address or a home directory left in a file is a leak
# rather than an untidiness. Both rules are stated positively so the test itself
# carries no personal data: every example address must sit on a domain RFC 2606
# reserves for documentation, and no path may name a real account.
test_no_personal_information_is_committed() {
    local files bad
    files=$(cd "$PROJECT_DIR" && git ls-files 2>/dev/null)
    if [ -z "$files" ]; then skip "not a git checkout"; return; fi

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

# Comments say what a file or a function is, at its top. Anything else is
# commentary, and commentary rots. Enforced rather than asked for, because it was
# asked for twice and drifted back both times.
#
# Shell has one comment character, so the rule there is placement: a header or a
# docblock at column zero, never a remark inside a body.
test_comments_are_docblocks_only() {
    local files f bad=0 out

    files=$(cd "$PROJECT_DIR" && git ls-files '*.ts' '*.mjs' '*.scss' '*.rs' 2>/dev/null)
    for f in $files; do
        [ -f "$PROJECT_DIR/$f" ] || continue
        out=$(grep -nE '^[[:space:]]*//' "$PROJECT_DIR/$f" 2>/dev/null |
              grep -vE '^[0-9]+:[[:space:]]*//[/!]' || true)
        if [ -n "$out" ]; then
            echo "        $f uses // comments, docblocks only:"
            printf '        %s\n' "$out" | head -3
            bad=$((bad + 1))
        fi
    done
    is "no // comments in TypeScript, SCSS or Rust" "$bad" "0"

    bad=0
    files=$(cd "$PROJECT_DIR" && git ls-files 'bin/*' 'lib/*.sh' 'test/*.sh' install.sh 2>/dev/null)
    for f in $files; do
        [ -f "$PROJECT_DIR/$f" ] || continue
        out=$(grep -nE '^[[:space:]]+#' "$PROJECT_DIR/$f" 2>/dev/null |
              grep -vE 'shellcheck|#!/' || true)
        if [ -n "$out" ]; then
            echo "        $f comments inside a body:"
            printf '        %s\n' "$out" | head -3
            bad=$((bad + 1))
        fi
    done
    is "no comments inside shell function bodies" "$bad" "0"
}
