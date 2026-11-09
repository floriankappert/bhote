#!/usr/bin/env bash
# Tests of herdr-app as the source of the agents: bhoted turns what `herdr-app ls`, `herdr-app status` and `hc ls` say into the shapes of
# herdr's pane list and workspace list, writes the agent files of this machine and of the others, and answers the script's herdr calls
# (agent prompt, pane run, agent focus, --machine). A fake herdr-app and a fake hc answer; nothing touches the real ones.
# Skipped without the bhoted binary. Usage: bash tests/herdr_app.sh
cd "$(dirname "$0")/.." || exit 1
unset HERDR_TAB_ID HERDR_PANE_ID CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_WORKSPACE_ID
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared FAKE_LOG=$T/log
unset BHOTE_HERDR_BACKEND   # herdr-app is the default; one check asks for the classic herdr on purpose
mkdir -p "$T/bin"; : > "$FAKE_LOG"
cat > "$T/bin/herdr-app" <<'SH'
#!/bin/sh
[ -n "${FAKE_APP_DOWN:-}" ] && exit 1
case "$1" in
 ls) cat <<'J'
{"spaces":[
 {"id":"sp1","name":"proj","folder":"/work/a","machine":null,"active":false,"tabs":[{"id":"tb1","name":"1","active":true,"terminals":[
   {"terminal":"t1","pane":"p1","status":"working","app":"claude-code","name":null,"sub_agents":[]},
   {"terminal":"t2","pane":"p2","status":null,"app":null,"name":null,"sub_agents":[]}]}]},
 {"id":"sp2","name":"other","folder":"/work/b","machine":null,"active":true,"tabs":[{"id":"tb2","name":"1","active":true,"terminals":[
   {"terminal":"t3","pane":"p3","status":"idle","app":"claude-code","name":null,"sub_agents":[]}]}]},
 {"id":"omarchy-flo/spr1","name":"IMS-wt1","folder":"","machine":"omarchy-flo","active":false,"tabs":[{"id":"omarchy-flo/tbr1","name":"1","active":true,"terminals":[
   {"terminal":"omarchy-flo/t1","pane":"omarchy-flo/pr1","status":"blocked","app":"claude-code","name":null,"sub_agents":[]}]}]}]}
J
 ;;
 status) echo '{"focused_pane":"p3","layout":"main"}' ;;
 do) shift; echo "app do $*" >> "$FAKE_LOG" ;;
esac
SH
cat > "$T/bin/hc" <<'SH'
#!/bin/sh
case "$1" in
 ls) st=connected; [ -n "${FAKE_MACHINE_DOWN:-}" ] && st=disconnected
     printf '{"machines":[{"name":"omarchy-flo","state":"%s"}],"terminals":[{"address":"t1","cwd":"/work/a"},{"address":"t2","cwd":"/work/a"},{"address":"t3","cwd":"/work/b"},{"address":"omarchy-flo/t1","cwd":"/home/f/ims"}]}\n' "$st" ;;
 send) shift; echo "hc send $*" >> "$FAKE_LOG" ;;
 read) echo "the screen of $2" ;;
esac
SH
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
echo "classic herdr $*" >> "$FAKE_LOG"; echo '{"result":{"panes":[]}}'
SH
chmod +x "$T/bin/"*; export PATH="$T/bin:$PATH"
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
BD=""; [ -x daemon/target/release/bhoted ] && BD=$PWD/daemon/target/release/bhoted
[ -n "$BD" ] || BD=$(command -v bhoted 2>/dev/null)
[ -n "$BD" ] || { echo "skipped: no bhoted binary (cd daemon && cargo build --release)"; exit 0; }
US=$(printf '\037'); export MACHINE_LABELS=omarchy-flo=Omarchy

# pane list: ids, agents and their states, the folder of hc, the focus of herdr-app
out=$("$BD" herdr pane list)
[ "$(printf '%s' "$out" | jq '.result.panes | length')" = 3 ] && ok "pane list: the three terminals of this computer, none of the other machine's" || bad "pane list count: $out"
[ "$(printf '%s' "$out" | jq -r '.result.panes[0] | [.pane_id, .agent, .agent_status, .cwd, .workspace_id, .tab_id] | join(" ")')" = "t1 claude-code working /work/a sp1 tb1" ] && ok "pane list: id, agent, state, folder, workspace and tab of an agent" || bad "pane t1: $out"
[ "$(printf '%s' "$out" | jq -r '.result.panes[1] | [(.agent | tostring), .agent_status] | join(" ")')" = "null unknown" ] && ok "pane list: a shell is no agent" || bad "pane t2: $out"
[ "$(printf '%s' "$out" | jq -r '[.result.panes[] | select(.focused) | .pane_id] | join(",")')" = t3 ] && ok "pane list: the focused pane is herdr-app's" || bad "focus: $out"

