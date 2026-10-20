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
 *"pane run"*|*"agent prompt"*|*"notification show"*|*"plugin action invoke"*|*"report-metadata"*|*"agent focus"*) echo "$*" >> "$FAKE_LOG" ;;
 *"machine list"*) printf 'id1\tLaptop\tflo@laptop\tdefault\tenabled\n' ;;
 *"pane get w1:p1"*) printf '%s\n' '{"id":"x","result":{"pane":{"agent":"claude","focused":true,"pane_id":"w1:p1"}}}' ;;
 *"pane get w2:p1"*) printf '%s\n' '{"id":"x","result":{"pane":{"agent":"claude","focused":false,"pane_id":"w2:p1"}}}' ;;
 *"pane list"*) printf '%s\n' '{"result":{"panes":[{"agent":"claude","agent_status":"idle","pane_id":"w1:p1","workspace_id":"w1","tokens":{"task":"∟ a\u001b[2Jb"}},{"agent":"claude","agent_status":"idle","pane_id":"w2:p1","workspace_id":"w2","cwd":"/repo/b"},{"agent":"claude","agent_status":"working","pane_id":"w3:p1","workspace_id":"w3","cwd":"/repo/a","tokens":{"now":"✻ Warm-up clock \u001b[2J"}},{"agent":"claude","agent_status":"idle","pane_id":"w4:p1;x","workspace_id":"w4"},{"agent":"claude","agent_status":"done","pane_id":"w9:p2","workspace_id":"w9"}]}}' ;;
 *"workspace list"*) printf '%s\n' '{"result":{"workspaces":[{"workspace_id":"w1","label":"Project A","tokens":{"tests":"◌ Test (C 30%)"}},{"workspace_id":"w2","label":"bil\u001b[31mendo"},{"workspace_id":"w3","label":"x","tokens":{"tests":"◌ Test (queued)"}}]}}' ;;
esac
SH
chmod +x "$T/bin/herdr"; export PATH="$T/bin:$PATH"
# shellcheck disable=SC1091
. ./bhote
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
cfg_set TESTS_BUSY off
out=$(collect_one Mac)
echo "$out" | grep -q "^Mac${US}w1:p1${US}idle" && ok "setting off: a running test does not change the state" || bad "default must ignore tests: $out"
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
topic_set "$ef" agent "Shop"; topic_set "$ef" agent_machine Mac; topic_set "$ef" agent_pane w5:p1; topic_set "$ef" agent_session "s-777"; topic_set "$ef" delegated 1
HERDR_PLUGIN_EVENT_JSON='{"event":"pane_agent_status_changed","data":{"pane_id":"w5:p1","agent_status":"working"}}' cli_event
[ "$(topic_get "$ef" status)" = now ] && ok "event: working keeps the topic running" || bad "working changed the topic"
[ -e "$SHARED_DIR/poke" ] && ok "event: the collector is poked (panels update at once)" || bad "no poke"
BHOTE_FAKE_QUESTION="Soll ich weitermachen?" HERDR_PLUGIN_EVENT_JSON='{"event":"pane_agent_status_changed","data":{"pane":{"pane_id":"w0:p0","agent_status":"done","agent_session":{"value":"s-777"}}}}' cli_event
[ "$(topic_get "$ef" status)" = review ] && ok "event: the agent's session finished with a question -> review" || bad "done event did not move to review: $(topic_get "$ef" status)"
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
topics_load; COLS=60; out=$(topics_block); case "$out" in *"handing over"*"busy → server"*) ok "transfer: the list says it is being handed over (the state on the right, where it goes under the task)" ;; *) bad "meta: $out" ;; esac
: > "$FAKE_LOG"; transfer_finish "$tf" "Mac/w2:p1" "feat/x" "abc123def"; wait; sleep 0.3
grep -q "^agent prompt w2:p1 Steal & Transfer: take over \"Big refactor\" (bhote topic $tid) from busy. The work is on branch feat/x" "$FAKE_LOG" && ok "handover: the target agent gets the branch and the handover" || bad "no takeover prompt: $(cat "$FAKE_LOG")"
[ "$(topic_get "$tf" agent_pane)" = w2:p1 ] && [ -z "$(topic_get "$tf" transfer_to)" ] && ok "handover: the topic now belongs to the target" || bad "topic not moved"
grep -q "notification show Transferred" "$FAKE_LOG" && ok "handover: a notification says so" || bad "no notification"
topic_new "Other" "" ""; of="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; : > "$FAKE_LOG"
transfer_start "$of" Mac w2:p1 server Mac w3:p1 busy 2>/dev/null && bad "transfer to a busy agent was accepted" || ok "transfer: a busy target is refused"
[ ! -s "$FAKE_LOG" ] && ok "transfer: nothing is sent when it is refused" || bad "sent although refused"
# the wait is over: a waiting topic of an agent, set to now, makes that agent go on
topic_new "Wait for deploy" "DevOps" "test the login"; wf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$wf" agent Shop; topic_set "$wf" agent_machine Mac; topic_set "$wf" agent_pane w1:p1
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
echo "$frame" | grep -q "LPT ◐ reconnecting" && echo "$frame" | grep -q "LPT/w3:a1|" && ok "panel: the machine (its code) says reconnecting, its agents stay listed" || bad "panel reconnecting: $(echo "$frame" | grep -n 'LPT\|┈')"
( AG_N=0; AG_LINES=(); while IFS= read -r l; do AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); done < "$AGENT_REMOTE"; agent_state_set s Laptop w3:p1; [ "$s" = off-working ] ) \
  && ok "remote: a topic's agent on it reads off-working (shown as offline)" || bad "agent state offline"
collect_remote; grep -q "^Laptop${US}w3:p1${US}working" "$AGENT_REMOTE" && [ ! -s "$SHARED_DIR/machines.offline" ] && ok "remote: back again, live states" || bad "back online"
# machine codes: three capitals from the name, unique over all machines; set by hand they are a record in the store (every
# machine shows the same); in the list MAC|name, only when more than one machine is listed
[ "$(machine_code_cands Mac | head -1)" = MAC ] && [ "$(machine_code_cands Omarchy | head -1)" = OMR ] && [ "$(machine_code_cands "MacBook Pro" | head -1)" = MBP ] \
  && ok "codes: made from the name (MAC, OMR, MBP)" || bad "code candidates: $(machine_code_cands Omarchy | head -3 | tr '\n' ' ')"
( HOST=Mac; AG_N=2; AG_LINES=($'Mac\037w1:p1\037idle\037a\037' $'Mac2\037w2:p1\037idle\037b\037'); MC_KEY=-; machine_codes_load
  machine_code_set a Mac; machine_code_set b Mac2; [ "$a" = MAC ] && [ "$b" != MAC ] && machine_code_ok "$b" ) && ok "codes: unique (a clash takes the next candidate)" || bad "codes not unique"
nt=$(BHOTE_NO_SYNC=1 bhote_cli list --all --json | jq length)
( AG_N=1; AG_LINES=($'Laptop\037w9:p2\037idle\037server\037'); machine_code_save Laptop zzz && MC_KEY=-; machine_codes_load; machine_code_set c Laptop; [ "$c" = ZZZ ] ) \
  && grep -q '^kind=machine' "$TOPIC_DIR/mach-laptop.topic" && ok "codes: set by hand (lower case taken), a record in the store" || bad "code save: $(cat "$TOPIC_DIR/mach-laptop.topic" 2>&1)"
[ "$(BHOTE_NO_SYNC=1 bhote_cli list --all --json | jq length)" = "$nt" ] && ok "codes: the record is no topic" || bad "code record listed as a topic"
( AG_N=1; AG_LINES=($'Laptop\037w9:p2\037idle\037server\037'); machine_code_save "$HOST" ZZZ 2>/dev/null ) && bad "a taken code was given twice" || ok "codes: a code another machine has is refused"
( machine_code_save Laptop AB 2>/dev/null ) && bad "a two-letter code was taken" || ok "codes: only three letters A-Z"
( AG_N=1; AG_LINES=($'Laptop\037w9:p2\037idle\037server\037'); machine_code_save Laptop "" && MC_KEY=-; machine_codes_load; machine_code_set c Laptop; [ "$c" = LPT ] ) && ok "codes: empty makes it from the name again" || bad "code reset"
frame=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=2; AG_LINES=($'Laptop\037w9:p2\037working\037Laptop / root\037' $'Laptop\037w9:p3\037working\037api\037'); agents_block )
echo "$frame" | grep -q "|" && bad "one machine: codes shown" || ok "codes: one machine listed, no codes"
frame=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=2; AG_LINES=($'Laptop\037w9:p2\037working\037Laptop / root\037' $"$HOST"$'\037w1:p1\037working\037api\037'); agents_block )
echo "$frame" | grep -q "LPT/w9:a2|root" && echo "$frame" | grep -q "w1:a1|api" && ! echo "$frame" | grep -qi "^ *laptop *$" && ok "codes: agent|name (LPT/w9:a2|root on another machine, w1:a1|api here), no machine sub-captions" || bad "codes in the list: $frame"
cfg_set REMOTE_AGENTS off
# projects: this machine's agents get repo and project tokens (only on a change); the panel reads them back
mkdir -p "$T/prj"; git -C "$T/prj" init -q -b main; git -C "$T/prj" remote add origin https://github.com/acme/shop.git
BHOTE_NO_SYNC=1 bhote_cli project add Shop >/dev/null; BHOTE_NO_SYNC=1 bhote_cli project pin Shop "$T/prj" >/dev/null
printf 'Mac%sw1:p1%st1%sa%s%s%s%s%s\n' "$US" "$US" "$US" "$US" "" "$US" "" "$US" > "$AGENT_META_L"; sed -i.bak "s#\$#$T/prj#" "$AGENT_META_L"
: > "$FAKE_LOG"; project_sync
grep -q "report-metadata w1:p1 --source bhote --token repo=github.com/acme/shop --token project=proj-shop" "$FAKE_LOG" && ok "projects: the agent gets its repo and project tokens" || bad "tokens: $(cat "$FAKE_LOG")"
sed -i.bak "s#${US}a${US}${US}${US}#${US}a${US}github.com/acme/shop${US}proj-shop${US}#" "$AGENT_META_L"; : > "$FAKE_LOG"; project_sync
[ -s "$FAKE_LOG" ] && bad "tokens set again: $(cat "$FAKE_LOG")" || ok "projects: unchanged tokens are not set again"
agent_meta Mac w1:p1 && [ "$AM_PROJECT" = proj-shop ] && [ "$AM_TAB" = t1 ] && ok "projects: agent_meta reads project and tab" || bad "agent_meta: $AM_PROJECT $AM_TAB"
# the agent list by project (the panel's own first, "no project" last), the topics of the panel's project
BHOTE_NO_SYNC=1 bhote_cli project add Zeta >/dev/null
printf 'Mac%sw1:p1%st1%sa%s%sproj-shop%s\nMac%sw2:p1%st2%sa%s%sproj-zeta%s\nMac%sw3:p1%st3%sa%s%s%s\nMac%sw9:p9%st2%s%s%s%s\n' \
  "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" > "$AGENT_META_L"
collect_local >/dev/null 2>&1; printf 'Mac%sw1:p1%st1%sa%s%sproj-shop%s\nMac%sw2:p1%st2%sa%s%sproj-zeta%s\nMac%sw3:p1%st3%sa%s%s%s\nMac%sw9:p9%st2%s%s%s%s\n' \
  "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" "$US" > "$AGENT_META_L"
grp=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=0; AG_LINES=(); while IFS= read -r l; do [ -n "$l" ] && { AG_LINES[$AG_N]=${l//${US}idle${US}/${US}working${US}}; AG_N=$(( AG_N + 1 )); }; done < "$AGENT_LOCAL"   # (the idle ones are on cards, not in this list)
  projects_load; CUR_PROJ=proj-zeta; agents_block | LC_ALL=C sed 's/\x1b\[[0-9;]*m//g' )
caps=$(echo "$grp" | grep '┈┈' | grep -v ' Agents ' | sed 's/ *┈*$//; s/^ *┈┈ //' | tr '\n' '|')
[ "$caps" = "Zeta|Shop|no project|" ] && ok "agents by project: the panel's own first, no project last" || bad "project captions: $caps"
echo "$grp" | grep -A1 '┈┈ Shop' | tail -1 | grep -q 'Project A' && ok "agents by project: the agent under its project" || bad "agent under project: $grp"
( HERDR_PANE_ID=w9:p9; current_project; [ "$CUR_PROJ" = proj-zeta ] ) && ok "the panel's project: of the agent in its tab" || bad "current project"
t1=$(BHOTE_NO_SYNC=1 bhote_cli add "Zeta work" -p Zeta --json | jq -r .id); BHOTE_NO_SYNC=1 bhote_cli add "Shop work" -p Shop >/dev/null; BHOTE_NO_SYNC=1 bhote_cli add "Loose work" >/dev/null
[ "$(BHOTE_NO_SYNC=1 bhote_cli show "$t1" --json | jq -r .project)" = Zeta ] && ok "add -p: the topic's project" || bad "topic project"
blk=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); NOW=$(date +%s); topics_load; projects_load; CUR_PROJ=proj-zeta; topics_projects; U_SCOPE=project; AG_N=0; topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*m//g' )
echo "$blk" | grep -q "Zeta work" && echo "$blk" | grep -q "Loose work" && ! echo "$blk" | grep -q "Shop work" && ok "scope project: its topics and those without one" || bad "scope: $blk"
blk=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); NOW=$(date +%s); topics_load; projects_load; CUR_PROJ=proj-zeta; topics_projects; U_SCOPE=all; AG_N=0; topics_block )
echo "$blk" | grep -q "Shop work" && ok "scope all: every topic" || bad "scope all"
# Enter on a topic: its agent is focused in herdr (as a click), when herdr knows it now; else 1 (the page opens)
jt=$(BHOTE_NO_SYNC=1 bhote_cli add "Jump topic" --json | jq -r .id); jf="$TOPIC_DIR/$jt.topic"
( SEL_ID=$jt; topics_load; AG_N=0; AG_LINES=(); topic_jump ) && bad "jump without an agent" || ok "enter: no agent, the page opens"
topic_set "$jf" agent_machine "$HOST"; topic_set "$jf" agent_pane "w2:p1"; : > "$FAKE_LOG"
( SEL_ID=$jt; topics_load; AG_N=0; AG_LINES=(); while IFS= read -r l; do [ -n "$l" ] && { AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); }; done < "$AGENT_LOCAL"; topic_jump ) \
  && sleep 0.3 && grep -q "agent focus w2:p1" "$FAKE_LOG" && ok "enter: the topic's agent is focused in herdr" || bad "jump: $(cat "$FAKE_LOG")"
