#!/usr/bin/env bash
# Tests of the inbox: `bhote inbox` and the Claude Code hook integrations/claude/hooks/inbox-hook.sh. A fake kharka (entries with stamps, put --if-hlc) and a fake hc;
# nothing touches the real ones. Fake data only. Usage: bash tests/inbox.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID
REPO=$PWD
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/cfg/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared HOME=$T/home BHOTE_API_CACHE=off BHOTE_INBOX=$T/inbox
mkdir -p "$T/bin" "$T/cfg" "$T/home"; export PATH="$T/bin:$PATH" FAKE_KHARKA=$T/kharka.db KHARKA=$T/bin/kharka HC_TERMINAL_ID=t1 HOST=Mac; : > "$BHOTE_CONFIG"
trap 'rm -rf "${T:?}"' EXIT
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
printf '#!/bin/sh\nexec bash %s/bhote "$@"\n' "$REPO" > "$T/bin/bhote"; chmod +x "$T/bin/bhote"
printf '#!/bin/sh\nexit 1\n' > "$T/bin/herdr"; chmod +x "$T/bin/herdr"
cat > "$T/bin/hc" <<'SH'
#!/bin/sh
[ "$1" = get ] && echo '{"labels":{"where":"Frodo Room / 1"}}'
SH
chmod +x "$T/bin/hc"
cat > "$T/bin/kharka" <<'PY'
#!/usr/bin/env python3
import json, os, sys, fcntl
db = os.environ["FAKE_KHARKA"]; lock = open(db + ".lock", "w"); fcntl.flock(lock, fcntl.LOCK_EX)
ents = {}
if os.path.exists(db):
    for l in open(db):
        e = json.loads(l); ents[e["key"]] = e
def save(): open(db, "w").write("".join(json.dumps(e) + "\n" for e in ents.values()))
a = sys.argv[1:]
with open(os.environ["FAKE_KHARKA"] + ".calls", "a") as c: c.write(" ".join(a[:2]) + "\n")
if a[0] == "whoami": print("node1")
elif a[0] == "ls":
    pre = [x for x in a[1:] if not x.startswith("--")]; pre = pre[0] if pre else ""
    for e in sorted(ents.values(), key=lambda e: e["key"]):
        if e["key"].startswith(pre): print(json.dumps(e) if "--entries" in a else e["key"])
elif a[0] == "get":
    k = [x for x in a[1:] if not x.startswith("--")][0]
    if k not in ents: sys.exit(3)
    print(json.dumps(ents[k] if "--entry" in a else ents[k]["value"]))
elif a[0] == "put":
    k = a[1]; v = json.loads(a[2])
    if "--if-hlc" in a and ents.get(k, {}).get("hlc") != a[a.index("--if-hlc") + 1]: sys.exit(4)
    n = int(open(db + ".n").read()) + 1 if os.path.exists(db + ".n") else 1; open(db + ".n", "w").write(str(n))
    ents[k] = {"key": k, "hlc": "h%d" % n, "value": v}; save(); print("h%d" % n)
elif a[0] == "del": ents.pop(a[1], None); save()
else: sys.exit(1)
PY
chmod +x "$T/bin/kharka"
Q() { kharka put "$1" "{\"from\":\"Sam\",\"fromRef\":\"Sam\",\"to\":\"Frodo Room\",\"title\":\"$3\",\"text\":\"$2\",\"at\":1,\"state\":\"queued\"}" >/dev/null; }
state() { kharka get "$1" | jq -r .state; }
HOOK=$REPO/integrations/claude/hooks/inbox-hook.sh
PTU='{"hook_event_name":"PostToolUse","tool_name":"Bash","session_id":"s"}'
UPS='{"hook_event_name":"UserPromptSubmit","prompt":"hi"}'

# 1. nothing pending: silent, and cheap (no markers: no process but the shell's own)
out=$(printf '%s' "$PTU" | bash "$HOOK"); rc=$?
[ -z "$out" ] && [ "$rc" = 0 ] && ok "hook: nothing pending, silent, exit 0" || bad "hook not silent: $out ($rc)"
s=$(date +%s%N 2>/dev/null || python3 -c 'import time;print(int(time.time()*1e9))'); for _ in 1 2 3 4 5 6 7 8 9 10; do printf '%s' "$PTU" | bash "$HOOK"; done
e=$(date +%s%N 2>/dev/null || python3 -c 'import time;print(int(time.time()*1e9))'); ms=$(( (e - s) / 10000000 ))
[ "$ms" -lt 15 ] && ok "hook: nothing pending costs ${ms} ms per call (bash start included)" || bad "hook too slow without markers: ${ms} ms"
[ ! -s "$FAKE_KHARKA.calls" ] && ok "hook: no kharka call without a marker" || bad "kharka was asked: $(cat "$FAKE_KHARKA.calls")"

