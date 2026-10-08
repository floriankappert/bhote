#!/usr/bin/env bash
# Tests of the deployment and the test monitor: the CI of a project found from its repository, GitHub Actions (a fake gh) and
# CircleCI (a fake curl: the token must never be on a command line), local runs (bhote test run) and status files, the
# panel block, the secret token in bhote config. No network. Usage: bash tests/monitor.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID CIRCLECI_TOKEN CIRCLE_TOKEN
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared HOME=$T/home
export FAKE_LOG=$T/fake.log FAKE_DIR=$T/fake
mkdir -p "$T/bin" "$FAKE_DIR" "$HOME"; echo NOTIFY=off > "$T/config"
cat > "$T/bin/gh" <<'SH'
#!/bin/sh
echo "gh $*" >> "$FAKE_LOG"
jqx=""; prev=""; for a in "$@"; do [ "$prev" = --jq ] && jqx=$a; prev=$a; done
out() { if [ -n "$jqx" ]; then jq -r "$jqx" "$1"; else cat "$1"; fi; }
case "$*" in
    "auth status"*) exit 0 ;;
    "api user"*) echo flo ;;
    "api repos/acme/rails/contents/.circleci/config.yml"*) printf '{"content":"%s"}' "$(base64 -w0 < "$FAKE_DIR/circle.yml" 2>/dev/null || base64 < "$FAKE_DIR/circle.yml" | tr -d '\n')" > "$FAKE_DIR/c.json"; out "$FAKE_DIR/c.json" ;;
    "api repos/"*) exit 1 ;;
    "workflow list -R acme/web"*) echo '[{"name":"Backend Deployment"},{"name":"Frontend Deployment"},{"name":"Lint"}]' > "$FAKE_DIR/w.json"; out "$FAKE_DIR/w.json" ;;
    "workflow list"*) echo '[]' > "$FAKE_DIR/w.json"; out "$FAKE_DIR/w.json" ;;
    "repo view acme/web"*) echo '{"defaultBranchRef":{"name":"main"}}' > "$FAKE_DIR/r.json"; out "$FAKE_DIR/r.json" ;;
    "run list"*) out "$FAKE_DIR/runs.json" ;;
    "run view"*) out "$FAKE_DIR/jobs.json" ;;
    *) exit 1 ;;
esac
SH
cat > "$T/bin/curl" <<'SH'
#!/bin/sh
cfg=$(cat); echo "curl $* | $cfg" >> "$FAKE_LOG"
case "$cfg" in *"Circle-Token: tok123"*) ;; *) exit 22 ;; esac
for a in "$@"; do url=$a; done
case "$url" in
    *"/pipeline?branch=develop") cat "$FAKE_DIR/pl-develop.json" ;;
    *"/pipeline?branch=main") cat "$FAKE_DIR/pl-main.json" ;;
    *"/pipeline/p1/workflow") echo '{"items":[{"id":"w1","status":"success","created_at":"2026-10-08T09:00:00.120Z"}]}' ;;
    *"/pipeline/p2/workflow") echo '{"items":[{"id":"w2","status":"on_hold","created_at":"2026-10-08T10:00:00.000Z"}]}' ;;
    *"/workflow/w1/job") echo '{"items":[{"name":"build","status":"success"},{"name":"deploy-staging","status":"success"}]}' ;;
    *"/workflow/w2/job") echo '{"items":[{"name":"build","status":"success"},{"name":"hold-for-production-approval","status":"on_hold"},{"name":"deploy-production","status":"blocked"}]}' ;;
    *"/pipeline") echo '{"items":[{"id":"p1","number":41,"vcs":{"branch":"develop","commit":{"subject":"Merge x"}}}]}' ;;
    *) exit 22 ;;
