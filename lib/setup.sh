#!/bin/bash
# Turning routing on and off, first run, and the health check.
#
# Sourced by bin/conductor-acct. Not executable on its own.

cmd_install() {
    ensure_root

    # A copy, not the checkout: Conductor never checks that
    # claude_code_executable_path exists, so a deleted clone or archived
    # workspace would stop every agent starting.
    #
    # Replace a symlink from older versions, keep a directory: `rm -f` on a
    # directory fails, and under `set -e` that left a stale copy deployed.
    if [ -L "$INSTALL_BIN" ]; then
        rm -f "$INSTALL_BIN"
    fi
    mkdir -p "$INSTALL_BIN" "$ACCOUNTS_ROOT/commands"

    if [ "$BIN_DIR" != "$INSTALL_BIN" ]; then
        local f
        for f in _resolve.sh claude-router codex-router conductor-acct; do
            cp "$BIN_DIR/$f" "$INSTALL_BIN/$f"
        done
        # Dispatch alone is useless, so lib/ travels with it.
        rm -rf "${INSTALL_BIN:?}/lib"
        mkdir -p "$INSTALL_BIN/lib"
        cp "$LIB_DIR"/*.sh "$INSTALL_BIN/lib/"
        cp "$REPO_DIR/commands/account.md" "$ACCOUNTS_ROOT/commands/account.md"
        printf '%s\n' "$REPO_DIR" > "$ACCOUNTS_ROOT/.source"
    fi
    chmod +x "$INSTALL_BIN/claude-router" "$INSTALL_BIN/codex-router" \
        "$INSTALL_BIN/conductor-acct"

    toml_set claude_code_executable_path "$INSTALL_BIN/claude-router"
    toml_set codex_executable_path "$INSTALL_BIN/codex-router"

    # Conductor passes settingSources ["user","project","local"] to the agent,
    # so a user-level command shows up in Conductor's own slash command menu.
    mkdir -p "$COMMANDS_DIR"
    ln -sfn "$ACCOUNTS_ROOT/commands/account.md" "$COMMANDS_DIR/account.md"

    echo "Installed to $INSTALL_BIN"
    echo "Wrote to $CONDUCTOR_SETTINGS:"
    echo "  claude_code_executable_path = \"$INSTALL_BIN/claude-router\""
    echo "  codex_executable_path       = \"$INSTALL_BIN/codex-router\""
    echo "  $COMMANDS_DIR/account.md  (adds /account inside Conductor)"
    echo
    echo "Re-run install after pulling changes; doctor warns when the copy is stale."
}

cmd_uninstall() {
    toml_unset claude_code_executable_path
    toml_unset codex_executable_path
    [ -L "$COMMANDS_DIR/account.md" ] && rm -f "$COMMANDS_DIR/account.md"
    echo "Removed the router paths from $CONDUCTOR_SETTINGS and the /account command. Restart Conductor."
}

cmd_setup() {
    ensure_root
    echo "conductor-acct $CONDUCTOR_ACCT_VERSION"
    echo

    local existing
    existing=$(list_profiles claude)
    if [ -z "$existing" ]; then
        cat <<EOF
No accounts yet. Sign in to each one, in this terminal:

  $SELF add personal
  $SELF add work

Then run $0 setup again.
EOF
        return 0
    fi

    echo "Accounts:"
    local profile label
    for profile in $existing; do
        label=$(label_of claude "$profile")
        printf '  %-14s %s\n' "$profile" "${label:-(not signed in)}"
    done
    echo

    if router_installed; then
        echo "Router is already on."
    else
        cmd_install
    fi
    echo
    cat <<EOF
Now pick an account per workspace:

  cd <a workspace>            $SELF use work
  cd <another workspace>      $SELF use personal

or, inside a Conductor chat, run /account.

Check any workspace with: $SELF which
EOF
}

cmd_doctor() {
    ensure_root
    local ok=1

    echo "version:  $CONDUCTOR_ACCT_VERSION"
    echo "root:     $ACCOUNTS_ROOT"

    local bin
    if bin=$(resolve_agent_binary claude "$SELF"); then
        echo "claude:   $bin ($("$bin" --version 2>/dev/null | head -1))"
    else
        echo "claude:   NOT FOUND"; ok=0
    fi

    if router_installed; then
        echo "router:   on, via claude_code_executable_path"
        # Pointing Conductor at a checkout is a foot-gun: delete the clone or
        # archive the workspace holding it and every agent stops starting.
        local wired
        wired=$(sed -n 's/^[[:space:]]*claude_code_executable_path[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' \
            "$CONDUCTOR_SETTINGS" 2>/dev/null | head -1)
        case "$wired" in
            "$INSTALL_BIN"/*) ;;
            *) echo "warn:     Conductor points at $wired, outside $ACCOUNTS_ROOT"
               echo "          re-run 'conductor-acct install' so it cannot be deleted"
               ok=0 ;;
        esac
        local f
        for f in _resolve.sh claude-router codex-router conductor-acct; do
            [ -f "$INSTALL_BIN/$f" ] || continue
            [ -f "$BIN_DIR/$f" ] || continue
            [ "$BIN_DIR" = "$INSTALL_BIN" ] && continue
            if ! cmp -s "$BIN_DIR/$f" "$INSTALL_BIN/$f"; then
                echo "warn:     installed $f differs from this checkout, re-run install"
            fi
        done
        # A stale library is harder to spot, because the CLI still starts.
        for f in "$LIB_DIR"/*.sh; do
            [ -f "$f" ] || continue
            [ "$LIB_DIR" = "$INSTALL_BIN/lib" ] && continue
            if ! cmp -s "$f" "$INSTALL_BIN/lib/$(basename "$f")"; then
                echo "warn:     installed lib/$(basename "$f") differs from this checkout, re-run install"
            fi
        done
    else
        echo "router:   off (repository bindings still work; conductor-acct install turns it on)"
    fi

    local profile item target
    for profile in $(list_profiles claude); do
        for item in $SHARED_LINKS; do
            target="$ACCOUNTS_ROOT/claude/$profile/$item"
            if [ -e "$target" ] && [ ! -L "$target" ]; then
                echo "warn:     $profile/$item is a real file, no longer shared with ~/.claude"
            fi
        done
        if ! profile_signed_in claude "$profile"; then
            echo "warn:     profile '$profile' is not signed in"
            ok=0
        fi
    done

    # A pair sharing one address sign each other out.
    local agent seen dup
    for agent in claude codex; do
        seen=""
        for profile in $(list_profiles "$agent"); do
            dup=$(label_of "$agent" "$profile")
            [ -n "$dup" ] || continue
            case " $seen " in
                *" $dup "*)
                    echo "warn:     $agent profiles share the address $dup"
                    echo "          one live token per account, so they will sign each other out"
                    ok=0
                    ;;
                *) seen="$seen $dup" ;;
            esac
        done
    done

    # Every route must still point at a profile that exists.
    local key routed
    while IFS=$'\t' read -r key routed; do
        case "$key" in ''|'#'*) continue ;; esac
        [ -n "$routed" ] || continue
        if [ ! -d "$ACCOUNTS_ROOT/claude/$routed" ] && [ ! -d "$ACCOUNTS_ROOT/codex/$routed" ]; then
            echo "warn:     route $key points at missing profile '$routed'"
            ok=0
        fi
    done < "$ROUTES_FILE"

    # End to end: run the router itself and see what it exports.
    if router_installed; then
        local ws resolved
        ws=$(current_workspace)
        if resolved=$(router_dry_run claude "$ws"); then
            if [ -n "$resolved" ]; then
                echo "dry run:  $ws -> $(profile_from_dir "$resolved")"
            else
                echo "dry run:  $ws -> default account (no route, no binding)"
            fi
        else
            echo "dry run:  FAILED, the router did not exec cleanly"; ok=0
        fi
    fi

    [ "$ok" -eq 1 ] && echo "OK"
}
