#!/usr/bin/env bash
# Tests of the command line (bhote add/list/show/done/...). Throw-away config and data; no network. Usage: bash tests/cli.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared
# a fake herdr first on the PATH: nothing reaches the herdr you work in (notifications, prompts); every call is logged
mkdir -p "$T/bin"; export FAKE_LOG=$T/herdr.log PATH="$T/bin:$PATH"
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
echo "$*" >> "$FAKE_LOG"
case "$*" in
 *"pane list"*) echo '{"result":{"panes":[{"agent":"claude","agent_status":"idle","pane_id":"w1:p1","workspace_id":"w1","tokens":{"repo":"github.com/acme/api"}},{"agent":"claude","agent_status":"idle","pane_id":"w2:p1","workspace_id":"w2"}]}}' ;;
 *"workspace list"*) echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"shopws"},{"workspace_id":"w2","label":"notes"}]}}' ;;
 *) exit 1 ;;
esac
SH
chmod +x "$T/bin/herdr"
B() { bash ./bhote "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }

out=$(B add "Kafka role for staging" @DevOps); echo "$out" | grep -q "status: waiting" && echo "$out" | grep -q "waiting for: DevOps" && ok "add @Name: topic waits for DevOps" || bad "add @Name: $out"
B add Release notes 2027 >/dev/null; B add --waiting Alex "Price list approval" >/dev/null; B add "Toast redesign" --status later >/dev/null
[ "$(B list | wc -l | tr -d ' ')" = 4 ] && ok "list: four topics" || bad "list count: $(B list)"
B list | head -1 | grep -q "^  1  now" && ok "list: numbered, ordered now/waiting/later" || bad "list order: $(B list)"

# references: number, title part (case-insensitive), id start
B done 1 >/dev/null; B list --all | grep -q "done .*Release notes" && ok "done by number" || bad "done 1: $(B list --all)"
B done "price list" >/dev/null; B list --all | grep -q "done .*Price list" && ok "done by part of the title" || bad "done by title"
id=$(B list --json --all | python3 -c "import sys,json; print([t for t in json.load(sys.stdin) if 'Kafka' in t['title']][0]['id'])")
B show "$id" | grep -q "Kafka role" && ok "show by the full id" || bad "show by id"
B show "${id:0:8}" >/dev/null 2>&1; [ "$?" = 2 ] && ok "an id start shared by several topics is reported as ambiguous" || bad "shared id prefix"

# ambiguity and misses
B add "Kafka monitoring" >/dev/null
B done kafka >/dev/null 2>"$T/err"; rc=$?
[ "$rc" = 2 ] && grep -q "matches 2 topics" "$T/err" && ok "ambiguous reference: exit code 2 and the candidates" || bad "ambiguous: rc=$rc $(cat "$T/err")"
B done "no such thing" >/dev/null 2>&1; [ "$?" = 1 ] && ok "unknown reference: exit code 1" || bad "unknown reference exit code"
B add >/dev/null 2>&1; [ "$?" = 1 ] && ok "add without title: exit code 1" || bad "add without title"

# JSON: valid, special characters survive
B add 'Quote "this" and \ that' >/dev/null
B list --json --all | python3 -c "import sys,json; d=json.load(sys.stdin); assert any(t['title']=='Quote \"this\" and \\\\ that' for t in d), d" && ok "json: valid, quotes and backslashes survive" || bad "json special characters"

# status changes
B wait "monitoring" "Sam" >/dev/null; B list | grep -q "Kafka monitoring  (Sam" && ok "wait <ref> <Name>" || bad "wait: $(B list)"
B now monitoring >/dev/null; B list | grep -q "now      Kafka monitoring$" && ok "now: back to now, no longer waiting" || bad "now: $(B list)"
B rename monitoring "Kafka monitoring v2" >/dev/null; B list | grep -q "v2" && ok "rename" || bad "rename"
B rm "v2" >/dev/null; B list --all | grep -q "v2" && bad "rm: still listed" || ok "rm: gone from the list"
B list --status later | grep -q "Toast redesign" && ok "list --status later" || bad "list --status"
B bogus >/dev/null 2>&1; [ "$?" = 1 ] && ok "unknown command: exit code 1" || bad "unknown command"

# the panel's data = the CLI's data: a topic made here is a file the panel reads
ls "$BHOTE_DATA/topics"/*.topic >/dev/null 2>&1 && ok "topics are plain files the panel reads" || bad "no files"
# exit codes and refs (hardening round 2)
B add "Exit code check" >/dev/null; [ $? = 0 ] && ok "exit code: add returns 0" || bad "add returned non-zero"
B list >/dev/null; [ $? = 0 ] && ok "exit code: list returns 0" || bad "list returned non-zero"
B show "Exit code" >/dev/null; [ $? = 0 ] && ok "exit code: show returns 0" || bad "show returned non-zero"
B done "Exit code" >/dev/null; [ $? = 0 ] && ok "exit code: done returns 0" || bad "done returned non-zero"
B show 999 >/dev/null 2>&1; [ $? = 1 ] && ok "a number out of range is an error, not a title search" || bad "out-of-range number not an error"
B show 01 >/dev/null 2>&1; [ $? = 0 ] && ok "a number with a leading zero is decimal (01 = 1)" || bad "leading zero"
B show 08 >/dev/null 2>&1; r=$?; [ "$r" = 0 ] || [ "$r" = 1 ] && ok "08 is not read as octal (no shell error)" || bad "08 crashed: $r"
f=$(ls "$T/data/topics"/*.topic | head -1); printf 'waiting_since=1x\n' >> "$f"
B list --json | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null && ok "list --json stays valid JSON with a broken number" || bad "invalid JSON from list"
B show 1 --json | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null && ok "show --json stays valid JSON" || bad "invalid JSON from show"
long=$(printf 'x%.0s' $(seq 1 200))
B add "Long description" -d "$long" >/dev/null 2>&1; d=$(B show "Long description" --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["description"]))')
[ "$d" = 140 ] && ok "descriptions are cut to 140 characters (add -d)" || bad "description length $d"
B desc "Long description" "$long" >/dev/null 2>&1; d=$(B show "Long description" --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["description"]))')
[ "$d" = 140 ] && ok "descriptions are cut to 140 characters (desc)" || bad "desc length $d"
# the JSON output matches docs/schema (checked with a small validator, no extra module needed)
cat > "$T/validate.py" <<'PY'
import json, sys, os
base = sys.argv[1]
def load(n): return json.load(open(os.path.join(base, n)))
T = {"string": str, "integer": int, "object": dict, "array": list, "null": type(None)}
def ok(v, s):
    if "$ref" in s: return ok(v, load(s["$ref"]))
    if "oneOf" in s: return sum(ok(v, x) for x in s["oneOf"]) == 1
    if "enum" in s and v not in s["enum"]: return False
    if "type" in s:
        ts = s["type"] if isinstance(s["type"], list) else [s["type"]]
        if not any(isinstance(v, T[t]) and not (t == "integer" and isinstance(v, bool)) for t in ts): return False
    if isinstance(v, dict):
        if any(k not in v for k in s.get("required", [])): return False
        if s.get("additionalProperties") is False and any(k not in s.get("properties", {}) for k in v): return False
        if any(not ok(v[k], ps) for k, ps in s.get("properties", {}).items() if k in v): return False
    if isinstance(v, list) and "items" in s: return all(ok(x, s["items"]) for x in v)
    if isinstance(v, str) and "maxLength" in s and len(v) > s["maxLength"]: return False
    return True
sys.exit(0 if ok(json.load(sys.stdin), load(sys.argv[2])) else 1)
PY
B list --all --json | python3 "$T/validate.py" docs/schema topic-list.schema.json && ok "schema: list --json matches topic-list.schema.json" || bad "list --json does not match the schema"
B show 1 --json | python3 "$T/validate.py" docs/schema topic.schema.json && ok "schema: show --json matches topic.schema.json" || bad "show --json does not match the schema"
B wait 1 Robin --json | python3 "$T/validate.py" docs/schema topic.schema.json && ok "schema: a changing command with --json matches too" || bad "wait --json does not match"
B version --json | python3 -c 'import json,sys; assert json.load(sys.stdin)["version"]' && ok "version --json" || bad "version --json"
# settings over the API
B config set DONE_MAX 3 >/dev/null && [ "$(B config get DONE_MAX)" = 3 ] && ok "config: set and get" || bad "config set/get"
B config set DONE_MAX lots >/dev/null 2>&1; [ $? = 1 ] && [ "$(B config get DONE_MAX)" = 3 ] && ok "config: a value of the wrong type is refused" || bad "config accepted a bad value"
B config set NOPE 1 >/dev/null 2>&1; [ $? = 1 ] && ok "config: an unknown key is refused" || bad "unknown key accepted"
B config set AGENT_CAN_CLOSE maybe >/dev/null 2>&1; [ $? = 1 ] && ok "config: bool settings take on/off only" || bad "bool accepted maybe"
B config unset DONE_MAX >/dev/null; [ "$(B config get DONE_MAX)" = 7 ] && ok "config: unset goes back to the default" || bad "unset"
B config list --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert any(x["key"]=="AGENT_CAN_CLOSE" and x["default"]=="on" for x in d)' && ok "config list --json" || bad "config list --json"
# review, and what an agent may do
B add "Agent task" >/dev/null; B review "Agent task" >/dev/null; [ "$(B show "Agent task" --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])')" = review ] && ok "review: status review" || bad "review"
B list | head -1 | grep -q review && ok "review topics come first in the list" || bad "review not first"
B config set AGENT_CAN_CLOSE off >/dev/null
CLAUDECODE=1 CLAUDE_CODE_SESSION_ID=abc-123 B done "Agent task" >/dev/null 2>&1
[ "$(B show "Agent task" --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])')" = review ] && ok "AGENT_CAN_CLOSE=off: an agent's done becomes review" || bad "agent closed although not allowed"
B done "Agent task" >/dev/null; [ "$(B show "Agent task" --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])')" = done ] && ok "AGENT_CAN_CLOSE=off: you can still close it" || bad "user could not close"
B config unset AGENT_CAN_CLOSE >/dev/null
B config set AGENT_TOPICS off >/dev/null; n0=$(B list --all --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')
out=$(CLAUDECODE=1 B add "Agent made" --json 2>/dev/null); rc=$?
[ "$rc" = 0 ] && [ "$out" = null ] && [ "$(B list --all --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')" = "$n0" ] && ok "AGENT_TOPICS=off: an agent's add creates nothing (null, exit 0)" || bad "agent created a topic although not allowed: rc=$rc $out"
B add "Mine anyway" >/dev/null && B show "Mine anyway" >/dev/null && ok "AGENT_TOPICS=off: you can still add topics" || bad "user could not add"
B config unset AGENT_TOPICS >/dev/null
n_plain=$(B dump | grep -c '^@@'); n_packed=$(B dump --packed | BHOTE_SOURCE_ONLY=1 bash -c '. ./bhote; dump_unpack' | grep -c '^@@')
[ "$n_plain" = "$n_packed" ] && [ "$n_plain" -gt 1 ] && ok "dump --packed (for herdr) unpacks to the same topics" || bad "packed dump differs: $n_plain vs $n_packed"
# the jump key: bhote writes its own block into herdr's config.toml; herdr checks it, a refused key leaves the file as it was
mkdir -p "$T/hb"; printf '%s\n' '#!/bin/sh' 'case "$*" in *reload-config*) if grep -q "key = \"bad" "$XDG_CONFIG_HOME/herdr/config.toml"; then echo "{\"result\":{\"diagnostics\":[{\"message\":\"no\"}],\"status\":\"rejected\"}}"; else echo "{\"result\":{\"diagnostics\":[],\"status\":\"applied\"}}"; fi ;; esac' > "$T/hb/herdr"; chmod +x "$T/hb/herdr"
HB() { PATH="$T/hb:$PATH" XDG_CONFIG_HOME=$T/xdg bash ./bhote "$@"; }
mkdir -p "$T/xdg/herdr"; printf '[keys]\nprefix = "ctrl+space"\n' > "$T/xdg/herdr/config.toml"
HB config set JUMP_KEY prefix+t >/dev/null && grep -q '^key = "prefix+t"' "$T/xdg/herdr/config.toml" && ok "jump key: written into herdr's config.toml" || bad "jump key not written"
cp "$T/xdg/herdr/config.toml" "$T/before"; HB config set JUMP_KEY bad+x >/dev/null 2>&1; rc=$?
[ "$rc" = 1 ] && cmp -s "$T/before" "$T/xdg/herdr/config.toml" && ok "jump key: a key herdr refuses changes nothing" || bad "refused key: rc=$rc"
HB config set JUMP_KEY off >/dev/null && ! grep -q 'bhote' "$T/xdg/herdr/config.toml" && grep -q 'prefix = "ctrl+space"' "$T/xdg/herdr/config.toml" && ok "jump key: off removes only bhote's block" || bad "off: $(cat "$T/xdg/herdr/config.toml")"
HB config set FIND_KEY ctrl+alt+t >/dev/null && grep -q '^command = "bhote.panel.search"' "$T/xdg/herdr/config.toml" && ok "find key: a popup running bhote find" || bad "find key: $(cat "$T/xdg/herdr/config.toml")"
HB config set JUMP_KEY ctrl+alt+t >/dev/null 2>&1 && bad "jump key = find key accepted" || ok "jump key and find key must differ"
HB config set FIND_KEY off >/dev/null && ! grep -q 'bhote' "$T/xdg/herdr/config.toml" && ok "find key: off removes the block" || bad "find off"
B find </dev/null >/dev/null 2>&1 && bad "find without a terminal" || ok "find: needs a terminal"
# next: a status of its own; the panel lists now, next, review, waiting (parked only counted, the last done ones above the agents)
B add "Up soon" -s next >/dev/null && B list -s next | grep -q "Up soon" && ok "next: add -s next" || bad "add -s next"
B next "Toast redesign" >/dev/null && [ "$(B show "Toast redesign" --json | jq -r .status)" = next ] && ok "next: bhote next <ref>" || bad "bhote next"
B add "Parked one" -s later >/dev/null; B add "Closed one" -s done >/dev/null
frame=$(BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=44 BHOTE_ROWS=80 bash ./bhote </dev/null)
case "$frame" in *NEXT*"Up soon"*) ok "panel: a NEXT group" ;; *) bad "panel: no NEXT group" ;; esac
case "$frame" in *"Parked one"*|*LATER*) bad "panel shows parked topics" ;; *) ok "panel: parked topics stay out" ;; esac
echo "$frame" | grep -q "1 parked" && ok "panel: says how many are parked" || bad "panel: no parked count"
case "$frame" in *"Up soon"*Done*"Closed one"*"═══"*) ok "panel: done topics in their own area above the agents" ;; *) bad "panel: no done area" ;; esac
for i in 1 2 3; do B add "Old done $i" -s done >/dev/null; done; sleep 1; B add "Old done 4" -s done >/dev/null; B config set DONE_MAX 3 >/dev/null
frame=$(BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=44 BHOTE_ROWS=80 bash ./bhote </dev/null)
sect=$(echo "$frame" | sed -n '/┈┈ Done/,/═══/p')
[ "$(echo "$sect" | grep -c '✓')" = 3 ] && echo "$sect" | grep -q '^  +[0-9]* more done' \
  && echo "$sect" | grep -q "✓ Old done 4" && ok "panel: only the last DONE_MAX done ones (a check each, newest first), +n more done" || bad "panel: DONE_MAX not kept: $sect"
# an agent takes over a topic it did not create: take, or now run by an agent
B add "Foreign topic" >/dev/null; out=$(CLAUDECODE=1 HERDR_PANE_ID=w9:p1 CLAUDE_CODE_SESSION_ID=abc-1 B take "Foreign topic" --json)
[ "$(echo "$out" | jq -r '.status + " " + .agent.pane + " " + .agent.session')" = "now w9:p1 abc-1" ] && ok "take: the agent becomes the topic's agent" || bad "take: $out"
B add "Other topic" -s next >/dev/null; out=$(CLAUDECODE=1 HERDR_PANE_ID=w8:p2 B now "Other topic" --json)
[ "$(echo "$out" | jq -r '.status + " " + .agent.pane')" = "now w8:p2" ] && ok "now by an agent: it takes the topic" || bad "now by agent: $out"
B take "Other topic" >/dev/null 2>&1 && bad "take outside an agent accepted" || ok "take: only for an agent in herdr"
# projects: records in the topic store (they travel with it), not topics; a worktree inherits its main repository's project
mkdir -p "$T/shop"; git -C "$T/shop" init -q -b main; git -C "$T/shop" remote add origin git@github.com:Acme/Shop.git
git -C "$T/shop" commit -q --allow-empty -m init; git -C "$T/shop" worktree add -q -b wt1 "$T/shop-wt1" 2>/dev/null
B project add "Shop" >/dev/null && B project add "Acme Marketing" >/dev/null && ok "project add" || bad "project add"
B project add "shop" >/dev/null 2>&1 && bad "the same project twice" || ok "project add: a name once"
[ "$(B project list --json | jq -r 'map(.name) | join(",")')" = "Acme Marketing,Shop" ] && ok "project list: by name" || bad "project list: $(B project list --json)"
B list --all | grep -q "Shop" && bad "a project shows up as a topic" || ok "projects are no topics"
B project pin Shop "$T/shop" >/dev/null && [ "$(B project list --json | jq -r '.[] | select(.name == "Shop") | .repos[0]')" = github.com/acme/shop ] && ok "project pin <folder>: the origin as the key" || bad "pin folder: $(B project list --json)"
[ "$(B project of "$T/shop-wt1")" = Shop ] && ok "project of a worktree: its main repository's project" || bad "worktree project: $(B project of "$T/shop-wt1" 2>&1)"
B project pin "Acme Marketing" ws:marketing >/dev/null && B project list | grep -q "ws:marketing" && ok "project pin ws:<workspace>" || bad "pin ws"
B project rename "Acme M" "Marketing" >/dev/null && B project list | grep -q "Marketing" && ok "project rename" || bad "rename"
# pin an agent (its number from bhote agents, or its name): its repository (a herdr token), else its workspace
B project add "Pinned" >/dev/null; B project pin Pinned 1 >/dev/null; B project pin Pinned notes >/dev/null
pinned=$(B project list --json | jq -c '.[] | select(.name == "Pinned") | [.repos, .workspaces]')
[ "$pinned" = '[["github.com/acme/api"],["notes"]]' ] && ok "project pin <agent>: its repository, else its workspace" || bad "pin agent: $pinned"
n0=$(B list --all --json | jq length); B add "Lost one" -p "No such project" >/dev/null 2>&1; rc=$?
[ "$rc" = 1 ] && [ "$(B list --all --json | jq length)" = "$n0" ] && ok "add -p with an unknown project: exit 1, no topic" || bad "add -p unknown: rc=$rc, $(B list --all --json | jq length) topics (was $n0)"
B project rm Marketing >/dev/null && ! B project list | grep -q "Marketing" && ok "project rm" || bad "project rm"
# an agent does not revive a topic the user parked or closed; the user (no agent) can, and `take` works on request
B add "Parked one" -s later >/dev/null; pid=$(B list --all --json | jq -r '[.[] | select(.title=="Parked one")][0].id')
CLAUDECODE=1 B now "$pid" >/dev/null 2>&1 && bad "an agent revived a parked topic" || ok "agent: bhote now refuses a parked topic"
[ "$(B show "$pid" --json | jq -r .status)" = later ] && ok "agent: the topic stays parked" || bad "parked topic moved"
CLAUDECODE=1 B review "$pid" >/dev/null 2>&1 && bad "an agent moved a parked topic to review" || ok "agent: bhote review refuses a parked topic"
B now "$pid" >/dev/null 2>&1; [ "$(B show "$pid" --json | jq -r .status)" = now ] && ok "user: bhote now works on a parked topic" || bad "user could not move it"
rm -rf "$T"; exit $fail
