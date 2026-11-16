#!/usr/bin/env bash
# Tests of tasks: the rename (bhote task, bhote topic, /task), dual-read of *.topic and *.task, unknown fields that survive every rewrite, the source of a task
# (derived at read time, chips, filter, CLI flags), the Linear adapter (triage only, against a fake curl; no key leaves the process, no real API is called),
# from-stash, and bhote migrate tasks (a fake kharka). Throw-away config and data. Usage: bash tests/tasks.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID LINEAR_API_KEY
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/cfg/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared HOME=$T/home PYTHONDONTWRITEBYTECODE=1 BHOTE_API_CACHE=off
mkdir -p "$T/bin" "$T/cfg" "$T/home" "$T/fake" "$T/data/topics"; export FAKE_DIR=$T/fake FAKE_LOG=$T/fake.log PATH="$T/bin:$PATH"; echo NOTIFY=off > "$BHOTE_CONFIG"
trap 'rm -rf "${T:?}"' EXIT
B() { bash ./bhote "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
D=$T/data/topics
clear_d() { rm -f "${D:?}"/* "${D:?}"/.pull.* 2>/dev/null; }
LKEY=lin_api_FAKEFAKEFAKE0123456789
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
exit 1
SH
chmod +x "$T/bin/herdr"
# ── a fake curl: answers from $FAKE_DIR; logs every call (its arguments, whether the key it got on stdin is the expected one); never a network
cat > "$T/bin/curl" <<'PY'
#!/usr/bin/env python3
import json, os, re, sys
a = sys.argv[1:]; url = ""; method = "GET"; body = None; i = 0
while i < len(a):
    if a[i] == "-X": method = a[i + 1]; i += 2
    elif a[i] == "--data-binary": p = a[i + 1]; body = open(p[1:]).read() if p.startswith("@") else p; i += 2
    elif a[i] in ("-K", "--max-time", "-w"): i += 2
    elif a[i].startswith("http"): url = a[i]; i += 1
    else: i += 1
cfgin = sys.stdin.read() if "-K" in a else ""
auth = re.search(r'header = "Authorization: ([^"]*)"', cfgin); auth = auth.group(1) if auth else ""
d = os.environ["FAKE_DIR"]
ok = "none" if not auth else ("ok" if auth == os.environ.get("EXPECT_LINEAR", "-") else "bad")
open(os.environ["FAKE_LOG"], "a").write(json.dumps({"argv": a, "method": method, "url": url, "auth": ok, "body": body}) + "\n")
def out(code, text): sys.stdout.write(text + "\n" + str(code)); sys.exit(0)
def f(n):
    p = os.path.join(d, n)
    return open(p).read() if os.path.exists(p) else None
if os.path.exists(os.path.join(d, "down")): sys.exit(7)
if ok == "bad" or ok == "none": out(401, "{}")
if "api.linear.app" in url:
    q = json.loads(body); qs = q["query"]
    if "issueUpdate" in qs: out(200, f("linear.update.json") or '{"data":{"issueUpdate":{"success":true}}}')
    if "states" in qs: out(200, f("linear.states.json"))
    if "issues(first" in qs:
        after = (q.get("variables") or {}).get("after")
        if "isMe: {eq" in qs: out(200, f("linear.mine.json") or '{"data":{"issues":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}')
        t = f("linear.triage.%s.json" % after) if after else f("linear.triage.json")
        out(200, t)
    if "issue(id" in qs:
        t = f("linear.issue.%s.json" % q["variables"]["id"]); out(200, t or '{"data":{"issue":null}}')
    out(400, "{}")
out(404, "{}")
PY
chmod +x "$T/bin/curl"
mk() { printf '%s\n' "id=$1" "title=$2" "status=${3:-next}" "created=100" "updated=${4:-100}" "deleted=0" "${@:5}" > "$D/$1.topic"; }

# ── 1. the names: bhote task <command>, bhote topic <command> (the old name stays), the old flat commands
out=$(B task add "Plan the week" --json); id=$(echo "$out" | jq -r .id); [ -n "$id" ] && ok "bhote task add makes a task" || bad "task add: $out"
B topic list | grep -q "Plan the week" && ok "bhote topic list is the old name of the same" || bad "topic list"
B task list | grep -q "Plan the week" && B list | grep -q "Plan the week" && ok "bhote task list = bhote list" || bad "task list"
B topic add "Via the old name" >/dev/null 2>"$T/err"; [ ! -s "$T/err" ] && ok "bhote topic: no hint on stderr when it is not a terminal (scripts stay clean)" || bad "hint in a pipe: $(cat "$T/err")"
B task show "$id" | grep -q "Plan the week" && B task done "$id" >/dev/null && [ "$(B show "$id" --json | jq -r .status)" = done ] && ok "bhote task show/done" || bad "task show/done"
B help | grep -q "bhote task <command>" && B help | grep -qi "topic is the old name" && ok "help names tasks and keeps the old name" || bad "help"
B help | grep -v "migrate tasks" | grep -qi "topics" && bad "help still says topics: $(B help | grep -v 'migrate tasks' | grep -i topics | head -2)" || ok "help says tasks everywhere"

# ── 2. dual-read: <id>.task is read like <id>.topic; a writer keeps the file it finds; both for one id are one task
clear_d; mk old-1 "In a topic file" next 200; printf '%s\n' "id=new-1" "title=In a task file" "status=now" "created=300" "updated=300" "deleted=0" > "$D/new-1.task"
n=$(B list --all --json | jq length); [ "$n" = 2 ] && ok "dual-read: .topic and .task files are both listed" || bad "dual-read count $n"
B show new-1 --json | jq -e '.title == "In a task file"' >/dev/null && ok "a .task file can be shown by its id" || bad "show .task"
B later new-1 >/dev/null; [ ! -e "$D/new-1.topic" ] && [ "$(grep '^status=' "$D/new-1.task")" = "status=later" ] && ok "a writer changes the .task file in place (no .topic appears)" || bad "writer split the file"
printf '%s\n' "id=old-1" "title=Same id as a task file" "status=done" "created=100" "updated=900" "deleted=0" > "$D/old-1.task"
n=$(B list --all --json | jq length); [ "$n" = 2 ] && ok "the same id as .topic and .task counts once" || bad "duplicate counted: $n"
rm "$D/old-1.task"
B add "Fresh" >/dev/null; ls "$D" | grep -q '^[0-9].*\.topic$' && ok "new tasks are still written as .topic (the writers do not flip in wave 1)" || bad "a writer wrote a new name"
if [ -x daemon/target/release/bhoted ]; then
    "$PWD/daemon/target/release/bhoted" index "$D" "$T/idx" >/dev/null 2>&1
    [ "$(head -1 "$T/idx" | awk '{print $1}')" = "#3" ] && ok "bhoted index counts .topic and .task files" || bad "bhoted index: $(head -1 "$T/idx")"
fi

# ── 3. unknown KEY=value fields survive every rewrite (the rollout risk: an older version must not drop what a newer one wrote)
clear_d; mk keep-1 "Carries more" next 100 "future_field=stays" "v2_flag=1" "source_ref=XYZ-9"
B now keep-1 >/dev/null; B review keep-1 >/dev/null 2>&1; B wait keep-1 Anna >/dev/null; B rename keep-1 "Renamed" >/dev/null; B desc keep-1 "A note" >/dev/null; B later keep-1 >/dev/null; B done keep-1 >/dev/null
grep -q '^future_field=stays$' "$D/keep-1.topic" && grep -q '^v2_flag=1$' "$D/keep-1.topic" && grep -q '^source_ref=XYZ-9$' "$D/keep-1.topic" && ok "unknown fields survive now, review, wait, rename, desc, later, done" || bad "a field was dropped: $(cat "$D/keep-1.topic")"
mk keep-2 "Hand written" next 100 "future_field=1"; cp "$D/keep-2.topic" "$T/before"; B list >/dev/null; B show keep-2 >/dev/null; B list --json >/dev/null; cmp -s "$T/before" "$D/keep-2.topic" && ok "reading (list, show, json, derived source) never rewrites a file" || bad "a read changed the file"
# kharka: a fake kharka (entries as JSON lines in a file); the push and the dump keep unknown keys, also with digits, and read bhote.tasks/ too
cat > "$T/bin/kharka" <<'PY'
#!/usr/bin/env python3
import json, os, sys
db = os.environ["FAKE_KHARKA"]; ents = {}
if os.path.exists(db):
    for l in open(db):
        e = json.loads(l); ents[e["key"]] = e
def save():
    open(db, "w").write("".join(json.dumps(e) + "\n" for e in ents.values()))
a = sys.argv[1:]
if a[0] == "ls":
    for e in ents.values(): print(json.dumps(e))
elif a[0] == "put":
    key = a[1]; val = json.loads(sys.stdin.read())
    if "--if-hlc" in a and ents.get(key, {}).get("hlc") != a[a.index("--if-hlc") + 1]: sys.exit(4)
    n = int(open(db + ".n").read() or 0) + 1 if os.path.exists(db + ".n") else 1; open(db + ".n", "w").write(str(n))
    ents[key] = {"key": key, "hlc": "h%d" % n, "value": val}; save()
elif a[0] == "del":
    key = a[1]
    if "--if-hlc" in a and ents.get(key, {}).get("hlc") != a[a.index("--if-hlc") + 1]: sys.exit(4)
    ents.pop(key, None); save()
elif a[0] == "status": print(json.dumps({"hub": "x", "online": True, "pending": 0}))
else: sys.exit(1)
PY
chmod +x "$T/bin/kharka"; export FAKE_KHARKA=$T/kharka.db KHARKA=$T/bin/kharka
clear_d; rm -f "$FAKE_KHARKA"; echo STORE=kharka >> "$BHOTE_CONFIG"
mk kk-1 "To kharka" next 500 "future_field=stays" "v2_flag=1"
B sync >/dev/null 2>&1; jq -e 'select(.key == "bhote.topics/kk-1") | .value | (.future_field == "stays" and .v2_flag == "1")' "$FAKE_KHARKA" >/dev/null && ok "kharka push keeps unknown fields (also with digits in the key)" || bad "kharka push dropped a field: $(cat "$FAKE_KHARKA")"
# another machine (a newer version after wave 2) wrote bhote.tasks/<id>: read like bhote.topics/<id>; a newer one of the two wins
printf '%s\n' '{"key":"bhote.tasks/kk-2","hlc":"hx1","value":{"id":"kk-2","title":"From tasks collection","status":"now","created":"600","updated":"600","deleted":"0","future_field":"yes"}}' >> "$FAKE_KHARKA"
B sync >/dev/null 2>&1; [ -f "$D/kk-2.topic" ] && grep -q '^future_field=yes$' "$D/kk-2.topic" && ok "kharka: bhote.tasks/<id> is read (dual-read), unknown fields included" || bad "bhote.tasks not read: $(ls "$D")"
printf '%s\n' '{"key":"bhote.topics/kk-2","hlc":"hx2","value":{"id":"kk-2","title":"Older in topics","status":"now","created":"600","updated":"500","deleted":"0"}}' >> "$FAKE_KHARKA"
B sync >/dev/null 2>&1; [ "$(grep '^title=' "$D/kk-2.topic")" = "title=From tasks collection" ] && ok "kharka: both collections for one id: the newer updated wins" || bad "wrong winner: $(cat "$D/kk-2.topic")"
B rename kk-2 "Edited here" >/dev/null 2>&1; B sync >/dev/null 2>&1
jq -e 'select(.key == "bhote.tasks/kk-2") | .value.title == "Edited here"' "$FAKE_KHARKA" >/dev/null && ok "kharka: a change goes back to the collection the entry lives in" || bad "push went to the wrong key: $(grep kk-2 "$FAKE_KHARKA" | cut -c1-200)"
sed -i.bak '/^STORE=kharka$/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"; unset KHARKA

# ── 4. the source of a task: derived at read time, written only by an edit or a flag
clear_d
mk s-man "A plain one" next 100
mk s-wait "Agent waiting topic" waiting 100 "agent=claude-a" "waiting_for=Anna"
mk s-auto "Question from x" review 100 "auto=1" "agent=x"
mk s-card "A card for an agent" now 100 "auto_now=1" "agent=y"
mk s-tak "Taken by an agent, no wait" now 100 "agent=z"
cp "$D/s-man.topic" "$T/man.before"
j=$(B list --all --json)
for p in 's-man manual' 's-wait agent' 's-auto agent' 's-card agent' 's-tak manual'; do set -- $p; [ "$(echo "$j" | jq -r --arg i "$1" '.[] | select(.id == $i) | .source')" = "$2" ] && ok "derived source of $1: $2" || bad "source of $1: $(echo "$j" | jq -c --arg i "$1" '.[] | select(.id == $i)')"; done
[ "$(echo "$j" | jq -r '.[] | select(.id == "s-wait") | .source_ref')" = claude-a ] && ok "a derived agent source carries the agent's name as its reference" || bad "derived ref"
cmp -s "$T/man.before" "$D/s-man.topic" && ok "deriving does not write the file" || bad "derive wrote the file"
B list --source agent | grep -q "Agent waiting topic" && ! B list --source agent | grep -q "A plain one" && ok "list --source agent" || bad "list --source: $(B list --source agent)"
B list --source manual --json | jq -e 'length == 2' >/dev/null && ok "list --source manual --json" || bad "list --source manual"
B list --source nope >/dev/null 2>&1; [ $? = 1 ] && ok "list --source with an unknown source is exit 1" || bad "unknown source accepted"
out=$(B add "From a flag" --source linear --source-ref ABC-5 --source-url https://linear.app/x/ABC-5 --json); echo "$out" | jq -e '.source == "linear" and .source_ref == "ABC-5" and .source_url == "https://linear.app/x/ABC-5" and .source_kind == null and .source_parent == null' >/dev/null && ok "add --source/--source-ref/--source-url, shown by --json" || bad "add source: $out"
B add "Bad source" --source jira >/dev/null 2>&1; [ $? = 1 ] && ok "add with an unknown source is exit 1 (nothing made)" || bad "bad source accepted"
B add "Bad url" --source linear --source-url "javascript:x" >/dev/null 2>&1; [ $? = 1 ] && ok "a source URL must be http(s)" || bad "bad url accepted"
B show "From a flag" | grep -q "source: linear ABC-5 https://linear.app/x/ABC-5" && ok "show names the source" || bad "show: $(B show 'From a flag')"
out=$(B add "By hand" --json); [ "$(echo "$out" | jq -r .source)" = manual ] && ok "a task made by hand is manual" || bad "manual: $out"
out=$(CLAUDECODE=1 HERDR_PANE_ID=w1:p1 B add "Agent made" --json 2>/dev/null); echo "$out" | jq -e '.source == "agent"' >/dev/null && ok "a task an agent makes via bhote add is agent" || bad "agent add: $out"
mk s-hand "Hand written source" next 100 "source=ims" "source_ref=IMS-77" "source_kind=work_item"
[ "$(B show s-hand --json | jq -r .source_kind)" = work_item ] && [ "$(B show s-hand --json | jq -r .source)" = manual ] && B list --all --json >/dev/null && ok "a legacy source (ims) is read without an error (derived as manual); source_kind stays readable" || bad "legacy source: $(B show s-hand --json)"
jq . docs/schema/task.schema.json >/dev/null 2>&1 && ok "docs/schema/task.schema.json is valid JSON" || bad "task schema"
jq -e '."$ref" // .allOf' docs/schema/topic.schema.json >/dev/null 2>&1 && ok "docs/schema/topic.schema.json stays (the old name points to the new schema)" || bad "old schema name"

# ── 5. from-stash: the stash entry becomes a task (source stash, reference = its id) and leaves the stash
clear_d
printf '%s\n' '[{"id":"1700000000000-1","text":"Call the accountant about the Q3 invoices and ask for the receipts","at":1},{"id":"1700000000000-2","text":"Other thing","at":2}]' > "$T/data/stash.json"
out=$(B task from-stash 1700000000000-1 -s next --json); echo "$out" | jq -e '.source == "stash" and .source_ref == "1700000000000-1" and .status == "next"' >/dev/null && ok "task from-stash: a task with source stash and the stash id" || bad "from-stash: $out"
[ "$(jq length "$T/data/stash.json")" = 1 ] && [ "$(jq -r '.[0].id' "$T/data/stash.json")" = "1700000000000-2" ] && ok "from-stash removes the entry from the stash" || bad "stash entry stayed: $(cat "$T/data/stash.json")"
B task from-stash nope >/dev/null 2>&1; [ $? = 1 ] && ok "from-stash with an unknown id is exit 1" || bad "unknown stash id"

# ── 6. the Linear adapter (a fake curl): only the issues in triage are mirrored
clear_d; : > "$FAKE_LOG"
export EXPECT_LINEAR=$LKEY
chk() { local f=$D/$1.topic; [ "$(grep "^$2=" "$f" 2>/dev/null | cut -d= -f2-)" = "$3" ] && ok "$4" || bad "$4: $(cat "$f" 2>&1 | tr '\n' ' ')"; }
cat > "$FAKE_DIR/linear.triage.json" <<'J'
{"data":{"issues":{"nodes":[
 {"identifier":"ABC-101","title":"Fix login","description":"Line one\nline two","url":"https://linear.app/acme/issue/ABC-101","state":{"name":"Triage","type":"triage"},"team":{"key":"ABC"}},
 {"identifier":"ABC-102","title":"Unassigned bug","description":null,"url":"https://linear.app/acme/issue/ABC-102","state":{"name":"Triage","type":"triage"},"team":{"key":"ABC"}},
 {"identifier":"ENG-7","title":"Other team","url":"https://linear.app/acme/issue/ENG-7","state":{"name":"Triage","type":"triage"},"team":{"key":"ENG"}},
 {"identifier":"ABC-103","title":"A server that ignores the filter","url":"https://linear.app/acme/issue/ABC-103","state":{"name":"In Progress","type":"started"},"team":{"key":"ABC"}}
],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}
J
out=$(B task sync --source linear 2>&1); echo "$out" | grep -q "skipped linear: no LINEAR_API_KEY" && [ ! -s "$FAKE_LOG" ] && ok "no key in the store: the source is skipped, nothing is asked" || bad "no key: $out / $(cat "$FAKE_LOG")"
export LINEAR_API_KEY=$LKEY
out=$(B task sync --source linear --json); echo "$out" | jq -e '.created == 3 and (.errors | length) == 0' >/dev/null && [ -f "$D/linear-ABC-101.topic" ] && [ -f "$D/linear-ENG-7.topic" ] && [ ! -e "$D/linear-ABC-103.topic" ] && ok "linear: the issues in triage become tasks (assigned or not); an issue that is not in triage is not" || bad "linear sync: $out / $(ls "$D")"
jq -e 'select(.body != null) | .body | fromjson | .query | select(contains("triage") and (contains("assignedIssues") | not) and (contains("dueDate") | not))' "$FAKE_LOG" >/dev/null && ok "the query asks for the triage state type, not for your assigned issues, and not for due dates" || bad "linear query: $(cut -c1-300 "$FAKE_LOG")"
for r in ABC-101 ABC-102 ENG-7; do chk linear-$r status next "linear $r: status next"; done
chk linear-ABC-101 source linear "source is linear"
chk linear-ABC-101 source_url https://linear.app/acme/issue/ABC-101 "the link of the issue is kept"
chk linear-ABC-101 title "Fix login" "the title is kept"
chk linear-ABC-101 description "Line one" "the first line of the description"
chk linear-ABC-101 source_status_raw Triage "the raw state name is kept (source_status_raw)"
grep -q '^source_due=\|^source_parent=' "$D/linear-ABC-101.topic" || ! grep -q '^source_kind=triage$' "$D/linear-ABC-101.topic" && bad "a mirrored task carries a due date or parent, or has no kind triage: $(cat "$D/linear-ABC-101.topic")" || ok "a mirrored task has kind triage, no due date, no parent"
n=$(ls "$D" | wc -l); B task sync --source linear >/dev/null; [ "$(ls "$D" | wc -l)" = "$n" ] && ok "sync twice: deduped by source and reference" || bad "duplicates after a second sync"
f=$D/linear-ABC-101.topic; cp "$f" "$T/lin.before"; B task sync --source linear >/dev/null; cmp -s "$f" "$T/lin.before" && ok "a sync with no change writes nothing" || bad "idle sync rewrote the file"
B list --source linear --json | jq -e 'length == 3' >/dev/null && ok "list --source linear" || bad "list linear"
B task sync --source ims >/dev/null 2>&1; [ $? = 1 ] && ok "task sync --source ims is refused (the IMS source is gone)" || bad "ims accepted"
# LINEAR_TEAMS: only those teams; the query names them
echo "LINEAR_TEAMS=abc" >> "$BHOTE_CONFIG"; clear_d; : > "$FAKE_LOG"; B task sync --source linear --json | jq -e '.created == 2' >/dev/null && [ ! -e "$D/linear-ENG-7.topic" ] && jq -e 'select(.body != null) | .body | fromjson | .query | select(contains("team: {key: {in: [\"ABC\"]"))' "$FAKE_LOG" >/dev/null && ok "LINEAR_TEAMS filters by team key (in the query and again on the answer)" || bad "team filter: $(ls "$D") $(cut -c1-300 "$FAKE_LOG")"
sed -i.bak '/^LINEAR_TEAMS=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"; B task sync --source linear >/dev/null
# the outside system decides: a change of the title wins; an issue that leaves triage (any way) is done here, never deleted
jq 'del(.data.issues.nodes[1]) | (.data.issues.nodes[0].title) = "Fix the login"' "$FAKE_DIR/linear.triage.json" > "$FAKE_DIR/x" && mv "$FAKE_DIR/x" "$FAKE_DIR/linear.triage.json"
: > "$FAKE_LOG"; out=$(B task sync --source linear --json)
grep -q '^title=Fix the login$' "$D/linear-ABC-101.topic" && ok "the outside system wins for the title" || bad "outside change not taken: $(cat "$D/linear-ABC-101.topic")"
[ -f "$D/linear-ABC-102.topic" ] && [ "$(grep '^status=' "$D/linear-ABC-102.topic")" = "status=done" ] && grep -q '^description=No longer in triage in Linear\.$' "$D/linear-ABC-102.topic" && echo "$out" | jq -e '.done == 1' >/dev/null && ok "an issue that left triage: marked done here, the file stays" || bad "left triage: $(cat "$D/linear-ABC-102.topic" 2>&1) $out"
! grep -q "issue(id" "$FAKE_LOG" && ok "no extra lookup per vanished issue" || bad "fetch per issue"
# it comes back to triage: the outside state wins again
jq '.data.issues.nodes += [{"identifier":"ABC-102","title":"Unassigned bug","url":"https://linear.app/acme/issue/ABC-102","state":{"name":"Triage","type":"triage"},"team":{"key":"ABC"}}]' "$FAKE_DIR/linear.triage.json" > "$FAKE_DIR/x" && mv "$FAKE_DIR/x" "$FAKE_DIR/linear.triage.json"
B task sync --source linear >/dev/null; [ "$(grep '^status=' "$D/linear-ABC-102.topic")" = "status=next" ] && ok "back in triage: the task is next again" || bad "back in triage: $(cat "$D/linear-ABC-102.topic")"
# a local change is kept while Linear is unchanged; written back only with TASK_WRITE=on, and only done
: > "$FAKE_LOG"; B done linear-ABC-101 >/dev/null; B now linear-ENG-7 >/dev/null; B task sync --source linear >/dev/null
[ "$(grep '^status=' "$D/linear-ABC-101.topic")" = "status=done" ] && ! grep -q issueUpdate "$FAKE_LOG" && ok "TASK_WRITE off: a local change stays local and is not written back" || bad "write-back with TASK_WRITE off"
echo '{"data":{"issue":{"id":"uuid-101","team":{"states":{"nodes":[{"id":"st-triage","name":"Triage","type":"triage","position":0},{"id":"st-todo","name":"Todo","type":"unstarted","position":1},{"id":"st-prog","name":"In Progress","type":"started","position":2},{"id":"st-done","name":"Done","type":"completed","position":4},{"id":"st-cancel","name":"Canceled","type":"canceled","position":5}]}}}}}' > "$FAKE_DIR/linear.states.json"
echo "TASK_WRITE=on" >> "$BHOTE_CONFIG"; : > "$FAKE_LOG"; out=$(B task sync --source linear --json)
jq -e 'select(.body != null) | .body | select(contains("issueUpdate")) | fromjson | .variables.s == "st-done"' "$FAKE_LOG" >/dev/null && ok "TASK_WRITE=on: done moves the issue to the Done state of its team" || bad "no write-back: $(cut -c1-300 "$FAKE_LOG")"
[ "$(grep -c issueUpdate "$FAKE_LOG")" = 1 ] && echo "$out" | jq -e '.written_back == 1 and .not_changeable == 0' >/dev/null && ! grep -q '^source_note=' "$D/linear-ENG-7.topic" && ok "other statuses (now) are not written back and are no error or note" || bad "non-done write-back: $out $(grep -c issueUpdate "$FAKE_LOG")"
: > "$FAKE_LOG"; B task sync --source linear >/dev/null; ! grep -q issueUpdate "$FAKE_LOG" && ok "a written-back change is not written again" || bad "write-back repeated"
sed -i.bak '/^TASK_WRITE=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"
# errors: nothing is marked done because of them
touch "$FAKE_DIR/down"; out=$(B task sync --source linear 2>&1); rc=$?; rm "$FAKE_DIR/down"; [ "$rc" = 1 ] && echo "$out" | grep -q "linear:" && ok "an unreachable Linear: exit 1, the tasks stay as they were" || bad "down: rc=$rc $out"
n=$(ls "$D" | wc -l); cp "$D/linear-ABC-101.topic" "$T/lin101"; out=$(EXPECT_LINEAR=other B task sync --source linear 2>&1); rc=$?
[ "$rc" = 1 ] && echo "$out" | grep -q "linear: Linear: the key is not accepted (HTTP 401)" && [ "$(ls "$D" | wc -l)" = "$n" ] && cmp -s "$T/lin101" "$D/linear-ABC-101.topic" && ok "Linear 401: a clear error, exit 1, no task written or changed" || bad "linear 401: rc=$rc $out"
: > "$FAKE_LOG"; jq '.data.issues.pageInfo = {"hasNextPage":true,"endCursor":"c1"}' "$FAKE_DIR/linear.triage.json" > "$FAKE_DIR/x" && cp "$FAKE_DIR/linear.triage.json" "$T/lin.keep" && mv "$FAKE_DIR/x" "$FAKE_DIR/linear.triage.json"
jq '.data.issues.nodes = []' "$FAKE_DIR/linear.triage.json" > "$FAKE_DIR/linear.triage.c1.json.tmp"; rm "$FAKE_DIR/linear.triage.c1.json.tmp"
st=$(grep -h '^status=' "$D"/*.topic | sort | tr '\n' ' '); out=$(B task sync --source linear 2>&1); rc=$?; [ "$rc" = 1 ] && [ "$(grep -h '^status=' "$D"/*.topic | sort | tr '\n' ' ')" = "$st" ] && ok "Linear: a failing second page: error, no partial writes, nothing marked done" || bad "linear page 2: rc=$rc $out"
cp "$T/lin.keep" "$FAKE_DIR/linear.triage.json"
# a task file of the removed IMS source (tombstone or not) is read without trouble and is never touched by a sync
mk ims-IMS-9 "Legacy IMS task" next 100 "source=ims" "source_ref=IMS-9" "source_kind=work_item" "source_due=2026-01-01" "deleted=0"; cp "$D/ims-IMS-9.topic" "$T/ims9"
B list --all --json | jq -e '.[] | select(.id == "ims-IMS-9") | .title == "Legacy IMS task"' >/dev/null && B show ims-IMS-9 >/dev/null && B task sync --source linear >/dev/null && cmp -s "$T/ims9" "$D/ims-IMS-9.topic" && ok "a legacy task with source=ims still reads and is left alone by the sync" || bad "legacy ims task: $(B show ims-IMS-9 2>&1)"
rm -f "$D/ims-IMS-9.topic"

# ── 7. bhote task prune: deletes the mirrored tasks of a source that were not changed here; replicates like bhote rm
clear_d; unset LINEAR_API_KEY
for n in 1 2 3; do mk lin-$n "Mirrored $n" next 100 "source=linear" "source_ref=ABC-$n" "source_id=w$n" "source_status=next"; done
mk lin-4 "Moved by hand" now 100 "source=linear" "source_ref=ABC-4" "source_status=next"
mk lin-5 "With an own field" next 100 "source=linear" "source_ref=ABC-5" "source_status=next" "project=bhote"
mk lin-6 "Edited note" next 150 "source=linear" "source_ref=ABC-6" "source_status=next" "description=My own note"
mk man-1 "A manual one" next 100
mk agt-1 "An agent one" waiting 100 "agent=x" "waiting_for=Anna"
mk stash-1 "From stash" next 100 "source=stash" "source_ref=17"
B task prune --source linear >/dev/null 2>&1; [ $? = 1 ] && [ "$(ls "$D" | wc -l | tr -d ' ')" = 9 ] && ok "prune without --dry-run or --yes refuses" || bad "prune ran without --yes"
B task prune >/dev/null 2>&1; [ $? = 1 ] && ok "prune needs --source" || bad "prune without source"
B task prune --source ims --yes >/dev/null 2>&1; [ $? = 1 ] && ok "prune accepts only linear" || bad "prune other source"
cat "$D"/*.topic | cksum > "$T/p.before"; out=$(B task prune --source linear --dry-run); [ "$(cat "$D"/*.topic | cksum)" = "$(cat "$T/p.before")" ] && echo "$out" | grep -q "3 would be deleted, 3 kept" && ok "prune --dry-run: counts, changes nothing" || bad "dry run: $out"
echo "$out" | grep -q "lin-4: status moved by hand" && echo "$out" | grep -q "lin-5: has its own field project" && echo "$out" | grep -q "lin-6: changed after" && ok "dry run names why a task stays" || bad "reasons: $out"
out=$(B task prune --source linear --yes --json); echo "$out" | jq -e '.deleted == 3 and .kept == 3' >/dev/null && ok "prune --yes --json counts" || bad "prune: $out"
for n in 1 2 3; do grep -q '^deleted=1$' "$D/lin-$n.topic" && grep -q '^pruned=1$' "$D/lin-$n.topic" || bad "lin-$n not a tombstone"; done
B list --all --json | jq -e '[.[].id] | (index("lin-1") == null) and (index("lin-4") != null) and (index("lin-5") != null) and (index("lin-6") != null) and (index("man-1") != null) and (index("agt-1") != null) and (index("stash-1") != null)' >/dev/null && ok "the pruned are gone from the list, the others stay" || bad "list after prune"
! grep -q '^deleted=1' "$D/man-1.topic" "$D/agt-1.topic" "$D/stash-1.topic" "$D/lin-4.topic" && ok "nothing else was touched" || bad "something else deleted"
out=$(B task prune --source linear --yes --json); echo "$out" | jq -e '.deleted == 0' >/dev/null && ok "prune again: nothing left to do" || bad "second prune: $out"
# the tombstone replicates like bhote rm: pushed to kharka with deleted=1
rm -f "$FAKE_KHARKA"; echo STORE=kharka >> "$BHOTE_CONFIG"; export KHARKA=$T/bin/kharka
mk lin-7 "Mirrored 7" next 100 "source=linear" "source_ref=ABC-7" "source_status=next"; B sync >/dev/null 2>&1
jq -e 'select(.key == "bhote.topics/lin-7") | .value.deleted == "0"' "$FAKE_KHARKA" >/dev/null || bad "setup: not in kharka"
B task prune --source linear --yes >/dev/null 2>&1; jq -e 'select(.key == "bhote.topics/lin-7") | .value | (.deleted == "1" and .pruned == "1")' "$FAKE_KHARKA" >/dev/null && ok "kharka: the pruned task is pushed as a tombstone (deleted=1), like bhote rm" || bad "tombstone not pushed"
sed -i.bak '/^STORE=kharka$/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"; unset KHARKA
# a pruned task whose issue is in triage again comes back with the next sync; one deleted by hand does not
clear_d; export LINEAR_API_KEY=$LKEY
B task sync --source linear >/dev/null; B rm linear-ABC-101 >/dev/null 2>&1; B task prune --source linear --yes >/dev/null
B task sync --source linear >/dev/null
[ "$(grep '^deleted=' "$D/linear-ABC-102.topic")" = "deleted=0" ] && ! grep -q '^pruned=1' "$D/linear-ABC-102.topic" && [ "$(grep '^deleted=' "$D/linear-ABC-101.topic")" = "deleted=1" ] && ok "a pruned task that qualifies again is back; one deleted by hand stays deleted" || bad "prune/back: $(cat "$D/linear-ABC-102.topic" "$D/linear-ABC-101.topic")"

# ── 8. secrets stay secret: never in an argument of curl, a cache entry, a task file, the output, or the docs; the store module is used internally
cat "$FAKE_LOG" > "$T/all.log"
! grep -q "$LKEY" "$T/all.log" && ok "no key in any argument of curl (it goes to curl on stdin)" || bad "a key was on a command line"
jq -e 'select(.auth == "bad")' "$T/all.log" >/dev/null 2>&1 && bad "curl got a wrong key" || ok "curl always got the right key on stdin"
! grep -rq "$LKEY" "$D" "$T/shared" 2>/dev/null && ok "no key in a task file or the shared folder" || bad "a key leaked into the data"
# the API cache: bodies are cached, the key is not (BHOTE_API_CACHE on, a throw-away folder)
CD=$T/apicache; : > "$FAKE_LOG"; BHOTE_API_CACHE=$CD B task sync --source linear >/dev/null; n1=$(wc -l < "$FAKE_LOG"); BHOTE_API_CACHE=$CD B task sync --source linear >/dev/null; n2=$(wc -l < "$FAKE_LOG")
[ "$n1" -gt 0 ] && [ "$n2" = "$n1" ] && ok "the second sync is answered by the shared API cache (no new request)" || bad "api cache: $n1 then $n2"
! grep -rq "$LKEY\|Bearer" "$CD" 2>/dev/null && ok "no key and no header in the API cache" || bad "key in cache"
touch "$FAKE_DIR/down"; BHOTE_API_CACHE=$CD B task sync --source linear --json | jq -e '(.errors | length) == 0' >/dev/null && ok "a fresh cache entry covers an unreachable system" || bad "cache not used when down"
rm "$FAKE_DIR/down"
# the secret store, internally: a key stored with bhote secret set is used (no environment variable), the guard of --reveal stays
unset LINEAR_API_KEY
mkdir -p "$T/kc"; cat > "$T/bin/security" <<'PY'
#!/usr/bin/env python3
import os, shlex, sys
d = os.environ["FAKE_KC"]
def run(argv):
    cmd, a = argv[0], argv[1:]
    acct = a[a.index("-a") + 1] if "-a" in a else ""
    f = os.path.join(d, acct)
    if cmd == "add-generic-password": open(f, "w").write(a[a.index("-w") + 1]); return 0
    if cmd == "find-generic-password":
        if not os.path.exists(f): return 44
        print(open(f).read()); return 0
    if cmd == "delete-generic-password":
        if os.path.exists(f): os.unlink(f)
        return 0
    return 1
if sys.argv[1:2] == ["-i"]: sys.exit(run(shlex.split(sys.stdin.readline())))
sys.exit(run(sys.argv[1:]))
PY
chmod +x "$T/bin/security"; export FAKE_KC=$T/kc
if [ "$(uname)" = Darwin ]; then
    clear_d
    printf '%s\n' "$LKEY" | B secret set LINEAR_API_KEY --stdin >/dev/null 2>&1 && B task sync --source linear --json | jq -e '.created > 0' >/dev/null && ok "the key is taken from the secret store (internal call, no env variable)" || bad "store key not used"
    CLAUDECODE=1 B secret get LINEAR_API_KEY --reveal >/dev/null 2>&1; [ $? != 0 ] && ok "bhote secret get --reveal is still refused inside Claude sessions" || bad "reveal guard gone"
    ! grep -rq "$LKEY" "$D" "$T/shared" 2>/dev/null && ok "(and the key from the store is in no file either)" || bad "store key leaked"
else ok "(the secret store test needs the macOS Keychain stub: skipped on this system)"; fi
if grep -rEq "lin_api_[A-Za-z0-9]{20,}" docs README.md CHANGELOG.md herdr-app/mods/bhote/tasksources.py; then bad "a key-like value in docs, README or the adapters"; else ok "no key-like value in docs, README, the adapters"; fi

# ── 8b. bhote task sync --dry-run: the same fetch and decisions, nothing written (no task file, tombstone, lock, API cache entry)
export LINEAR_API_KEY=$LKEY
sed -i.bak '/^TASK_/d;/^LINEAR_TEAMS=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"
clear_d; rm -rf "$T/shared" "$T/dryc"; : > "$FAKE_LOG"
snap() { { ls -la "$D" "$T/dryc" 2>&1; find "$T/shared" -type f 2>/dev/null | sort; cat "$D"/* 2>/dev/null; } | grep -v '^total' | cksum; }
s0=$(snap); dry=$(BHOTE_API_CACHE=$T/dryc B task sync --source linear --dry-run --json); s1=$(snap)
[ -z "$(ls -A "$D")" ] && [ "$s0" = "$s1" ] && [ ! -e "$T/dryc" ] && [ ! -e "$T/shared/tasksync.lock" ] && ok "dry-run: no task file, no lock, no API cache entry, no change in the data or shared folder" || bad "dry-run wrote something: $(ls -A "$D") $(ls "$T/dryc" 2>&1)"
echo "$dry" | jq -e '.dry_run == true and (.errors | length) == 0 and .totals.would_create == 3 and .totals.would_update == 0 and (.candidates | length) == 3' >/dev/null && ok "dry-run --json: counts and one candidate per would-be task" || bad "dry json: $dry"
echo "$dry" | jq -e '.by_source.linear.would_create == 3 and .by_source.linear.would_skip == 1 and .by_source.linear.skip_reasons["not triage"] == 1 and (.by_source.linear | has("task") | not)' >/dev/null && ok "dry-run: grouped by source only; the skip reason is 'not in triage'" || bad "dry by source: $(echo "$dry" | jq -c .by_source)"
echo "$dry" | jq -e '(.candidates | map(.status) | unique) == ["next"] and (.candidates[0] | has("due") | not) and (.candidates[0] | has("kind") | not)' >/dev/null && ok "dry-run: candidates are status next, without kind or due date" || bad "candidates: $(echo "$dry" | jq -c .candidates)"
! echo "$dry" | grep -q "$LKEY" && ! grep -q "$LKEY" <(B task sync --source linear --dry-run 2>&1) && ok "dry-run output holds no key" || bad "key in the dry-run output"
out=$(B task sync --source linear -n); echo "$out" | grep -q "^dry run: nothing is written" && echo "$out" | grep -q "^linear: 3 would create" && echo "$out" | grep -q "would skip (1 not triage)" && echo "$out" | grep -q "^create  linear  \[next\]" && ok "dry-run text: counts per source, the skip reason, the candidates" || bad "dry text: $out"
real=$(B task sync --source linear --json)
echo "$real" | jq -e --argjson d "$dry" '.created == $d.totals.would_create and .updated == $d.totals.would_update and .done == $d.totals.would_close' >/dev/null && [ "$(ls "$D" | wc -l)" -eq "$(echo "$dry" | jq .totals.would_create)" ] && ok "the real sync then does what the dry run said" || bad "real vs dry: $real"
dry2=$(B task sync --source linear --dry-run --json); echo "$dry2" | jq -e '.totals.would_create == 0 and .totals.would_update == 0 and .totals.would_close == 0 and .by_source.linear.unchanged == 3' >/dev/null && ok "after the sync: nothing to create, update or close" || bad "dry after sync: $dry2"
# update, close, hand-deleted, write-back: announced, not done
f1=$D/linear-ABC-101.topic; sed -i.bak 's/^title=.*/title=Renamed here/' "$f1"; rm -f "$f1.bak"; B rm linear-ABC-102 >/dev/null 2>&1
jq 'del(.data.issues.nodes[1])' "$FAKE_DIR/linear.triage.json" > "$FAKE_DIR/x" && mv "$FAKE_DIR/x" "$FAKE_DIR/linear.triage.json"
cp "$f1" "$T/f1.before"; s0=$(snap); dry3=$(B task sync --source linear --dry-run --json); s1=$(snap)
echo "$dry3" | jq -e '.by_source.linear.would_update == 1 and .by_source.linear.would_close == 1 and .by_source.linear.would_skip == 2' >/dev/null && [ "$s0" = "$s1" ] && cmp -s "$f1" "$T/f1.before" && ok "dry-run: a changed title is a would-update, an issue that left triage a would-close, a hand-deleted one a skip; nothing written" || bad "dry update/close: $(echo "$dry3" | jq -c .by_source)"
B task sync --source linear --json | jq -e --argjson d "$dry3" '.updated == $d.totals.would_update and .done == $d.totals.would_close' >/dev/null && ok "update and close counts match the real sync, too" || bad "update/close mismatch"
touch "$FAKE_DIR/down"; B task sync --source linear --dry-run >/dev/null 2>&1; rc=$?; rm "$FAKE_DIR/down"; [ "$rc" = 1 ] && ok "dry-run with an unreachable system: exit 1" || bad "dry down rc=$rc"
B task sync --dry-run --bogus >/dev/null 2>&1; [ $? = 1 ] && ok "dry-run: unknown option still refused" || bad "bogus option"

# ── 9. bhote migrate tasks (wave 2): prepared and tested with fixtures only
rm -rf "${D:?}"; mkdir -p "$D"; rm -f "$FAKE_KHARKA"; sed -i.bak '/^STORE=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"
mk m-1 "One" next 100 "future_field=stays"; mk m-2 "Two" now 100; printf '%s\n' "id=proj-x" "kind=project" "title=X" "created=1" "updated=1" "deleted=0" > "$D/proj-x.topic"
mk m-3 "Three (older topic file)" next 40; printf '%s\n' "id=m-3" "title=Three (task file)" "status=next" "created=1" "updated=50" "deleted=0" > "$D/m-3.task"
mk m-4 "Four (newer topic file)" next 90; printf '%s\n' "id=m-4" "title=Four (older task file)" "status=next" "created=1" "updated=10" "deleted=0" > "$D/m-4.task"
B migrate tasks >/dev/null 2>&1; [ $? = 1 ] && ls "$D"/*.topic >/dev/null 2>&1 && ok "migrate without --dry-run or --yes refuses and changes nothing" || bad "migrate ran without --yes"
ls "$D" > "$T/before.ls"; out=$(B migrate tasks --dry-run); [ "$(ls "$D")" = "$(cat "$T/before.ls")" ] && echo "$out" | grep -q "m-1.topic -> m-1.task" && echo "$out" | grep -q "nothing changed" && ok "migrate --dry-run lists the plan and changes nothing" || bad "dry run: $out"
B migrate tasks --dry-run --json | jq -e '.dry_run == true and .files == 5 and .duplicates == 2' >/dev/null && ok "migrate --dry-run --json counts" || bad "dry json: $(B migrate tasks --dry-run --json)"
out=$(B migrate tasks --yes); ls "$D"/*.topic >/dev/null 2>&1 && bad "a .topic is left: $(ls "$D")" || ok "migrate --yes: every .topic became .task"
grep -q '^future_field=stays$' "$D/m-1.task" && [ -f "$D/proj-x.task" ] && ok "contents (also unknown fields) and records travel with the rename" || bad "migrate lost something"
[ "$(grep '^title=' "$D/m-3.task")" = "title=Three (task file)" ] && [ "$(grep '^title=' "$D/m-4.task")" = "title=Four (newer topic file)" ] && ok "a .topic next to a .task: the newer one stays" || bad "duplicate rule: $(cat "$D/m-3.task" "$D/m-4.task")"
before=$(cat "$D"/*.task | cksum); out=$(B migrate tasks --yes --json); [ "$(cat "$D"/*.task | cksum)" = "$before" ] && echo "$out" | jq -e '.files == 0 and .failed == 0' >/dev/null && ok "migrate is idempotent (the second run does nothing)" || bad "second run: $out"
B list --all --json | jq -e 'length == 4' >/dev/null && B now m-2 >/dev/null && [ -f "$D/m-2.task" ] && [ ! -e "$D/m-2.topic" ] && ok "after the migration bhote reads and writes the .task files in place (wave 1 code on a migrated folder)" || bad "migrated folder not usable"
# kharka entries
echo STORE=kharka >> "$BHOTE_CONFIG"; export KHARKA=$T/bin/kharka
printf '%s\n' '{"key":"bhote.topics/k-1","hlc":"a1","value":{"id":"k-1","title":"K one","status":"next","created":"1","updated":"5","deleted":"0","future_field":"x"}}' '{"key":"bhote.projects/proj-k","hlc":"a2","value":{"id":"proj-k","kind":"project","title":"K","updated":"1","deleted":"0"}}' > "$FAKE_KHARKA"
B migrate tasks --dry-run >/dev/null; ! grep -q "bhote.tasks" "$FAKE_KHARKA" && ok "kharka: the dry run changes no entry" || bad "dry run wrote to kharka"
B migrate tasks --yes >/dev/null; jq -e 'select(.key == "bhote.tasks/k-1") | .value.future_field == "x"' "$FAKE_KHARKA" >/dev/null && ! grep -q '"bhote.topics/k-1"' "$FAKE_KHARKA" && grep -q '"bhote.projects/proj-k"' "$FAKE_KHARKA" && ok "kharka: bhote.topics/<id> moves to bhote.tasks/<id> (fields kept), other collections stay" || bad "kharka migrate: $(cut -c1-300 "$FAKE_KHARKA")"
B migrate tasks --yes --json | jq -e '.kharka_entries == 0' >/dev/null && ok "kharka: the second run finds nothing to do" || bad "kharka not idempotent"

[ "$fail" = 0 ]
