#!/usr/bin/env bash
# Tests of the agent lists: a running test makes an agent busy, escape codes are stripped, and exactly one panel collects.
# A fake herdr answers; nothing touches the real one. Usage: bash tests/agents.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared
mkdir -p "$T/bin"
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
[ -n "${FAKE_DOWN:-}" ] && case "$*" in *--machine*) exit 1 ;; esac   # the other machine does not answer
case "$*" in
 *"pane run"*|*"agent prompt"*|*"notification show"*|*"plugin action invoke"*|*"report-metadata"*) echo "$*" >> "$FAKE_LOG" ;;
 *"machine list"*) printf 'id1\tLaptop\tflo@laptop\tdefault\tenabled\n' ;;
 *"pane list"*) printf '%s\n' '{"result":{"panes":[{"agent":"claude","agent_status":"idle","pane_id":"w1:p1","workspace_id":"w1","tokens":{"task":"∟ a\u001b[2Jb"}},{"agent":"claude","agent_status":"idle","pane_id":"w2:p1","workspace_id":"w2","cwd":"/repo/b"},{"agent":"claude","agent_status":"working","pane_id":"w3:p1","workspace_id":"w3","cwd":"/repo/a"},{"agent":"claude","agent_status":"idle","pane_id":"w4:p1;x","workspace_id":"w4"},{"agent":"claude","agent_status":"done","pane_id":"w9:p2","workspace_id":"w9"}]}}' ;;
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
grep -q "^agent prompt w1:p1 Your bhote topic: Deploy fix. needs care (bhote topic $NEW_TOPIC_ID: when you are finished, run \`bhote review $NEW_TOPIC_ID\`" "$FAKE_LOG" && ok "delegate: a local agent gets the topic with its id and how to report back, no --machine" || bad "local delegate: $(cat "$FAKE_LOG")"
[ "$(topic_get "$TOPIC_DIR/$NEW_TOPIC_ID.topic" agent_pane)" = "w1:p1" ] && ok "delegate: the topic remembers the agent" || bad "agent not stored"
topic_new "Second" "" ""; SEL_ID=$NEW_TOPIC_ID; topics_load; pick_build
[ "${#PICK_MAP[@]}" = 1 ] && ok "picker: an agent that holds a running topic is not offered again" || bad "taken agent still offered (${#PICK_MAP[@]})"
pick_do 1; wait
grep -q -e "^--machine Laptop agent prompt w9:p2 Your bhote topic: Second (bhote topic" "$FAKE_LOG" && ok "delegate: a remote agent is reached with --machine" || bad "remote delegate: $(cat "$FAKE_LOG")"
AG_LINES=($'Mac\037w1:p1\037working\037Project\037' $'Laptop\037w9:p2\037done\037server\037'); : > "$FAKE_LOG"
topic_new "Third" "" ""; SEL_ID=$NEW_TOPIC_ID; topics_load; PICK_MAP=(0); pick_do 1; wait
[ ! -s "$FAKE_LOG" ] && ok "delegate: an agent that became busy meanwhile gets nothing" || bad "sent to a busy agent: $(cat "$FAKE_LOG")"

