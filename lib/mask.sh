#!/bin/bash
# Masking an address for anything that renders on screen.
#
# Sourced by bin/conductor-acct. Not executable on its own.

mask_part() {
    local s="$1" n=${#1}
    if [ "$n" -le 2 ]; then printf '**'
    elif [ "$n" -le 5 ]; then printf '%s**' "${s:0:1}"
    elif [ "$n" -le 8 ]; then printf '%s**%s' "${s:0:2}" "${s:$((n - 1))}"
    else printf '%s**%s' "${s:0:3}" "${s:$((n - 3))}"
    fi
}

mask_email() {
    local raw="${1:-}" local_part host suffix domain
    [ -n "$raw" ] || return 0
    case "$raw" in
        *@*)
            local_part=${raw%@*}
            domain=${raw##*@}
            ;;
        *) mask_part "$raw"; echo; return 0 ;;
    esac
    [ -n "$local_part" ] || { printf '@'; mask_part "$domain"; echo; return 0; }
    case "$domain" in
        *.*) host=${domain%%.*}; suffix=".${domain#*.}" ;;
        *) host="$domain"; suffix="" ;;
    esac
    printf '%s@%s%s\n' "$(mask_part "$local_part")" "$(mask_part "$host")" "$suffix"
}

# Display commands mask by default only when asked, because a terminal is where
# you go to read the real address. The chat card passes --mask.
label_for_display() {
    local agent="$1" profile="$2" masked="$3" label
    label=$(label_of "$agent" "$profile")
    if [ -n "$label" ] && [ "$masked" = "1" ]; then
        mask_email "$label"
    else
        printf '%s\n' "$label"
    fi
}