read -r sid _ sact < "$SHARED_DIR/select" 2>/dev/null; [ "$sid $sact" = "$jt list" ] && ok "enter: the panels select the topic (the one in the agent's tab gets the focus)" || bad "jump select: $(cat "$SHARED_DIR/select" 2>&1)"
topic_set "$jf" agent_pane "w7:p7"; ( SEL_ID=$jt; topics_load; AG_N=0; AG_LINES=(); while IFS= read -r l; do [ -n "$l" ] && { AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); }; done < "$AGENT_LOCAL"; topic_jump ) \
  && bad "jump to a gone agent" || ok "enter: a gone agent, the page opens"
# GitHub: the branch of a topic's agent, read by the machine it runs on; a line in the panel, inline in the search, in the JSON
mkdir -p "$T/repo/src"; git -C "$T/repo" init -q -b feature/login 2>/dev/null
bid=$(BHOTE_NO_SYNC=1 bhote_cli add "Login form" --json | jq -r .id); bf="$TOPIC_DIR/$bid.topic"
topic_set "$bf" agent "Project A"; topic_set "$bf" agent_machine "$HOST"; topic_set "$bf" agent_pane "w1:p1"
printf 'w1:p1%s%s\n' "$US" "$T/repo/src" > "$AGENT_CWD"; branch_sync
[ "$(topic_get "$bf" agent_branch)" = feature/login ] && ok "github: the agent's branch is written into its topic" || bad "branch_sync: '$(topic_get "$bf" agent_branch)'"
u=$(topic_get "$bf" updated); sleep 1; branch_sync; [ "$(topic_get "$bf" updated)" = "$u" ] && ok "github: an unchanged branch is not written again" || bad "branch rewritten"
frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=150 bash ./bhote </dev/null)
echo "$frame" | grep -q "⎇ feature/login" && ok "github: a branch line under the topic" || bad "panel branch line: $(echo "$frame" | wc -l) lines"
( topics_load; AG_N=0; AG_LINES=(); find_items; for (( k = 0; k < ${#FI_KIND[@]}; k++ )); do [ "${FI_REF[$k]}" = "$bid" ] && break; done
  case "${FI_LABEL[$k]}" in *"⎇ feature/login"*) exit 0 ;; *) exit 1 ;; esac ) && ok "github: the branch inline in the search" || bad "search branch"
[ "$(BHOTE_NO_SYNC=1 bhote_cli show "$bid" --json | jq -r .agent.branch)" = feature/login ] && ok "github: agent.branch in the JSON" || bad "json branch"
cfg_set GITHUB_BRANCHES off; frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=150 bash ./bhote </dev/null)
echo "$frame" | grep -q "⎇" && bad "branch shown with GITHUB_BRANCHES=off" || ok "github: off hides the branch"
# a machine that does not answer is "unknown", not "gone" (a topic must not be taken for an agent that left)
( cfg_set REMOTE_AGENTS on; machine_known() { return 0; }; FAKE_DOWN=1 agent_live Laptop w1:p1; [ "$LIVE_ST" = unknown ] ) && ok "agent_live: no answer from herdr is unknown, not gone" || bad "agent_live down: $LIVE_ST"
( machine_known() { return 0; }; agent_live "$HOST" w1:p1; [ "$LIVE_ST" = idle ] || [ "$LIVE_ST" = working ] || [ "$LIVE_ST" = done ] ) && ok "agent_live: a local answer is read" || bad "agent_live local: $LIVE_ST"
# an agent without a topic: a review topic only when it asks something (blocked, or its last line is a question); gone when it works again
ev() { HERDR_PLUGIN_EVENT_JSON=$(printf '{"data":{"pane_id":"%s","agent_status":"%s"}}' "$1" "$2") BHOTE_NO_SYNC=1 cli_event; }
rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/evstate"; cfg_set AUTO_REVIEW on; printf '%s\n' "$HOST${US}w9:p1${US}idle${US}Shop (main)${US}task" > "$AGENT_LOCAL"
ev w9:p1 idle; [ "$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | wc -l | tr -d ' ')" = 0 ] && ok "auto review: an agent that was never busy gets no topic" || bad "auto review at start"
BHOTE_FAKE_QUESTION="" ev w9:p1 working; BHOTE_FAKE_QUESTION="" ev w9:p1 idle; [ "$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | wc -l | tr -d ' ')" = 0 ] && ok "auto review: a turn without a question makes no topic" || bad "auto review for a plain hand-back"
ev w9:p1 working; BHOTE_FAKE_QUESTION="Soll ich pushen?" ev w9:p1 idle; af=$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | head -1)
[ -n "$af" ] && [ "$(topic_get "$af" status)" = review ] && [ "$(topic_get "$af" auto)" = 1 ] && topic_get "$af" title | grep -q "Shop (main)" && [ "$(topic_get "$af" description)" = "Soll ich pushen?" ] && ok "auto review: a question makes a review topic with the question" || bad "auto review: $af"
BHOTE_FAKE_QUESTION="Soll ich pushen?" ev w9:p1 idle; [ "$(ls "$TOPIC_DIR"/*.topic | wc -l | tr -d ' ')" = 1 ] && ok "auto review: once only" || bad "auto review twice"
ev w9:p1 working; [ "$(topic_get "$af" deleted)" = 1 ] && ok "auto review: gone when the agent works again" || bad "auto review not removed"
rm -f "$TOPIC_DIR"/*.topic; ev w9:p1 working; ev w9:p1 blocked; [ "$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | wc -l | tr -d ' ')" = 1 ] && ok "auto review: an agent that waits for an answer (blocked) gets a topic" || bad "auto review blocked"
# a topic of its own: now -> review when it hands back, and back to now when the user answered
rm -f "$TOPIC_DIR"/*.topic; topic_new "Real work" "" "x"; tf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$tf" agent_pane w9:p1; topic_set "$tf" agent_machine "$HOST"; topic_set "$tf" status now
ev w9:p1 working; BHOTE_FAKE_QUESTION="Soll ich pushen?" ev w9:p1 idle; [ "$(topic_get "$tf" status)" = review ] && ok "auto review: a topic in work whose agent asks goes to review" || bad "now->review: $(topic_get "$tf" status)"
ev w9:p1 working; [ "$(topic_get "$tf" status)" = now ] && ok "auto review: and back to now when the agent works again" || bad "review->now: $(topic_get "$tf" status)"
# a topic the agent put under review itself (bhote review) goes back to now when it starts working again; a working event while it already works changes nothing
topic_set "$tf" status review; topic_set "$tf" auto_back ""; rm -rf "$SHARED_DIR/evstate"; ev w9:p1 idle
ev w9:p1 working; [ "$(topic_get "$tf" status)" = now ] && ok "review: a topic handed back with bhote review goes to now when the agent works again" || bad "review->now (manual): $(topic_get "$tf" status)"
topic_set "$tf" status review; ev w9:p1 working; [ "$(topic_get "$tf" status)" = review ] && ok "review: a repeated working event while it works changes nothing" || bad "review kept: $(topic_get "$tf" status)"
# which pane you are in: herdr's word for this panel's pane; the head shows it (a dot when it is, dimmed when another pane is)
( HERDR_PANE_ID=w1:p1; [ "$(pane_focused)" = true ] || exit 1; HERDR_PANE_ID=w2:p1; [ "$(pane_focused)" = false ] || exit 2; HERDR_PANE_ID=w7:p9; [ -z "$(pane_focused)" ] || exit 3
  COLS=44; ROWS=30; ui_load; PANE_FOCUS=true; a=$(view_head); PANE_FOCUS=false; b=$(view_head); PANE_FOCUS=""; c=$(view_head)
  case "$a" in *●*) ;; *) exit 4 ;; esac; case "$b$c" in *●*) exit 5 ;; esac
  [ "$(printf '%s\n' "$a" | head -2 | tail -1 | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | wc -m)" -le $(( COLS - 1 )) ] || exit 6   # (the dot takes the room of the padding: the line stays inside the pane)
  exit 0 ) && ok "focus: the head shows whether this pane is the one you are in" || bad "pane focus: step $?"
# a card on now with an agent: like herdr's agent card (icon, "machine · agent", "∟ the topic"), with the card's bar and block edges; no meta line while it works
( rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "Omarchy${US}w9:p1${US}working${US}bhote${US}x" > "$AGENT_LOCAL"; AG_N=0; AG_LINES=(); ui_load
  topic_new "Card topic" "" "a description"; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$cf" agent bhote; topic_set "$cf" agent_machine Omarchy; topic_set "$cf" agent_pane w9:p1; topic_set "$cf" status now
  AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}x"); COLS=44; topics_load; out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g')
  printf '%s\n' "$out" | grep -q "Omarchy · bhote" || exit 1
  printf '%s\n' "$out" | grep -q "∟ x" || exit 2                                                                                  # (the agent's task, as herdr shows it)
  printf '%s\n' "$out" | grep -q "∟ Card topic" && exit 6
  AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}"); out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "∟ Card topic" || exit 7   # (no task: the topic's title)
  AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}x")
  printf '%s\n' "$out" | grep -q "▄▄▄\|▀▀▀" && exit 3; printf '%s\n' "$out" | grep -q "▌" || exit 3   # (the card has no frame rows, only its bar)
  printf '%s\n' "$out" | grep -q "Omarchy · bhote *working" || exit 4                                                      # (the state is on the right of the first row)
  [ "$(printf '%s\n' "$out" | grep -c "▌")" = 3 ] || exit 9                                                                  # (the agent, its task, and the description: this is the selected card; without a branch or a live text nothing else)
  AG_LINES=("Omarchy${US}w9:p1${US}blocked${US}bhote${US}x"); out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "needs you" || exit 5   # what the icon does not say stays
  exit 0 ) && ok "now card: like herdr's agent card, the state on the right of the first row, no frame rows" || bad "agent card: step $?"
# /clear: the pane runs a new session. An event without a session id (or one of another session) does not hand the old session's topic over;
# when the new session starts working, the topic follows it, and its idle then hands the topic back for review
evs() { HERDR_PLUGIN_EVENT_JSON=$(printf '{"data":{"pane_id":"%s","agent_status":"%s","agent_session":{"value":"%s"}}}' "$1" "$2" "$3") BHOTE_NO_SYNC=1 cli_event; }
rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/evstate"; topic_new "Cleared" "" "x"; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"
topic_set "$cf" agent_pane w9:p1; topic_set "$cf" agent_machine "$HOST"; topic_set "$cf" agent_session s-old; topic_set "$cf" status now; topic_set "$cf" delegated 1
ev w9:p1 working; ev w9:p1 idle; [ "$(topic_get "$cf" status)" = now ] && ok "clear: an idle event without a session id leaves the old session's topic alone" || bad "clear idle: $(topic_get "$cf" status)"
evs w9:p1 working s-new; [ "$(topic_get "$cf" agent_session)" = s-new ] && [ "$(topic_get "$cf" status)" = now ] && ok "clear: the new session that starts working takes the pane's topic over" || bad "clear adopt: $(topic_get "$cf" agent_session) $(topic_get "$cf" status)"
BHOTE_FAKE_QUESTION="Ok so?" evs w9:p1 idle s-new; [ "$(topic_get "$cf" status)" = review ] && ok "clear: and its idle with a question puts the topic under review" || bad "clear review: $(topic_get "$cf" status)"
# a working agent without a topic gets a card on now (after 15 s of work); gone when it stops or gets a topic of its own
(
rm -f "$TOPIC_DIR"/*.topic; rm -rf "$RUN_DIR/cards"; printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}Fix the export" > "$AGENT_LOCAL"
agent_cards; [ "$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | wc -l | tr -d ' ')" = 0 ] && ok "agent cards: no card before 15 s of work" || bad "card too early"
printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards; agent_cards
cf=$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | head -1)
[ "$(ls "$TOPIC_DIR"/*.topic | wc -l | tr -d ' ')" = 1 ] && [ "$(topic_get "$cf" status)" = now ] && [ "$(topic_get "$cf" title)" = "Fix the export" ] && [ "$(topic_get "$cf" agent_pane)" = w9:p1 ] && ok "agent cards: a working agent without a topic gets one card on now" || bad "agent card: $cf"
printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; agent_cards; [ "$(topic_get "$cf" title)" = "Next step" ] && ok "agent cards: the title follows what the agent works on" || bad "card title"
topic_new "Own topic" "" ""; of="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$of" agent_pane w9:p1; topic_set "$of" agent_machine "$HOST"
agent_cards; [ "$(topic_get "$cf" deleted)" = 1 ] && ok "agent cards: the card goes when the agent gets a topic of its own" || bad "card stayed with own topic"
rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards; cf=$(ls "$TOPIC_DIR"/*.topic | head -1)
printf '%s\n' "$HOST${US}w9:p1${US}idle${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; agent_cards; [ "$(topic_get "$cf" status)" = finished ] && [ "$(topic_get "$cf" deleted)" != 1 ] && ok "agent cards: the card is recently finished when the agent stops" || bad "card when idle: $(topic_get "$cf" status)"
# a card an agent works with (bhote current gives it, the agent hands it over with bhote review) is a topic of its own: it stays when the agent stops
rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards; cf=$(ls "$TOPIC_DIR"/*.topic | head -1)
( BHOTE_NO_SYNC=1 bhote_cli review "$(topic_get "$cf" id)" --summary "Done, please check" >/dev/null 2>&1 )
printf '%s\n' "$HOST${US}w9:p1${US}idle${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; agent_cards
[ "$(topic_get "$cf" deleted)" != 1 ] && [ "$(topic_get "$cf" status)" = review ] && [ -z "$(topic_get "$cf" auto_now)" ] \
    && ok "agent cards: a card handed over with bhote review stays when the agent stops (a topic of its own)" || bad "used card: deleted=$(topic_get "$cf" deleted) status=$(topic_get "$cf" status) auto_now=$(topic_get "$cf" auto_now)"
rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards; cf=$(ls "$TOPIC_DIR"/*.topic | head -1)
topic_set "$cf" status later; printf '%s\n' "$HOST${US}w9:p1${US}idle${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; agent_cards
[ "$(topic_get "$cf" deleted)" != 1 ] && [ -z "$(topic_get "$cf" auto_now)" ] && ok "agent cards: a card moved off now (in the panel) stays" || bad "moved card: deleted=$(topic_get "$cf" deleted)"
rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; topic_new "Waits" "" ""; wf=$TOPIC_DIR/$NEW_TOPIC_ID.topic
topic_set "$wf" agent Shop; topic_set "$wf" agent_machine "$HOST"; topic_set "$wf" agent_pane w9:p1; topic_set "$wf" status waiting; printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards
cf=$(grep -l '^auto_now=1' "$TOPIC_DIR"/*.topic 2>/dev/null | head -1)
[ -n "$cf" ] && [ "$(topic_get "$cf" status)" = now ] && ok "agent cards: a working agent with only waiting topics gets a card on now" || bad "no card for an agent with only a waiting topic"
topic_set "$wf" status now; agent_cards; [ "$(topic_get "$cf" deleted)" = 1 ] && ok "agent cards: that card goes when a topic of the agent is on now" || bad "card stayed"
cfg_set AGENT_CARDS off; printf '%s\n' "$HOST${US}w9:p1${US}working${US}Shop (main)${US}x" > "$AGENT_LOCAL"; rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "$(( $(date +%s) - 20 ))" > "$RUN_DIR/cards/w9_p1"; agent_cards
[ "$(ls "$TOPIC_DIR"/*.topic 2>/dev/null | wc -l | tr -d ' ')" = 0 ] && ok "agent cards: AGENT_CARDS=off makes none" || bad "card with setting off"
) && true
# groups show ten lines, then a selectable "+N more"; open it with Enter/click, close it again; the old ones (ARCHIVE_DAYS) are archived
(
rm -f "$TOPIC_DIR"/*.topic; EXPANDED=" "; SEL_ID=""; NOW=$(date +%s)
for k in 1 2 3 4 5 6 7 8 9 10 11 12; do topic_new "Next $k" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status next; done
topic_new "Old one" "" ""; of="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$of" status later; topic_set "$of" updated $(( NOW - 8 * 86400 ))
topics_load; topics_plan
v=0; for (( i = 0; i < T_N; i++ )); do [ "${PLAN_VIS[$i]}" = 1 ] && v=$(( v + 1 )); done
[ "$v" = 10 ] && [ "$PLAN_ARCH" = 1 ] && [ "$PLAN_N_next" = 12 ] && ok "lines: ten of a group, the one older than seven days is archived" || bad "plan: vis=$v arch=$PLAN_ARCH next=$PLAN_N_next"
[ "${SEL_LIST[10]}" = more:next ] && [ "${#SEL_LIST[@]}" = 11 ] && ok "lines: the arrow keys reach '+N more'" || bad "sel list: ${SEL_LIST[*]}"
SEL_ID=${SEL_LIST[9]}; sel_move +1; [ "$SEL_ID" = more:next ] && ok "sel_move: down from the tenth lands on '+2 more'" || bad "sel_move to more: $SEL_ID"
[ -z "$(sel_file)" ] && ok "'+N more' is no topic: the actions find no file" || bad "sel_file on more"
toggle_expand next; topics_plan; v=0; for (( i = 0; i < T_N; i++ )); do [ "${PLAN_VIS[$i]}" = 1 ] && v=$(( v + 1 )); done
[ "$v" = 12 ] && [ "${SEL_LIST[${#SEL_LIST[@]}-1]}" = more:next ] && ok "'+N more' opened: all lines, and a 'show less' row to select" || bad "expanded: vis=$v last=${SEL_LIST[*]}"
toggle_expand next; topics_plan; [ "${PLAN_BEFORE[$(( 0 ))]:-}" = "" ] && ok "toggle: closed again" || bad "toggle close"
) && true
# checking off a topic: the selection goes to the next one of its section (else the one before), not to the done list
(
rm -f "$TOPIC_DIR"/*.topic; EXPANDED=" "; SEL_ID=""
for k in 1 2 3; do topic_new "Nx $k" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status next; eval "nx$k=$NEW_TOPIC_ID"; done
for k in 1 2; do topic_new "Lt $k" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status later; eval "lt$k=$NEW_TOPIC_ID"; done
topics_load; topics_plan; l=("${SEL_LIST[@]}")                                   # (same-second ids: the order is the file order)
SEL_ID=${l[1]}; sel_leave; [ "$SEL_ID" = "${l[2]}" ] && ok "x: the selection goes to the next item of its section" || bad "sel_leave middle: $SEL_ID"
SEL_ID=${l[2]}; sel_leave; [ "$SEL_ID" = "${l[1]}" ] && ok "x: the last of a section: the one before it" || bad "sel_leave last: $SEL_ID"
SEL_ID=${l[4]}; sel_leave; [ "$SEL_ID" = "${l[3]}" ] && ok "x: in Later it stays in Later" || bad "sel_leave later: $SEL_ID"
) && true
# the status testing follows the live test status files of the agent's worktree (TEST_STATUS_DIR): running tests = testing, also with a working agent
(
rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/evstate"; cfg_set TESTS_BUSY on; notify() { :; }
wt="$T/trees/Shop"; mkdir -p "$wt" "$T/live"; cfg_set TEST_STATUS_DIR "$T/live"; printf '%s\n' "w1:p1${US}$wt" > "$AGENT_CWD"
live() { printf '{"tree":"Shop","suite":"server","done":3,"total":10,"startedAt":1,"updatedAt":%s000,"ok":%s}\n' "$(date +%s)" "$1" > "$T/live/Shop.server.json"; }
printf '%s\n' "$HOST${US}w1:p1${US}working${US}Shop${US}t" > "$AGENT_LOCAL"
topic_new "Test work" "" "x"; tf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$tf" agent_pane w1:p1; topic_set "$tf" agent_machine "$HOST"; topic_set "$tf" status now
pane_tests_running w1:p1 && bad "no live file: no tests" || ok "testing: no status file, no tests running"
live null; pane_tests_running w1:p1 && ok "testing: a fresh unfinished status file means tests run" || bad "tests not seen"
tests_sync; [ "$(topic_get "$tf" status)" = testing ] && ok "testing: tests run in its worktree: the topic is in testing (agent working too)" || bad "now->testing: $(topic_get "$tf" status)"
# the testing card: one bar per running suite (two at most), the suite's name in front
printf '{"tree":"Shop","suite":"client","done":8,"total":10,"startedAt":1,"updatedAt":%s000,"ok":null}\n' "$(date +%s)" > "$T/live/Shop.client.json"
printf '{"tree":"Shop","suite":"lint","done":1,"total":2,"startedAt":1,"updatedAt":%s000,"ok":null}\n' "$(date +%s)" > "$T/live/Shop.lint.json"; tests_sync
[ "$(cat "$SHARED_DIR/testprog/${HOST}_w1_p1" 2>/dev/null)" = "$(printf '8 10 client\n1 2 lint')" ] && ok "testing: the progress of two suites is written (the first two by name: client, lint)" || bad "testprog: $(cat "$SHARED_DIR/testprog/${HOST}_w1_p1" 2>&1)"
printf '%s\n' "$HOST${US}w1:p1${US}idle${US}Shop${US}t" > "$AGENT_LOCAL"; AG_N=1; AG_LINES=("$HOST${US}w1:p1${US}idle${US}Shop${US}t"); COLS=50; ROWS=40; NOW=$(date +%s); topics_load
out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); nb=$(printf '%s\n' "$out" | grep -c '■.*%')
printf '%s\n' "$out" | grep -q 'client .*80%' && printf '%s\n' "$out" | grep -q 'lint .*50%' && [ "$nb" = 2 ] && ok "testing: the card shows two bars with their percent" || bad "testing bars ($nb): $out"
rm -f "$T/live/Shop.client.json" "$T/live/Shop.lint.json"; tests_sync
printf '%s\n' "$HOST${US}w1:p1${US}idle${US}Shop${US}t" > "$AGENT_LOCAL"; tests_sync; [ "$(topic_get "$tf" status)" = testing ] && ok "testing: an idle agent, tests still run: still testing" || bad "testing hold: $(topic_get "$tf" status)"
live true; tests_sync; [ "$(topic_get "$tf" status)" = review ] && ok "testing: tests done, the agent idle: review" || bad "testing->review: $(topic_get "$tf" status)"
topic_set "$tf" status now; live null; tests_sync; printf '%s\n' "$HOST${US}w1:p1${US}working${US}Shop${US}t" > "$AGENT_LOCAL"; live true; tests_sync
[ "$(topic_get "$tf" status)" = now ] && ok "testing: tests done, the agent works: back to now" || bad "testing->now: $(topic_get "$tf" status)"
printf '{"tree":"Shop","suite":"server","done":3,"total":10,"startedAt":1,"updatedAt":1000,"ok":null}\n' > "$T/live/Shop.server.json"; pane_tests_running w1:p1 && bad "a stale file counts" || ok "testing: a status file untouched for 30 s counts as gone"
) && true
# auto-assign: the project decides which agents come into question; a topic without a project gets it from its words
(
rm -f "$TOPIC_DIR"/*.topic; project_new "Shop"; shop=$PROJ_ID; project_new "Acme"; bil=$PROJ_ID; project_new "Acme Marketing"; mkt=$PROJ_ID
printf '%s\n' "$HOST${US}w1:p1${US}idle${US}shop-agent${US}t" "$HOST${US}w2:p1${US}idle${US}bil-agent${US}t" > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
printf '%s\n' "$HOST${US}w1:p1${US}t1${US}a${US}${US}$shop${US}/x" "$HOST${US}w2:p1${US}t2${US}a${US}${US}$bil${US}/y" > "$AGENT_META_L"
agent_live() { LIVE_ST=idle; LIVE_SESS=s1; }; agent_taken() { return 1; }; notify() { :; }; agent_send() { echo "$1/$2" >> "$T/sent.log"; }
cfg_set AUTO_ASSIGN on; : > "$T/sent.log"
project_autoselect "Fix the Shop export"; [ "$PROJ_ID" = "$shop" ] && ok "project_autoselect: a project name as a word" || bad "autoselect shop: $PROJ_ID"
project_autoselect "Acme Marketing page" ; [ "$PROJ_ID" = "$mkt" ] && ok "project_autoselect: the longest name wins" || bad "autoselect longest: $PROJ_ID"
project_autoselect "workshop planning" && bad "autoselect: shop inside a word" || ok "project_autoselect: not inside a word"
topic_new "Fix the Shop export" "" ""; t1="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$t1" updated 1
topic_new "Something without a hint" "" ""; t2="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$t2" updated 1
auto_assign
[ "$(topic_get "$t1" agent_pane)" = w1:p1 ] && [ "$(topic_get "$t1" project)" = "$shop" ] && ok "auto-assign: the project is chosen from the words and its agent is used" || bad "auto-assign shop: $(topic_get "$t1" agent_pane) $(topic_get "$t1" project)"
[ -z "$(topic_get "$t2" agent_pane)" ] && [ -z "$(topic_get "$t2" project)" ] && ok "auto-assign: a topic without a project does not start" || bad "auto-assign without project started"
topic_set "$t2" project "$bil"; topic_set "$t2" updated 1; topic_set "$t1" agent_pane ""; auto_assign
[ "$(topic_get "$t2" agent_pane)" = w2:p1 ] && ok "auto-assign: a chosen project picks the agent of that project only" || bad "auto-assign bil: $(topic_get "$t2" agent_pane)"
) && true
# auto-assign: a project without a free agent does not hold up the topics of the other projects
(
rm -f "$TOPIC_DIR"/*.topic; project_new "Alpha"; pa=$PROJ_ID; project_new "Beta"; pb=$PROJ_ID
printf '%s\n' "$HOST${US}w1:p1${US}working${US}alpha-agent${US}t" "$HOST${US}w2:p1${US}idle${US}beta-agent${US}t" > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
printf '%s\n' "$HOST${US}w1:p1${US}t1${US}a${US}${US}$pa${US}/x" "$HOST${US}w2:p1${US}t2${US}a${US}${US}$pb${US}/y" > "$AGENT_META_L"
agent_live() { LIVE_ST=idle; LIVE_SESS=s1; }; agent_taken() { return 1; }; notify() { :; }; agent_send() { :; }
cfg_set AUTO_ASSIGN on
for p in Alpha Beta; do   # (fixed file names: Alpha comes first, as the topics are read in file order)
    topic_new "$p work" "" ""; f="$TOPIC_DIR/1-$p.topic"; mv "$TOPIC_DIR/$NEW_TOPIC_ID.topic" "$f"
    topic_set "$f" project "$( [ $p = Alpha ] && echo "$pa" || echo "$pb")"; topic_set "$f" updated 1
done
auto_assign; topics_load
for (( i = 0; i < T_N; i++ )); do printf '%s=%s ' "${T_TITLE[$i]}" "${T_AP[$i]:--}"; done > "$T/aa.out"
grep -q "Alpha work=- " "$T/aa.out" && grep -q "Beta work=w2:p1 " "$T/aa.out" ) && ok "auto-assign: a project without a free agent does not hold up the others" || bad "auto-assign stops at a busy project: $(cat "$T/aa.out")"
# the agent list says which topic an agent works on (not for a parked one)
cfg_set HOST_LABEL Mac; rm -f "$TOPIC_DIR"/*.topic; topic_new "Fix the export" "" ""; wt="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$wt" agent_pane w1:p1; topic_set "$wt" agent_machine "$HOST"; topic_set "$wt" status now
frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=70 BHOTE_ROWS=150 bash ./bhote </dev/null)
echo "$frame" | grep -q " Fix th…" && ok "agent list: the topic an agent works on stands behind its name" || bad "no topic behind the agent"
topic_set "$wt" status later
frame=$(BHOTE_SOURCE_ONLY= BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=70 BHOTE_ROWS=150 bash ./bhote </dev/null)
echo "$frame" | grep -q " Fix th…" && bad "a parked topic is shown behind the agent" || ok "agent list: a parked topic is not shown"
# agent numbers: a number of its own per agent on its machine, the same for good
( HOST=Mac; rm -f "$TOPIC_DIR"/*.topic; AN_AT=-999
  printf '%s\n' "Mac${US}w1:p1${US}idle${US}alpha${US}t" "Mac${US}w2:p1${US}idle${US}beta${US}t" > "$AGENT_LOCAL"; agentnum_assign
  agentnum_set a Mac alpha; agentnum_set b Mac beta; [ "$a" = 1 ] && [ "$b" = 2 ] || exit 1
  printf '%s\n' "Mac${US}w2:p1${US}idle${US}beta${US}t" "Mac${US}w3:p1${US}idle${US}gamma${US}t" > "$AGENT_LOCAL"; agentnum_assign   # alpha went away, gamma came
  AN_AT=-999; agentnum_load; agentnum_set a Mac alpha; agentnum_set b Mac beta; agentnum_set c Mac gamma; [ "$a" = 1 ] && [ "$b" = 2 ] && [ "$c" = 3 ] || exit 2
  printf '%s\n' "Mac${US}w1:p1${US}idle${US}alpha${US}t" > "$AGENT_LOCAL"; agentnum_assign; AN_AT=-999; agentnum_load; agentnum_set a Mac alpha; [ "$a" = 1 ] || exit 3   # alpha is back: still 1
  printf '%s\n' "Other${US}w1:p1${US}idle${US}remote${US}t" > "$AGENT_LOCAL"; agentnum_assign; agentnum_set r Other remote; [ -z "$r" ] || exit 4   # only the own machine hands out numbers
  exit 0 ) && ok "agent numbers: stable, next free, kept when an agent is away, only for this machine" || bad "agent numbers: step $?"
( HOST=Mac; AG_N=2; AG_LINES=($'Mac\037w1:p1\037idle\037alpha\037' $'Laptop\037w2:p1\037idle\037beta\037'); MC_N=0; AN_AT=-999
  rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "Mac${US}w1:p1${US}idle${US}alpha${US}t" > "$AGENT_LOCAL"; agentnum_assign
  printf 'id=an-Laptop-beta\nkind=agentnum\ntitle=Laptop/beta\nnum=7\nupdated=1\ndeleted=0\n' > "$TOPIC_DIR/an-Laptop-beta.topic"; AN_AT=-999
  agent_code_set x Mac alpha; agent_code_set y Laptop beta; case "$x$y" in [A-Z][A-Z][A-Z]1[A-Z][A-Z][A-Z]7) ;; *) exit 1 ;; esac
  agent_ref "$(echo "$y" | tr '[:upper:]' '[:lower:]')" && [ "$AR_NAME" = beta ] ) && ok "agent numbers: a code (any case) finds the agent" || bad "agent code ref"
# an agent's name is herdr's workspace and pane number (w3:a1 for the pane w3:p1), with the machine's code in front on another machine (MAC/w3:a1);
# a ref in either form finds it, a name that two machines have asks for the machine, and the old code (MAC4) stays a ref
( HOST=Mac; MC_N=0; AG_N=3; AG_LINES=($'Mac\037w1:p1\037idle\037alpha\037' $'Laptop\037w2:p1\037idle\037beta\037' $'Laptop\037w1:p1\037idle\037gamma\037')
  agent_pane_code_set a Mac w1:p1; agent_pane_code_set b Laptop w2:p1; agent_pane_code_set c Mac weird; agent_pane_code_set g Laptop w1:p1
  [ "$a" = w1:a1 ] && case "$b" in */w2:a1) ;; *) exit 1 ;; esac && [ -z "$c" ] || exit 2
  agent_ref w2:a1 && [ "$AR_NAME" = beta ] || exit 3
  agent_ref W2:A1 && [ "$AR_NAME" = beta ] || exit 4
  agent_ref w1:a1 >/dev/null 2>&1; [ $? = 2 ] || exit 5
  agent_ref "$g" && [ "$AR_NAME" = gamma ] || exit 6
  agent_ref "Mac/w1:p1" && [ "$AR_NAME" = alpha ] || exit 7
  exit 0 ) && ok "agent names: w3:a1 (MAC/w3:a1 on another machine), found by it in any case, asks for the machine when two have it" || bad "agent pane code: step $?"
