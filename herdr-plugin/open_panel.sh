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
width=${BHOTE_PANEL_WIDTH:-56}
# the settings PANEL_MIN_WIDTH / PANEL_MAX_WIDTH (settings › Panel width) bound the width a new panel gets, as they bound an open one
if [ -r "$cfg" ]; then
    lo=$(sed -n 's/^PANEL_MIN_WIDTH=//p' "$cfg" | tail -1); hi=$(sed -n 's/^PANEL_MAX_WIDTH=//p' "$cfg" | tail -1)
    case "$hi" in ''|*[!0-9]*) ;; *) [ "$width" -gt "$hi" ] && width=$hi ;; esac
    case "$lo" in ''|*[!0-9]*) ;; *) [ "$width" -lt "$lo" ] && width=$lo ;; esac
fi
keep=${BHOTE_AGENT_MIN_WIDTH:-64}
# PANEL_LAYOUT=split (and the Rust panel, BHOTED_PANEL=on): two panels per tab, the left one (label bhote-left, PANEL_LEFT_WIDTH columns) and the right one (bhote-right)
split=0; lw=40
if [ -r "$cfg" ]; then
    [ "$(sed -n 's/^PANEL_LAYOUT=//p' "$cfg" | tail -1)" = split ] && [ "$(sed -n 's/^BHOTED_PANEL=//p' "$cfg" | tail -1)" = on ] && split=1
    v=$(sed -n 's/^PANEL_LEFT_WIDTH=//p' "$cfg" | tail -1); case "$v" in ''|*[!0-9]*) ;; *) [ "$v" -ge 30 ] && [ "$v" -le 64 ] && lw=$v ;; esac
fi
mode=${1:-startup}
# BHOTE_DRY=1 only prints what would be done
act() { if [ -n "${BHOTE_DRY:-}" ]; then echo "herdr $*"; else "$herdr" "$@"; fi; }

