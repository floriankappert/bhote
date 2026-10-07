#!/bin/sh
# Puts `bhote` in a pane on the right of every tab (next to its agent pane), unless the tab already has one. Needs jq.
#   startup / event: every tab (switch off with AUTOSTART=off in ~/.config/bhote/config)
#   open:            only the focused tab, also when the autostart is off ("Bhote: open panel")
# A pane LABELLED bhote (this plugin labels the panes it makes) that does not run it any more, e.g. restored after a restart,
# gets bhote started in it. A pane that is only TITLED Bhote counts as "already there" and never gets anything typed into it.
herdr=${HERDR_BIN_PATH:-herdr}
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
cfg=${BHOTE_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/bhote/config}
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
  ( [.[] | select((.agent // "") == "" and ((.label // "") == "bhote" or (.terminal_title_stripped // "") == "Bhote"))][0] ) as $b |
  ( ([.[] | select(.agent)][0]) // .[0] ) as $t |
  [ .[0].tab_id, ($b.pane_id // ""), ($b.terminal_title_stripped // ""), ($b.label // ""), $t.pane_id, (if any(.[]; .focused) then "1" else "0" end) ] | map(tostring | gsub("[|\u0001-\u001f]"; "")) | join("|")')

# the command typed into a NEW (or our own, labelled) pane: the full path, single-quoted
case "$bhote" in *"'"*) exit 0 ;; esac
run="'$bhote'"
printf '%s\n' "$tabs" | while IFS='|' read -r tab have title label target focused; do
    [ -n "$tab" ] || continue
    [ "$mode" = open ] && [ "$focused" != 1 ] && continue
    if [ -n "$have" ]; then
        [ "$title" = Bhote ] || { [ "$label" = bhote ] && act pane run "$have" "$run" >/dev/null 2>&1; }
        continue
    fi
    total=$("$herdr" pane layout --pane "$target" 2>/dev/null | jq -r --arg p "$target" '[.result.layout.panes[] | select(.pane_id == $p)][0].rect.width // empty')
    [ -n "$total" ] && [ "$total" -ge $(( width + keep )) ] || continue    # the agent keeps at least $keep columns
    # ratio = the share the EXISTING pane keeps
    ratio=$(LC_ALL=C awk -v t="$total" -v w="$width" 'BEGIN { printf "%.3f", (t - w) / t }')      # a dot, never a decimal comma
    if [ -n "${BHOTE_DRY:-}" ]; then act pane split --pane "$target" --direction right --ratio "$ratio" --no-focus; echo "herdr pane rename <new> bhote"; echo "herdr pane run <new> $run"; continue; fi
    new=$("$herdr" pane split --pane "$target" --direction right --ratio "$ratio" --no-focus 2>/dev/null | jq -r '.result.pane.pane_id // empty')
    case "$new" in ''|*[!A-Za-z0-9:_-]*) continue ;; esac
    act pane rename "$new" bhote >/dev/null 2>&1                    # the label tells this plugin later that the pane is ours
    act pane run "$new" "$run" >/dev/null 2>&1
done
exit 0
