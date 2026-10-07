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
rm -rf "$T"; exit $fail
