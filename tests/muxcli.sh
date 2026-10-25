#!/usr/bin/env bash
# The herdr verbs of the native mux (pane, workspace, tab, agent focus, api snapshot): same words, same JSON shapes as herdr.
# A real server on a throw-away socket; skipped without the daemon binary or jq. Usage: bash tests/muxcli.sh
cd "$(dirname "$0")/.." || exit 1
BIN=$PWD/daemon/target/release/bhoted
[ -x "$BIN" ] && command -v jq >/dev/null 2>&1 || { echo skipped; exit 0; }
T=$(mktemp -d); printf "AUTOSTART=off\n" > "$T/config"; export BHOTE_CONFIG=$T/config BHOTE_MUX_SOCK=$T/mux.sock SHELL=/bin/sh HOME=$T ENV=/dev/null
unset HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID CLAUDECODE
mkdir -p "$T/bin"; ln -s "$BIN" "$T/bin/herdr"; export PATH="$T/bin:$PATH"
( cd "$T" && exec setsid "$BIN" mux >/dev/null 2>&1 ) &
for _ in $(seq 50); do herdr pane list >/dev/null 2>&1 && break; sleep 0.1; done
trap '"$BIN" mux-call server.stop >/dev/null 2>&1; rm -rf "$T"' EXIT
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
t() { # t <description> <jq filter that must be true on the output of the rest>
    local d=$1 f=$2; shift 2; local out; out=$("$@" 2>&1)
    printf '%s' "$out" | jq -e "$f" >/dev/null 2>&1 && ok "$d" || bad "$d: $out"
}
t "pane list: one pane w1:p1 with the herdr fields" '.result.panes | length == 1 and (.[0] | .pane_id == "w1:p1" and .tab_id == "w1:t1" and .workspace_id == "w1" and .focused == true and has("cwd") and has("terminal_id") and (.scroll | has("viewport_rows")))' herdr pane list
t "workspace list" '.result.workspaces[0] | .workspace_id == "w1" and .number == 1 and .pane_count == 1 and .tab_count == 1 and .active_tab_id == "w1:t1"' herdr workspace list
t "tab list" '.result.tabs[0] | .tab_id == "w1:t1" and .label == "1"' herdr tab list
W=$(herdr pane layout --pane w1:p1 | jq -r .result.layout.area.width)
t "layout: one pane fills the area" '.result.layout.panes | length == 1 and (.[0].rect | .x == 0 and .y == 0)' herdr pane layout --pane w1:p1
# split: the old pane keeps the ratio, the new one is on the right; no focus change
new=$(herdr pane split --pane w1:p1 --direction right --ratio 0.7 --no-focus | jq -r .result.pane.pane_id)
[ "$new" = w1:p2 ] && ok "split returns the new pane" || bad "split: $new"
t "split --no-focus keeps the focus on w1:p1" '.result.panes | map(select(.focused)) | .[0].pane_id == "w1:p1"' herdr pane list
t "layout: 70/30 and one divider cell" '(.result.layout.panes | map(.rect.width) | add) == ('"$W"' - 1) and (.result.layout.panes[0].rect.width > .result.layout.panes[1].rect.width) and (.result.layout.splits | length == 1) and .result.layout.splits[0].direction == "right"' herdr pane layout --pane w1:p1
herdr pane swap --pane w1:p2 --direction left >/dev/null
t "swap: the new pane is on the left now" '.result.layout.panes[0].pane_id == "w1:p2"' herdr pane layout --pane w1:p1
a=$(herdr pane layout --pane w1:p1 | jq -r '.result.layout.panes[0].rect.width')
herdr pane resize --pane w1:p2 --direction left --amount 0.2 >/dev/null
b=$(herdr pane layout --pane w1:p1 | jq -r '.result.layout.panes[0].rect.width')
[ "$b" -lt "$a" ] && ok "resize left shrinks the pane at the left edge ($a -> $b)" || bad "resize: $a -> $b"
herdr pane rename w1:p2 bhote-left >/dev/null
t "rename: the label is in the list" '.result.panes[] | select(.pane_id == "w1:p2") | .label == "bhote-left"' herdr pane list
herdr pane run w1:p1 "echo run_$((20+22))" >/dev/null; sleep 0.6
herdr pane read w1:p1 --source recent-unwrapped --lines 20 | grep -q run_42 && ok "run types the command, read returns the screen text" || bad "run/read: $(herdr pane read w1:p1)"
herdr pane run w1:p1 'herdr pane list | wc -c > '"$T"'/inner' >/dev/null; sleep 0.8
[ "$(cat "$T/inner" 2>/dev/null || echo 0)" -gt 100 ] && ok "inside a pane the herdr link answers (the scripts of bhote work unchanged)" || bad "inner herdr: $(cat "$T/inner" 2>&1)"
herdr pane run w1:p1 'echo "$HERDR_PANE_ID $HERDR_TAB_ID $HERDR_WORKSPACE_ID" > '"$T"'/ids' >/dev/null; sleep 0.6
[ "$(cat "$T/ids")" = "w1:p1 w1:t1 w1" ] && ok "the pane knows its ids (HERDR_PANE_ID, HERDR_TAB_ID, HERDR_WORKSPACE_ID)" || bad "ids: $(cat "$T/ids")"
t "process-info: the foreground processes" '.result.process_info | has("foreground_process_group_id") and (.foreground_processes | type == "array")' herdr pane process-info --pane w1:p1
# metadata tokens as the cards use them
herdr pane report-metadata w1:p1 --source bhote --token repo=github.com/acme/api >/dev/null
t "report-metadata: the token shows in the pane" '.result.panes[] | select(.pane_id == "w1:p1") | .tokens.repo == "github.com/acme/api"' herdr pane list
herdr pane report-metadata w1:p1 --source bhote --clear-token repo >/dev/null
t "report-metadata: cleared" '.result.panes[] | select(.pane_id == "w1:p1") | has("tokens") | not' herdr pane list
# workspaces and tabs
herdr workspace create --label second --focus >/dev/null
t "workspace create --focus: two workspaces, the new one focused" '.result.workspaces | length == 2 and .[1].focused == true and .[1].label == "second" and .[1].workspace_id == "w2"' herdr workspace list
herdr workspace focus w1 >/dev/null
t "workspace focus" '.result.workspaces[0].focused == true' herdr workspace list
herdr tab create --workspace w1 --label two >/dev/null
t "tab create: a second tab with a pane of its own" '.result.tabs | map(select(.workspace_id == "w1")) | length == 2' herdr tab list
t "pane list has the pane of the new tab" '.result.panes | map(.tab_id) | unique | length == 3' herdr pane list
herdr agent focus w2:p1 >/dev/null
t "agent focus switches the workspace and focuses the pane" '.result.panes[] | select(.pane_id == "w2:p1") | .focused == true' herdr pane list
t "api snapshot has panes, workspaces and tabs" '.result.snapshot | (.panes | length) == 4 and (.workspaces | length) == 2 and (.tabs | length) == 3' herdr api snapshot
# close: a pane, a workspace
herdr pane close w1:p2 >/dev/null
t "pane close: the sibling takes the whole width" '.result.layout.panes | length == 1 and .[0].rect.width == '"$W"'' herdr pane layout --pane w1:p1
herdr workspace close w2 >/dev/null
t "workspace close" '.result.workspaces | length == 1' herdr workspace list
herdr pane get nope >/dev/null 2>&1 && bad "an unknown pane is an error" || ok "an unknown pane is an error"
exit $fail