# events from herdr: an agent that stops working moves its topic to review and notifies
unset CLAUDECODE; : > "$FAKE_LOG"
topic_new "Evented" "" ""; ef="$TOPIC_DIR/$NEW_TOPIC_ID.topic"
topic_set "$ef" agent "IMS"; topic_set "$ef" agent_machine Mac; topic_set "$ef" agent_pane w5:p1; topic_set "$ef" agent_session "s-777"; topic_set "$ef" delegated 1
HERDR_PLUGIN_EVENT_JSON='{"event":"pane_agent_status_changed","data":{"pane_id":"w5:p1","agent_status":"working"}}' cli_event
[ "$(topic_get "$ef" status)" = now ] && ok "event: working keeps the topic running" || bad "working changed the topic"
[ -e "$SHARED_DIR/poke" ] && ok "event: the collector is poked (panels update at once)" || bad "no poke"
HERDR_PLUGIN_EVENT_JSON='{"event":"pane_agent_status_changed","data":{"pane":{"pane_id":"w0:p0","agent_status":"done","agent_session":{"value":"s-777"}}}}' cli_event
[ "$(topic_get "$ef" status)" = review ] && ok "event: the agent's session finished -> review" || bad "done event did not move to review: $(topic_get "$ef" status)"
for _w in 1 2 3 4 5 6; do grep -q "notification show" "$FAKE_LOG" && break; sleep 0.5; done; grep -q "notification show Ready for review" "$FAKE_LOG" && ok "event: a herdr notification says it is ready for review" || bad "no notification: $(cat "$FAKE_LOG")"
topic_new "Fresh" "" ""; ff="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$ff" agent_pane w6:p1; topic_set "$ff" agent_machine Mac; topic_set "$ff" delegated "$(date +%s)"
HERDR_PLUGIN_EVENT_JSON='{"data":{"pane_id":"w6:p1","agent_status":"idle"}}' cli_event
[ "$(topic_get "$ff" status)" = now ] && ok "event: the idle right after handing over is ignored" || bad "fresh delegation moved to review"
cfg_set NOTIFY off; : > "$FAKE_LOG"; topic_set "$ff" delegated 1
HERDR_PLUGIN_EVENT_JSON='{"data":{"pane_id":"w6:p1","agent_status":"done"}}' cli_event
[ ! -s "$FAKE_LOG" ] && ok "NOTIFY=off: no notification" || bad "notified although NOTIFY=off"
cfg_set NOTIFY on
# bhote current: the topic of the agent that asks
topic_new "Mine" "" "my part"; mf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mf" agent_session "sess-42"
out=$(CLAUDE_CODE_SESSION_ID=sess-42 cli_current); case "$out" in *"Mine. my part (bhote topic $NEW_TOPIC_ID"*) ok "current: an agent gets its own topic and how to report back" ;; *) bad "current: $out" ;; esac
out=$(CLAUDE_CODE_SESSION_ID=nobody HERDR_PANE_ID=zz:p9 cli_current); [ -z "$out" ] && ok "current: nothing for an agent without a topic" || bad "current for nobody: $out"

# Steal & Transfer: the source agent is told to commit and hand over, the target takes the branch over
unset CLAUDECODE; : > "$FAKE_LOG"; HOST=Mac
topic_new "Big refactor" "" ""; tf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; tid=$NEW_TOPIC_ID
transfer_start "$tf" Mac w3:p1 busy Mac w2:p1 server; wait
grep -q "^agent prompt w3:p1 Steal & Transfer (bhote topic $tid" "$FAKE_LOG" && ok "transfer: the source agent is told to stop, commit and hand over" || bad "no handover prompt: $(cat "$FAKE_LOG")"
grep -q "bhote handover $tid --to 'Mac/w2:p1' --branch" "$FAKE_LOG" && ok "transfer: it gets the exact handover command" || bad "handover command missing"
grep -q "do not create a new branch" "$FAKE_LOG" && ok "transfer: the branch is kept (no new branch)" || bad "branch rule missing"
[ "$(topic_get "$tf" transfer_to)" = "Mac/w2:p1" ] && ok "transfer: the topic remembers where it goes" || bad "transfer_to: $(topic_get "$tf" transfer_to)"
topics_load; COLS=60; out=$(topics_block); case "$out" in *"busy → server · handing over"*) ok "transfer: the list says it is being handed over" ;; *) bad "meta: $out" ;; esac
: > "$FAKE_LOG"; transfer_finish "$tf" "Mac/w2:p1" "feat/x" "abc123def"; wait; sleep 0.3
grep -q "^agent prompt w2:p1 Steal & Transfer: take over \"Big refactor\" (bhote topic $tid) from busy. The work is on branch feat/x" "$FAKE_LOG" && ok "handover: the target agent gets the branch and the handover" || bad "no takeover prompt: $(cat "$FAKE_LOG")"
[ "$(topic_get "$tf" agent_pane)" = w2:p1 ] && [ -z "$(topic_get "$tf" transfer_to)" ] && ok "handover: the topic now belongs to the target" || bad "topic not moved"
grep -q "notification show Transferred" "$FAKE_LOG" && ok "handover: a notification says so" || bad "no notification"
topic_new "Other" "" ""; of="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; : > "$FAKE_LOG"
transfer_start "$of" Mac w2:p1 server Mac w3:p1 busy 2>/dev/null && bad "transfer to a busy agent was accepted" || ok "transfer: a busy target is refused"
[ ! -s "$FAKE_LOG" ] && ok "transfer: nothing is sent when it is refused" || bad "sent although refused"
# the wait is over: a waiting topic of an agent, set to now, makes that agent go on
topic_new "Wait for deploy" "DevOps" "test the login"; wf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$wf" agent IMS; topic_set "$wf" agent_machine Mac; topic_set "$wf" agent_pane w1:p1
: > "$FAKE_LOG"; topic_resume "$wf"; wait
grep -q "^agent prompt w1:p1 The wait is over (DevOps): Wait for deploy. Go on with: test the login" "$FAKE_LOG" && ok "resume: the waiting agent is told to go on" || bad "no resume prompt: $(cat "$FAKE_LOG")"
[ "$(topic_get "$wf" status)" = now ] && ok "resume: the topic is running again" || bad "status after resume"
# agent refs
AG_N=3; AG_LINES=($'Mac\037w1:p1\037idle\037Project\037' $'Laptop\037w9:p2\037done\037server\037' $'Mac\037w3:p1\037working\037Project two\037')
agent_ref 2 && [ "$AR_P" = w9:p2 ] && ok "agent ref: a number" || bad "agent ref number"
agent_ref "Laptop/w9:p2" && [ "$AR_NAME" = server ] && ok "agent ref: machine/pane" || bad "agent ref machine/pane"
agent_ref "server" && [ "$AR_M" = Laptop ] && ok "agent ref: part of the name" || bad "agent ref name"
agent_ref "project" 2>/dev/null; [ $? = 2 ] && ok "agent ref: ambiguous is exit 2" || bad "agent ref ambiguous"

