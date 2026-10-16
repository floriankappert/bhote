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
    "api repos/acme/IMS/contents/.circleci/config.yml"*) printf '{"content":"%s"}' "$(base64 -w0 < "$FAKE_DIR/ims-setup.yml" 2>/dev/null || base64 < "$FAKE_DIR/ims-setup.yml" | tr -d '\n')" > "$FAKE_DIR/c.json"; out "$FAKE_DIR/c.json" ;;
    "api repos/acme/IMS/contents/.circleci/continue.yml"*) printf '{"content":"%s"}' "$(base64 -w0 < "$FAKE_DIR/ims-continue.yml" 2>/dev/null || base64 < "$FAKE_DIR/ims-continue.yml" | tr -d '\n')" > "$FAKE_DIR/c.json"; out "$FAKE_DIR/c.json" ;;
    "api repos/"*) exit 1 ;;
    "pr list -R acme/ims --head feat/credits"*) echo '[{"number":123}]' > "$FAKE_DIR/pr.json"; out "$FAKE_DIR/pr.json" ;;
    "pr list"*) echo '[]' > "$FAKE_DIR/pr.json"; out "$FAKE_DIR/pr.json" ;;
    "workflow list -R acme/web"*) echo '[{"name":"Backend Deployment"},{"name":"Frontend Deployment"},{"name":"Lint"}]' > "$FAKE_DIR/w.json"; out "$FAKE_DIR/w.json" ;;
    "workflow list"*) echo '[]' > "$FAKE_DIR/w.json"; out "$FAKE_DIR/w.json" ;;
    "repo view acme/IMS"*|"repo view acme/ims"*) echo '{"nameWithOwner":"acme/IMS"}' > "$FAKE_DIR/r.json"; out "$FAKE_DIR/r.json" ;;
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
    *"/project/gh/acme/ims/pipeline?branch=main") cat "$FAKE_DIR/pl-ims.json" ;;
    *"/pipeline?branch=develop") cat "$FAKE_DIR/pl-develop.json" ;;
    *"/pipeline?branch=main") cat "$FAKE_DIR/pl-main.json" ;;
    *"/pipeline/i1/workflow") cat "$FAKE_DIR/wf-i1.json" ;;
    *"/workflow/iw1/job") cat "$FAKE_DIR/jobs-iw1.json" ;;
    *"/workflow/iw2/job") cat "$FAKE_DIR/jobs-iw2.json" ;;
    *"/workflow/iw3/job") cat "$FAKE_DIR/jobs-iw3.json" ;;
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

# a dynamic CircleCI config (setup: true, the jobs in continue.yml): detect reads the continuation; deploys main only, a label for each deploy job
cat > "$FAKE_DIR/ims-setup.yml" <<'Y'
version: 2.1
setup: true
workflows:
  path-filter:
    jobs:
      - path-filtering/filter:
          config-path: .circleci/continue.yml
Y
cat > "$FAKE_DIR/ims-continue.yml" <<'Y'
version: 2.1
workflows:
  ims-backend-workflow:
    jobs:
      # Branches only: proves it builds. On main the deploy runs, and a deploy still runs its own tests.
      - build-backend
      - notify-backend-tests-failed:
          requires:
            - build-backend: [failed]
          filters:
            branches:
              only: main
      - build-images-backend:
          filters:
            branches:
              only: main
      - deploy-ecs-backend:
          requires:
            - build-backend
            - build-images-backend
          filters:
            branches:
              only: main
  ims-frontend-workflow:
    jobs:
      - build-frontend:
          filters:
            branches:
              ignore: main
      - release-and-deploy-frontend:
          filters:
            branches:
              only: main
jobs:
  build-backend: {}
Y
B project add IMS >/dev/null; topic_set "$TOPIC_DIR/proj-ims.topic" repos github.com/acme/ims
out=$(B monitor detect IMS 2>&1)
[ "$(topic_get "$TOPIC_DIR/proj-ims.topic" ci)" = circleci:gh/acme/IMS ] && [ "$(topic_get "$TOPIC_DIR/proj-ims.topic" deploy_branches)" = "main=main" ] \
  && [ "$(topic_get "$TOPIC_DIR/proj-ims.topic" deploy_names)" = "deploy-ecs-backend=Backend,release-and-deploy-frontend=Frontend" ] \
  && ok "detect: a dynamic CircleCI config is read through its continuation (deploy jobs, labels, main only), the slug as GitHub spells it" || bad "detect ims: $out · $(topic_get "$TOPIC_DIR/proj-ims.topic" deploy_branches) · $(topic_get "$TOPIC_DIR/proj-ims.topic" deploy_names)"
