#!/bin/sh
# focus.sh panel|search: brings the terminal window that runs herdr to the front, then the bhote panel there (panel) or the
# bhote search popup (search). Called by the Omarchy bar widget. Needs hyprctl and jq.
PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
command -v hyprctl >/dev/null 2>&1 && command -v jq >/dev/null 2>&1 || exit 0
has_herdr() {  # has_herdr <pid>: 0 when herdr runs below that process
    for c in $(pgrep -P "$1" 2>/dev/null); do
        [ "$(cat "/proc/$c/comm" 2>/dev/null)" = herdr ] && return 0
        has_herdr "$c" && return 0
    done
    return 1
}
win=""
for p in $(hyprctl clients -j | jq -r '.[].pid'); do has_herdr "$p" && { win=$p; break; }; done
[ -n "$win" ] || { notify-send "bhote" "herdr is not running in a terminal window" 2>/dev/null; exit 0; }
hyprctl dispatch focuswindow "pid:$win" >/dev/null 2>&1
case "${1:-panel}" in
    search) herdr plugin action invoke search --plugin bhote.panel >/dev/null 2>&1 ;;
    *) title=$(herdr pane list 2>/dev/null | jq -r '[.result.panes[] | select(.focused)][0].terminal_title_stripped // empty')
       [ "$title" = Bhote ] || herdr plugin action invoke focus --plugin bhote.panel >/dev/null 2>&1 ;;
esac
exit 0