# workspace list: labels, numbers, the most urgent state of the agents
out=$("$BD" herdr workspace list)
[ "$(printf '%s' "$out" | jq -r '[.result.workspaces[] | "\(.number):\(.label):\(.agent_status):\(.focused)"] | join(" ")')" = "1:proj:working:false 2:other:idle:true" ] && ok "workspace list: number, label, state and focus" || bad "workspaces: $out"

# another machine, asked by bhote's label
out=$("$BD" herdr --machine Omarchy pane list)
[ "$(printf '%s' "$out" | jq -r '.result.panes | map("\(.pane_id):\(.agent_status):\(.workspace_id)") | join(" ")')" = "t1:blocked:spr1" ] && ok "--machine Omarchy: that machine's panes, ids without the machine" || bad "remote panes: $out"
out=$("$BD" herdr --machine omarchy-flo pane get t1)
[ "$(printf '%s' "$out" | jq -r '.result.pane.agent_status')" = blocked ] && ok "--machine <name> works as well, pane get finds the pane" || bad "pane get: $out"

# the machines herdr-app shows, as `herdr machine list` prints them (the script asks it before it sends to another machine)
[ "$("$BD" herdr machine list)" = "$(printf 'omarchy-flo\tOmarchy\tomarchy-flo\tdefault\tenabled')" ] && ok "machine list: id, bhote's label, host, profile, enabled" || bad "machine list: $("$BD" herdr machine list)"