echo '{"items":[{"id":"i1","number":77,"vcs":{"branch":"main","commit":{"subject":"Add credits (#123)"}}}]}' > "$FAKE_DIR/pl-ims.json"
echo '{"items":[{"id":"iw1","name":"ims-backend-workflow","status":"running","created_at":"2026-10-16T09:00:00.000Z"},{"id":"iw2","name":"ims-frontend-workflow","status":"success","created_at":"2026-10-16T09:00:00.000Z"},{"id":"iw3","name":"path-filter","status":"success","created_at":"2026-10-16T09:00:00.000Z"}]}' > "$FAKE_DIR/wf-i1.json"
echo '{"items":[{"name":"build-backend","status":"success"},{"name":"notify-backend-tests-failed","status":"not_run"},{"name":"build-images-backend","status":"running"},{"name":"deploy-ecs-backend","status":"blocked"}]}' > "$FAKE_DIR/jobs-iw1.json"
echo '{"items":[{"name":"release-and-deploy-frontend","status":"success"}]}' > "$FAKE_DIR/jobs-iw2.json"
echo '{"items":[{"name":"path-filtering/filter","status":"success"}]}' > "$FAKE_DIR/jobs-iw3.json"
d=$(deploys_circleci IMS gh/acme/ims "main=main" "deploy-ecs-backend=Backend,release-and-deploy-frontend=Frontend")
echo "$d" | grep -q "IMS${US}Backend${US}running${US}1${US}3${US}Add credits (#123)${US}https://app.circleci.com/pipelines/gh/acme/ims/77/workflows/iw1" \
  && echo "$d" | grep -q "IMS${US}Frontend${US}ok${US}1${US}1${US}" && [ "$(printf '%s\n' "$d" | awk 'END { print NR }')" = 2 ] \
  && ok "circleci: one deployment per workflow with a deploy job (labelled by it), no notify job in the progress, the setup workflow is none" || bad "circleci ims: $d"
d=$(deploys_circleci IMS gh/acme/ims "main=main" "")
echo "$d" | grep -q "IMS${US}Backend${US}running" && echo "$d" | grep -q "IMS${US}Frontend${US}ok" && ok "circleci: without a names map the label is the last word of the deploy job" || bad "circleci ims labels: $d"
ls "$SHARED_DIR/ccjobs/iw2" >/dev/null 2>&1 && [ ! -e "$SHARED_DIR/ccjobs/iw1" ] && ok "circleci: the jobs of a finished workflow are kept, a running one is read again" || bad "circleci job cache: $(ls "$SHARED_DIR/ccjobs" 2>&1)"

# an agent's deployment: main is deployed, its commit title ends with the pull request ("(#123)"): the topic of the agent whose branch has that merged PR waits for it
topic_new "Credits work" "" "" >/dev/null; cf="$TOPIC_DIR/$NEW_TOPIC_ID.topic"
topic_set "$cf" status now; topic_set "$cf" agent_pane w1:p1; topic_set "$cf" agent_machine "$HOST"; topic_set "$cf" agent_branch feat/credits; topic_set "$cf" project proj-ims
topic_new "Other work" "" "" >/dev/null; of="$TOPIC_DIR/$NEW_TOPIC_ID.topic"
topic_set "$of" status now; topic_set "$of" agent_pane w2:p1; topic_set "$of" agent_machine "$HOST"; topic_set "$of" agent_branch feat/other; topic_set "$of" project proj-ims
printf '%s\n' "$(date +%s)${US}IMS${US}Backend${US}running${US}1${US}3${US}Add credits (#123)${US}https://app.circleci.com/pipelines/gh/acme/ims/77/workflows/iw1" > "$SHARED_DIR/deploys.list"
deploy_sync
[ "$(topic_get "$cf" status)" = waiting ] && [ "$(topic_get "$cf" deploy_wait)" = 1 ] && [ "$(topic_get "$cf" waiting_for)" = "Deployment Backend" ] \
  && [ "$(topic_get "$cf" deploy_url)" = "https://app.circleci.com/pipelines/gh/acme/ims/77/workflows/iw1" ] && [ "$(topic_get "$of" status)" = now ] \
  && ok "circleci: the agent whose branch has the merged pull request waits for the deployment of main (another agent does not)" || bad "deploy_sync by PR: $(topic_get "$cf" status) / $(topic_get "$of" status)"
