#!/bin/sh
# Jumps between the agent and the bhote panel of the focused tab, and back. Bind it to a key in herdr's config.toml:
#   [[keys.command]]
#   key = "prefix+t"
#   type = "plugin_action"
#   command = "bhote.panel.focus"
# From the panel it goes back to the pane that was focused before (else the tab's agent). A tab without a panel gets one
# first (like "Bhote: open panel"). Needs jq and perl (for herdr's socket: the CLI focuses only by direction).
# BHOTE_DRY=1 only prints what would be focused.
herdr=${HERDR_BIN_PATH:-herdr}
sock=${HERDR_SOCKET_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/herdr/herdr.sock}
state=${HERDR_PLUGIN_STATE_DIR:-${TMPDIR:-/tmp}}
command -v jq >/dev/null 2>&1 || exit 0

focus() {  # focus <pane id>: pane.focus through the socket
    case "$1" in ''|*[!A-Za-z0-9:_-]*) return 1 ;; esac
    if [ -n "${BHOTE_DRY:-}" ]; then echo "focus $1"; return 0; fi
    perl -MIO::Socket::UNIX -e '
        my $s = IO::Socket::UNIX->new(Peer => $ARGV[0]) or exit 1;
        print $s qq({"id":"bhote-focus","method":"pane.focus","params":{"pane_id":"$ARGV[1]"}}\n);
        my $r = <$s>; exit(defined $r && $r =~ /"result"/ ? 0 : 1)' "$sock" "$1"
}
# one line per pane of the tab: pane|is the panel (1/0)|is an agent (1/0)|focused (1/0)
tab_panes() {
    "$herdr" pane list 2>/dev/null | jq -r --arg t "$1" '.result.panes[] | select(.tab_id == $t)
        | [.pane_id, (if (.agent // "") == "" and (.terminal_title_stripped // "") == "Bhote" then "1" else "0" end),
           (if (.agent // "") != "" then "1" else "0" end), (if .focused then "1" else "0" end)]
        | map(tostring | gsub("[|\u0001-\u001f]"; "")) | join("|")'
}

cur=${HERDR_PANE_ID:-}
[ -n "$cur" ] || cur=$("$herdr" pane list 2>/dev/null | jq -r '[.result.panes[] | select(.focused)][0].pane_id // empty')
case "$cur" in ''|*[!A-Za-z0-9:_-]*) exit 0 ;; esac
tab=$("$herdr" pane list 2>/dev/null | jq -r --arg p "$cur" '[.result.panes[] | select(.pane_id == $p)][0].tab_id // empty')
case "$tab" in ''|*[!A-Za-z0-9:_-]*) exit 0 ;; esac
mem="$state/last-$(printf '%s' "$tab" | tr -c 'A-Za-z0-9_-' '_')"

list=$(tab_panes "$tab")
panel=$(printf '%s\n' "$list" | awk -F'|' '$2 == 1 { print $1; exit }')
if [ -n "$panel" ] && [ "$cur" = "$panel" ]; then
    # back: the pane we came from, if it is still in this tab; else the tab's first agent; else the pane to the left
    back=$(cat "$mem" 2>/dev/null)
    printf '%s\n' "$list" | awk -F'|' -v b="$back" '$1 == b { f = 1 } END { exit !f }' || back=""
    [ -n "$back" ] || back=$(printf '%s\n' "$list" | awk -F'|' '$3 == 1 { print $1; exit }')
    if [ -n "$back" ]; then focus "$back"
    elif [ -n "${BHOTE_DRY:-}" ]; then echo "focus left of $cur"
    else "$herdr" pane focus --pane "$cur" --direction left >/dev/null 2>&1; fi
    exit 0
fi
if [ -z "$panel" ]; then                                         # no panel in this tab yet: open one, then go there
    [ -n "${BHOTE_DRY:-}" ] && { echo "open panel in $tab"; exit 0; }
    sh "$(dirname "$0")/open_panel.sh" open >/dev/null 2>&1
    i=0; while [ -z "$panel" ] && [ "$i" -lt 10 ]; do           # the new pane gets its title once bhote runs
        sleep 0.3; i=$((i + 1))
        panel=$(tab_panes "$tab" | awk -F'|' '$2 == 1 { print $1; exit }')
    done
    [ -n "$panel" ] || exit 0
fi
[ -n "${BHOTE_DRY:-}" ] || printf '%s\n' "$cur" > "$mem" 2>/dev/null
focus "$panel"
exit 0
