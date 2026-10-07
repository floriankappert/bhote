#!/usr/bin/env bash
# Tests of the command line (bhote add/list/show/done/...). Throw-away config and data; no network. Usage: bash tests/cli.sh
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared
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
rm -rf "$T"; exit $fail