# one collector per machine: the first live process leads, the others follow; a dead leader is replaced
sleep 30 & A=$!; sleep 30 & B=$!
collector_lead "$A" && ok "collector: first panel becomes the collector" || bad "first panel did not lead"
collector_lead "$B" && bad "collector: a second panel also leads" || ok "collector: the second panel follows"
collector_lead "$A" && ok "collector: the leader stays the leader" || bad "leader lost the lead"
kill "$A"; wait "$A" 2>/dev/null
collector_lead "$B" && ok "collector: a follower takes over when the leader is gone" || bad "no takeover"
kill "$B"; wait "$B" 2>/dev/null
[ "$(ls -ld "$SHARED_DIR" | cut -c1-10)" = "drwx------" ] && ok "the shared folder is private (0700)" || bad "shared folder is open"
# hardening: a handover needs a pending transfer to exactly that agent; another status ends a pending transfer
out=$(BHOTE_NO_SYNC=1 cli_handover "$tid" --to Mac/w2:p1 2>&1); [ $? = 1 ] && case "$out" in *"no transfer is pending"*) true ;; *) false ;; esac && ok "handover: refused without a pending transfer" || bad "handover without transfer: $out"
topic_set "$tf" transfer_to "Mac/w2:p1"
out=$(BHOTE_NO_SYNC=1 cli_handover "$tid" --to Mac/w9:p9 2>&1); [ $? = 1 ] && ok "handover: refused for another target" || bad "handover to another target: $out"
out=$(BHOTE_NO_SYNC=1 cli_handover "$tid" extra --to Mac/w2:p1 2>&1); [ $? = 1 ] && ok "handover: refused with a second topic word" || bad "handover extra arg: $out"
topic_set "$tf" status later; [ -z "$(topic_get "$tf" transfer_to)" ] && ok "status: parking a topic ends a pending transfer" || bad "transfer_to kept after later"
# the transfer picker: index 0 does not exist there (bash 4 would read the last agent)
: > "$FAKE_LOG"; SEL_ID=$tid; topic_set "$tf" status now; PICK_MODE=transfer; pick_do 0; wait
[ ! -s "$FAKE_LOG" ] && ok "transfer picker: index 0 does nothing" || bad "pick_do 0 sent: $(cat "$FAKE_LOG")"
# "just mark as started" forgets the old agent
PICK_MODE=start; pick_do 0; [ -z "$(topic_get "$tf" agent_pane)$(topic_get "$tf" agent_machine)$(topic_get "$tf" agent_session)" ] && ok "just started: no agent left on the topic" || bad "agent fields kept"
# a source agent that waits for a permission cannot hand over
blocked_live() { LIVE_ST=blocked; LIVE_SESS=""; LIVE_CWD=""; }
( agent_live() { if [ "$2" = w3:p1 ]; then blocked_live; else LIVE_ST=idle; fi; }
  transfer_start "$tf" Mac w3:p1 busy Mac w2:p1 server 2>/dev/null ) && bad "blocked source accepted" || ok "transfer: a blocked source agent is refused"