# the topics are read once per change, per process: a subshell (the collector) that changes a topic AND loads must not make the parent think its own copy is current
( rm -f "$TOPIC_DIR"/*.topic; topic_new "Shared view" "" "" >/dev/null; f="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topics_load; [ "${T_STATUS[0]}" = now ] || exit 1
  ( topic_set "$f" status review; topics_load; [ "${T_STATUS[0]}" = review ] ) || exit 2
  topics_load; [ "${T_STATUS[0]}" = review ] || exit 3                                    # (the subshell read the new state; this copy must read it, too)
  ( topic_set "$f" title "Renamed" ) ; topics_load; [ "${T_TITLE[0]}" = Renamed ] || exit 4  # (a change this process did not even load)
  topics_load; n1=${T_N}; topic_new "Another" "" "" >/dev/null; topics_load; [ "${T_N}" = $(( n1 + 1 )) ] || exit 5   # (a new file)
  rm -f "$f"; topics_load; [ "${T_N}" = "$n1" ] || exit 6                                # (a file gone)
  exit 0 ) && ok "topics: read again when another process changed them, also one that loaded them first" || bad "topics_load shared state: step $?"
# Steal & Transfer into another project is refused (only on purpose)
( agent_live() { LIVE_ST=idle; LIVE_SESS=s1; LIVE_CWD=/x; }; notify() { :; }; agent_send() { :; }
  rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "Mac${US}w2:p1${US}t${US}a${US}${US}proj-shop${US}/y" > "$AGENT_META_L"; : > "$AGENT_META_R"
  topic_new "Doorbell fix" "" ""; xf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$xf" project proj-omarchy; topic_set "$xf" agent_pane w3:p1; topic_set "$xf" agent_machine Mac
  transfer_start "$xf" Mac w3:p1 root Mac w2:p1 "Shop (main)" 2>/dev/null; [ $? = 3 ] || exit 1
  [ -z "$(topic_get "$xf" transfer_to)" ] || exit 2
  TRANSFER_FORCE=1 transfer_start "$xf" Mac w3:p1 root Mac w2:p1 "Shop (main)" 2>/dev/null; [ -n "$(topic_get "$xf" transfer_to)" ] || exit 3
  exit 0 ) && ok "transfer: an agent of another project is refused, unless forced" || bad "transfer project guard: step $?"
# the group IDLE: the agents that are idle and have no card; a click on one shows the agent (ref agent:<machine>/<pane>); the Agents list keeps only the others
( cfg_set CARD_GROUPING status; mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic "$RUN_DIR/toff"; : > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
  topic_new "Has a card" "" ""; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$cf" agent a2; topic_set "$cf" agent_machine Omarchy; topic_set "$cf" agent_pane w2:p1; topic_set "$cf" status now
  AG_LINES=("Omarchy${US}w1:p1${US}idle${US}Free one${US}its last task" "Omarchy${US}w2:p1${US}idle${US}a2${US}x" "Omarchy${US}w3:p1${US}working${US}Busy${US}y" "Omarchy${US}w4:p1${US}blocked${US}Asks${US}z"); AG_N=4
  printf '%s\n' "${AG_LINES[@]}" > "$AGENT_LOCAL"; COLS=50; ROWS=60; ui_load; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); NOW=$(date +%s)
  out=$(topics_block cards | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); idle=$(printf '%s\n' "$out" | sed -n '/^  IDLE/,$p')
  printf '%s\n' "$idle" | grep -q "Omarchy · Free one" && printf '%s\n' "$idle" | grep -q "∟ its last task" || exit 1        # the free idle agent, with its last task
  printf '%s\n' "$idle" | grep -q "a2" && exit 2                                                                          # (the idle agent that has a card is not there twice)
  printf '%s\n' "$idle" | grep -q "Busy\|Asks" && exit 3                                                                   # (only idle ones)
  ag=$(agents_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$ag" | grep -q "Free one" && exit 4; printf '%s\n' "$ag" | grep -q "Busy" && printf '%s\n' "$ag" | grep -q "Asks" || exit 5   # the Agents list: the others
  fr=$(bhote_view); r=$(awk -F'\036' '$2 == "agent:Omarchy/w1:p1" { print $1; exit }' "$RUN_DIR/tmap"); [ -n "$r" ] || exit 6
  MOUSE_KEY=""; mouse_event "0;5;$r" M; [ "$MOUSE_KEY" = AGENT ] && [ "$CLICK_M" = Omarchy ] && [ "$CLICK_P" = w1:p1 ] || exit 7   # a click: that agent is shown
  exit 0 ) && ok "IDLE: the idle agents without a card, one card each (a click shows the agent); the Agents list keeps the others" || bad "idle group: step $?"
# ungrouped (the default): no headings, the cards in the order of the agents (as herdr lists them), idle ones too, cards without an agent after them
( cfg_set CARD_GROUPING ungrouped; mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic "$RUN_DIR/toff"; : > "$AGENT_REMOTE"; HOST=Omarchy
  topic_new "Third agent topic" "" ""; t3="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$t3" agent Zed; topic_set "$t3" agent_machine Omarchy; topic_set "$t3" agent_pane w3:p1; topic_set "$t3" status review
  topic_new "First agent topic" "" ""; t1="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$t1" agent Alpha; topic_set "$t1" agent_machine Omarchy; topic_set "$t1" agent_pane w1:p1; topic_set "$t1" status now
  topic_new "No agent" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status review
  AG_LINES=("Omarchy${US}w1:p1${US}working${US}Alpha${US}x" "Omarchy${US}w2:p1${US}idle${US}Beta${US}b task" "Omarchy${US}w3:p1${US}idle${US}Zed${US}z"); AG_N=3
  printf '%s\n' "${AG_LINES[@]}" > "$AGENT_LOCAL"; COLS=50; ROWS=60; ui_load; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); NOW=$(date +%s)
  out=$(topics_block cards | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g')
  printf '%s\n' "$out" | grep -q "^  NOW\|^  REVIEW\|^  IDLE\|^  RECENTLY" && exit 1                                    # (no group headings)
  order=$(printf '%s\n' "$out" | grep -o 'Omarchy · [A-Za-z]*\|^▌ [^O]*No agent' | tr '\n' ' ')
  case "$order" in "Omarchy · Alpha Omarchy · Beta Omarchy · Zed "*) ;; *) exit 2 ;; esac                                  # (the order of the agents; the idle one between them)
  printf '%s\n' "$out" | grep -q "No agent" || exit 3                                                                       # (the card without an agent is there too)
  [ "$(printf '%s\n' "$out" | grep -n "No agent" | cut -d: -f1)" -gt "$(printf '%s\n' "$out" | grep -n "Zed" | cut -d: -f1)" ] || exit 4   # (after the cards of the agents)
  exit 0 ) && ok "cards ungrouped: no headings, in the order of the agents, idle ones between, cards without an agent after them" || bad "ungrouped: step $?"
# nothing chosen: the active card is the one of the agent in this panel's tab (not the first one of the list)
( cfg_set CARD_GROUPING status; mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic; : > "$AGENT_REMOTE"; HOST=Omarchy
  topic_new "Other agent" "" ""; o1="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$o1" agent Beta; topic_set "$o1" agent_machine Omarchy; topic_set "$o1" agent_pane w1:p1; topic_set "$o1" status review
  topic_new "Mine" "" ""; mine="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mine" agent Alpha; topic_set "$mine" agent_machine Omarchy; topic_set "$mine" agent_pane w9:p1; topic_set "$mine" status now
  printf 'Omarchy%sw1:p1%st1%sx\nOmarchy%sw9:p1%st9%sy\n' "$US" "$US" "$US" "$US" "$US" "$US" > "$AGENT_META_L"
  topics_load; SEL_ID=""; HERDR_TAB_ID=""; sel_index; first=$SEL_IDX; [ "${T_ID[$first]}" = "$(topic_id "$o1")" ] || exit 1      # (no tab known: the first card, the review one)
  HERDR_TAB_ID=t9; sel_index; [ "${T_ID[$SEL_IDX]}" = "$(topic_id "$mine")" ] || exit 2                                       # (in tab t9: its agent's card)
  SEL_ID=$(topic_id "$o1"); sel_index; [ "${T_ID[$SEL_IDX]}" = "$(topic_id "$o1")" ] || exit 3                               # (a chosen one stays)
  exit 0 ) && ok "the active card: nothing chosen selects the card of the agent in this panel's tab" || bad "active card: step $?"
# a recently finished card with the same title as another card of its agent (at work, review, waiting, a newer finished one) is put away; a different title stays
( rm -f "$TOPIC_DIR"/*.topic; HOST=Omarchy
  mk() { topic_new "$1" "" ""; local f="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$f" agent a1; topic_set "$f" agent_machine Omarchy; topic_set "$f" agent_pane w4:p4; topic_set "$f" status "$2"; printf '%s' "$f"; }
  now_f=$(mk "Same work" now); old_f=$(mk "Same work" finished); other_f=$(mk "Other work" finished); topics_load; agent_dedup
  [ "$(topic_get "$old_f" status)" = later ] && [ "$(topic_get "$old_f" dismissed)" = 1 ] || exit 1      # (the same work twice: put away)
  [ "$(topic_get "$other_f" status)" = finished ] && [ "$(topic_get "$now_f" status)" = now ] || exit 2   # (another title stays, the card at work stays)
  exit 0 ) && ok "a finished card that repeats a card of its agent is put away" || bad "finished duplicate: step $?"
# one topic per agent is bound to a deployment (its newest), and an agent whose topic waits for a deployment gets no second card (that would be bound again, for ever)
( mkdir -p "$RUN_DIR/cards"; rm -f "$TOPIC_DIR"/*.topic; : > "$AGENT_REMOTE"; HOST=Omarchy
  mk() { topic_new "$1" "" ""; local f="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$f" agent a5; topic_set "$f" agent_machine Omarchy; topic_set "$f" agent_pane w3:p5; topic_set "$f" agent_branch fix/z; topic_set "$f" status finished; sed -i.bak "s/^updated=.*/updated=$2/" "$f"; rm -f "$f.bak"; printf '%s' "$f"; }
  now=$(date +%s); o1=$(mk "Old one" $(( now - 60 ))); o2=$(mk "Old two" $(( now - 30 ))); nw=$(mk "Newest" "$now")
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}2${US}fix/z${US}https://ci/9" > "$SHARED_DIR/deploys.list"; topics_load; deploy_sync
  [ "$(topic_get "$nw" status)" = waiting ] && [ "$(topic_get "$o1" status)" = finished ] && [ "$(topic_get "$o2" status)" = finished ] || exit 1   # (only the newest topic of the agent)
  n0=$(ls "$TOPIC_DIR"/*.topic | wc -l); cfg_set AGENT_CARDS on; echo 1 > "$RUN_DIR/cards/w3_p5"; printf '%s\n' "Omarchy${US}w3:p5${US}working${US}a5${US}task" > "$AGENT_LOCAL"
  topics_load; agent_cards; [ "$(ls "$TOPIC_DIR"/*.topic | wc -l)" = "$n0" ] || exit 2       # (it waits for the deployment: that is its card)
  exit 0 ) && ok "deployment: one topic per agent is bound (its newest), and an agent whose topic waits for it gets no second card" || bad "deploy newest: step $?"
# the CI test jobs that run for the branch of a card: a bar each (the CircleCI jobs have no progress of their own: they are 0 of 1 until they are over)
( mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic; : > "$AGENT_REMOTE"
  topic_new "Pushed" "" ""; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$cf" agent a1; topic_set "$cf" agent_machine Omarchy; topic_set "$cf" agent_pane w1:p1; topic_set "$cf" agent_branch fix/ci; topic_set "$cf" status now
  AG_LINES=("Omarchy${US}w1:p1${US}working${US}a1${US}x"); AG_N=1; printf '%s\n' "${AG_LINES[@]}" > "$AGENT_LOCAL"; COLS=60; ROWS=60; ui_load; NOW=$(date +%s)
  printf '%s\n' "$NOW${US}IMS${US}build-backend${US}running${US}0${US}1${US}fix/ci|~|${US}https://ci/1" "$NOW${US}IMS${US}build-frontend${US}running${US}0${US}1${US}other|~|${US}https://ci/2" "$NOW${US}IMS${US}server${US}ok${US}6${US}6${US}fix/ci|~|37 passed${US}" > "$SHARED_DIR/tests.list"
  out=$(topics_block cards | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "build-backend ■" || exit 1
  printf '%s\n' "$out" | grep -q "build-frontend" && exit 2; printf '%s\n' "$out" | grep -q "server ■" && exit 3          # (another branch's job, a job that is over: no bar)
  exit 0 ) && ok "cards: a CI test job that runs for the card's branch has a bar on it" || bad "CI bars: step $?"
# a click on a topic: every row of an entry selects it, captions and gaps do not; the window stays while the selection is in it
( mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic "$RUN_DIR/toff"; AG_N=0; AG_LINES=(); : > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
  for i in 1 2 3 4 5 6 7 8 9 10; do topic_new "Click topic $i" "desc $i" "" >/dev/null; done
  COLS=44; ROWS=30; ui_load; fr=$(bhote_view); [ -s "$RUN_DIR/tmap" ] || exit 1
  while IFS=$'\036' read -r r t; do ln=$(printf '%s\n' "$fr" | sed -n "${r}p")
      case "$ln" in *"Click topic"*|*"∟"*|*"desc "*) ;; *) exit 2 ;; esac; done < "$RUN_DIR/tmap"
  hd=$(( $(head -1 "$RUN_DIR/tmap" | cut -d$'\036' -f1) - 1 )); mouse_event "0;5;$hd" M; [ -z "$MOUSE_KEY" ] || exit 3   # the caption
  r=$(awk -F'\036' 'NR > 1 && $2 != p { n++ } { p = $2 } n == 3 { print $1; exit }' "$RUN_DIR/tmap"); mouse_event "0;5;$r" M
  [ "$MOUSE_KEY" = TOPIC ] || exit 4; off=$(cat "$RUN_DIR/toff")
  SEL_ID=$CLICK_T; ui_load; fr2=$(bhote_view); [ "$(cat "$RUN_DIR/toff")" = "$off" ] || exit 5
  nf() { sed -n "${1}p" | sed 's/┊//g; s/^ *//; s/ *$//'; }   # (the selected entry has a dotted frame, in the gaps and at its sides)
  [ "$(printf '%s\n' "$fr" | nf "$r")" = "$(printf '%s\n' "$fr2" | nf "$r")" ] || exit 6
  [ "$(printf '%s\n' "$fr" | wc -l)" = "$(printf '%s\n' "$fr2" | wc -l)" ] || exit 7
  exit 0 ) && ok "click: a topic's rows select it, the list does not jump" || bad "topic click: step $?"