[ -x "$bhote" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
if [ "$mode" != open ] && [ -r "$cfg" ] && grep -qx 'AUTOSTART=off' "$cfg"; then exit 0; fi

# one run at a time: two runs (startup and an event) that both see "no panel" would split the tab twice
lock="${TMPDIR:-/tmp}/bhote-open.$(id -u).lock"
if ! mkdir "$lock" 2>/dev/null; then
    [ -n "$(find "$lock" -maxdepth 0 -mmin +1 2>/dev/null)" ] && rmdir "$lock" 2>/dev/null   # left by a killed run
    exit 0
fi
trap 'rmdir "$lock" 2>/dev/null' EXIT
panes=$("$herdr" pane list 2>/dev/null) || exit 0
# one line per tab: tab, pane that is the bhote panel ("" if none), its title, the pane to split (an agent pane first), focused 1/0
tabs=$(printf '%s' "$panes" | jq -r '
  .result.panes | group_by(.tab_id)[] |
  ( [.[] | select((.agent // "") == "" and (.label // "") != "bhote-left" and ((.label // "") == "bhote" or (.label // "") == "bhote-right" or (.terminal_title_stripped // "") == "Bhote"))][0] ) as $b |
  ( [.[] | select((.agent // "") == "" and (.label // "") == "bhote-left")][0] ) as $lf |
  ( ([.[] | select(.agent)][0]) // .[0] ) as $t |
  [ .[0].tab_id, ($b.pane_id // ""), ($b.terminal_title_stripped // ""), ($b.label // ""), $t.pane_id, (if any(.[]; .focused) then "1" else "0" end), ($lf.pane_id // "") ] | map(tostring | gsub("[|\u0001-\u001f]"; "")) | join("|")')

# the command typed into a NEW (or our own, labelled) pane: the full path, single-quoted
case "$bhote" in *"'"*) exit 0 ;; esac
run="'$bhote'"
# the right panel (or the single one) of a tab, made next to its agent pane: label $2
make_right() {  # make_right <agent pane> <label>
    total=$("$herdr" pane layout --pane "$1" 2>/dev/null | jq -r --arg p "$1" '[.result.layout.panes[] | select(.pane_id == $p)][0].rect.width // empty')
    [ -n "$total" ] && [ "$total" -ge $(( width + keep )) ] || return 0    # the agent keeps at least $keep columns
    # ratio = the share the EXISTING pane keeps
    ratio=$(LC_ALL=C awk -v t="$total" -v w="$width" 'BEGIN { printf "%.3f", (t - w) / t }')      # a dot, never a decimal comma
    if [ -n "${BHOTE_DRY:-}" ]; then act pane split --pane "$1" --direction right --ratio "$ratio" --no-focus; echo "herdr pane rename <new> $2"; echo "herdr pane run <new> $run"; return 0; fi
    new=$("$herdr" pane split --pane "$1" --direction right --ratio "$ratio" --no-focus 2>/dev/null | jq -r '.result.pane.pane_id // empty')
    case "$new" in ''|*[!A-Za-z0-9:_-]*) return 0 ;; esac
    act pane rename "$new" "$2" >/dev/null 2>&1                    # the label tells this plugin later that the pane is ours (and the panel which side it is)
    act pane run "$new" "$run" >/dev/null 2>&1
}
# the left panel: split the agent pane (the new pane is on its right), swap it to the left, then make it $lw columns wide
make_left() {  # make_left <agent pane>
    total=$("$herdr" pane layout --pane "$1" 2>/dev/null | jq -r --arg p "$1" '[.result.layout.panes[] | select(.pane_id == $p)][0].rect.width // empty')
    [ -n "$total" ] && [ "$total" -ge $(( lw + keep )) ] || return 0
    ratio=$(LC_ALL=C awk -v t="$total" -v w="$lw" 'BEGIN { printf "%.3f", (t - w) / t }')
    if [ -n "${BHOTE_DRY:-}" ]; then act pane split --pane "$1" --direction right --ratio "$ratio" --no-focus; echo "herdr pane swap --pane <new> --direction left"; echo "herdr pane resize <new> to $lw columns"; echo "herdr pane rename <new> bhote-left"; echo "herdr pane run <new> $run"; return 0; fi
    new=$("$herdr" pane split --pane "$1" --direction right --ratio "$ratio" --no-focus 2>/dev/null | jq -r '.result.pane.pane_id // empty')
    case "$new" in ''|*[!A-Za-z0-9:_-]*) return 0 ;; esac
    act pane swap --pane "$new" --direction left >/dev/null 2>&1
    wn=$("$herdr" pane layout --pane "$new" 2>/dev/null | jq -r --arg n "$new" --arg a "$1" '[.result.layout.panes[] | select(.pane_id == $n)][0].rect.width as $w | [.result.layout.panes[] | select(.pane_id == $a)][0].rect.width as $x | "\($w // 0) \($x // 0)"')
    set -- "$1" $wn
    if [ "${2:-0}" -gt "$lw" ] && [ $(( ${2:-0} + ${3:-0} )) -gt 0 ]; then
        amt=$(LC_ALL=C awk -v a="$2" -v b="$3" -v w="$lw" 'BEGIN { printf "%.4f", (a - w) / (a + b) }')
        act pane resize --pane "$new" --direction left --amount "$amt" >/dev/null 2>&1
    fi
    act pane rename "$new" bhote-left >/dev/null 2>&1
    act pane run "$new" "$run" >/dev/null 2>&1
}
printf '%s\n' "$tabs" | while IFS='|' read -r tab have title label target focused left; do
    [ -n "$tab" ] || continue
    [ "$mode" = open ] && [ "$focused" != 1 ] && continue
    if [ "$split" = 1 ]; then
        want=bhote-right
        [ -n "$have" ] && [ "$label" = bhote ] && act pane rename "$have" bhote-right >/dev/null 2>&1    # (a panel of the single layout is the right one now; it learns its side at its next start: bhote reload)
    else
        want=bhote
        [ -n "$left" ] && act pane close "$left" >/dev/null 2>&1                                            # (the single layout: no left panel)
        [ -n "$have" ] && [ "$label" = bhote-right ] && act pane rename "$have" bhote >/dev/null 2>&1
    fi
    if [ -n "$have" ]; then
        [ "$title" = Bhote ] || { { [ "$label" = bhote ] || [ "$label" = bhote-right ]; } && act pane run "$have" "$run" >/dev/null 2>&1; }
    else
        make_right "$target" "$want"
    fi
    if [ "$split" = 1 ]; then
        if [ -z "$left" ]; then make_left "$target"; fi
    fi
done
exit 0