# 2. a message for another agent: the marker is there, but the hook shows nothing of it and claims nothing
Q msg/node1/gandalf/1-1 "for gandalf only" ""; mkdir -p "$BHOTE_INBOX"; : > "$BHOTE_INBOX/node1-gandalf"
out=$(printf '%s' "$PTU" | bash "$HOOK")
[ -z "$out" ] && [ "$(state msg/node1/gandalf/1-1)" = queued ] && ok "hook: another agent's message is not shown and not claimed" || bad "touched another agent's message: $out"

# 3. a message for this agent (node1/frodoroom): delivered once, valid JSON per the contract
Q msg/node1/frodoroom/2-1 "Which branch do you use?" "Question"; : > "$BHOTE_INBOX/node1-frodoroom"
out=$(printf '%s' "$PTU" | bash "$HOOK"); rc=$?
echo "$out" | jq -e '.hookSpecificOutput.hookEventName == "PostToolUse" and (.hookSpecificOutput.additionalContext | contains("Which branch do you use?") and contains("Message from Sam - Question") and contains("bhote send Sam")) and (.systemMessage | type == "string")' >/dev/null && [ "$rc" = 0 ] && ok "hook: PostToolUse prints valid JSON with additionalContext (sender, text, how to answer)" || bad "bad output: $out ($rc)"
echo "$out" | grep -q "for gandalf only" && bad "another agent's text reached this context" || ok "hook: nothing of another agent's message in the output"
[ "$(state msg/node1/frodoroom/2-1)" = delivered ] && kharka get msg/node1/frodoroom/2-1 | jq -e '.deliveredAt > 0' >/dev/null && ok "the message is claimed: state delivered with deliveredAt" || bad "not claimed: $(kharka get msg/node1/frodoroom/2-1)"
[ ! -e "$BHOTE_INBOX/node1-frodoroom" ] && [ -e "$BHOTE_INBOX/node1-gandalf" ] && ok "the marker of this agent is gone, the other one stays" || bad "markers: $(ls "$BHOTE_INBOX")"
out=$(printf '%s' "$PTU" | bash "$HOOK"); [ -z "$out" ] && ok "hook: the same message is not delivered twice" || bad "delivered twice: $out"

# 4. UserPromptSubmit has its own event name; another event gets nothing and claims nothing
Q msg/node1/frodoroom/3-1 "second" ""; : > "$BHOTE_INBOX/node1-frodoroom"
out=$(printf '%s' '{"hook_event_name":"Stop"}' | bash "$HOOK"); [ -z "$out" ] && [ "$(state msg/node1/frodoroom/3-1)" = queued ] && ok "hook: the Stop event delivers nothing and claims nothing" || bad "Stop delivered: $out"
out=$(printf '%s' "$UPS" | bash "$HOOK"); echo "$out" | jq -e '.hookSpecificOutput.hookEventName == "UserPromptSubmit" and (.hookSpecificOutput.additionalContext | contains("second"))' >/dev/null && ok "hook: UserPromptSubmit output carries its own event name" || bad "UserPromptSubmit: $out"

# 5. the ear: off = held; on again = delivered
Q msg/node1/frodoroom/4-1 "held back" ""; : > "$BHOTE_INBOX/node1-frodoroom"; kharka put listen/node1/frodoroom '{"listening":false}' >/dev/null
out=$(printf '%s' "$PTU" | bash "$HOOK"); [ -z "$out" ] && [ "$(state msg/node1/frodoroom/4-1)" = queued ] && ok "ear off: nothing delivered, the message waits" || bad "delivered with ear off: $out"
kharka put listen/node1/frodoroom '{"listening":true}' >/dev/null
out=$(printf '%s' "$PTU" | bash "$HOOK"); echo "$out" | grep -q "held back" && ok "ear on: it is delivered" || bad "not delivered with ear on: $out"
echo AGENT_LISTEN=off > "$BHOTE_CONFIG"; kharka del listen/node1/frodoroom
Q msg/node1/frodoroom/5-1 "default off" ""; : > "$BHOTE_INBOX/node1-frodoroom"
out=$(printf '%s' "$PTU" | bash "$HOOK"); [ -z "$out" ] && ok "no ear record: AGENT_LISTEN=off holds it" || bad "AGENT_LISTEN ignored: $out"
: > "$BHOTE_CONFIG"