( mkdir -p "$RUN_DIR"; printf '9\036t1\n9\036t1\n' > "$RUN_DIR/tmap"; : > "$RUN_DIR/divrow"; CLICK_LAST=""
  mouse_event "0;5;9" M; [ "$MOUSE_KEY" = TOPIC ] || exit 1; mouse_event "0;5;9" m; mouse_event "0;5;9" M; [ "$MOUSE_KEY" = OPEN ] || exit 2
  mouse_event "0;5;9" M; [ "$MOUSE_KEY" = TOPIC ] || exit 3; CLICK_AT=$(( CLICK_AT - 600 )); mouse_event "0;5;9" M; [ "$MOUSE_KEY" = TOPIC ] || exit 4
  exit 0 ) && ok "click: two quick presses on a topic are a double click (OPEN), slow ones are not" || bad "double click: step $?"
( mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic; topic_new "Page topic" "" "" >/dev/null; SEL_ID=$NEW_TOPIC_ID
  COLS=44; ROWS=60; out=$(bhote_paint menu); [ -s "$RUN_DIR/pmap" ] || exit 1
  r=$(awk -F'\036' '$2 == 1 { print $1 }' "$RUN_DIR/pmap"); ln=$(printf '%s\n' "$out" | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | sed -n "${r}p")
  menu_build "$TOPIC_DIR/$SEL_ID.topic"; case "$ln" in *"2 "*"${MENU_ITEMS[1]#*|}"*) ;; *) exit 2 ;; esac
  mouse_event "0;8;2" M; [ "$MOUSE_KEY" != ITEM ] || exit 4
  r=$(awk -F'\036' '$2 == -2 { print $1; exit }' "$RUN_DIR/pmap"); printf '%s\n' "$out" | sed -n "${r}p" | grep -q "Page topic" || exit 6
  grep -q $'\036-1$' "$RUN_DIR/pmap" || exit 8                     # the description (empty: its hint line) too
  out=$(bhote_paint main); [ ! -s "$RUN_DIR/pmap" ] || exit 5
  exit 0 ) && ok "click: actions, title and description on the topic's page are rows of their own (ITEM with the index), the list has none" || bad "action click: step $?"
