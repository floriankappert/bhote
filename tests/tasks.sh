#!/usr/bin/env bash
# Tests of tasks: the rename (bhote task, bhote topic, /task), dual-read of *.topic and *.task, unknown fields that survive every rewrite, the source of a task
# (derived at read time, chips, filter, CLI flags), the adapters (Linear and IMS against a fake curl; no key leaves the process, no real API is called),
# from-stash, and bhote migrate tasks (a fake kharka). Throw-away config and data. Usage: bash tests/tasks.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID LINEAR_API_KEY IMS_API_KEY
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/cfg/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared HOME=$T/home PYTHONDONTWRITEBYTECODE=1 BHOTE_API_CACHE=off
mkdir -p "$T/bin" "$T/cfg" "$T/home" "$T/fake" "$T/data/topics"; export FAKE_DIR=$T/fake FAKE_LOG=$T/fake.log PATH="$T/bin:$PATH"; echo NOTIFY=off > "$BHOTE_CONFIG"
trap 'rm -rf "${T:?}"' EXIT
B() { bash ./bhote "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
D=$T/data/topics
clear_d() { rm -f "${D:?}"/* "${D:?}"/.pull.* 2>/dev/null; }
LKEY=lin_api_FAKEFAKEFAKE0123456789; IKEY=ims_FAKEFAKEFAKE0123456789
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
ok = "none" if not auth else ("ok" if auth in (os.environ.get("EXPECT_LINEAR", "-"), "Bearer " + os.environ.get("EXPECT_IMS", "-")) else "bad")
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
    if "assignedIssues" in qs:
        after = (q.get("variables") or {}).get("after")
        t = f("linear.assigned.%s.json" % after) if after else f("linear.assigned.json")
        out(200, t)
    if "issue(id" in qs:
        t = f("linear.issue.%s.json" % q["variables"]["id"]); out(200, t or '{"data":{"issue":null}}')
    out(400, "{}")
if "ims-api.test" in url:
    path = url.split("ims-api.test", 1)[1]
    if method == "PATCH": out(int(f("ims.patch.status") or 200), "{}")
    if method == "PUT": out(int(f("ims.put.status") or 200), "{}")
    if path.startswith("/api/tasks/my-work-items"): out(200, f("ims.my-work-items.json") or "[]")
    if path.startswith("/api/tasks?"): out(200, f("ims.tasks.json") or "[]")
    m = re.match(r"/api/company/work-items/(\w+)$", path)
    if m:
        t = f("ims.work-item.%s.json" % m.group(1)); out(200 if t else 404, t or "{}")
    if path.startswith("/api/company/work-items?"): out(200, f("ims.search.json") or "[]")
    m = re.match(r"/api/tasks/(\w+)$", path)
    if m:
        t = f("ims.task.%s.json" % m.group(1)); out(200 if t else 404, t or "{}")
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
out=$(B add "From a flag" --source linear --source-ref IMS-5 --source-url https://linear.app/x/IMS-5 --json); echo "$out" | jq -e '.source == "linear" and .source_ref == "IMS-5" and .source_url == "https://linear.app/x/IMS-5" and .source_kind == null and .source_parent == null' >/dev/null && ok "add --source/--source-ref/--source-url, shown by --json" || bad "add source: $out"
B add "Bad source" --source jira >/dev/null 2>&1; [ $? = 1 ] && ok "add with an unknown source is exit 1 (nothing made)" || bad "bad source accepted"
B add "Bad url" --source linear --source-url "javascript:x" >/dev/null 2>&1; [ $? = 1 ] && ok "a source URL must be http(s)" || bad "bad url accepted"
B show "From a flag" | grep -q "source: linear IMS-5 https://linear.app/x/IMS-5" && ok "show names the source" || bad "show: $(B show 'From a flag')"
out=$(B add "By hand" --json); [ "$(echo "$out" | jq -r .source)" = manual ] && ok "a task made by hand is manual" || bad "manual: $out"
out=$(CLAUDECODE=1 HERDR_PANE_ID=w1:p1 B add "Agent made" --json 2>/dev/null); echo "$out" | jq -e '.source == "agent"' >/dev/null && ok "a task an agent makes via bhote add is agent" || bad "agent add: $out"
mk s-hand "Hand written source" next 100 "source=ims" "source_ref=IMS-77" "source_kind=work_item"
[ "$(B show s-hand --json | jq -r .source_kind)" = work_item ] && ok "stored source fields are shown" || bad "stored fields"
jq . docs/schema/task.schema.json >/dev/null 2>&1 && ok "docs/schema/task.schema.json is valid JSON" || bad "task schema"
jq -e '."$ref" // .allOf' docs/schema/topic.schema.json >/dev/null 2>&1 && ok "docs/schema/topic.schema.json stays (the old name points to the new schema)" || bad "old schema name"

# ── 5. from-stash: the stash entry becomes a task (source stash, reference = its id) and leaves the stash
clear_d
printf '%s\n' '[{"id":"1700000000000-1","text":"Call the accountant about the Q3 invoices and ask for the receipts","at":1},{"id":"1700000000000-2","text":"Other thing","at":2}]' > "$T/data/stash.json"
out=$(B task from-stash 1700000000000-1 -s next --json); echo "$out" | jq -e '.source == "stash" and .source_ref == "1700000000000-1" and .status == "next"' >/dev/null && ok "task from-stash: a task with source stash and the stash id" || bad "from-stash: $out"
[ "$(jq length "$T/data/stash.json")" = 1 ] && [ "$(jq -r '.[0].id' "$T/data/stash.json")" = "1700000000000-2" ] && ok "from-stash removes the entry from the stash" || bad "stash entry stayed: $(cat "$T/data/stash.json")"
B task from-stash nope >/dev/null 2>&1; [ $? = 1 ] && ok "from-stash with an unknown id is exit 1" || bad "unknown stash id"

# ── 6. the adapters (a fake curl): Linear
clear_d; : > "$FAKE_LOG"
export EXPECT_LINEAR=$LKEY EXPECT_IMS=$IKEY
cat > "$FAKE_DIR/linear.assigned.json" <<'J'
{"data":{"viewer":{"assignedIssues":{"nodes":[
 {"identifier":"IMS-101","title":"Fix login","description":"Line one\nline two","url":"https://linear.app/acme/issue/IMS-101","priority":2,"updatedAt":"2026-11-01T10:00:00Z","state":{"name":"In Progress","type":"started"},"team":{"key":"IMS"},"cycle":{"number":5,"name":"C5"}},
 {"identifier":"IMS-102","title":"Backlog thing","description":null,"url":"https://linear.app/acme/issue/IMS-102","state":{"name":"Backlog","type":"backlog"},"team":{"key":"IMS"},"cycle":null},
 {"identifier":"ENG-7","title":"Other team","url":"https://linear.app/acme/issue/ENG-7","state":{"name":"Todo","type":"unstarted"},"team":{"key":"ENG"}},
 {"identifier":"IMS-103","title":"Weird state","url":"https://linear.app/acme/issue/IMS-103","state":{"name":"Triage","type":"unstarted"},"team":{"key":"IMS"}},
 {"identifier":"IMS-104","title":"In review one","url":"https://linear.app/acme/issue/IMS-104","state":{"name":"In Review","type":"started"},"team":{"key":"IMS"}},
 {"identifier":"IMS-105","title":"Todo one","url":"https://linear.app/acme/issue/IMS-105","state":{"name":"Todo","type":"unstarted"},"team":{"key":"IMS"}}
],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}
J
out=$(B task sync --source linear 2>&1); echo "$out" | grep -q "skipped linear: no LINEAR_API_KEY" && [ ! -s "$FAKE_LOG" ] && ok "no key in the store: the source is skipped, nothing is asked" || bad "no key: $out / $(cat "$FAKE_LOG")"
export LINEAR_API_KEY=$LKEY
out=$(B task sync --source linear --json); echo "$out" | jq -e '.created == 6 and (.errors | length) == 0' >/dev/null && ok "linear sync: six issues become six tasks" || bad "linear sync: $out"
for p in 'IMS-101 now' 'IMS-102 later' 'ENG-7 next' 'IMS-103 next' 'IMS-104 review' 'IMS-105 next'; do set -- $p; f=$(grep -l "^source_ref=$1\$" "$D"/*.topic | head -1); [ "$(grep '^status=' "$f" | cut -d= -f2)" = "$2" ] && ok "linear state of $1 -> $2" || bad "linear status of $1: $(cat "$f")"; done
f=$D/linear-IMS-101.topic; [ -f "$f" ] && grep -q '^source=linear$' "$f" && grep -q '^source_url=https://linear.app/acme/issue/IMS-101$' "$f" && grep -q '^title=Fix login$' "$f" && grep -q '^description=Line one$' "$f" && ok "a mirrored task: deterministic id, source fields, title, first line of the description" || bad "mirror file: $(cat "$f")"
n=$(ls "$D" | wc -l); B task sync --source linear >/dev/null; [ "$(ls "$D" | wc -l)" = "$n" ] && ok "sync twice: deduped by source and reference" || bad "duplicates after a second sync"
cp "$f" "$T/lin.before"; B task sync --source linear >/dev/null; cmp -s "$f" "$T/lin.before" && ok "a sync with no change writes nothing" || bad "idle sync rewrote the file"
B list --source linear --json | jq -e 'length == 6' >/dev/null && ok "list --source linear" || bad "list linear"
echo "LINEAR_TEAMS=ims" >> "$BHOTE_CONFIG"; clear_d; B task sync --source linear --json | jq -e '.created == 5' >/dev/null && [ ! -e "$D/linear-ENG-7.topic" ] && ok "LINEAR_TEAMS filters by team key" || bad "team filter"
sed -i.bak '/^LINEAR_TEAMS=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"; B task sync --source linear >/dev/null
# the outside system is authoritative: a change there wins; an issue that is gone (or done) there is done here, never deleted
sed 's/"In Progress","type":"started"/"In Review","type":"started"/; s/Fix login/Fix the login/' "$FAKE_DIR/linear.assigned.json" | jq 'del(.data.viewer.assignedIssues.nodes[1])' > "$FAKE_DIR/linear.assigned.json.new"; mv "$FAKE_DIR/linear.assigned.json.new" "$FAKE_DIR/linear.assigned.json"
echo '{"data":{"issue":{"identifier":"IMS-102","title":"Backlog thing","description":null,"url":"https://linear.app/acme/issue/IMS-102","state":{"name":"Canceled","type":"canceled"},"team":{"key":"IMS"}}}}' > "$FAKE_DIR/linear.issue.IMS-102.json"
B task sync --source linear >/dev/null
[ "$(grep '^status=' "$D/linear-IMS-101.topic")" = "status=review" ] && grep -q '^title=Fix the login$' "$D/linear-IMS-101.topic" && ok "the outside system wins for status and title" || bad "outside change not taken: $(cat "$D/linear-IMS-101.topic")"
[ -f "$D/linear-IMS-102.topic" ] && [ "$(grep '^status=' "$D/linear-IMS-102.topic")" = "status=done" ] && grep -q '^description=Canceled in Linear\.$' "$D/linear-IMS-102.topic" && ok "an issue that is gone: marked done (a cancel says so), the file stays" || bad "vanished issue: $(cat "$D/linear-IMS-102.topic" 2>&1)"
# a local status change is kept while the outside is unchanged, and written back only with TASK_WRITE=on
: > "$FAKE_LOG"; B done linear-IMS-105 >/dev/null; B task sync --source linear >/dev/null
[ "$(grep '^status=' "$D/linear-IMS-105.topic")" = "status=done" ] && ! grep -q issueUpdate "$FAKE_LOG" && ok "TASK_WRITE off: a local change stays local and is not written back" || bad "write-back with TASK_WRITE off: $(grep -c issueUpdate "$FAKE_LOG")"
echo '{"data":{"issue":{"id":"uuid-105","team":{"states":{"nodes":[{"id":"st-todo","name":"Todo","type":"unstarted","position":1},{"id":"st-prog","name":"In Progress","type":"started","position":2},{"id":"st-done","name":"Done","type":"completed","position":4}]}}}}}' > "$FAKE_DIR/linear.states.json"
echo "TASK_WRITE=on" >> "$BHOTE_CONFIG"; : > "$FAKE_LOG"; B task sync --source linear >/dev/null
jq -e 'select(.body != null) | .body | select(contains("issueUpdate")) | fromjson | .variables.s == "st-done"' "$FAKE_LOG" >/dev/null && ok "TASK_WRITE=on: the state moves through issueUpdate (Done state)" || bad "no write-back: $(cut -c1-300 "$FAKE_LOG")"
jq 'del(.data.viewer.assignedIssues.nodes[] | select(.identifier == "IMS-105"))' "$FAKE_DIR/linear.assigned.json" > "$FAKE_DIR/x" && mv "$FAKE_DIR/x" "$FAKE_DIR/linear.assigned.json"   # (Linear now has it done: it leaves the open list)
: > "$FAKE_LOG"; B task sync --source linear >/dev/null; ! grep -q issueUpdate "$FAKE_LOG" && [ "$(grep '^status=' "$D/linear-IMS-105.topic")" = "status=done" ] && ok "a written-back change is not written again" || bad "write-back repeated"
B wait linear-IMS-104 Anna >/dev/null; : > "$FAKE_LOG"; B task sync --source linear --json | jq -e '.not_changeable == 1' >/dev/null && ok "a status Linear has no state for (waiting): not changeable, no error" || bad "waiting write-back"
: > "$FAKE_LOG"; B task sync --source linear >/dev/null; ! grep -q issueUpdate "$FAKE_LOG" && ok "not changeable is not retried (no error loop)" || bad "retry loop"
sed -i.bak '/^TASK_WRITE=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"
touch "$FAKE_DIR/down"; out=$(B task sync --source linear 2>&1); rc=$?; rm "$FAKE_DIR/down"; [ "$rc" = 1 ] && echo "$out" | grep -q "linear:" && ok "an unreachable Linear: exit 1, the tasks stay as they were" || bad "down: rc=$rc $out"
[ "$(grep '^status=' "$D/linear-IMS-105.topic")" = "status=done" ] && [ "$(grep '^status=' "$D/linear-IMS-101.topic")" != "status=done" ] && ok "(and nothing was marked done because of the error)" || bad "tasks changed on error"

# ── 7. the IMS: tasks and work items
clear_d; : > "$FAKE_LOG"; unset LINEAR_API_KEY; export IMS_API_KEY=$IKEY; echo "IMS_URL=https://ims-api.test" >> "$BHOTE_CONFIG"
cat > "$FAKE_DIR/ims.my-work-items.json" <<'J'
{"data":[
 {"id":901,"item_number":42,"title":"Credits for invoices","description":"A ticket","status":"In Progress","type":"Story","remote_id":"BIL-7","deleted_at":null},
 {"id":902,"item_number":43,"title":"An insight","description":"","status":"Open","type":"Insight","deleted_at":null},
 {"id":903,"item_number":44,"title":"Blocked ticket","status":"Blocked","deleted_at":null},
 {"id":904,"item_number":45,"title":"Deleted ticket","status":"Open","deleted_at":"2026-01-01T00:00:00Z"},
 {"id":905,"item_number":46,"title":"Finished ticket","status":"Erledigt","deleted_at":null},
 {"id":906,"item_number":47,"title":"Unusual status","status":"Zur Abnahme","deleted_at":null}
]}
J
cat > "$FAKE_DIR/ims.tasks.json" <<'J'
[
 {"id":11,"title":"Call Anna","description":"personal","status":"Open","work_item_id":901,"deleted_at":null},
 {"id":12,"title":"Prepare the demo","status":"In Bearbeitung","work_item_id":777,"deleted_at":null},
 {"id":13,"title":"Done task","status":"Closed","deleted_at":null},
 {"id":14,"title":"Review the PR","status":"Review","deleted_at":null}
]
J
echo '{"data":{"id":777,"item_number":9,"title":"Parent not mine","status":"Open"}}' > "$FAKE_DIR/ims.work-item.777.json"
out=$(B task sync --source ims --json); echo "$out" | jq -e '.created == 7 and (.errors | length) == 0' >/dev/null && ok "ims sync: work items and tasks become tasks (deleted and done ones do not)" || bad "ims sync: $out"
chk() { local f=$D/$1.topic; [ "$(grep "^$2=" "$f" 2>/dev/null | cut -d= -f2-)" = "$3" ] && ok "$4" || bad "$4: $(cat "$f" 2>&1 | tr '\n' ' ')"; }
chk ims-IMS-42 status now "IMS work item 'In Progress' -> now"
chk ims-IMS-42 source_kind work_item "IMS-42 is a work item (chip: IMS ticket)"
chk ims-IMS-42 description "Jira BIL-7: A ticket" "a Jira link shows its key in the description"
chk ims-IMS-43 status next "IMS 'Open' -> next"
chk ims-IMS-44 status waiting "IMS 'Blocked' -> waiting"
chk ims-IMS-47 status next "an unknown IMS status -> next"
chk ims-IMS-T11 source_kind task "an IMS task has kind task (chip: IMS task)"
chk ims-IMS-T11 source_parent IMS-42 "tasks.work_item_id becomes source_parent (IMS-42), the item is not listed twice"
chk ims-IMS-T12 source_parent IMS-9 "a parent that is not mine is looked up once for its number"
chk ims-IMS-T12 status now "IMS 'In Bearbeitung' -> now"
chk ims-IMS-T14 status review "IMS 'Review' -> review"
[ ! -e "$D/ims-IMS-45.topic" ] && [ ! -e "$D/ims-IMS-46.topic" ] && [ ! -e "$D/ims-IMS-T13.topic" ] && ok "soft-deleted and done IMS items are not mirrored" || bad "deleted/done mirrored: $(ls "$D")"
[ "$(B show ims-IMS-T11 --json | jq -r '.source_url')" = "https://ims-api.test/api/tasks/11" ] && ok "without IMS_WEB_URL the link is the item's API address" || bad "ims url"
echo "IMS_STATUS_MAP=Zur Abnahme=review,Open=later" >> "$BHOTE_CONFIG"; B task sync --source ims >/dev/null
[ "$(grep '^status=' "$D/ims-IMS-47.topic")" = "status=review" ] && [ "$(grep '^status=' "$D/ims-IMS-43.topic")" = "status=later" ] && ok "IMS_STATUS_MAP replaces the defaults (Zur Abnahme -> review, Open -> later): the new mapping is the outside's word" || bad "status map: $(cat "$D/ims-IMS-47.topic")"
sed -i.bak '/^IMS_STATUS_MAP=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"; clear_d; B task sync --source ims >/dev/null
# write-back: PATCH for work items (a 409 for an Insight: not changeable, never an error loop), PUT for tasks
echo "TASK_WRITE=on" >> "$BHOTE_CONFIG"
B done ims-IMS-42 >/dev/null; : > "$FAKE_LOG"; B task sync --source ims >/dev/null
jq -e 'select(.method == "PATCH") | (.url | endswith("/api/company/work-items/901")) and (.body | fromjson | .status == "Done")' "$FAKE_LOG" >/dev/null && ok "TASK_WRITE=on: a work item is PATCHed with its status" || bad "no PATCH: $(cut -c1-250 "$FAKE_LOG")"
echo 409 > "$FAKE_DIR/ims.patch.status"; B done ims-IMS-43 >/dev/null; : > "$FAKE_LOG"; B task sync --source ims --json | jq -e '.not_changeable >= 1' >/dev/null && ok "a 409 (Insight, moves by workflow) is 'not changeable', not an error" || bad "409 handling"
grep -q '^source_note=not changeable in IMS$' "$D/ims-IMS-43.topic" && ok "the note says so" || bad "no note: $(cat "$D/ims-IMS-43.topic")"
: > "$FAKE_LOG"; B task sync --source ims >/dev/null; ! grep -q '"PATCH"' "$FAKE_LOG" && ok "and it is not tried again" || bad "409 retried"
rm "$FAKE_DIR/ims.patch.status"; B done ims-IMS-T14 >/dev/null; : > "$FAKE_LOG"; B task sync --source ims >/dev/null
jq -e 'select(.method == "PUT") | (.url | endswith("/api/tasks/14"))' "$FAKE_LOG" >/dev/null && ok "an IMS task is written back with PUT" || bad "no PUT: $(cut -c1-200 "$FAKE_LOG")"
sed -i.bak '/^TASK_WRITE=/d' "$BHOTE_CONFIG"; rm -f "$BHOTE_CONFIG.bak"
# an item that disappeared
jq 'del(.data[0])' "$FAKE_DIR/ims.my-work-items.json" > "$FAKE_DIR/x" && mv "$FAKE_DIR/x" "$FAKE_DIR/ims.my-work-items.json"; B task sync --source ims >/dev/null
[ -f "$D/ims-IMS-42.topic" ] && [ "$(grep '^status=' "$D/ims-IMS-42.topic")" = "status=done" ] && grep -q '^description=No longer open in IMS\.$' "$D/ims-IMS-42.topic" && ok "an IMS item that disappeared: done here, not deleted" || bad "disappeared item"
# both sources at once
export LINEAR_API_KEY=$LKEY
out=$(B task sync --json); echo "$out" | jq -e '(.sources | index("ims")) != null and (.sources | index("linear")) != null' >/dev/null && ok "task sync without --source runs every source with a key" || bad "both: $out"

# ── 8. secrets stay secret: never in an argument of curl, a cache entry, a task file, the output, or the docs; the store module is used internally
cat "$FAKE_LOG" > "$T/all.log"
! grep -q "$LKEY\|$IKEY" "$T/all.log" && ok "no key in any argument of curl (it goes to curl on stdin)" || bad "a key was on a command line"
jq -e 'select(.auth == "bad")' "$T/all.log" >/dev/null 2>&1 && bad "curl got a wrong key" || ok "curl always got the right key on stdin"
! grep -rq "$LKEY\|$IKEY" "$D" "$T/shared" 2>/dev/null && ok "no key in a task file or the shared folder" || bad "a key leaked into the data"
# the API cache: bodies are cached, the key is not (BHOTE_API_CACHE on, a throw-away folder)
CD=$T/apicache; : > "$FAKE_LOG"; BHOTE_API_CACHE=$CD B task sync --source ims >/dev/null; n1=$(wc -l < "$FAKE_LOG"); BHOTE_API_CACHE=$CD B task sync --source ims >/dev/null; n2=$(wc -l < "$FAKE_LOG")
[ "$n1" -gt 0 ] && [ "$n2" = "$n1" ] && ok "the second sync is answered by the shared API cache (no new request)" || bad "api cache: $n1 then $n2"
! grep -rq "$LKEY\|$IKEY\|Bearer" "$CD" 2>/dev/null && ok "no key and no header in the API cache" || bad "key in cache"
touch "$FAKE_DIR/down"; BHOTE_API_CACHE=$CD B task sync --source ims --json | jq -e '(.errors | length) == 0' >/dev/null && ok "a fresh cache entry covers an unreachable system" || bad "cache not used when down"
rm "$FAKE_DIR/down"
# the secret store, internally: a key stored with bhote secret set is used (no environment variable), the guard of --reveal stays
unset LINEAR_API_KEY IMS_API_KEY
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
    printf '%s\n' "$IKEY" | B secret set IMS_API_KEY --stdin >/dev/null 2>&1 && B task sync --source ims --json | jq -e '.created > 0' >/dev/null && ok "the key is taken from the secret store (internal call, no env variable)" || bad "store key not used"
    CLAUDECODE=1 B secret get IMS_API_KEY --reveal >/dev/null 2>&1; [ $? != 0 ] && ok "bhote secret get --reveal is still refused inside Claude sessions" || bad "reveal guard gone"
    ! grep -rq "$IKEY" "$D" "$T/shared" 2>/dev/null && ok "(and the key from the store is in no file either)" || bad "store key leaked"
else ok "(the secret store test needs the macOS Keychain stub: skipped on this system)"; fi
if grep -rEq "lin_api_[A-Za-z0-9]{20,}|ims_[A-Za-z0-9]{24,}" docs README.md CHANGELOG.md herdr-app/mods/bhote/tasksources.py; then bad "a key-like value in docs, README or the adapters"; else ok "no key-like value in docs, README, the adapters"; fi

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
