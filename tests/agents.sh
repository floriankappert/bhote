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
topic_set "$ef" agent "Shop"; topic_set "$ef" agent_machine Mac; topic_set "$ef" agent_pane w5:p1; topic_set "$ef" agent_session "s-777"; topic_set "$ef" delegated 1
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
echo "$frame" | grep -q "LPT ◐ reconnecting" && echo "$frame" | grep -q "LPT|" && ok "panel: the machine (its code) says reconnecting, its agents stay listed" || bad "panel reconnecting: $(echo "$frame" | grep -n 'LPT\|┈')"
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
frame=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=2; AG_LINES=($'Laptop\037w9:p2\037idle\037Laptop / root\037' $'Laptop\037w9:p3\037idle\037api\037'); agents_block )
echo "$frame" | grep -q "|" && bad "one machine: codes shown" || ok "codes: one machine listed, no codes"
frame=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=2; AG_LINES=($'Laptop\037w9:p2\037idle\037Laptop / root\037' $"$HOST"$'\037w1:p1\037idle\037api\037'); agents_block )
echo "$frame" | grep -q "LPT|root" && echo "$frame" | grep -q "|api" && ! echo "$frame" | grep -qi "^ *laptop *$" && ok "codes: CODE|name, no machine sub-captions (Laptop / root is LPT|root)" || bad "codes in the list: $frame"
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
grp=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); AG_N=0; AG_LINES=(); while IFS= read -r l; do [ -n "$l" ] && { AG_LINES[$AG_N]=$l; AG_N=$(( AG_N + 1 )); }; done < "$AGENT_LOCAL"
  projects_load; CUR_PROJ=proj-zeta; agents_block | sed 's/\x1b\[[0-9;]*m//g' )