# the topic's menu over the list: the right button, the › on the selected card, and the menu itself (rows with the action's index)
( mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic "$RUN_DIR/toff"; AG_N=0; AG_LINES=(); : > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
  for i in 1 2 3; do topic_new "Ctx topic $i" "" "" >/dev/null; done
  SEL_ID=$NEW_TOPIC_ID; COLS=44; ROWS=40; out=$(bhote_paint main); [ -s "$RUN_DIR/tmap" ] || exit 1
  r=$(awk -F'\036' -v id="$SEL_ID" '{ t = $2; sub(/!.*/, "", t) } t == id { print $1; exit }' "$RUN_DIR/tmap")
  ln=$(printf '%s\n' "$out" | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | sed -n "${r}p"); case "$ln" in *"›"*) exit 2 ;; *"Ctx topic 3"*) ;; *) exit 2 ;; esac   # (the title row of the selected card: no button)
  mouse_event "0;5;$r" M; [ "$MOUSE_KEY" = TOPIC ] || exit 4                                                              # the title itself selects
  mouse_event "2;5;$r" M; [ "$MOUSE_KEY" = CTX ] && [ "$CLICK_T" = "$SEL_ID" ] || exit 5                                  # the right button
  mouse_event "2;5;1" M; [ "$MOUSE_KEY" = CTX ] && [ -z "$CLICK_T" ] || exit 6                                                      # (not on a topic: no topic)
  exit 0 ) && ok "the right button reports CTX (the topic page opens); a card has no › button" || bad "context menu: step $?"