# push: an agent event here tells the other machines at once (only with REMOTE_AGENTS on); remote-changed marks a query
: > "$FAKE_LOG"; cfg_set REMOTE_AGENTS on
HERDR_PLUGIN_EVENT_JSON='{"data":{"pane_id":"w1:p1","agent_status":"working"}}' cli_event
grep -q -- "--machine Laptop plugin action invoke changed --plugin bhote.panel" "$FAKE_LOG" && ok "push: the other machine is told at once" || bad "no push: $(cat "$FAKE_LOG")"
cfg_set PUSH_STATES off; : > "$FAKE_LOG"; HERDR_PLUGIN_EVENT_JSON='{"data":{"pane_id":"w1:p1","agent_status":"idle"}}' cli_event
grep -q "invoke changed" "$FAKE_LOG" && bad "pushed although PUSH_STATES=off" || ok "push: PUSH_STATES=off keeps quiet"
rm -f "$SHARED_DIR/remote.due"; ( bhote_cli remote-changed ); [ -e "$SHARED_DIR/remote.due" ] && ok "remote-changed: the collector asks the other machines at once" || bad "remote.due missing"
# a slow merge (over herdr a few seconds) must not hold up a push: the collector merges in the background
: > "$SHARED_DIR/agents.remote.tmp.99999"; touch -t 202001010000 "$SHARED_DIR/agents.remote.tmp.99999"
( store_sync() { sleep 3; }; collect_local() { :; }; collect_remote() { :; }
  sleep 30 & P=$!; rm -f "$SHARED_DIR/collector.lock" "$SHARED_DIR/remote.due"; collector "$P" & C=$!
  sleep 1; : > "$SHARED_DIR/remote.due"; : > "$SHARED_DIR/poke"; sleep 0.8
  [ -e "$SHARED_DIR/remote.due" ] && r=1 || r=0; kill "$C" "$P" 2>/dev/null; wait "$C" "$P" 2>/dev/null; exit $r ) \
  && ok "collector: a push is taken while a slow merge runs" || bad "collector: the push waited for the merge"
[ -e "$SHARED_DIR/agents.remote.tmp.99999" ] && bad "an old temp list was left" || ok "collector: old temp lists of a killed collector are removed"
# a machine that does not answer (herdr reconnecting): its agents stay as seen last, muted, with no work and no dancing star
cfg_set REMOTE_AGENTS on; rm -f "$AGENT_REMOTE" "$SHARED_DIR/machines.offline"; collect_remote
grep -q "^Laptop${US}w3:p1${US}working" "$AGENT_REMOTE" && ok "remote: the other machine's agents are there" || bad "remote agents: $(cat "$AGENT_REMOTE")"
FAKE_DOWN=1 collect_remote
grep -q "^Laptop${US}w3:p1${US}off-working" "$AGENT_REMOTE" && grep -qx Laptop "$SHARED_DIR/machines.offline" && ok "remote: unreachable, its agents kept as off-<state>" || bad "offline keep: $(cat "$AGENT_REMOTE")"
FAKE_DOWN=1 collect_remote; grep -q "${US}off-off-" "$AGENT_REMOTE" && bad "off- doubled" || ok "remote: still away, marked once"
collect_local; frame=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=0; AG_LINES=()
  while IFS= read -r l; do [ -n "$l" ] && { AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); }; done < <(agents_all); agents_block )
echo "$frame" | grep -q "Laptop ◐ reconnecting" && ok "panel: the machine says reconnecting, its agents stay listed" || bad "panel reconnecting: $(echo "$frame" | grep -n 'Laptop\|┈')"
( AG_N=0; AG_LINES=(); while IFS= read -r l; do AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); done < "$AGENT_REMOTE"; agent_state_set s Laptop w3:p1; [ "$s" = off-working ] ) \
  && ok "remote: a topic's agent on it reads off-working (shown as offline)" || bad "agent state offline"