# what the script sends: a prompt, a command, a focus, a read
: > "$FAKE_LOG"
"$BD" herdr agent prompt t1 "start the topic" && grep -qx 'hc send t1 start the topic' "$FAKE_LOG" && ok "agent prompt: pasted into the terminal with hc send" || bad "agent prompt: $(cat "$FAKE_LOG")"
: > "$FAKE_LOG"; "$BD" herdr --machine Omarchy agent prompt t1 "hello" ; grep -qx 'hc send omarchy-flo/t1 hello' "$FAKE_LOG" && ok "agent prompt on another machine: omarchy-flo/t1" || bad "remote prompt: $(cat "$FAKE_LOG")"
: > "$FAKE_LOG"; "$BD" herdr pane run t2 "bhote"; grep -qx 'hc send t2 bhote' "$FAKE_LOG" && ok "pane run: the command is typed into the pane's shell" || bad "pane run: $(cat "$FAKE_LOG")"
: > "$FAKE_LOG"; "$BD" herdr agent focus t3; "$BD" herdr --machine Omarchy agent focus t1; "$BD" herdr workspace focus sp2
[ "$(cat "$FAKE_LOG")" = "app do agent.focus --terminal t3
app do agent.focus --terminal omarchy-flo/t1
app do space.select --id sp2" ] && ok "agent focus and workspace focus are herdr-app commands" || bad "focus: $(cat "$FAKE_LOG")"
[ "$("$BD" herdr pane read t1 --lines 5)" = "the screen of t1" ] && ok "pane read: hc read" || bad "pane read"
"$BD" herdr plugin list >/dev/null 2>&1; [ $? = 1 ] && ok "what herdr-app does not have is an error (exit 1)" || bad "plugin list must fail"

# the files of this machine
S=$T/s1; mkdir -m 700 "$S"
"$BD" collect "$S" Mac --config "$T/none" && ok "collect succeeds" || bad "collect failed"
[ "$(cat "$S/agents.local")" = "Mac${US}t1${US}working${US}proj${US}
Mac${US}t3${US}idle${US}other${US}" ] && ok "agents.local: this computer's agents (no shell), the name of their space" || bad "agents.local: $(cat -v "$S/agents.local")"
grep -qx "Mac${US}t1${US}tb1${US}a${US}${US}${US}/work/a" "$S/agents.meta.local" && grep -qx "Mac${US}t2${US}tb1${US}${US}${US}${US}/work/a" "$S/agents.meta.local" && ok "agents.meta.local: every pane, 'a' marks the agents, the folder" || bad "meta: $(cat -v "$S/agents.meta.local")"
[ "$(cat "$S/focus.pane")" = "t3 tb2" ] && ok "focus.pane: the focused pane and its tab" || bad "focus.pane: $(cat -v "$S/focus.pane")"
grep -qx "2${US}sp2${US}other${US}1${US}idle" "$S/workspaces.local" && ok "workspaces.local: number, id, label, focus, state" || bad "workspaces.local: $(cat -v "$S/workspaces.local")"

# the other machines
printf 'MACHINE_LABELS=omarchy-flo=Omarchy\n' > "$T/cfg"
"$BD" collect-remote "$S" Mac --config "$T/cfg" && ok "collect-remote succeeds" || bad "collect-remote failed"
[ "$(cat "$S/agents.remote")" = "Omarchy${US}t1${US}blocked${US}IMS-wt1${US}" ] && ok "agents.remote: the other machine under bhote's label" || bad "agents.remote: $(cat -v "$S/agents.remote")"
[ "$(cat "$S/agents.nodes")" = "Omarchy${US}omarchy-flo${US}0${US}" ] && [ ! -s "$S/machines.offline" ] && ok "agents.nodes: the machine is up, nobody is offline" || bad "nodes: $(cat -v "$S/agents.nodes")"
FAKE_MACHINE_DOWN=1 "$BD" collect-remote "$S" Mac --config "$T/cfg"
[ "$(cat "$S/agents.remote")" = "Omarchy${US}t1${US}off-blocked${US}IMS-wt1${US}" ] && grep -qx Omarchy "$S/machines.offline" && ok "a machine that is down keeps its agents, muted (off-<state>)" || bad "down: $(cat -v "$S/agents.remote")"
printf 'MACHINE_LABELS=omarchy-flo=Omarchy\nMACHINES_OFF=Omarchy\n' > "$T/cfg2"; "$BD" collect-remote "$S" Mac --config "$T/cfg2"
[ ! -s "$S/agents.remote" ] && ok "MACHINES_OFF keeps a machine out" || bad "MACHINES_OFF: $(cat -v "$S/agents.remote")"

# herdr-app does not answer: the old files stay, the exit code says so
before=$(cat "$S/agents.local"); FAKE_APP_DOWN=1 "$BD" collect "$S" Mac --config "$T/none"; rc=$?
[ "$rc" = 1 ] && [ "$(cat "$S/agents.local")" = "$before" ] && ok "herdr-app silent: collect fails and the last agents stay" || bad "app down: rc=$rc"

# the script: its herdr calls go to herdr-app, the classic backend to the old herdr
# shellcheck disable=SC1091
. ./bhote
HOST=Mac; BHOTE_INFO_DONE=1   # (bhote_info sets them when the script runs)
export BHOTED_BIN=$BD
[ "$(herdr pane list | jq '.result.panes | length')" = 3 ] && ok "script: herdr pane list is answered from herdr-app" || bad "script wrapper"
[ "$(herdr --machine Omarchy pane list | jq -r '.result.panes[0].pane_id')" = t1 ] && ok "script: the label of a machine reaches the daemon (MACHINE_LABELS)" || bad "script label"
cfg_set MACHINE_LABELS omarchy-flo=Omarchy; : > "$FAKE_LOG"
[ "$(herdr --machine Omarchy pane list | jq -r '.result.panes[0].pane_id')" = t1 ] && ok "script: the label comes from the settings" || bad "script label from settings"
BHOTE_HERDR_BACKEND=classic herdr pane list >/dev/null; grep -q '^classic herdr pane list' "$FAKE_LOG" && ok "BHOTE_HERDR_BACKEND=classic: the old herdr answers" || bad "classic backend"
: > "$FAKE_LOG"; AGENT_SEND_SYNC=1 agent_send "$HOST" t1 "a prompt from bhote"; grep -qx 'hc send t1 a prompt from bhote' "$FAKE_LOG" && ok "script: agent_send reaches the agent through hc send" || bad "agent_send: $(cat "$FAKE_LOG")"
: > "$FAKE_LOG"; machine_known Omarchy && AGENT_SEND_SYNC=1 agent_send Omarchy t1 "to the other machine"; grep -qx 'hc send omarchy-flo/t1 to the other machine' "$FAKE_LOG" && ok "script: a prompt to another machine's agent (machine_known, label, omarchy-flo/t1)" || bad "remote agent_send: $(cat "$FAKE_LOG")"
SHARED_DIR=$T/s2; mkdir -m 700 "$SHARED_DIR"; AGENT_LOCAL=$T/al; AGENT_REMOTE=$T/ar
collect_local; [ "$(wc -l < "$AGENT_LOCAL" | tr -d ' ')" = 2 ] && ok "script: collect_local fills the agent list from herdr-app" || bad "collect_local"
rm -rf "$T"; exit $fail