topic_set "$cf" status now; topic_set "$cf" deploy_wait ""
printf '%s\n' "$(date +%s)${US}IMS${US}Backend${US}running${US}1${US}3${US}Merge pull request #123 from acme/feat/cre${US}https://app.circleci.com/pipelines/gh/acme/ims/78/workflows/iw9" > "$SHARED_DIR/deploys.list"
deploy_sync
[ "$(topic_get "$cf" status)" = waiting ] && [ "$(topic_get "$cf" deploy_url)" = "https://app.circleci.com/pipelines/gh/acme/ims/78/workflows/iw9" ] \
  && ok "circleci: a merge commit's title (\"Merge pull request #123 from …\") binds the same way" || bad "deploy_sync by merge commit: $(topic_get "$cf" status)"

# local runs: bhote test run passes output and exit code through and keeps a summary; status files
out=$(cd "$T" && bash "$OLDPWD/bhote" test run -n unit -- sh -c 'echo "12 runs, 30 assertions, 1 failures, 0 errors, 0 skips"; exit 3'); rc=$?
[ "$rc" = 3 ] && echo "$out" | grep -q "12 runs" && ok "test run: output and exit code as the command's" || bad "test run rc=$rc: $out"
r=$(cat "$BHOTE_DATA"/testruns/*.run); echo "$r" | grep -q "^exit=3" && echo "$r" | grep -q "^summary=12 runs · 1 failures · 0 errors" && ok "test run: a record with the Minitest summary" || bad "run record: $r"
printf 'Test Files  3 passed (3)\n      Tests  2 failed | 40 passed (42)\n' > "$T/v.log"; [ "$(test_summary "$T/v.log")" = "2 failed · 40 passed" ] && ok "summary: Vitest" || bad "vitest: $(test_summary "$T/v.log")"
printf 'ok   a\nok   b\nFAIL c\n' > "$T/b.log"; [ "$(test_summary "$T/b.log")" = "2 ok · 1 FAIL" ] && ok "summary: bhote's own tests" || bad "bhote summary: $(test_summary "$T/b.log")"
mkdir -p "$HOME/.cache/bhote/test-status"; ms=$(( $(date +%s) * 1000 ))
printf '{"tree":"web-wt1","suite":"server","done":5,"total":10,"passed":40,"failed":0,"startedAt":%s,"updatedAt":%s,"finished":false,"ok":null}' "$ms" "$ms" > "$HOME/.cache/bhote/test-status/web-wt1.server.json"
printf '{"tree":"web-wt1","suite":"client","done":9,"total":9,"passed":90,"failed":2,"startedAt":1000,"updatedAt":2000,"finished":true,"ok":false}' > "$HOME/.cache/bhote/test-status/web-wt1.client.json"
l=$(tests_local)
echo "$l" | grep -q "web-wt1${US}server${US}running${US}5${US}10" && echo "$l" | grep -q "web-wt1${US}client${US}failed${US}9${US}9${US}2 failed · 90 passed" && ok "status files: running with progress, failed with counts" || bad "status files: $l"
printf '{"tree":"evil\\u001b[31m","suite":"s","done":"a[$(touch %s/pwned)]","total":"9","passed":1,"failed":0,"startedAt":%s,"updatedAt":%s,"ok":null}' "$T" "$ms" "$ms" > "$HOME/.cache/bhote/test-status/evil.json"
l=$(tests_local | grep evil); rm -f "$HOME/.cache/bhote/test-status/evil.json"
case "$l" in *$'\033'*) bad "status file: escape code kept" ;; *) [ -n "$l" ] && ok "status file: control characters are cleaned" || bad "status file: dropped: $l" ;; esac
printf 'x  3 runs, 9 assertions, 1 failures, 0 errors, 0 skips\n' > "$T/m.log"; [ "$(test_summary "$T/m.log")" = "3 runs · 1 failures · 0 errors" ] && ok "summary: Minitest after other text" || bad "minitest: $(test_summary "$T/m.log")"
[ -e "$T/pwned" ] && bad "status file: a command ran" || ok "status file: a number field cannot run a command"

# one round, newest first; the panel block; the monitor off: nothing
echo "TEST_MONITOR=on" >> "$T/config"; echo "DEPLOY_MONITOR=on" >> "$T/config"
monitor_fetch deploys; monitor_fetch tests
head -1 "$SHARED_DIR/deploys.list" | grep -q "${US}running${US}" && sort -t "$US" -k1,1nr -c "$SHARED_DIR/deploys.list" && ok "fetch: one list, newest first" || bad "deploys.list: $(cat "$SHARED_DIR/deploys.list")"
blk=$( COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitors_block )
echo "$blk" | grep -q "Deployments" && echo "$blk" | grep -q "Web · Backend" && echo "$blk" | grep -q "approval" && echo "$blk" | grep -q "Tests" \
  && [ "$(echo "$blk" | grep -c "Web · \|Rails · \|web-wt1 · \|Plain · ")" -le 14 ] && ok "panel: both blocks, seven entries each at most" || bad "block: $blk"
echo "$blk" | grep -q "Web · Backend (feat/login)" && ok "panel: a running entry says in brackets what goes live" || bad "running note: $blk"
# the small view of a section: how many run, and how many of the LAST TEN failed (a failure further back does not count); the caption is the button
cp "$SHARED_DIR/deploys.list" "$T/deploys.keep"; t0=$(date +%s)
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
    case $i in 1|2) st=running ;; 3|11) st=failed ;; *) st=ok ;; esac
    printf '%s\n' "$(( t0 - i ))${US}Web${US}Backend${US}$st${US}1${US}4${US}fix/b$i${US}https://ci/$i"
done > "$SHARED_DIR/deploys.list"
section_toggle deploys; section_compact deploys && ok "section: the toggle switches to counts only" || bad "toggle"
blk=$( COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitor_block deploys 4 | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' )
case "$blk" in *"2 running"*"1 failed"*) [ "$(printf '%s\n' "$blk" | wc -l | tr -d ' ')" = 2 ] && ok "deployments, small: 2 running · 1 failed (the failure beyond the last ten is not counted), two lines" || bad "small lines: $blk" ;; *) bad "small view: $blk" ;; esac
case "$blk" in *"Web · Backend"*) bad "small view still lists entries" ;; *) ok "deployments, small: no entries" ;; esac
case "$blk" in *"$(printf '\035cap:deploys\035')"*) ok "section: the caption carries its ref (a click toggles)" ;; *) bad "caption ref" ;; esac
section_toggle deploys; ! section_compact deploys && ok "section: toggled back" || bad "toggle back"
# a failure that a newer run of the same project and name made good is "resolved", in the small view and in the list
{ printf '%s\n' "$(( t0 - 1 ))${US}Web${US}Backend${US}ok${US}${US}${US}fix/n1${US}u1" "$(( t0 - 2 ))${US}Web${US}Backend${US}failed${US}${US}${US}fix/n2${US}u2" \
    "$(( t0 - 3 ))${US}Web${US}Frontend${US}failed${US}${US}${US}fix/n3${US}u3" "$(( t0 - 4 ))${US}Web${US}Backend${US}running${US}1${US}4${US}fix/n4${US}u4"; } > "$SHARED_DIR/deploys.list"
section_toggle deploys
blk=$( COLS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); monitor_block deploys 4 | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' )
case "$blk" in *"1 running · ✗ 1 failed · ✓ 1 resolved"*) ok "deployments, small: open failures and resolved ones are told apart" ;; *) bad "resolved (small): $blk" ;; esac
section_toggle deploys
blk=$( COLS=60; RULE_LINE=$(hline 55); DASH_LINE=$(dline 55); monitor_block deploys 4 | LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' )
[ "$(printf '%s\n' "$blk" | grep -c "failed · resolved")" = 1 ] && [ "$(printf '%s\n' "$blk" | grep "Frontend" | grep -c resolved)" = 0 ] && ok "deployments: a failure with a newer success says resolved, an open one does not" || bad "resolved (list): $blk"
# nothing running: the small view has no colour on it (grey), with something running it is orange
printf '%s\n' "$(( t0 - 1 ))${US}Web${US}Backend${US}ok${US}${US}${US}fix/z${US}uz" > "$SHARED_DIR/deploys.list"; section_toggle deploys
blk=$( ORANGE=ORG; D=DIM; BOLD=B; X=; COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitor_block deploys 4 )
case "$blk" in *ORG*) bad "0 running: still coloured: $blk" ;; *"DIM0 running"*) ok "small view: 0 running has no colour" ;; *) bad "0 running: $blk" ;; esac
printf '%s\n' "$(( t0 - 1 ))${US}Web${US}Backend${US}running${US}1${US}4${US}fix/z${US}uz" > "$SHARED_DIR/deploys.list"
blk=$( ORANGE=ORG; D=DIM; BOLD=B; X=; COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitor_block deploys 4 )
case "$blk" in *"BORG1 running"*) ok "small view: with something running it is orange" ;; *) bad "1 running: $blk" ;; esac
section_toggle deploys
cp "$T/deploys.keep" "$SHARED_DIR/deploys.list"
cfg_set DEPLOY_MONITOR off; cfg_set TEST_MONITOR off; [ -z "$( COLS=50; RULE_LINE=$(hline 45); DASH_LINE=$(dline 45); monitors_block )" ] && ok "panel: monitors off, no block" || bad "block while off"

# new features: the head says so until the wizard ran at the current level; the wizard then offers only the new steps
NEWFEAT=1; COLS=60; view_head | grep -q "New Features Available · w Start Wizard" && ok "head: new features line" || bad "head: no new features line"
NEWFEAT=0; view_head | grep -q "New Features" && bad "head: line without new features" || ok "head: no line when up to date"
[ "$(step_level monitors)" -gt 1 ] && [ "$(step_level checks)" = 1 ] && ok "wizard: the monitor step is a level-2 step" || bad "step_level"

# updates: the newest tag against this version; the head and the command that updates a checkout
version_gt 0.5.10 0.5.9 && ! version_gt 0.5.1 0.5.1 && ! version_gt 0.4.9 0.5.0 && version_gt 1.0.0 0.9.9 && ok "update: versions compare numerically" || bad "version_gt"
mkdir -p "$DATA_DIR"; printf 'checked=1\nlatest=9.9.9\n' > "$UPDATE_FILE"; update_load; [ "$UPD_LATEST" = 9.9.9 ] && ok "update: a newer version is noticed" || bad "update_load: $UPD_LATEST"
printf 'checked=1\nlatest=0.0.1\n' > "$UPDATE_FILE"; update_load; [ -z "$UPD_LATEST" ] && ok "update: an older one is not" || bad "update_load old: $UPD_LATEST"
printf 'latest=1.2.3; touch %s/pwn\n' "$T" > "$UPDATE_FILE"; update_load; [ -z "$UPD_LATEST" ] && [ ! -e "$T/pwn" ] && ok "update: a bad file is ignored" || bad "update_load bad"
case "$(update_command)" in "git -C "*" pull"|*"pacman"*|"brew update"*) ok "update: a command for this install" ;; *) bad "update_command: $(update_command)" ;; esac

# the keys of the herdr you type into: another machine's config.toml (a copy), nothing before it arrives
cfg_set HERDR_KEYS_MACHINE Elsewhere; keys_cache_set; rm -f "$KEYS_CACHE"; focus_key_set KK; [ -z "$KK" ] && ok "keys: none until the other machine's config is there" || bad "keys before: $KK"
printf '[keys]\nprefix = "ctrl+b"\n[[keys.command]]\nkey = "prefix+t"\ncommand = "bhote.panel.focus"\n' > "$KEYS_CACHE"; focus_key_set KK; [ "$KK" = "Ctrl+B|T" ] && ok "keys: read from the other machine's config" || bad "keys after: $KK"
cfg_set HERDR_KEYS_MACHINE any; rm -f "$KEYS_CACHE"

# the version in the panel: on its release tag it is the version, past the tag (local work) its local build number
( vd=$T/vl; rm -rf "$vd"; mkdir -p "$vd"; cp ./bhote "$vd/bhote"; cd "$vd" || exit 9
  git init -q . && git -c user.name=t -c user.email=t@t add bhote && git -c user.name=t -c user.email=t@t commit -qm one && git tag "v$VERSION" || exit 8
  lab() ( BHOTE_SOURCE_ONLY=1; . "$vd/bhote"; version_label_set; printf '%s' "$VERSION_LABEL" )
  [ "$(lab)" = "$VERSION" ] || exit 1
  echo '# a local change' >> bhote; git -c user.name=t -c user.email=t@t commit -qam two
  [ "$(lab)" = "$LOCAL_BUILD" ] && [ "$LOCAL_BUILD" = "0.9.1.3 Preview" ] || exit 2
  exit 0 ) && ok "version: the release tag shows the version, a checkout past it its local build (0.9.1.3 Preview)" || bad "version label: step $?"
# the monitor bar is built from whole cells, also where the shell counts bytes (bash 3.2 / macOS without a UTF-8 locale cut ■ in half: ��)
( export LC_ALL=C; mon_bar_set b 7 10 5; [ "$MB_ON" = "■■■■" ] && [ "$MB_OFF" = "■" ] && [ "$b" = "■■■■■" ] ) \
    && ok "monitor bar: filled and empty cells are whole characters (also in the C locale)" || bad "monitor bar cells: on=$MB_ON off=$MB_OFF"
# the topic page: a done topic gets actions that make sense; the numbers line up
mf=$TOPIC_DIR/menutest.topic; printf 'id=menutest\ntitle=Menu\nstatus=done\nupdated=1\ndeleted=0\n' > "$mf"; menu_build "$mf"
case "${MENU_ITEMS[*]}" in *"start|"*|*"wait|"*|*"slack|"*|*"done|"*) bad "done menu: ${MENU_ITEMS[*]}" ;; *"reopen|"*"next|"*"later|"*"delete|"*) ok "menu: a done topic can be reopened, moved, renamed, deleted" ;; *) bad "done menu: ${MENU_ITEMS[*]}" ;; esac
printf 'id=menutest\ntitle=Menu\nstatus=later\nupdated=1\ndeleted=0\n' > "$mf"; menu_build "$mf"
case "${MENU_ITEMS[*]}" in *"later|"*) bad "later menu offers Park for later" ;; *) ok "menu: a parked topic cannot be parked again" ;; esac
printf 'id=menutest\ntitle=Menu\nstatus=now\nupdated=1\ndeleted=0\ntransfer_to=Mac/w2:p1\ntransfer_to_name=server\n' > "$mf"; menu_build "$mf"
case "${MENU_ITEMS[0]}" in "cancelxfer|Cancel the hand-over to server") ok "menu: a pending hand-over can be cancelled" ;; *) bad "cancel item: ${MENU_ITEMS[0]}" ;; esac
# the actions follow the status: the natural next step comes first, an agent on the topic can be shown, nothing offers the status it has
mt() { printf 'id=menutest\ntitle=Menu\nstatus=%s\nupdated=1\ndeleted=0\n%b' "$1" "${2:-}" > "$mf"; menu_build "$mf"; }
mt review; case "${MENU_ITEMS[0]}" in "now|"*) ok "menu: a topic in review goes back to now first" ;; *) bad "review menu: ${MENU_ITEMS[*]}" ;; esac
mt now; case "${MENU_ITEMS[0]}" in "review|"*) ok "menu: a topic on now can be handed to review" ;; *) bad "now menu: ${MENU_ITEMS[*]}" ;; esac
case "${MENU_ITEMS[*]}" in *"|Back to now"*|*"now|"*) bad "now menu offers now: ${MENU_ITEMS[*]}" ;; *) ok "menu: now does not offer now" ;; esac
mt next; case "${MENU_ITEMS[*]}" in *"next|"*) bad "next menu offers Up next" ;; "now|"*) ok "menu: an up-next topic can be worked on now" ;; *) bad "next menu: ${MENU_ITEMS[*]}" ;; esac
mt waiting 'waiting_for=Alex\n'; case "${MENU_ITEMS[0]}" in "now|The wait is over"*) ok "menu: a waiting topic without an agent: the wait is over → now" ;; *) bad "waiting menu: ${MENU_ITEMS[*]}" ;; esac
mt review 'agent=bhote\nagent_machine=Omarchy\nagent_pane=w1:p1\n'
case "${MENU_ITEMS[0]}" in "jump|Show topic") ok "menu: an agent on the topic can be shown" ;; *) bad "jump item: ${MENU_ITEMS[*]}" ;; esac
case "${MENU_ITEMS[*]}" in *"start|"*) bad "start offered with an agent on the topic" ;; *"transfer|"*) ok "menu: with an agent: transfer, no second start" ;; *) bad "agent menu: ${MENU_ITEMS[*]}" ;; esac
mt review; case "${MENU_ITEMS[*]}" in *"jump|"*|*"transfer|"*) bad "agentless menu: ${MENU_ITEMS[*]}" ;; *"start|"*) ok "menu: without an agent: start, no show or transfer" ;; *) bad "agentless: ${MENU_ITEMS[*]}" ;; esac
# "Review ansehen" (hotkey 1 or its button): the agent is shown and the button goes, the note stays, the topic stays in review
mt review 'review_note=Result is there\nreview_ask=Review ansehen\nreview_reply=@jump\nagent_machine=Omarchy\nagent_pane=w1:p1\n'
jr=$(agent_jump() { echo "jumped $1 $2"; }; review_act "$mf" 1)   # (a subshell: the stub stays out of the tests below)
[ "$jr" = "jumped Omarchy w1:p1" ] && [ -z "$(topic_get "$mf" review_ask)" ] && [ "$(topic_get "$mf" review_note)" = "Result is there" ] \
    && [ "$(topic_get "$mf" status)" = review ] && ok "review: 'Review ansehen' jumps to the agent and the button goes" || bad "review look: '$jr' ask='$(topic_get "$mf" review_ask)'"
rm -f "$mf"
a1=$(COLS=40 menu_num_line "" 9 "Describe"); a2=$(COLS=40 menu_num_line "" 10 "Delete"); [ "${a1#*Describe}" != "$a1" ] && [ "$(printf '%s' "$a1" | sed 's/Describe.*//' | wc -c)" = "$(printf '%s' "$a2" | sed 's/Delete.*//' | wc -c)" ] && ok "menu: the text of 9 and 10 starts in the same column" || bad "menu numbers: [$a1] [$a2]"

# a done topic that is taken up again says so in its description (once, with the new date)
rf=$TOPIC_DIR/reopen.topic; printf 'id=reopen\ntitle=R\nstatus=done\ndescription=Check the export\nupdated=1\ndeleted=0\n' > "$rf"
topic_set "$rf" status next; d1=$(topic_get "$rf" description); case "$d1" in "Reopened "????-??-??". Check the export") ok "reopen: the description says Reopened <date>" ;; *) bad "reopen note: $d1" ;; esac
topic_set "$rf" status done; topic_set "$rf" status later; d2=$(topic_get "$rf" description); [ "$(printf '%s' "$d2" | grep -o 'Reopened' | wc -l | tr -d ' ')" = 1 ] && ok "reopen: the note is not doubled" || bad "reopen doubled: $d2"
topic_set "$rf" status now; [ "$(topic_get "$rf" description)" = "$d2" ] && ok "reopen: other moves leave the description" || bad "reopen: description changed"
rm -f "$rf"