collect_remote; grep -q "^Laptop${US}w3:p1${US}working" "$AGENT_REMOTE" && [ ! -s "$SHARED_DIR/machines.offline" ] && ok "remote: back again, live states" || bad "back online"
cfg_set REMOTE_AGENTS off
# projects: this machine's agents get repo and project tokens (only on a change); the panel reads them back
mkdir -p "$T/prj"; git -C "$T/prj" init -q -b main; git -C "$T/prj" remote add origin https://github.com/acme/ims.git
BHOTE_NO_SYNC=1 bhote_cli project add IMS >/dev/null; BHOTE_NO_SYNC=1 bhote_cli project pin IMS "$T/prj" >/dev/null
printf 'Mac%sw1:p1%st1%sa%s%s%s%s%s\n' "$US" "$US" "$US" "$US" "" "$US" "" "$US" > "$AGENT_META_L"; sed -i.bak "s#\$#$T/prj#" "$AGENT_META_L"
: > "$FAKE_LOG"; project_sync
grep -q "report-metadata --source bhote --token repo=github.com/acme/ims --token project=proj-ims w1:p1" "$FAKE_LOG" && ok "projects: the agent gets its repo and project tokens" || bad "tokens: $(cat "$FAKE_LOG")"
sed -i.bak "s#${US}a${US}${US}${US}#${US}a${US}github.com/acme/ims${US}proj-ims${US}#" "$AGENT_META_L"; : > "$FAKE_LOG"; project_sync
[ -s "$FAKE_LOG" ] && bad "tokens set again: $(cat "$FAKE_LOG")" || ok "projects: unchanged tokens are not set again"
agent_meta Mac w1:p1 && [ "$AM_PROJECT" = proj-ims ] && [ "$AM_TAB" = t1 ] && ok "projects: agent_meta reads project and tab" || bad "agent_meta: $AM_PROJECT $AM_TAB"
# GitHub: the branch of a topic's agent, read by the machine it runs on; a line in the panel, inline in the search, in the JSON
mkdir -p "$T/repo/src"; git -C "$T/repo" init -q -b feature/login 2>/dev/null
bid=$(BHOTE_NO_SYNC=1 bhote_cli add "Login form" --json | jq -r .id); bf="$TOPIC_DIR/$bid.topic"
topic_set "$bf" agent "Project A"; topic_set "$bf" agent_machine "$HOST"; topic_set "$bf" agent_pane "w1:p1"
printf 'w1:p1%s%s\n' "$US" "$T/repo/src" > "$AGENT_CWD"; branch_sync
[ "$(topic_get "$bf" agent_branch)" = feature/login ] && ok "github: the agent's branch is written into its topic" || bad "branch_sync: '$(topic_get "$bf" agent_branch)'"
u=$(topic_get "$bf" updated); sleep 1; branch_sync; [ "$(topic_get "$bf" updated)" = "$u" ] && ok "github: an unchanged branch is not written again" || bad "branch rewritten"
frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=60 bash ./bhote </dev/null)
echo "$frame" | grep -A3 "Login form" | grep -q "⎇ feature/login" && ok "github: a branch line under the topic" || bad "panel branch line: $(echo "$frame" | grep -A3 'Login form')"
( topics_load; AG_N=0; AG_LINES=(); find_items; for (( k = 0; k < ${#FI_KIND[@]}; k++ )); do [ "${FI_REF[$k]}" = "$bid" ] && break; done
  case "${FI_LABEL[$k]}" in *"⎇ feature/login"*) exit 0 ;; *) exit 1 ;; esac ) && ok "github: the branch inline in the search" || bad "search branch"
[ "$(BHOTE_NO_SYNC=1 bhote_cli show "$bid" --json | jq -r .agent.branch)" = feature/login ] && ok "github: agent.branch in the JSON" || bad "json branch"
cfg_set GITHUB_BRANCHES off; frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=60 bash ./bhote </dev/null)
echo "$frame" | grep -q "⎇" && bad "branch shown with GITHUB_BRANCHES=off" || ok "github: off hides the branch"
rm -rf "$T"; exit $fail
