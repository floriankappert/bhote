#!/bin/sh
# Puts `bhote` in a pane on the right of every tab (next to its agent pane), unless the tab already has one. Needs jq.
#   startup / event: every tab (switch off with AUTOSTART=off in ~/.config/bhote/config)
#   open:            only the focused tab, also when the autostart is off ("Bhote: open panel")
# A pane that is called bhote but does not run it (restored after a restart) just gets `bhote` started in it.
herdr=${HERDR_BIN_PATH:-herdr}
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
cfg=${BHOTE_CONFIG:-$HOME/.config/bhote/config}
width=${BHOTE_PANEL_WIDTH:-44}
keep=${BHOTE_AGENT_MIN_WIDTH:-64}
mode=${1:-startup}
# BHOTE_DRY=1 only prints what would be done
act() { if [ -n "${BHOTE_DRY:-}" ]; then echo "herdr $*"; else "$herdr" "$@"; fi; }

[ -x "$bhote" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
if [ "$mode" != open ] && [ -r "$cfg" ] && grep -qx 'AUTOSTART=off' "$cfg"; then exit 0; fi

panes=$("$herdr" pane list 2>/dev/null) || exit 0
# one line per tab: tab, pane that is the bhote panel ("" if none), its title, the pane to split (an agent pane first), focused 1/0
tabs=$(printf '%s' "$panes" | jq -r '
  .result.panes | group_by(.tab_id)[] |
  ( [.[] | select((.agent // "") == "" and ((.label // "" | ascii_downcase) == "bhote" or (.terminal_title_stripped // "" | ascii_downcase) == "bhote"))][0] ) as $b |
  ( ([.[] | select(.agent)][0]) // .[0] ) as $t |
  [ .[0].tab_id, ($b.pane_id // ""), ($b.terminal_title_stripped // ""), $t.pane_id, (if any(.[]; .focused) then "1" else "0" end) ] | join("|")')

printf '%s\n' "$tabs" | while IFS='|' read -r tab have title target focused; do
    [ -n "$tab" ] || continue
    [ "$mode" = open ] && [ "$focused" != 1 ] && continue
    if [ -n "$have" ]; then
        [ "$title" = Bhote ] || act pane run "$have" bhote >/dev/null 2>&1
        continue
    fi
    total=$("$herdr" pane layout --pane "$target" 2>/dev/null | jq -r --arg p "$target" '[.result.layout.panes[] | select(.pane_id == $p)][0].rect.width // empty')
    [ -n "$total" ] && [ "$total" -ge $(( width + keep )) ] || continue    # the agent keeps at least $keep columns
    # ratio = the share the EXISTING pane keeps
    ratio=$(awk -v t="$total" -v w="$width" 'BEGIN { printf "%.3f", (t - w) / t }')
    if [ -n "${BHOTE_DRY:-}" ]; then act pane split --pane "$target" --direction right --ratio "$ratio" --no-focus; echo "herdr pane run <new> bhote"; continue; fi
    new=$("$herdr" pane split --pane "$target" --direction right --ratio "$ratio" --no-focus 2>/dev/null | jq -r '.result.pane.pane_id // empty')
    [ -n "$new" ] && act pane run "$new" bhote >/dev/null 2>&1
done
exit 0