# a click: a card shows its agent, a line (next, waiting, later, done) opens its page
( rm -f "$TOPIC_DIR"/*.topic
  for st in now review next later done; do topic_new "T $st" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status "$st"; eval "id_$st=$NEW_TOPIC_ID"; done
  topic_new "T waits" "Karen" ""; id_w=$NEW_TOPIC_ID; topic_new "T deploy" "" ""; id_d=$NEW_TOPIC_ID; deploy_wait_set "$TOPIC_DIR/$id_d.topic"; topics_load
  topic_is_card "$id_now" && topic_is_card "$id_review" && topic_is_card "$id_d" || exit 1
  topic_is_card "$id_next" || topic_is_card "$id_later" || topic_is_card "$id_done" || topic_is_card "$id_w" && exit 2
  exit 0 ) && ok "click: cards (now, review, deployment wait) show the agent, lines (next, later, done, waiting) open the page" || bad "topic_is_card: step $?"
# an unchanged agent list stays fresh (bhote find then takes it instead of asking herdr again), and the revision stays
( L=$T/fresh.list; printf 'a\n' > "$L"; touch -t 202001010000 "$L"; printf 'a\n' > "$L.new"; r0=$(cat "$SHARED_DIR/agents.rev" 2>/dev/null)
  agents_put "$L.new" "$L"; [ -n "$(find "$L" -mmin -1)" ] && [ "$(cat "$SHARED_DIR/agents.rev" 2>/dev/null)" = "$r0" ] ) \
  && ok "agent lists: an unchanged list counts as fresh, without a new revision" || bad "unchanged agent list looks stale"
# no agents: no error text in the panel (no agent map yet), and an old map does not stay (no star, no click on a gone agent)
( R=$(mktemp -d); RUN_DIR=$R; echo 5 > "$R/divrow"
  err=$( { spin_cells; } 2>&1 ); [ -z "$err" ] || exit 1
  printf '12\n0\036Mac%sw1:p1%sworking\036\n' "$US" "$US" > "$R/agmap"; spin_cells; [ -n "$SPIN_CELLS" ] || exit 2
  AG_N=0; AG_LINES=(); COLS=44; ROWS=40; agents_block >/dev/null; spin_cells; [ -z "$SPIN_CELLS" ] || exit 3
  rm -rf "$R"; exit 0 ) && ok "no agents: no error text, and the old agent map is gone" || bad "agent map without agents: step $?"
# the search popup: no free agents; the done topics only the newest 25 (+ a count) until something is typed, then all of them
( rm -f "$TOPIC_DIR"/*.topic; for i in $(seq 1 30); do topic_new "Done topic $i" "" "" >/dev/null; f="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$f" status done
      { grep -v '^updated=' "$f"; echo "updated=$(( 1700000000 + i ))"; } > "$f.x" && mv "$f.x" "$f"; done          # (distinct times: the newest is topic 30)
  topic_new "Open topic" "" "" >/dev/null
  AG_N=2; AG_LINES=("Mac${US}w1:p1${US}idle${US}IMS free${US}" "Mac${US}w2:p1${US}working${US}IMS busy${US}x"); topics_load; find_items
  find_filter ""; [ "$FI_MORE" = 5 ] || exit 1; [ "${#FI_HIT[@]}" = 27 ] || exit 2                      # 25 done + the open one + the busy agent
  find_filter "IMS free"; [ "${#FI_HIT[@]}" = 0 ] || exit 3                                              # a free agent is not there, not even in the search
  find_filter "done topic"; [ "${#FI_HIT[@]}" = 30 ] && [ "$FI_MORE" = 0 ] || exit 4                      # the search reaches every done topic
  find_filter ""; first=${FI_HIT[1]}; [ "${FI_NAME[$first]}" = "Done topic 30" ] || exit 5                # the newest done topic first
  COLS=80; ROWS=60; find_filter ""; out=$(find_draw "" 0 ""); case "$out" in *"+5 more done tasks"*) ;; *) exit 6 ;; esac
  exit 0 ) && ok "find popup: no free agents, the newest 25 done topics and +N more, the search finds all" || bad "find popup done/free: step $?"
# the ssh masters herdr leaves behind: the one of a gone herdr is ended (ssh -O exit) and its folder removed; a running herdr's stays
( H=$T/htmp; mkdir -p "$H/keep"; export BHOTE_HERDR_TMP=$H BHOTE_SSH=$T/muxssh
  printf '#!/bin/sh\necho "$*" >> "%s"\n' "$T/mux.log" > "$BHOTE_SSH"; chmod +x "$BHOTE_SSH"
  sh -c 'exit 0' & gone=$!; wait "$gone"; kill -0 "$gone" 2>/dev/null && exit 9   # a pid that is surely gone
  sock() { perl -MIO::Socket::UNIX -e 'IO::Socket::UNIX->new(Type => SOCK_STREAM(), Local => $ARGV[0], Listen => 1) or exit 1' "$1"; }
  mkdir "$H/herdr-ssh-$gone-0" "$H/herdr-ssh-$$-0" "$H/herdr-ssh-x-0"; : > "$H/herdr-ssh-$gone-0/config"
  sock "$H/herdr-ssh-$gone-0/cm-abc" && sock "$H/herdr-ssh-$$-0/cm-def" || exit 1
  ln -s "$H/keep" "$H/herdr-ssh-$gone-1"
  herdr_mux_sweep
  [ ! -e "$H/herdr-ssh-$gone-0" ] || exit 2
  grep -q -- "-O exit -S $H/herdr-ssh-$gone-0/cm-abc" "$T/mux.log" || exit 3
  [ -S "$H/herdr-ssh-$$-0/cm-def" ] && ! grep -q cm-def "$T/mux.log" || exit 4   # its herdr still runs: untouched
  [ -d "$H/herdr-ssh-x-0" ] && [ -L "$H/herdr-ssh-$gone-1" ] && [ -d "$H/keep" ] || exit 5   # not herdr's, a link: untouched
  exit 0 ) && ok "herdr's ssh masters: a gone herdr's is ended and removed, a running one's and anything else stays" || bad "mux sweep: step $?"
# what the agent does right now (herdr's third line): collected per pane, shown orange under the agent card with the dancing star in front of it
( : > "$T/now.out"; COLLECT_NOW="$T/now.out" collect_one Mac >/dev/null; [ "$(cat "$T/now.out")" = "Mac${US}w3:p1${US}✻ Warm-up clock [2J" ] || exit 1                  # (one line, for the working agent that has a now token)
  ! grep -q $'\033' "$T/now.out" || exit 2                                                       # (no escape code survives)
  AGENT_NOW_L=$T/now.l; AGENT_NOW_R=$T/now.r; printf 'Omarchy%sw9:p1%s✻ Building the warm-up clock\n' "$US" "$US" > "$AGENT_NOW_L"; : > "$AGENT_NOW_R"
  agent_now_set v Omarchy w9:p1; [ "$v" = "Building the warm-up clock" ] || exit 3                  # (herdr's star is cut off: bhote's own dances)
  agent_now_set v Omarchy w1:p1; [ -z "$v" ] || exit 4
  rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "Omarchy${US}w9:p1${US}working${US}bhote${US}x" > "$AGENT_LOCAL"
  topic_new "Card topic" ""  ""; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$cf" agent bhote; topic_set "$cf" agent_machine Omarchy; topic_set "$cf" agent_pane w9:p1; topic_set "$cf" status now
  AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}x"); COLS=50; ROWS=40; SPIN_CH=✶; ui_load; topics_load
  out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "▌   Building the warm-up clock" || exit 5
  printf '%s\n' "$out" | tr -d '\034\035' | grep -c "Building the warm-up" | grep -q '^1$' || exit 6
  AG_LINES=("Omarchy${US}w9:p1${US}idle${US}bhote${US}x"); out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); case "$out" in *"Building the warm-up"*) exit 7 ;; esac   # (only while it works)
  AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}x"); SPIN_CELLS=""; RUN_DIR=$T/run; mkdir -p "$RUN_DIR"
  HEAD=$(view_head >/dev/null 2>&1); bhote_view >/dev/null 2>&1 || true; sr=$(cat "$RUN_DIR/spinrows" 2>/dev/null); case "$sr" in *:3:*:3:*) exit 8 ;; *:3:[nsh]*) ;; *) exit 9 ;; esac   # (one star in the card: the first row; the now line has none)
  exit 0 ) && ok "agent card: what the agent does right now, orange without a star, only while it works" || bad "agent now line: step $?"
# dismissing a review card: the topic is put away; its agent working again opens the card again (as its work card), the agent working now does it at once
( rm -f "$TOPIC_DIR"/*.topic; AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}idle${US}bhote${US}x"); : > "$SHARED_DIR/evstate/w9_p1" 2>/dev/null
  topic_new "Result to read" "" ""; df="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$df" agent bhote; topic_set "$df" agent_machine "$HOST"; topic_set "$df" agent_pane w9:p1; topic_set "$df" status review
  review_dismiss "$df" && [ "$(topic_get "$df" status)" = later ] && [ "$(topic_get "$df" dismissed)" = 1 ] || exit 1               # idle agent: put away
  topics_load; blk=$(COLS=50; ROWS=40; NOW=$(date +%s); topics_block); case "$blk" in *"Result to read"*) exit 2 ;; esac            # (gone from the list)
  host_save=$HOST; HOST=Omarchy; topic_set "$df" agent_machine Omarchy; ev w9:p1 working; HOST=$host_save
  [ "$(topic_get "$df" status)" = now ] && [ -z "$(topic_get "$df" dismissed)" ] || exit 3                                         # the agent works again: the card is back
  topic_set "$df" status review; AG_LINES=("$HOST${US}w9:p1${US}working${US}bhote${US}x"); topic_set "$df" agent_machine "$HOST"
  review_dismiss "$df" && [ "$(topic_get "$df" status)" = now ] || exit 4                                                           # the agent works right now: straight back to now
  topic_set "$df" status next; review_dismiss "$df" && exit 5                                                                       # only review can be dismissed
  topic_new "No agent" "" ""; nf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$nf" status review; review_dismiss "$nf" && [ "$(topic_get "$nf" status)" = later ] || exit 6
  exit 0 ) && ok "dismiss: the review card closes; the agent working again opens it again" || bad "dismiss: step $?"
# a review card goes by itself when its agent works again after it was idle (decided in the session); not before it was idle, not Slack's, not a deployment's
( rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/revidle"
  mk() { topic_new "$1" "" ""; mkf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mkf" agent bhote; topic_set "$mkf" agent_machine "$HOST"; topic_set "$mkf" agent_pane "$2"; topic_set "$mkf" status review; }
  setst() { printf '%s\n%s\n%s\n' "$HOST${US}w5:p1${US}$1${US}b${US}x" "$HOST${US}w6:p1${US}$1${US}b${US}x" "$HOST${US}w7:p1${US}$1${US}b${US}x" > "$AGENT_LOCAL"; }
  mk "Asked" w5:p1; a=$mkf; mk "From Slack" w6:p1; sl=$mkf; topic_set "$sl" slack_msg "hello"; mk "Deployed" w7:p1; dp=$mkf; topic_set "$dp" review_src deploy
  setst working; review_follow; [ "$(topic_get "$a" status)" = review ] || exit 1          # still writing its hand-over: stays
  setst idle; review_follow; setst working; review_follow
  [ "$(topic_get "$a" status)" = now ] || exit 2                                                                             # idle first, then working: the decision was made
  [ "$(topic_get "$sl" status)" = review ] && [ "$(topic_get "$dp" status)" = review ] || exit 3                          # (an answer in Slack, a finished deployment: stay)
  mk "Question from bhote" w5:p1; q=$mkf; topic_set "$q" auto 1; setst idle; review_follow; setst working; review_follow; [ "$(topic_get "$q" deleted)" = 1 ] || exit 4
  exit 0 ) && ok "review: it goes by itself when the agent works again after it was idle" || bad "review follow: step $?"
# an auto question card goes when its agent is idle and the pane shows no question any more (two looks 8 s apart); a blocked agent keeps it; another machine: after FINISHED_MINUTES
( rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/qidle"; herdr() { return 0; }
  mk() { topic_new "Question from bhote" "" ""; mkf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mkf" auto 1; topic_set "$mkf" status review; topic_set "$mkf" agent_machine "$1"; topic_set "$mkf" agent_pane "$2"; }
  mk "$HOST" w5:p1; q=$mkf; mk "$HOST" w6:p1; b=$mkf
  printf '%s\n%s\n' "$HOST${US}w5:p1${US}idle${US}b${US}x" "$HOST${US}w6:p1${US}blocked${US}b${US}x" > "$AGENT_LOCAL"
  auto_question_follow; [ "$(topic_get "$q" deleted)" != 1 ] || exit 1                                                    # (first look only notes it)
  printf '%s\n' "$(( $(date +%s) - 20 ))" > "$SHARED_DIR/qidle/$(basename "$q" .topic)"
  auto_question_follow; [ "$(topic_get "$q" deleted)" = 1 ] && [ "$(topic_get "$b" deleted)" != 1 ] || exit 2
  exit 0 ) && ok "auto question: it goes when the agent is idle and nothing is asked any more" || bad "auto question follow: step $?"
# an agent with several recently finished cards: only the newest comes back on now when it works again (not all of them: two cards for one agent), none when it has one on now
( rm -f "$TOPIC_DIR"/*.topic
  mk() { topic_new "$1" "" ""; mkf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mkf" agent_pane w4:p1; topic_set "$mkf" agent_machine "$HOST"; topic_set "$mkf" agent_session s1; topic_set "$mkf" status "$2"; topic_set "$mkf" updated "$3"; }
  mk Old finished 100; o=$mkf; mk New finished 200; n=$mkf; mk Other finished 300; ot=$mkf; topic_set "$ot" agent_session s9; topic_set "$ot" agent_pane w5:p1
  topics_load; agent_reopen_pick s1 w4:p1 "$HOST" 1; [ "${T_TITLE[$REOPEN_IDX]}" = New ] || exit 1                      # (the newest of this agent, not another agent's)
  topic_set "$o" status now; topics_load; agent_reopen_pick s1 w4:p1 "$HOST" 1; [ "$REOPEN_IDX" = -1 ] || exit 2         # (it has a card on now already: no second one)
  exit 0 ) && ok "agent cards: of several finished cards only the newest of an agent reopens" || bad "reopen pick: step $?"
# an agent with two cards on now/testing keeps the newest; the older one is recently finished (an agent's own card and another agent's are left alone)
( rm -f "$TOPIC_DIR"/*.topic
  mk() { topic_new "$1" "" ""; mkf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mkf" agent_pane "$2"; topic_set "$mkf" agent_machine "$HOST"; topic_set "$mkf" agent_session s1; topic_set "$mkf" status "$3"; }
  mk Old w4:p1 testing; o=$mkf; sleep 1; mk New w4:p1 testing; n=$mkf; mk Other w5:p1 now; ot=$mkf
  agent_dedup; [ "$(topic_get "$o" status)" = finished ] && [ "$(topic_get "$n" status)" = testing ] && [ "$(topic_get "$ot" status)" = now ] || exit 1
  agent_dedup; [ "$(topic_get "$n" status)" = testing ] || exit 2
  exit 0 ) && ok "agent cards: one agent keeps one card on now/testing (the newest)" || bad "agent dedup: step $?"
# the old ones (ARCHIVE_DAYS) count with the done ones, in one section: the last line says "✓ 1 done · ▤ 1 archived"
( rm -f "$TOPIC_DIR"/*.topic; NOW=$(date +%s); COLS=60; FIND_KEY_SHOW="Ctrl+T"; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55)
  nc() { LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'; }
  topic_new "Fresh" "" ""; topic_new "Finished" "" ""; ff="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$ff" status done
  topic_new "Ancient" "" ""; af="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$af" status later; topic_set "$af" updated $(( NOW - 30 * 86400 ))
  topics_load; [ "$(archived_count)" = 1 ] || exit 1
  out=$(done_block 4 | nc); last=$(printf '%s\n' "$out" | tail -1)
  case "$last" in *"✓ 1 done · ▤ 1 archived") ;; *) exit 2 ;; esac
  case "$out" in *"Archived"*) exit 3 ;; esac                                                                               # (no section of its own)
  [ "$(printf '%s\n' "$out" | grep -c 'Done')" = 1 ] || exit 4
  section_toggle done; out=$(done_block 4 | nc); [ "$(printf '%s\n' "$out" | wc -l | tr -d ' ')" = 2 ] && case "$out" in *"✓ 1 done · ▤ 1 archived") ;; *) false ;; esac || exit 5   # (small view: caption and the line)
  section_toggle done; topic_set "$ff" status later; topics_load                                                           # (nothing done: the archived ones alone)
  out=$(done_block 4 | nc); case "$out" in *"Done"*"▤ 1 archived"*) ;; *) exit 6 ;; esac; case "$out" in *"✓"*) exit 7 ;; esac
  topic_set "$af" updated "$NOW"; topics_load; [ -z "$(done_block 4)" ] || exit 8                                          # (nothing done, nothing archived: no section)
  exit 0 ) && ok "done and archived: one section, ✓ done · ▤ archived on its last line" || bad "done/archived: step $?"
# the machine name in an agent card has the colour herdr gives it in its agent rows (pink wins), and the third line is indented like the rows above
( color=1; rgb() { printf '\033[38;2;%sm' "$1"; }; BOLD=$'\033[1m'
  HERDR_CONFIG=$T/h.toml; printf '%s\n' 'rows = [["state_icon", { token = "machine", fg = "#112233", bold = true, rules = [{ equals = "Local", hide = true }] }]]' > "$HERDR_CONFIG"
  MACH_FG_CACHE=""; machine_fg_set v; [ "$v" = $'\033[38;2;17;34;51m' ] || exit 1                                   # (herdr's own colour)
  : > "$HERDR_CONFIG"; MACH_FG_CACHE=""; machine_fg_set v; [ "$v" = $'\033[38;2;255;62;165m' ] || exit 2             # (none there: herdr's default pink)
  rm -f "$TOPIC_DIR"/*.topic; printf 'Omarchy%sw9:p1%s✻ Doing it\n' "$US" "$US" > "$AGENT_NOW_L"; printf '%s\n' "Omarchy${US}w9:p1${US}working${US}bhote${US}x" > "$AGENT_LOCAL"
  topic_new "Card" ""  "Beschr"; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$cf" agent bhote; topic_set "$cf" agent_machine Omarchy; topic_set "$cf" agent_pane w9:p1; topic_set "$cf" status now; topic_set "$cf" agent_branch fix/x
  cfg_set GITHUB_BRANCHES on; AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}working${US}bhote${US}x"); COLS=50; ROWS=40; SPIN_CH=✶; ui_load; topics_load
  out=$(topics_block); printf '%s' "$out" | grep -q $'\033\[38;2;255;62;165m\033\[1mOmarchy' || exit 3                  # (pink, bold)
  plain=$(printf '%s\n' "$out" | perl -pe 's/\x1b\[[0-9;]*[A-Za-z]//g; s/\x1c.\x1c//g; s/\x1d[^\x1d]*\x1d//g' | tr -d '\001'); r1=$(printf '%s\n' "$plain" | grep "Omarchy · bhote"); r3=$(printf '%s\n' "$plain" | grep "Doing it")
  [ "${r1%%Omarchy*}" = "▌ ✶ " ] && [ "${r3%%Doing*}" = "▌   " ] || { printf 'r1=[%s] r3=[%s]\n' "$r1" "$r3" | cat -v >&2; exit 4; }
  r4=$(printf '%s\n' "$plain" | grep "Beschr"); r5=$(printf '%s\n' "$plain" | grep "⎇ fix/x"); [ "${r4%%Beschr*}" = "▌   " ] && [ "${r5%%⎇*}" = "▌   " ] || exit 5   # (the description and the branch too)                                           # (text under text: column 5)
  echo "r1=[${r1%%Omarchy*}] r3=[${r3%%Doing*}]" | cat -v >&2; exit 0 ) && ok "agent card: machine name in herdr's colour (pink), the now line indented like the rows above" || bad "card colour/indent: step $?"
# recently finished: a now topic whose agent stops without a question lands there (a question: review), comes back on now when the agent works again,
# goes by itself after FINISHED_MINUTES; the card bhote makes for an agent without a topic goes the same way; key c / x end it
( cfg_set CARD_GROUPING status; rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/evstate"; HOST=Omarchy
  topic_new "Work" "" ""; wf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$wf" agent bhote; topic_set "$wf" agent_machine Omarchy; topic_set "$wf" agent_pane w9:p1; topic_set "$wf" status now; topic_set "$wf" delegated 1
  BHOTE_FAKE_QUESTION="" ev w9:p1 working; BHOTE_FAKE_QUESTION="" ev w9:p1 idle
  [ "$(topic_get "$wf" status)" = finished ] || exit 1                                            # no question: recently finished, no review
  ev w9:p1 working; [ "$(topic_get "$wf" status)" = now ] || exit 2                                # the agent works again: back on now
  BHOTE_FAKE_QUESTION="Soll ich pushen?" ev w9:p1 idle; [ "$(topic_get "$wf" status)" = review ] || exit 3   # a question: the user has to look
  topic_set "$wf" status finished; topics_load; COLS=50; ROWS=40; NOW=$(date +%s); AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}idle${US}bhote${US}x"); ui_load
  out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "RECENTLY FINISHED" && printf '%s\n' "$out" | grep -q "✓ Omarchy · bhote" && printf '%s\n' "$out" | grep -q "finished " || exit 4
  cfg_set FINISHED_MINUTES 1; touch -d "2 minutes ago" "$wf" 2>/dev/null; sed -i.bak 's/^updated=.*/updated=1000/' "$wf"; finished_expire
  [ "$(topic_get "$wf" status)" = later ] && [ "$(topic_get "$wf" dismissed)" = 1 ] || exit 5       # after the time: put away (like a dismissed review card)
  ev w9:p1 working; [ "$(topic_get "$wf" status)" = now ] || exit 6                                # ... and it comes back when the agent works
  topic_set "$wf" status finished; AG_N=1; AG_LINES=("Omarchy${US}w9:p1${US}idle${US}bhote${US}x"); review_dismiss "$wf"; [ "$(topic_get "$wf" status)" = later ] || exit 7   # c on the card (its agent is idle)
  cfg_set FINISHED_MINUTES 0; topic_set "$wf" status finished; sed -i.bak 's/^updated=.*/updated=1000/' "$wf"; finished_expire; [ "$(topic_get "$wf" status)" = finished ] || exit 8   # 0: never
  # the card of an agent without a topic
  rm -f "$TOPIC_DIR"/*.topic; cfg_set FINISHED_MINUTES 1; cfg_set AGENT_CARDS on; RUN_DIR=$T/run; mkdir -p "$RUN_DIR/cards"; : > "$RUN_DIR/cards/w8_p1"; echo 1 > "$RUN_DIR/cards/w8_p1"
  printf '%s\n' "Omarchy${US}w8:p1${US}working${US}claude${US}Doing it" > "$AGENT_LOCAL"; agent_cards; cf=$(ls "$TOPIC_DIR"/*.topic | head -1); [ "$(topic_get "$cf" auto_now)" = 1 ] && [ "$(topic_get "$cf" status)" = now ] || exit 9
  printf '%s\n' "Omarchy${US}w8:p1${US}idle${US}claude${US}Doing it" > "$AGENT_LOCAL"; agent_cards; [ "$(topic_get "$cf" status)" = finished ] && [ "$(topic_get "$cf" deleted)" != 1 ] || exit 10   # stopped: recently finished
  printf '%s\n' "Omarchy${US}w8:p1${US}working${US}claude${US}Doing it" > "$AGENT_LOCAL"; agent_cards; [ "$(topic_get "$cf" status)" = now ] || exit 11                  # works again: the same card is back on now
  printf '%s\n' "Omarchy${US}w8:p1${US}idle${US}claude${US}Doing it" > "$AGENT_LOCAL"; agent_cards; sed -i.bak 's/^updated=.*/updated=1000/' "$cf"; finished_expire; [ "$(topic_get "$cf" deleted)" = 1 ] || exit 12   # after the time: gone
  exit 0 ) && ok "recently finished: a stopped agent without a question, back on now when it works, dismissed by itself, c ends it" || bad "recently finished: step $?"
