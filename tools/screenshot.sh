#!/usr/bin/env bash
# Renders the panel with demo topics and agents (a fake herdr, throw-away data) and writes docs/screenshot.svg.
# Usage: bash tools/screenshot.sh [out.svg]   (needs python3; nothing touches your own config, data or herdr)
cd "$(dirname "$0")/.." || exit 1
out=${1:-docs/screenshot.svg}
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID
export TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared HOME=$T PATH="$T/bin:$PATH" LC_ALL=en_US.UTF-8
mkdir -p "$T/bin"
cat > "$T/bin/herdr" <<'FAKE'
#!/bin/sh
# the panes carry the project tokens bhote would report itself (PS = Shop's id, PA = API's id, from the demo setup)
PS=$(cat "$TMPDIR/ps" 2>/dev/null); PA=$(cat "$TMPDIR/pa" 2>/dev/null)
case "$*" in
 *"pane list"*) printf '{"result":{"panes":[
  {"agent":"claude","agent_status":"working","pane_id":"w1:p1","workspace_id":"w1","tab_id":"t1","tokens":{"project":"%s"}},
  {"agent":"claude","agent_status":"blocked","pane_id":"w2:p1","workspace_id":"w2","tab_id":"t2","tokens":{"project":"%s"}},
  {"agent":"claude","agent_status":"idle","pane_id":"w3:p1","workspace_id":"w3","tab_id":"t3"},
  {"agent":"codex","agent_status":"working","pane_id":"w4:p1","workspace_id":"w4","tab_id":"t4","tokens":{"project":"%s"}},
  {"agent":"claude","agent_status":"idle","pane_id":"w5:p1","workspace_id":"w5","tab_id":"t5","tokens":{"project":"%s"}}]}}\n' "$PS" "$PS" "$PA" "$PA" ;;
 *"workspace list"*) printf '%s\n' '{"result":{"workspaces":[
  {"workspace_id":"w1","label":"shop"},{"workspace_id":"w2","label":"shop-wt1"},{"workspace_id":"w3","label":"notes"},
  {"workspace_id":"w4","label":"api"},{"workspace_id":"w5","label":"api-wt2"}]}}' ;;
esac
exit 0
FAKE
chmod +x "$T/bin/herdr"
printf 'SETUP_DONE=1\nWIZARD_LEVEL=99\nTHEME=catppuccin\nUPDATE_CHECK=off\nNOTIFY=off\nPROJECT_COLORS=shop=teal,api=mauve\n' > "$BHOTE_CONFIG"
b() { bash ./bhote "$@" >/dev/null; }
b project add Shop; b project add API
bash ./bhote project list --json | python3 -c 'import json,sys; [open(sys.argv[1]+"/p"+p["name"][0].lower(),"w").write(p["id"]) for p in json.load(sys.stdin)]' "$T"
b add "Checkout: retry failed payments" -d "Stripe webhooks time out under load" -s now
b add "Release notes for 2.4" -s next
b add "Staging deploy of the API" -w "DevOps" -d "then run the e2e suite against staging"
b add "Review: cache headers for /assets" -d "PR #412, ready for a look"; b review "cache headers"
b add "Rename the export job"; b done "export job"
b add "Fix flaky login test"; b done "flaky login"
BHOTE_FORCE_COLOR=1 BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=46 BHOTE_ROWS=${ROWS:-40} bash ./bhote > "$T/frame"
[ -n "${RAW:-}" ] && { cat "$T/frame"; exit 0; }
python3 -I tools/ansi2svg.py "$T/frame" > "$out" && echo "wrote $out"