caps=$(echo "$grp" | grep '┈┈' | sed 's/ *┈*$//; s/^ *┈┈ //' | tr '\n' '|')
[ "$caps" = "Zeta|Shop|no project|" ] && ok "agents by project: the panel's own first, no project last" || bad "project captions: $caps"
echo "$grp" | grep -A1 '┈┈ Shop' | tail -1 | grep -q 'Project A' && ok "agents by project: the agent under its project" || bad "agent under project: $grp"
( HERDR_PANE_ID=w9:p9; current_project; [ "$CUR_PROJ" = proj-zeta ] ) && ok "the panel's project: of the agent in its tab" || bad "current project"
t1=$(BHOTE_NO_SYNC=1 bhote_cli add "Zeta work" -p Zeta --json | jq -r .id); BHOTE_NO_SYNC=1 bhote_cli add "Shop work" -p Shop >/dev/null; BHOTE_NO_SYNC=1 bhote_cli add "Loose work" >/dev/null
[ "$(BHOTE_NO_SYNC=1 bhote_cli show "$t1" --json | jq -r .project)" = Zeta ] && ok "add -p: the topic's project" || bad "topic project"
blk=$( COLS=60; ROWS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); NOW=$(date +%s); topics_load; projects_load; CUR_PROJ=proj-zeta; topics_projects; U_SCOPE=project; AG_N=0; topics_block | sed 's/\x1b\[[0-9;]*m//g' )
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
echo "$frame" | grep -A3 "Login form" | grep -q "⎇ feature/login" && ok "github: a branch line under the topic" || bad "panel branch line: $(echo "$frame" | grep -A3 'Login form')"
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
ev w9:p1 working; ev w9:p1 idle; [ "$(topic_get "$tf" status)" = review ] && ok "auto review: a topic in work goes to review" || bad "now->review: $(topic_get "$tf" status)"
ev w9:p1 working; [ "$(topic_get "$tf" status)" = now ] && ok "auto review: and back to now when the agent works again" || bad "review->now: $(topic_get "$tf" status)"
# a topic the agent put under review itself (bhote review) goes back to now when it starts working again; a working event while it already works changes nothing
topic_set "$tf" status review; topic_set "$tf" auto_back ""; rm -rf "$SHARED_DIR/evstate"; ev w9:p1 idle
ev w9:p1 working; [ "$(topic_get "$tf" status)" = now ] && ok "review: a topic handed back with bhote review goes to now when the agent works again" || bad "review->now (manual): $(topic_get "$tf" status)"
topic_set "$tf" status review; ev w9:p1 working; [ "$(topic_get "$tf" status)" = review ] && ok "review: a repeated working event while it works changes nothing" || bad "review kept: $(topic_get "$tf" status)"
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
printf '%s\n' "$HOST${US}w9:p1${US}idle${US}Shop (main)${US}Next step" > "$AGENT_LOCAL"; agent_cards; [ "$(topic_get "$cf" deleted)" = 1 ] && ok "agent cards: the card goes when the agent stops" || bad "card stayed when idle"
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
# Steal & Transfer into another project is refused (only on purpose)
( agent_live() { LIVE_ST=idle; LIVE_SESS=s1; LIVE_CWD=/x; }; notify() { :; }; agent_send() { :; }
  rm -f "$TOPIC_DIR"/*.topic; printf '%s\n' "Mac${US}w2:p1${US}t${US}a${US}${US}proj-shop${US}/y" > "$AGENT_META_L"; : > "$AGENT_META_R"
  topic_new "Doorbell fix" "" ""; xf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"; topic_set "$xf" project proj-omarchy; topic_set "$xf" agent_pane w3:p1; topic_set "$xf" agent_machine Mac
  transfer_start "$xf" Mac w3:p1 root Mac w2:p1 "Shop (main)" 2>/dev/null; [ $? = 3 ] || exit 1
  [ -z "$(topic_get "$xf" transfer_to)" ] || exit 2
  TRANSFER_FORCE=1 transfer_start "$xf" Mac w3:p1 root Mac w2:p1 "Shop (main)" 2>/dev/null; [ -n "$(topic_get "$xf" transfer_to)" ] || exit 3
  exit 0 ) && ok "transfer: an agent of another project is refused, unless forced" || bad "transfer project guard: step $?"
# a click on a topic: every row of an entry selects it, captions and gaps do not; the window stays while the selection is in it
( mkdir -p "$RUN_DIR"; rm -f "$TOPIC_DIR"/*.topic "$RUN_DIR/toff"; AG_N=0; AG_LINES=(); : > "$AGENT_LOCAL"; : > "$AGENT_REMOTE"
  for i in 1 2 3 4 5 6 7 8 9 10; do topic_new "Click topic $i" "desc $i" "" >/dev/null; done
  COLS=44; ROWS=30; ui_load; fr=$(bhote_view); [ -s "$RUN_DIR/tmap" ] || exit 1
  while IFS=$'\036' read -r r t; do ln=$(printf '%s\n' "$fr" | sed -n "${r}p")
      case "$ln" in *"Click topic"*|*"∟"*|*▄*|*▀*) ;; *) exit 2 ;; esac; done < "$RUN_DIR/tmap"
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
  r=$(awk -F'\036' '$2 == 1 { print $1 }' "$RUN_DIR/pmap"); ln=$(printf '%s\n' "$out" | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | sed -n "${r}p")
  menu_build "$TOPIC_DIR/$SEL_ID.topic"; case "$ln" in *"2 "*"${MENU_ITEMS[1]#*|}"*) ;; *) exit 2 ;; esac
  mouse_event "0;8;$r" M; [ "$MOUSE_KEY" = ITEM ] && [ "$CLICK_I" = 1 ] || exit 3
  mouse_event "0;8;2" M; [ "$MOUSE_KEY" != ITEM ] || exit 4
  r=$(awk -F'\036' '$2 == -2 { print $1; exit }' "$RUN_DIR/pmap"); printf '%s\n' "$out" | sed -n "${r}p" | grep -q "Page topic" || exit 6
  mouse_event "0;8;$r" M; [ "$MOUSE_KEY" = ITEM ] && [ "$CLICK_I" = -2 ] || exit 7
  grep -q $'\036-1$' "$RUN_DIR/pmap" || exit 8                     # the description (empty: its hint line) too
  out=$(bhote_paint main); [ ! -s "$RUN_DIR/pmap" ] || exit 5
  exit 0 ) && ok "click: actions, title and description on the topic's page are rows of their own (ITEM with the index), the list has none" || bad "action click: step $?"
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
rm -rf "$T"; exit $fail