# a topic on now whose agent stopped and no event moved it: recently finished after two looks; a stale "working" right after the stop does not reopen a finished card
( rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/nowidle" "$SHARED_DIR/revidle"; HOST=Omarchy
  topic_new "Stuck" "" ""; sf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$sf" agent bhote; topic_set "$sf" agent_machine Omarchy; topic_set "$sf" agent_pane w9:p1; topic_set "$sf" status now; sed -i.bak 's/^updated=.*/updated=1000/' "$sf"
  setagent() { printf '%s\n' "Omarchy${US}w9:p1${US}$1${US}bhote${US}x" > "$AGENT_LOCAL"; }
  setagent working; now_idle_follow; now_idle_follow; [ "$(topic_get "$sf" status)" = now ] || exit 1                      # it works: stays on now
  setagent idle; now_idle_follow; [ "$(topic_get "$sf" status)" = now ] || exit 2                                          # seen idle once: not yet
  echo $(( $(date +%s) - 10 )) > "$SHARED_DIR/nowidle/$(topic_id "$sf")"; herdr() { return 0; }; now_idle_follow
  [ "$(topic_get "$sf" status)" = finished ] || exit 3                                                                      # idle for 8 s and more: recently finished
  setagent working; review_follow; [ "$(topic_get "$sf" status)" = finished ] || exit 4                                    # (the list was stale: a working right after the stop does not reopen it)
  setagent idle; review_follow; setagent working; review_follow; [ "$(topic_get "$sf" status)" = now ] || exit 5            # (idle seen, then working: reopened)
  topic_set "$sf" delegated "$(date +%s)"; setagent idle; now_idle_follow; now_idle_follow; [ "$(topic_get "$sf" status)" = now ] || exit 6   # just handed to the agent: it has not started yet
  exit 0 ) && ok "now with an idle agent: recently finished after a while (a missed event); a stale working does not reopen it" || bad "now idle follow: step $?"
# the agent of a card is gone (session ended): the review card closes, a card bhote made is deleted, a topic on now becomes recently finished; not when herdr is silent
( rm -f "$TOPIC_DIR"/*.topic; rm -rf "$SHARED_DIR/agentgone"; HOST=Omarchy
  printf '%s\n' "Omarchy${US}w1:p1${US}idle${US}a${US}x" > "$AGENT_LOCAL"; printf '%s\n' "Box${US}w2:p1${US}idle${US}b${US}y" > "$AGENT_REMOTE"
  mk() { topic_new "$1" "" ""; mkf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mkf" agent_machine "$2"; topic_set "$mkf" agent_pane "$3"; topic_set "$mkf" status "$4"; }
  mk "Gone review" Box w9:p1 review; g1=$mkf; mk "Gone question" Box w9:p2 review; g2=$mkf; topic_set "$g2" auto 1; mk "Gone now" Omarchy w9:p3 now; g3=$mkf
  mk "Present" Box w2:p1 review; g4=$mkf; mk "Unknown machine" Nowhere w1:p1 review; g5=$mkf
  past() { for f in "$SHARED_DIR"/agentgone/*; do echo $(( $(date +%s) - 40 )) > "$f"; done; }
  herdr() { return 1; }; agent_gone_follow; past; agent_gone_follow
  [ "$(topic_get "$g1" status)" = review ] || exit 1                                                                       # herdr silent: nobody is gone
  herdr() { return 0; }; agent_gone_follow; past; agent_gone_follow
  [ "$(topic_get "$g1" status)" = later ] && [ "$(topic_get "$g1" dismissed)" = 1 ] || exit 2                              # its session ended: the review card closes
  [ "$(topic_get "$g2" deleted)" = 1 ] || exit 3                                                                            # a card bhote made itself: gone
  [ "$(topic_get "$g3" status)" = finished ] || exit 4                                                                      # a topic on now: recently finished
  [ "$(topic_get "$g4" status)" = review ] && [ "$(topic_get "$g5" status)" = review ] || exit 5                           # (the agent is there; a machine that is not in the lists)
  : > "$AGENT_REMOTE"; mk "Machine without agents" Box w3:p1 review; g6=$mkf; agent_gone_follow; past; agent_gone_follow; [ "$(topic_get "$g6" status)" = review ] || exit 6   # (a machine with no agents listed: left alone)
  exit 0 ) && ok "gone agent: its review card closes (a made card goes, now becomes recently finished), only when herdr answers" || bad "gone agent: step $?"
# the agent overview as counts (the ↕ on the divider), and keys that only move the selection draw from what is loaded
( rm -f "$TOPIC_DIR"/*.topic; RUN_DIR=$T/run2; mkdir -p "$RUN_DIR"; COLS=50; ROWS=40; TOO_SMALL=0; view=main
  printf '%s\n' "Omarchy${US}w1:p1${US}working${US}a${US}x" "Omarchy${US}w2:p1${US}testing${US}b${US}x" "Omarchy${US}w3:p1${US}idle${US}c${US}x" "Omarchy${US}w4:p1${US}done${US}d${US}x" "Omarchy${US}w5:p1${US}blocked${US}e${US}x" > "$AGENT_LOCAL"
  cfg_set AGENTS_COMPACT on; ui_load; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); NOW=$(date +%s)
  out=$(bhote_draw main | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "✶ 2 active · ◉ 1 need you · ○ 1 inactive" || exit 1            # working + testing, blocked, done (an idle agent is on a card)
  printf '%s\n' "$out" | grep -q "↕" || exit 2                                                                                                  # the button on the divider
  cfg_set AGENTS_COMPACT off; ui_load; out=$(bhote_draw main | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "active" && exit 3     # (the full list again)
  echo 30 > "$RUN_DIR/divrow"; MOUSE_KEY=""; mouse_event "0;47;30" M; [ "$MOUSE_KEY" = AGMENU ] || exit 4                                  # a click on the button
  MOUSE_KEY=""; mouse_event "0;10;30" M; [ -z "$MOUSE_KEY" ] || exit 5                                                                            # elsewhere on the divider: nothing (no drag handle)
  # the ↕ is lit while the pointer is on it
  mouse_event "35;47;30" M; [ "$MOUSE_KEY" = HOVER ] && [ "$HOVER_BTN" = 1 ] || exit 8; MOUSE_KEY=""; mouse_event "35;48;30" M; [ -z "$MOUSE_KEY" ] || exit 9      # (on it: a redraw once)
  ( ORANGE=LIT; BOLD=B; D=DIM; X=; HOVER_BTN=1; agent_divider | grep -q "BLIT↕" ) || exit 10; ( ORANGE=LIT; BOLD=B; D=DIM; X=; HOVER_BTN=0; agent_divider | grep -q "DIM↕" ) || exit 11
  mouse_event "35;5;30" M; [ "$MOUSE_KEY" = HOVER ] && [ "$HOVER_BTN" = 0 ] || exit 12                                                                              # (off it: a redraw again)
  # nav-only: what changed on disk is not read again, the next full paint reads it (drawn by bash: the daemon's blocks read the files themselves, a frame sooner)
  cfg_set BHOTED_FRAME off; topic_new "First" "" ""; ui_load; topic_new "Added later" "" ""
  p1=$(NAV_ONLY=1; bhote_paint main | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); case "$p1" in *"Added later"*) exit 6 ;; esac
  p2=$(bhote_paint main | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); case "$p2" in *"Added later"*) ;; *) exit 7 ;; esac
  exit 0 ) && ok "agent overview: counts only (↕ on the divider); a key that only moves the selection does not reload" || bad "compact/nav: step $?"
# done topics and the lines of next, waiting and parked topics have a hover like the cards: a step lighter under the pointer
( rm -f "$TOPIC_DIR"/*.topic; COLS=50; ROWS=40; NOW=$(date +%s); AG_N=0; AG_LINES=(); ui_load; CARDHOV=HOVBG; DSEL=DS; LSEL=LS; SELBG=SELX
  topic_new "Finished one" "" ""; d1="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$d1" status done; did=$(topic_id "$d1")
  topic_new "Up next one" "" ""; n1="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$n1" status next; nid=$(topic_id "$n1")
  topics_load; HOVER_ID=""; out=$(done_block 4); case "$out" in *HOVBG*) exit 1 ;; esac                                      # (nobody hovered: plain)
  HOVER_ID=$did; out=$(done_block 4); case "$out" in *HOVBG*"Finished one"*) ;; *) exit 2 ;; esac                           # (the done one under the pointer)
  case "$out" in *"$(printf '\035%s\035' "$did")"*) ;; *) exit 3 ;; esac                                                       # (and it stays clickable: its ref is still there)
  HOVER_ID=$nid; SEL_IDX=-99; out=$(topic_line "$(for ((k=0;k<T_N;k++)); do [ "${T_ID[$k]}" = "$nid" ] && echo $k; done)"); case "$out" in *HOVBG*"Up next one"*) ;; *) exit 4 ;; esac   # (a line of the list)
  cfg_set DONE_COMPACT on; topic_new "Finished two" "" ""; topic_set "$TOPIC_DIR/$NEW_TOPIC_ID.topic" status done; topics_load
  out=$(done_block 4 | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); case "$out" in *"2 done"*) ;; *) exit 5 ;; esac; case "$out" in *"Finished one"*) exit 6 ;; esac   # (done, small: the number, no rows)
  case "$out" in *"$(printf '\035cap:done\035')"*) ;; *) exit 7 ;; esac; cfg_set DONE_COMPACT off
  exit 0 ) && ok "hover: done topics and the lines of the list are a step lighter under the pointer" || bad "hover lines: step $?"
# waiting for a deployment: its own group, bound to the deployment of its project (progress bar), review when that is over
( cfg_set CARD_GROUPING status; rm -f "$TOPIC_DIR"/*.topic "$SHARED_DIR/deploys.list"; projects_load; project_new IMS >/dev/null; projects_load; pid=$PROJ_ID
  topic_new "Payments Module" "" "check in production"; df="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$df" project "$pid"
  topic_new "Plain wait" "Alex" ""; pf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"
  deploy_wait_set "$df" Backend; topics_load
  [ "${T_DEP[0]:-}" = 1 ] && [ "${T_STATUS[0]}" = waiting ] && [ "${T_TITLE[0]}" = "Payments Module" ] && [ "${T_DEP[1]:-}" != 1 ] || exit 1   # (the group comes before the plain waiting ones)
  now=$(date +%s); printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}44${US}56${US}fix/x${US}https://ci/1" "$now${US}IMS${US}Frontend${US}running${US}1${US}4${US}fix/y${US}https://ci/2" > "$SHARED_DIR/deploys.list"
  deploy_lookup "$df" && [ "$DL_URL" = https://ci/1 ] && [ "$DL_STATE" = running ] || exit 2                   # (the label narrows it: Backend, not Frontend)
  COLS=50; ROWS=60; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); NOW=$now; AG_N=0; AG_LINES=(); ui_load; projects_load; topics_load
  out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g')
  printf '%s\n' "$out" | grep -q "DEPLOYING" || exit 3
  printf '%s\n' "$out" | grep -q "IMS · Backend · deploying" || exit 4
  bar=$(printf '%s\n' "$out" | LC_ALL=C grep -a "78%"); [ -n "$bar" ] && [ "$(printf '%s' "$bar" | LC_ALL=C grep -a -o '■' | wc -l)" -ge 38 ] || exit 5                                                              # 44 of 56 steps
  printf '%s\n' "$out" | grep -q "Plain wait" || exit 6
  [ "$(printf '%s\n' "$out" | grep -n "DEPLOYING" | head -1 | cut -d: -f1)" -lt "$(printf '%s\n' "$out" | grep -n "Plain wait" | head -1 | cut -d: -f1)" ] || exit 7
  # without a deployment of its project (an agent said "waiting for a deployment" and none runs) it is no card in DEPLOYING: a line in the Waiting list
  mv "$SHARED_DIR/deploys.list" "$SHARED_DIR/deploys.keep"; printf '%s\n' "$now${US}Other${US}Backend${US}running${US}1${US}2${US}fix/o${US}https://ci/9" > "$SHARED_DIR/deploys.list"
  ui_load; topics_load; deploy_cards_prune; [ "${T_DNO[0]:-}" = 1 ] || exit 20
  out=$(topics_block | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g'); printf '%s\n' "$out" | grep -q "DEPLOYING" && exit 21
  printf '%s\n' "$out" | grep -q "Payments Module" || exit 22
  mv "$SHARED_DIR/deploys.keep" "$SHARED_DIR/deploys.list"; topics_load
  deploy_sync; [ "$(topic_get "$df" status)" = waiting ] && [ "$(topic_get "$df" deploy_url)" = https://ci/1 ] || exit 8   # bound, still under way
  printf '%s\n' "$now${US}IMS${US}Backend${US}ok${US}${US}${US}fix/x${US}https://ci/1" > "$SHARED_DIR/deploys.list"
  deploy_sync; [ "$(topic_get "$df" status)" = review ] && [ -z "$(topic_get "$df" deploy_wait)" ] && [ -z "$(topic_get "$df" deploy_url)" ] || exit 9
  case "$(topic_get "$df" review_note)" in *"went through"*) ;; *) exit 10 ;; esac
  [ "$(topic_get "$df" review_reply)" = @jump ] || exit 11
  deploy_wait_set "$df"; now=$(( $(date +%s) + 5 )); printf '%s\n' "$now${US}IMS${US}Backend${US}failed${US}${US}${US}fix/x${US}https://ci/9" > "$SHARED_DIR/deploys.list"
  deploy_sync; [ "$(topic_get "$df" status)" = review ] && case "$(topic_get "$df" review_note)" in *"failed"*) true ;; *) false ;; esac || exit 12
  # an agent's topic on the branch that is being deployed goes there by itself
  topic_new "On a branch" "" ""; bf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$bf" agent_pane w3:p1; topic_set "$bf" agent_machine Omarchy; topic_set "$bf" agent_branch fix/z; topic_set "$bf" status now
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}2${US}fix/z${US}https://ci/3" > "$SHARED_DIR/deploys.list"
  deploy_sync; [ "$(topic_get "$bf" status)" = waiting ] && [ "$(topic_get "$bf" deploy_wait)" = 1 ] || exit 13
  topic_set "$bf" status now; [ -z "$(topic_get "$bf" deploy_wait)" ] || exit 14                                  # leaving the wait ends it
  topic_set "$bf" status finished; deploy_sync; [ "$(topic_get "$bf" status)" = waiting ] && [ "$(topic_get "$bf" deploy_wait)" = 1 ] || exit 30   # (its agent has stopped: the branch is still on its way, not "recently finished")
  topic_set "$bf" status now
  # the agent went back to its own branch after merging: the last commit in its folder is the merge of the pull request that is being deployed
  hd=$T/headrepo; mkdir -p "$hd"; git -C "$hd" init -q . && git -C "$hd" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "Merge pull request #77 from acme/feat-q"
  printf '%s\n' "w3:p8${US}$hd" > "$T/head.cwd"; AGENT_CWD=$T/head.cwd
  topic_new "Went back" "" ""; hf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$hf" agent_pane w3:p8; topic_set "$hf" agent_machine "$HOST"; topic_set "$hf" agent_branch wip/own-branch; topic_set "$hf" status finished
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}2${US}feat-q (#77)${US}https://ci/77" > "$SHARED_DIR/deploys.list"
  deploy_sync; [ "$(topic_get "$hf" status)" = waiting ] && [ "$(topic_get "$hf" deploy_wait)" = 1 ] || exit 31
  topic_set "$hf" status finished; topic_set "$hf" deploy_wait ""; sed -i.bak 's/^updated=.*/updated=1000000000/' "$hf"; rm -f "$hf.bak"; topics_load   # (finished long before the deployment began)
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}2${US}feat-q (#77)${US}https://ci/77" > "$SHARED_DIR/deploys.list"; deploy_sync; [ "$(topic_get "$hf" status)" = finished ] || exit 33   # an old finished topic of that agent is not what is deployed
  topic_set "$hf" status finished; topic_set "$hf" deploy_wait ""; printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}2${US}feat-other (#78)${US}https://ci/78" > "$SHARED_DIR/deploys.list"
  deploy_sync; [ "$(topic_get "$hf" status)" = finished ] || exit 32                                              # (another pull request: it stays)
  # a topic with a branch waits for the deployment of that branch, not for the newest one of the project (also when it was bound to another one before)
  topic_new "Mine" "" ""; mf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$mf" project "$pid"; topic_set "$mf" agent_branch fix/cancellation-service-until-end; deploy_wait_set "$mf"; now=$(date +%s)
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}64${US}70${US}feat/contract-sheet-catalogue-g…${US}https://ci/7" > "$SHARED_DIR/deploys.list"
  deploy_lookup "$mf" && exit 15                                                                                      # (another branch: no deployment seen yet)
  topic_set "$mf" deploy_url https://ci/7; deploy_lookup "$mf" && exit 16                                               # (bound wrongly before: ignored)
  printf '%s\n' "$now${US}IMS${US}Backend${US}running${US}1${US}70${US}fix/cancellation-service-un…${US}https://ci/8" "$now${US}IMS${US}Backend${US}running${US}64${US}70${US}feat/contract-sheet-catalogue-g…${US}https://ci/7" > "$SHARED_DIR/deploys.list"
  topic_set "$mf" deploy_url ""; deploy_lookup "$mf" && [ "$DL_URL" = https://ci/8 ] || exit 17                        # (the cut note matches by its start)
  exit 0 ) && ok "deployment wait: own group with the progress, review when the deployment is over, an agent's branch finds it" || bad "deployment wait: step $?"