# 6. two consumers: exactly one delivers (the hook and a second consumer at the same time; and the panel's claim)
Q msg/node1/frodoroom/6-1 "only once" ""
a=$T/a.out; b=$T/b.out
( bash "$REPO/bhote" inbox --as node1/frodoroom > "$a" ) & ( bash "$REPO/bhote" inbox --as node1/frodoroom > "$b" ) & wait
n=$(cat "$a" "$b" | grep -c "only once")
[ "$n" = 1 ] && ok "two consumers at once: delivered exactly once" || bad "delivered $n times"
Q msg/node1/frodoroom/7-1 "panel was first" ""
h=$(kharka get --entry msg/node1/frodoroom/7-1 | jq -r .hlc)
kharka put msg/node1/frodoroom/7-1 '{"state":"claimed","claimedAt":'"$(date +%s)"',"text":"panel was first"}' --if-hlc "$h" >/dev/null   # the panel's claim
out=$(bash "$REPO/bhote" inbox --as node1/frodoroom); [ -z "$out" ] && ok "a message the panel claimed is not shown by the hook" || bad "shown after the panel's claim: $out"
kharka put msg/node1/frodoroom/7-1 '{"state":"claimed","claimedAt":1,"from":"Sam","text":"panel died"}' >/dev/null
out=$(bash "$REPO/bhote" inbox --as node1/frodoroom); echo "$out" | grep -q "panel died" && ok "a claim older than 5 minutes (panel gone) is taken over" || bad "stale claim stays: $out"
kharka put msg/node1/frodoroom/7-1 '{"state":"claimed","claimedAt":'"$(date +%s)"',"text":"fresh"}' --if-hlc stale >/dev/null 2>&1; [ $? = 4 ] && ok "put --if-hlc with an old stamp is refused (exit 4)" || bad "stale put accepted"

# 7. bhote inbox: --json, --peek, long text by reference, the limit of five
Q msg/node1/frodoroom/8-1 "peek me" "T"
bash "$REPO/bhote" inbox --as node1/frodoroom --peek | grep -q "peek me" && [ "$(state msg/node1/frodoroom/8-1)" = queued ] && ok "inbox --peek shows and does not claim" || bad "peek claimed"
bash "$REPO/bhote" inbox --as node1/frodoroom --json | jq -e '.[0].text == "peek me" and .[0].from == "Sam"' >/dev/null && ok "inbox --json: an array of the messages" || bad "json wrong"
long=$(printf 'x%.0s' $(seq 1 1600)); Q msg/node1/frodoroom/9-1 "$long" ""
out=$(bash "$REPO/bhote" inbox --as node1/frodoroom); echo "$out" | grep -q "Read it with: .*kharka get msg/node1/frodoroom/9-1" && ! echo "$out" | grep -q "$long" && ok "a long text comes as the way to read it" || bad "long text: ${out:0:200}"
for i in 1 2 3 4 5 6 7; do Q "msg/node1/frodoroom/10-$i" "bulk $i" ""; done; : > "$BHOTE_INBOX/node1-frodoroom"
out=$(printf '%s' "$PTU" | bash "$HOOK"); echo "$out" | jq -e '.hookSpecificOutput.additionalContext | contains("2 more waiting")' >/dev/null && [ -e "$BHOTE_INBOX/node1-frodoroom" ] && ok "at most 5 per call; the marker stays while more wait" || bad "limit: $out"
printf '%s' "$PTU" | bash "$HOOK" | jq -e '.hookSpecificOutput.additionalContext | contains("bulk 6") and contains("bulk 7")' >/dev/null && [ ! -e "$BHOTE_INBOX/node1-frodoroom" ] && ok "the rest comes with the next call, then the marker goes" || bad "rest not delivered"

# 8. markers: bhote send to a local agent writes one; --mark replaces the set
: > "$FAKE_KHARKA"; rm -rf "$BHOTE_INBOX"
bash "$REPO/bhote" inbox --mark node1-aaa node1-bbb; [ -e "$BHOTE_INBOX/node1-aaa" ] && [ -e "$BHOTE_INBOX/node1-bbb" ] && ok "inbox --mark writes the markers" || bad "mark: $(ls "$BHOTE_INBOX")"
bash "$REPO/bhote" inbox --mark node1-bbb; [ ! -e "$BHOTE_INBOX/node1-aaa" ] && [ -e "$BHOTE_INBOX/node1-bbb" ] && ok "inbox --mark removes the markers that no longer wait" || bad "mark removal"
bash "$REPO/bhote" inbox --mark; [ -z "$(ls "$BHOTE_INBOX")" ] && ok "inbox --mark without names clears all" || bad "not cleared"
exit $fail
