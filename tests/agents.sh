#!/usr/bin/env bash
# Tests of the agent lists: a running test makes an agent busy, escape codes are stripped, and exactly one panel collects.
# A fake herdr answers; nothing touches the real one. Usage: bash tests/agents.sh
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared
mkdir -p "$T/bin"
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
case "$*" in
 *"pane run"*) echo "$*" >> "$FAKE_LOG" ;;
 *"pane list"*) printf '%s\n' '{"result":{"panes":[{"agent":"claude","agent_status":"idle","pane_id":"w1:p1","workspace_id":"w1","tokens":{"task":"∟ a\u001b[2Jb"}},{"agent":"claude","agent_status":"idle","pane_id":"w2:p1","workspace_id":"w2"},{"agent":"claude","agent_status":"working","pane_id":"w3:p1","workspace_id":"w3"},{"agent":"claude","agent_status":"idle","pane_id":"w4:p1;x","workspace_id":"w4"},{"agent":"claude","agent_status":"done","pane_id":"w9:p2","workspace_id":"w9"}]}}' ;;
 *"workspace list"*) printf '%s\n' '{"result":{"workspaces":[{"workspace_id":"w1","label":"Project A","tokens":{"tests":"◌ Test (C 30%)"}},{"workspace_id":"w2","label":"bil\u001b[31mendo"},{"workspace_id":"w3","label":"x","tokens":{"tests":"◌ Test (queued)"}}]}}' ;;
esac
SH
chmod +x "$T/bin/herdr"; export PATH="$T/bin:$PATH"
# shellcheck disable=SC1091
. ./bhote
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
out=$(collect_one Mac)
echo "$out" | grep -q "^Mac${US}w1:p1${US}idle" && ok "setting off (default): a running test does not change the state" || bad "default must ignore tests: $out"
cfg_set TESTS_BUSY on
out=$(collect_one Mac)
echo "$out" | grep -q "^Mac${US}w1:p1${US}testing" && ok "a running test makes a free agent busy (testing)" || bad "testing state: $out"
echo "$out" | grep -q "^Mac${US}w3:p1${US}working" && ok "a working agent stays working" || bad "working state: $out"
echo "$out" | grep -q "^Mac${US}w2:p1${US}idle" && ok "an agent without a test stays free (idle)" || bad "idle state: $out"
case "$out" in *$'\033'*) bad "escape code reached the agent list" ;; *) ok "escape codes are stripped from names and tasks" ;; esac
case "$out" in *'w4:p1;x'*) bad "odd pane id accepted" ;; *) ok "a pane id with odd characters is dropped" ;; esac

# delegating a topic: the title and description go to a FREE agent, with --machine when it lives on another machine
export FAKE_LOG=$T/herdr.log; : > "$FAKE_LOG"; HOST=Mac
topic_new "Deploy fix" "" "needs care"; SEL_ID=$NEW_TOPIC_ID; topics_load
AG_N=3; AG_LINES=($'Mac\037w1:p1\037idle\037Project\037' $'Laptop\037w9:p2\037done\037server\037' $'Mac\037w3:p1\037working\037busy\037')
pick_build
[ "${#PICK_MAP[@]}" = 2 ] && ok "picker: only the two free agents are offered (the working one is not)" || bad "picker offers ${#PICK_MAP[@]} agents"
pick_do 1; wait
grep -qx "pane run w1:p1 -- Deploy fix. needs care" "$FAKE_LOG" && ok "delegate: a local agent gets the text, no --machine" || bad "local delegate: $(cat "$FAKE_LOG")"
[ "$(topic_get "$TOPIC_DIR/$NEW_TOPIC_ID.topic" agent_pane)" = "w1:p1" ] && ok "delegate: the topic remembers the agent" || bad "agent not stored"
topic_new "Second" "" ""; SEL_ID=$NEW_TOPIC_ID; topics_load; pick_build
[ "${#PICK_MAP[@]}" = 1 ] && ok "picker: an agent that holds a running topic is not offered again" || bad "taken agent still offered (${#PICK_MAP[@]})"
pick_do 1; wait
grep -qx -e "--machine Laptop pane run w9:p2 -- Second" "$FAKE_LOG" && ok "delegate: a remote agent is reached with --machine" || bad "remote delegate: $(cat "$FAKE_LOG")"
AG_LINES=($'Mac\037w1:p1\037working\037Project\037' $'Laptop\037w9:p2\037done\037server\037'); : > "$FAKE_LOG"
topic_new "Third" "" ""; SEL_ID=$NEW_TOPIC_ID; topics_load; PICK_MAP=(0); pick_do 1; wait
[ ! -s "$FAKE_LOG" ] && ok "delegate: an agent that became busy meanwhile gets nothing" || bad "sent to a busy agent: $(cat "$FAKE_LOG")"

# one collector per machine: the first live process leads, the others follow; a dead leader is replaced
sleep 30 & A=$!; sleep 30 & B=$!
collector_lead "$A" && ok "collector: first panel becomes the collector" || bad "first panel did not lead"
collector_lead "$B" && bad "collector: a second panel also leads" || ok "collector: the second panel follows"
collector_lead "$A" && ok "collector: the leader stays the leader" || bad "leader lost the lead"
kill "$A"; wait "$A" 2>/dev/null
collector_lead "$B" && ok "collector: a follower takes over when the leader is gone" || bad "no takeover"
kill "$B"; wait "$B" 2>/dev/null
[ "$(ls -ld "$SHARED_DIR" | cut -c1-10)" = "drwx------" ] && ok "the shared folder is private (0700)" || bad "shared folder is open"
rm -rf "$T"; exit $fail