# the Rust daemon writes the same agent files as collect_local (fake herdr, escape codes, a running test); skipped without the binary
BD=""; bhoted_bin_set BD
if [ -n "$BD" ]; then
  for tb in on off; do
    cfg_set TESTS_BUSY $tb; PS=$T/par-$tb; rm -rf "$PS"; mkdir -p "$PS/b" "$PS/r"
    ( SHARED_DIR=$PS/b; AGENT_LOCAL=$SHARED_DIR/agents.local; AGENT_META_L=$SHARED_DIR/agents.meta.local; AGENT_NOW_L=$SHARED_DIR/agents.now.local; AGENT_CWD=$SHARED_DIR/agents.cwd
      collect_local ) >/dev/null 2>&1
    "$BD" collect "$PS/r" "$HOST" --config "$CONFIG_FILE" || bad "bhoted collect failed"
    pf=1; for f in agents.local agents.meta.local agents.now.local agents.cwd focus.pane; do cmp -s "$PS/b/$f" "$PS/r/$f" || { pf=0; echo "  differs: $f"; command diff <(command cat -v "$PS/b/$f") <(command cat -v "$PS/r/$f") | head -3; }; done
    [ "$pf" = 1 ] && ok "bhoted: the same agent files as collect_local (TESTS_BUSY=$tb)" || bad "bhoted agent files differ (TESTS_BUSY=$tb)"
  done
  cfg_set TESTS_BUSY on
fi

# the other machines through kharka: bhoted writes the same files as collect_remote_kharka (a fake kharka: a fresh machine, a stale one,
# a node the hub says is offline, control characters, a running test in a workspace); skipped without the binary
if [ -n "$BD" ]; then
  cat > "$T/bin/kharka" <<'SH'
#!/bin/sh
nowms=$(( $(date +%s) * 1000 ))
case "$*" in
  whoami) echo thisnode ;;
  "ls herdr/ --entries")
    printf '%s\n' "{\"key\":\"herdr/thisnode\",\"hlc\":\"$nowms-0-thisnode\",\"value\":{\"label\":\"Linux\",\"herdr\":\"online\",\"agents\":[{\"pane\":\"w1:p1\",\"status\":\"idle\"}]}}"
    printf '%s\n' "{\"key\":\"herdr/macnode\",\"hlc\":\"$nowms-0-macnode\",\"value\":{\"label\":\"Mac Book\",\"herdr\":\"online\",\"commands\":[\"agent.prompt\",\"agent.read\"],\"workspaces\":[{\"id\":\"w1\",\"label\":\"IMS (main)\",\"tokens\":{\"tests\":\"◌ running\"}},{\"id\":\"w2\",\"label\":\"Other\"}],\"agents\":[{\"pane\":\"w1:p1\",\"workspace\":\"w1\",\"status\":\"idle\",\"tab\":\"w1:t1\",\"cwd\":\"/Users/x\",\"tokens\":{\"task\":\"∟ Do a\\u001b[2Jthing\",\"repo\":\"github.com/a/b\",\"project\":\"proj-ims\",\"now\":\"✳ working here\"}},{\"pane\":\"w2:p1\",\"workspace\":\"w2\",\"status\":\"working\",\"tokens\":{}},{\"pane\":\"bad pane\",\"status\":\"idle\"}]}}"
    printf '%s\n' "{\"key\":\"herdr/oldnode\",\"hlc\":\"$(( nowms - 200000 ))-0-oldnode\",\"value\":{\"label\":\"Old\",\"herdr\":\"online\",\"agents\":[{\"pane\":\"w9:p1\",\"workspace\":\"w9\",\"status\":\"done\"}]}}"
    printf '%s\n' "{\"key\":\"herdr/downnode\",\"hlc\":\"$nowms-0-downnode\",\"value\":{\"label\":\"Down\",\"herdr\":\"online\",\"agents\":[{\"pane\":\"w3:p1\",\"status\":\"idle\"}]}}"
    printf '%s\n' "{\"key\":\"herdr/hidden\",\"hlc\":\"$nowms-0-hidden\",\"value\":{\"label\":\"Hidden\",\"herdr\":\"online\",\"agents\":[{\"pane\":\"w4:p1\",\"status\":\"idle\"}]}}" ;;
  "ls _nodes/ --entries")
    printf '%s\n' '{"key":"_nodes/macnode","value":{"online":true}}' '{"key":"_nodes/downnode","value":{"online":false}}' ;;
  *) exit 1 ;;
esac
SH
  chmod +x "$T/bin/kharka"
  for tb in on off; do
    cfg_set TESTS_BUSY $tb; cfg_set MACHINES_OFF Hidden; PS=$T/rem-$tb; rm -rf "$PS"; mkdir -p "$PS/b" "$PS/r"
    ( SHARED_DIR=$PS/b; AGENT_REMOTE=$SHARED_DIR/agents.remote; AGENT_META_R=$SHARED_DIR/agents.meta.remote; AGENT_NOW_R=$SHARED_DIR/agents.now.remote; AGENT_NODES=$SHARED_DIR/agents.nodes
      printf 'thisnode\n' > "$SHARED_DIR/kharka.self"; collect_remote_kharka ) >/dev/null 2>&1
    printf 'thisnode\n' > "$PS/r/kharka.self"
    "$BD" collect-remote "$PS/r" "$HOST" --config "$CONFIG_FILE" || bad "bhoted collect-remote failed"
    pf=1; for f in agents.remote agents.meta.remote agents.now.remote agents.nodes machines.offline; do cmp -s "$PS/b/$f" "$PS/r/$f" || { pf=0; echo "  differs: $f"; command diff <(command cat -v "$PS/b/$f") <(command cat -v "$PS/r/$f") | head -3; }; done
    [ "$pf" = 1 ] && [ -s "$PS/r/agents.remote" ] && ok "bhoted: the same remote agent files as collect_remote_kharka (TESTS_BUSY=$tb)" || bad "bhoted remote agent files differ (TESTS_BUSY=$tb)"
  done
  # the daemon of kharka does not answer: every machine keeps its agents, muted
  printf '#!/bin/sh\nexit 1\n' > "$T/bin/kharka"; PS=$T/rem-off
  ( SHARED_DIR=$PS/b; AGENT_REMOTE=$SHARED_DIR/agents.remote; AGENT_META_R=$SHARED_DIR/agents.meta.remote; AGENT_NOW_R=$SHARED_DIR/agents.now.remote; AGENT_NODES=$SHARED_DIR/agents.nodes; collect_remote_kharka ) >/dev/null 2>&1
  "$BD" collect-remote "$PS/r" "$HOST" --config "$CONFIG_FILE"
  cmp -s "$PS/b/agents.remote" "$PS/r/agents.remote" && cmp -s "$PS/b/machines.offline" "$PS/r/machines.offline" && grep -q "off-" "$PS/r/agents.remote" && ok "bhoted: no answer from kharka keeps the agents, muted, as the script does" || bad "bhoted: the muted list differs"
  rm -f "$T/bin/kharka"; cfg_set TESTS_BUSY on; cfg_set MACHINES_OFF ""
fi

rm -rf "$T"; exit $fail
