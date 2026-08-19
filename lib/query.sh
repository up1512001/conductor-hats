#!/bin/bash
# Machine-readable output, and reading Conductor's own databases.
#
# Sourced by bin/conductor-acct. Not executable on its own.

cmd_json() {
    ensure_root
    local ws repo agent profile label resolved current first
    ws=$(current_workspace "${1:-}")
    repo=$(repo_root_for "$ws")

    printf '{"workspace":"%s","repo":"%s","enabled":%s,"providers":[' \
        "$(json_escape "$ws")" "$(json_escape "$repo")" \
        "$(router_installed && echo true || echo false)"

    local firstp=1
    for agent in claude codex; do
        resolved=$(effective_dir "$agent" "$ws" "$repo")
        current=$(profile_from_dir "$resolved")
        [ "$firstp" -eq 1 ] || printf ','
        firstp=0
        printf '{"agent":"%s","current":"%s","accounts":[' "$agent" "$(json_escape "$current")"
        first=1
        for profile in $(list_profiles "$agent"); do
            label=$(label_of "$agent" "$profile")
            [ "$first" -eq 1 ] || printf ','
            first=0
            # signedIn is asked of the keychain, not inferred from the address:
            # a profile can hold credentials with no address cached yet, and
            # reading that as "not signed in" is what made the panel offer to
            # sign in an account that already was.
            printf '{"name":"%s","email":"%s","active":%s,"signedIn":%s}' \
                "$(json_escape "$profile")" "$(json_escape "$label")" \
                "$([ "$profile" = "$current" ] && echo true || echo false)" \
                "$(profile_signed_in "$agent" "$profile" && echo true || echo false)"
        done
        printf ']}'
    done
    printf ']}\n'
}

json_escape() {
    printf '%s' "${1:-}" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

# Every Conductor on this machine, not just the shipping one. A modified copy
# built by tools/make-dev-conductor.sh keeps its own database under its own
# identifier, so a panel running inside that copy has to be told about its own
# workspaces rather than the real app's. Globbing means neither has to know the
# other exists, which also covers the copy's data directory not being named
# quite what its identifier says.
conductor_dbs() {
    local d
    if [ -n "${CONDUCTOR_DB:-}" ]; then
        [ -f "$CONDUCTOR_DB" ] && printf '%s\n' "$CONDUCTOR_DB"
        return 0
    fi
    for d in "$HOME/Library/Application Support/"com.conductor*/conductor.db; do
        [ -f "$d" ] && printf '%s\n' "$d"
    done
}

db_query() {
    local sql="$1" db
    command -v sqlite3 >/dev/null || die "sqlite3 not found"
    # read, not $(...): these paths contain "Application Support".
    while IFS= read -r db; do
        [ -n "$db" ] || continue
        sqlite3 -separator "$(printf '\t')" "file:$db?mode=ro" "$sql" 2>/dev/null
    done <<EOF
$(conductor_dbs)
EOF
}

# Conductor's webview uses an in-memory router, so the panel cannot read the
# current workspace from the URL. It matches what is on screen against this
# list instead. Name first so the UI can match on the visible label.
cmd_workspaces() {
    db_query "select replace(workspace_path, rtrim(workspace_path, replace(workspace_path, '/', '')), ''), workspace_path
              from workspaces
              where workspace_path is not null and state != 'archived'" | sort -u
}

cmd_repos() {
    db_query "select replace(root_path, rtrim(root_path, replace(root_path, '/', '')), ''), root_path
              from repos where root_path is not null" | sort -u
}

cmd_resolve() {
    local id="${1:-}"
    [ -n "$id" ] || die "usage: conductor-acct resolve <workspace-id>"
    case "$id" in
        *[!a-zA-Z0-9-]*) die "not a workspace id" ;;
    esac
    db_query "select workspace_path from workspaces
              where id='$id' and workspace_path is not null" | head -1
}

cmd_resolve_repo() {
    local id="${1:-}"
    [ -n "$id" ] || die "usage: conductor-acct resolve-repo <repository-id>"
    case "$id" in
        *[!a-zA-Z0-9-]*) die "not a repository id" ;;
    esac
    db_query "select root_path from repos
              where id='$id' and root_path is not null" | head -1
}