# done topics: the machine code of the agent that did it, only with more than one machine
( HOST=Mac; AG_N=2; AG_LINES=($'Mac\037w1:p1\037idle\037a\037' $'Laptop\037w2:p1\037idle\037b\037'); MC_N=0; T_AM=(Laptop); done_mc 0; case "$DONE_MC" in [A-Z][A-Z][A-Z]) exit 0 ;; *) exit 1 ;; esac ) && ok "done: the machine code stands before the time" || bad "done_mc with two machines"
( HOST=Mac; AG_N=1; AG_LINES=($'Mac\037w1:p1\037idle\037a\037'); MC_N=0; T_AM=(Mac); done_mc 0; [ -z "$DONE_MC" ] ) && ok "done: no code with one machine" || bad "done_mc with one machine: $DONE_MC"

# keys as they are typed: Ctrl+B|T, Option on a Mac, Alt elsewhere
( HERDR_PREFIX=ctrl+b; KEYS_OS=Darwin; key_pretty a prefix+t; key_pretty b ctrl+alt+t; KEYS_OS=Linux; key_pretty c ctrl+alt+t; [ "$a $b $c" = "Ctrl+B|T Ctrl+Option+T Ctrl+Alt+T" ] ) && ok "keys: readable names, Option on a Mac and Alt elsewhere" || bad "key_pretty"

# the token is a secret: bhote config never prints it
B config list | grep -q tok123 && bad "config list prints the token" || ok "config: the token is never printed"
[ "$(B config get CIRCLECI_TOKEN)" = "(set)" ] && ok "config get: (set)" || bad "config get: $(B config get CIRCLECI_TOKEN)"
rm -rf "$T"; exit $fail
