#!/bin/bash
# Making a new workspace's first chat ask which account to use.
#
# Sourced by bin/conductor-acct. Not executable on its own.

# Marks the block this writes into a repository's settings, so it can be found
# again and replaced rather than appended to.
ASK_MARKER="conductor-hats"

ask_snippet() {
    cat <<EOF
Account check for this workspace (managed by $ASK_MARKER).
Before your first action, run: $INSTALL_BIN/conductor-acct check
If it prints NEEDS_ACCOUNT, this is a new workspace with no account chosen yet:
use mcp__conductor__AskUserQuestion to ask which account it should use, with one
option per profile from '$INSTALL_BIN/conductor-acct list', then run
'$INSTALL_BIN/conductor-acct use <profile>' and tell the user in one line that it
applies from the next chat in this workspace. If it prints anything else, say
nothing about accounts and get on with what the user asked for.
EOF
}

# Writes prompts.general, which Conductor appends to every agent session in the
# repository. Combined with `check` above it only speaks up in a workspace that
# has not chosen yet, so it behaves like a prompt on workspace creation.
cmd_ask() {
    local mode="${1:-status}" repo file
    repo=$(repo_root_for "${2:-}")
    file="$repo/.conductor/settings.local.toml"

    case "$mode" in
        status)
            if [ -f "$file" ] && grep -q "$ASK_MARKER" "$file"; then
                echo "on   $file"
            else
                echo "off  ($file)"
            fi
            return 0
            ;;
        on) ;;
        off)
            if [ -f "$file" ] && grep -q "$ASK_MARKER" "$file"; then
                toml_drop_prompts_general "$file"
                echo "Removed the account prompt from $file"
            else
                echo "Not enabled for $repo"
            fi
            return 0
            ;;
        *) die "usage: conductor-acct ask on|off|status [repo]" ;;
    esac

    ensure_root
    if [ -f "$file" ] && grep -q '^[[:space:]]*general[[:space:]]*=' "$file" &&
       ! grep -q "$ASK_MARKER" "$file"; then
        echo "$file already sets prompts.general, and overwriting someone else's" >&2
        echo "prompt would be rude. Append this to it by hand:" >&2
        echo >&2
        ask_snippet >&2
        exit 1
    fi

    toml_drop_prompts_general "$file"
    mkdir -p "$(dirname "$file")"
    {
        printf '\n[prompts]\ngeneral = """\n'
        ask_snippet
        printf '"""\n'
    } >> "$file"

    echo "New workspaces in $repo will ask which account to use, in the chat."
    echo "  $file"
    echo
    echo "It stays quiet once a workspace has an account, and in every workspace"
    echo "that already has one."
}

toml_drop_prompts_general() {
    local file="$1" tmp
    [ -f "$file" ] || return 0
    tmp=$(mktemp)
    awk '
        BEGIN { skip = 0 }
        /^[[:space:]]*general[[:space:]]*=[[:space:]]*"""/ { skip = 1; next }
        skip && /^"""/ { skip = 0; next }
        skip { next }
        /^[[:space:]]*\[prompts\][[:space:]]*$/ { held = held $0 "\n"; next }
        {
            if (held != "" && $0 ~ /^[[:space:]]*$/) { next }
            if (held != "") { printf "%s", held; held = "" }
            print
        }
    ' "$file" > "$tmp"
    awk 'BEGIN{RS="";ORS=""} {gsub(/\n*\[prompts\]\n*$/, "\n"); print}' "$tmp" > "$file"
    rm -f "$tmp"
}

# install records the checkout it came from, so update can find it again even
# when conductor-acct is being run from the deployed copy.
cmd_update() {
    local proj git_root
    proj=$(read_line_from "$ACCOUNTS_ROOT/.source" 2>/dev/null || true)
    [ -n "$proj" ] || proj="$REPO_DIR"
    [ -x "$proj/bin/conductor-acct" ] ||
        die "no checkout at $proj; re-run install from one"

    git_root=$(git -C "$proj" rev-parse --show-toplevel 2>/dev/null) ||
        die "$proj is not inside a git checkout"

    echo "Updating $git_root"
    git -C "$git_root" pull --ff-only ||
        die "pull failed; sort it out in $git_root and run update again"
    echo
    "$proj/bin/conductor-acct" install
}

# Machine-readable state for the patched-UI panel. Hand-rolled JSON because a
# jq dependency for four fields is not worth it.