esac
SH
chmod +x "$T/bin/gh" "$T/bin/curl"; export PATH="$T/bin:$PATH"
B() { bash ./bhote "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
# shellcheck disable=SC1091
BHOTE_SOURCE_ONLY=1 . ./bhote
bhote_info >/dev/null 2>&1; mkdir -p "$SHARED_DIR" "$TOPIC_DIR"

# a CircleCI config: the deploy jobs and their branches (a list under requires: is no job; post-steps end a branch list)
cat > "$FAKE_DIR/circle.yml" <<'Y'
version: 2.1
jobs:
  build: {}
workflows:
  app:
    jobs:
      - build
      - deploy-staging:
          requires:
            - build
          filters:
            branches:
              only:
                - develop
          post-steps:
            - jira/notify:
                environment_type: staging
      - hold-for-production-approval:
          type: approval
          filters: { branches: { only: main } }
      - deploy-production:
          requires: [hold-for-production-approval]
          filters:
            branches:
              only: main
Y
[ "$(mon_deploy_branches < "$FAKE_DIR/circle.yml")" = "develop=Staging,main=Production" ] && ok "detect: CircleCI deploy jobs and their branches" || bad "branches: $(mon_deploy_branches < "$FAKE_DIR/circle.yml")"

# projects: CircleCI found from the repository, GitHub Actions with its deploy workflows, none
B project add Rails >/dev/null
topic_set "$TOPIC_DIR/proj-rails.topic" repos github.com/acme/rails
B project add Web >/dev/null; topic_set "$TOPIC_DIR/proj-web.topic" repos github.com/acme/web
B project add Plain >/dev/null; topic_set "$TOPIC_DIR/proj-plain.topic" repos github.com/acme/plain
out=$(B monitor detect 2>&1)
[ "$(topic_get "$TOPIC_DIR/proj-rails.topic" ci)" = circleci:gh/acme/rails ] && [ "$(topic_get "$TOPIC_DIR/proj-rails.topic" deploy_branches)" = "develop=Staging,main=Production" ] \
  && ok "detect: CircleCI project" || bad "detect rails: $out"
[ "$(topic_get "$TOPIC_DIR/proj-web.topic" ci)" = github:acme/web ] && [ "$(topic_get "$TOPIC_DIR/proj-web.topic" deploy_names)" = "Backend Deployment=Backend,Frontend Deployment=Frontend" ] \
  && [ "$(topic_get "$TOPIC_DIR/proj-web.topic" deploy_branches)" = main ] && ok "detect: GitHub Actions deploy workflows on the default branch" || bad "detect web: $out"
[ -z "$(topic_get "$TOPIC_DIR/proj-plain.topic" ci)" ] && echo "$out" | grep -q "Plain: no CI found" && ok "detect: no CI, said so" || bad "detect plain: $out"

# GitHub Actions: deploy runs on the deploy branch only, states mapped, the branch of the merged PR as the note
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
cat > "$FAKE_DIR/runs.json" <<J
[{"databaseId":1,"workflowName":"Backend Deployment","status":"in_progress","conclusion":"","createdAt":"$now","headBranch":"main","displayTitle":"Merge pull request #7 from acme/feat/login","url":"u1"},
 {"databaseId":2,"workflowName":"Frontend Deployment","status":"completed","conclusion":"failure","createdAt":"2026-10-08T08:00:00Z","headBranch":"main","displayTitle":"x","url":"u2"},
 {"databaseId":3,"workflowName":"Backend Deployment","status":"completed","conclusion":"success","createdAt":"2026-10-08T07:00:00Z","headBranch":"feat/x","displayTitle":"x","url":"u3"},
 {"databaseId":4,"workflowName":"Lint","status":"completed","conclusion":"success","createdAt":"2026-10-08T06:00:00Z","headBranch":"main","displayTitle":"x","url":"u4"},
 {"databaseId":5,"workflowName":"Frontend Deployment","status":"waiting","conclusion":"","createdAt":"2026-10-08T05:00:00Z","headBranch":"main","displayTitle":"x","url":"u5"}]
J
echo '{"jobs":[{"name":"Backend Tests (1/2)","status":"completed","conclusion":"success","startedAt":"2026-10-08T07:00:00Z","steps":[{"status":"completed"},{"status":"in_progress"}]},{"name":"Backend Tests (2/2)","status":"completed","conclusion":"failure","startedAt":"2026-10-08T07:00:01Z","steps":[{"status":"completed"}]},{"name":"Deploy","status":"completed","conclusion":"success","startedAt":"2026-10-08T07:00:00Z","steps":[]}]}' > "$FAKE_DIR/jobs.json"
d=$(deploys_github Web acme/web main "Backend Deployment=Backend,Frontend Deployment=Frontend")
[ "$(echo "$d" | wc -l | tr -d ' ')" = 3 ] && ! echo "$d" | grep -q "u3\|u4" && ok "github: deploy workflows on the deploy branch only" || bad "github filter: $d"
echo "$d" | grep -q "Web${US}Backend${US}running${US}2${US}3${US}feat/login${US}u1" && ok "github: a running deploy with its progress and the PR branch" || bad "github running: $d"
echo "$d" | grep -q "${US}failed${US}.*u2" && echo "$d" | grep -q "${US}approval${US}.*u5" && ok "github: failed and waiting for approval" || bad "github states: $d"
t=$(tests_github Web acme/web "")
echo "$t" | grep -q "Backend Tests${US}failed${US}2${US}2${US}" && ok "github tests: the shards of a job are one entry (any failure: failed)" || bad "github tests: $t"

# CircleCI: the token goes to curl on stdin; staging done, production waits for approval
d=$(deploys_circleci Rails gh/acme/rails "develop=Staging,main=Production" "")
[ -z "$d" ] && ok "circleci: no token, nothing (and quiet)" || bad "circleci without token: $d"
echo 'CIRCLECI_TOKEN=tok123' >> "$T/config"
echo '{"items":[{"id":"p1","number":41,"vcs":{"branch":"develop","commit":{"subject":"Merge x"}}}]}' > "$FAKE_DIR/pl-develop.json"
echo '{"items":[{"id":"p2","number":42,"vcs":{"branch":"main","commit":{"subject":"Release 5"}}}]}' > "$FAKE_DIR/pl-main.json"
d=$(deploys_circleci Rails gh/acme/rails "develop=Staging,main=Production" "")
echo "$d" | grep -q "Rails${US}Staging${US}ok${US}2${US}2${US}Merge x" && echo "$d" | grep -q "Rails${US}Production${US}approval${US}1${US}3${US}Release 5${US}https://app.circleci.com/pipelines/gh/acme/rails/42/workflows/w2" \
  && ok "circleci: staging deployed, production waits for approval" || bad "circleci: $d"
grep -q "tok123" <(grep "^curl " "$FAKE_LOG" | sed 's/ | .*//') && bad "the token is on curl's command line" || ok "circleci: the token never on a command line"
t=$(tests_circleci Rails gh/acme/rails "")
echo "$t" | grep -q "Rails${US}build${US}ok" && ok "circleci tests: the build job of the last pipeline" || bad "circleci tests: $t"

# local runs: bhote test run passes output and exit code through and keeps a summary; status files
out=$(cd "$T" && bash "$OLDPWD/bhote" test run -n unit -- sh -c 'echo "12 runs, 30 assertions, 1 failures, 0 errors, 0 skips"; exit 3'); rc=$?
[ "$rc" = 3 ] && echo "$out" | grep -q "12 runs" && ok "test run: output and exit code as the command's" || bad "test run rc=$rc: $out"
r=$(cat "$BHOTE_DATA"/testruns/*.run); echo "$r" | grep -q "^exit=3" && echo "$r" | grep -q "^summary=12 runs · 1 failures · 0 errors" && ok "test run: a record with the Minitest summary" || bad "run record: $r"
printf 'Test Files  3 passed (3)\n      Tests  2 failed | 40 passed (42)\n' > "$T/v.log"; [ "$(test_summary "$T/v.log")" = "2 failed · 40 passed" ] && ok "summary: Vitest" || bad "vitest: $(test_summary "$T/v.log")"
printf 'ok   a\nok   b\nFAIL c\n' > "$T/b.log"; [ "$(test_summary "$T/b.log")" = "2 ok · 1 FAIL" ] && ok "summary: bhote's own tests" || bad "bhote summary: $(test_summary "$T/b.log")"
mkdir -p "$HOME/.cache/ims-test-status"; ms=$(( $(date +%s) * 1000 ))
printf '{"tree":"web-wt1","suite":"server","done":5,"total":10,"passed":40,"failed":0,"startedAt":%s,"updatedAt":%s,"finished":false,"ok":null}' "$ms" "$ms" > "$HOME/.cache/ims-test-status/web-wt1.server.json"
printf '{"tree":"web-wt1","suite":"client","done":9,"total":9,"passed":90,"failed":2,"startedAt":1000,"updatedAt":2000,"finished":true,"ok":false}' > "$HOME/.cache/ims-test-status/web-wt1.client.json"
l=$(tests_local)
echo "$l" | grep -q "web-wt1${US}server${US}running${US}5${US}10" && echo "$l" | grep -q "web-wt1${US}client${US}failed${US}9${US}9${US}2 failed · 90 passed" && ok "status files: running with progress, failed with counts" || bad "status files: $l"
printf '{"tree":"evil\\u001b[31m","suite":"s","done":"a[$(touch %s/pwned)]","total":"9","passed":1,"failed":0,"startedAt":%s,"updatedAt":%s,"ok":null}' "$T" "$ms" "$ms" > "$HOME/.cache/ims-test-status/evil.json"
l=$(tests_local | grep evil); rm -f "$HOME/.cache/ims-test-status/evil.json"
case "$l" in *$'\033'*) bad "status file: escape code kept" ;; *) [ -n "$l" ] && ok "status file: control characters are cleaned" || bad "status file: dropped: $l" ;; esac
printf 'x  3 runs, 9 assertions, 1 failures, 0 errors, 0 skips\n' > "$T/m.log"; [ "$(test_summary "$T/m.log")" = "3 runs · 1 failures · 0 errors" ] && ok "summary: Minitest after other text" || bad "minitest: $(test_summary "$T/m.log")"
[ -e "$T/pwned" ] && bad "status file: a command ran" || ok "status file: a number field cannot run a command"

# one round, newest first; the panel block; the monitor off: nothing
echo "TEST_MONITOR=on" >> "$T/config"; echo "DEPLOY_MONITOR=on" >> "$T/config"
monitor_fetch deploys; monitor_fetch tests
head -1 "$SHARED_DIR/deploys.list" | grep -q "${US}running${US}" && sort -t "$US" -k1,1nr -c "$SHARED_DIR/deploys.list" && ok "fetch: one list, newest first" || bad "deploys.list: $(cat "$SHARED_DIR/deploys.list")"
blk=$( COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitors_block )
echo "$blk" | grep -q "Deployments" && echo "$blk" | grep -q "Web · Backend" && echo "$blk" | grep -q "approval" && echo "$blk" | grep -q "Tests" \
  && [ "$(echo "$blk" | grep -c "Web · \|Rails · \|web-wt1 · \|Plain · ")" -le 10 ] && ok "panel: both blocks, five entries each at most" || bad "block: $blk"
cfg_set DEPLOY_MONITOR off; cfg_set TEST_MONITOR off; [ -z "$( COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitors_block )" ] && ok "panel: monitors off, no block" || bad "block while off"

# new features: the head says so until the wizard ran at the current level; the wizard then offers only the new steps
NEWFEAT=1; COLS=60; view_head | grep -q "New Features Available · w Start Wizard" && ok "head: new features line" || bad "head: no new features line"
NEWFEAT=0; view_head | grep -q "New Features" && bad "head: line without new features" || ok "head: no line when up to date"
[ "$(step_level monitors)" -gt 1 ] && [ "$(step_level checks)" = 1 ] && ok "wizard: the monitor step is a level-2 step" || bad "step_level"

# the token is a secret: bhote config never prints it
B config list | grep -q tok123 && bad "config list prints the token" || ok "config: the token is never printed"
[ "$(B config get CIRCLECI_TOKEN)" = "(set)" ] && ok "config get: (set)" || bad "config get: $(B config get CIRCLECI_TOKEN)"
rm -rf "$T"; exit $fail
