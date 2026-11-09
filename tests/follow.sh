#!/usr/bin/env bash
# The follow-ups of the collector in Rust (bhoted follow --shadow) against the bash functions: the same fixture, the same state, and
# the same decisions (every topic_set, topic_new, notify and new record is logged by the script when BHOTE_INTENT_LOG is set).
# Skipped without the binary (cargo build --release in daemon/). Usage: bash tests/follow.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T
mkdir -p "$T/bin"
cat > "$T/bin/herdr" <<'SH'
#!/bin/sh
case "$*" in
  "pane list") echo '{"result":{"panes":[]}}' ;;
  "pane read P13 "*) printf 'Working on it\nShould I continue with the migration?\n──────────\n❯ \n──────────\n  ⏵⏵ auto mode on\n' ;;
  "pane read "*) printf 'All done.\n──────────\n❯ \n──────────\n  ⏵⏵ auto mode on\n' ;;
esac
SH
chmod +x "$T/bin/herdr"; export PATH="$T/bin:$PATH"
# shellcheck disable=SC1091
. ./bhote
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
BD=""; bhoted_bin_set BD
[ -n "$BD" ] || { echo "skipped: no bhoted binary"; rm -rf "$T"; exit 0; }
N=$(date +%s); H=Linux
tp() { local d=$1 id=$2; shift 2; printf '%s\n' "id=$id" "$@" "updated=${UPD:-$N}" "deleted=0" > "$d/topics/$id.topic"; }
fixture() {  # fixture <dir>: topics, agents and state files, deterministic
    local d=$1 US=$'\037'; rm -rf "$d"; mkdir -p "$d/topics" "$d/shared/shadow" "$d/shared/revidle" "$d/shared/nowidle" "$d/shared/agentgone" "$d/shared/qidle" "$d/shared/cards" "$d/shared/shadow/revidle" "$d/shared/shadow/nowidle" "$d/shared/shadow/agentgone" "$d/shared/shadow/qidle" "$d/shared/shadow/cards" "$d/run/cards" "$d/repo/.git"
    echo "ref: refs/heads/feat/x" > "$d/repo/.git/HEAD"
    printf 'NOTIFY=off\nWAIT_REMIND=12\nFINISHED_MINUTES=30\nTEST_STATUS_DIR=%s\n' "$d/ts" > "$d/config"; mkdir -p "$d/ts" "$d/wt40" "$d/wt41" "$d/repo2/.git"
    UPD=$N tp "$d" w1 title=Wait\ one status=waiting waiting_for=Anna waiting_since=$(( N - 13 * 3600 ))
    UPD=$N tp "$d" w2 title=Wait\ two status=waiting waiting_for=Bob waiting_since=$(( N - 3600 ))
    UPD=$N tp "$d" w3 title=Deploy\ wait status=waiting deploy_wait=1 waiting_since=$(( N - 20 * 3600 ))
    UPD=$(( N - 3600 )) tp "$d" fe1 title=Auto\ card status=finished auto_now=1 agent_pane=P5 agent_machine=$H agent=a5
    UPD=$(( N - 3600 )) tp "$d" fe2 title=Manual status=finished agent_pane=P6 agent_machine=$H agent=a6
    UPD=$(( N - 60 )) tp "$d" fe3 title=Fresh status=finished agent_pane=P6b agent_machine=$H
    UPD=$N tp "$d" 1000-1 title=Dedup\ old status=now agent_pane=P1 agent_machine=$H
    UPD=$N tp "$d" 2000-1 title=Dedup\ new status=now agent_pane=P1 agent_machine=$H
    UPD=$N tp "$d" 3000-1 title=Handing\ over status=testing agent_pane=P1 agent_machine=$H transfer_to_name=x
    UPD=$N tp "$d" ca1 title=Old\ title status=finished auto_now=1 agent_pane=P2 agent_machine=$H
    UPD=$N tp "$d" ca2 title=Idle\ card status=now auto_now=1 agent_pane=P3 agent_machine=$H
    UPD=$N tp "$d" ca3 title=Owned\ card status=now auto_now=1 agent_pane=P4 agent_machine=$H
    UPD=$N tp "$d" ca3b title=Owner status=now agent_pane=P4 agent_machine=$H
    UPD=$N tp "$d" ca4 title=Moved status=review auto_now=1 agent_pane=P16 agent_machine=$H
    UPD=$N tp "$d" rf1 title=Review\ one status=review agent_pane=P10 agent_machine=$H
    UPD=$N tp "$d" rf2 title=Dismissed status=later dismissed=1 agent_pane=P11 agent_machine=$H
    UPD=$N tp "$d" rf3 title=Auto\ review status=review auto=1 agent_pane=P17 agent_machine=$H
    UPD=$(( N - 200 )) tp "$d" ni1 title=Stopped status=now agent_pane=P12 agent_machine=$H delegated=$(( N - 200 )) agent=a12
    UPD=$(( N - 200 )) tp "$d" ni2 title=Asks status=now agent_pane=P13 agent_machine=$H delegated=$(( N - 200 )) agent=a13
    UPD=$N tp "$d" ag1 title=Gone status=now agent_pane=P20 agent_machine=$H
    UPD=$N tp "$d" ag2 title=Gone\ auto status=review auto=1 agent_pane=P21 agent_machine=$H
    UPD=$N tp "$d" aq1 title=Question status=review auto=1 agent_pane=P14 agent_machine=$H
    UPD=$N tp "$d" bs1 title=Branch status=now agent_pane=P15 agent_machine=$H agent_branch=old
    printf '%s\n' "$H${US}P1${US}working${US}a1${US}task1" "$H${US}P2${US}working${US}a2${US}Task two" "$H${US}P3${US}idle${US}a3${US}" "$H${US}P4${US}working${US}a4${US}" \
        "$H${US}P7${US}working${US}a7${US}Nothing yet" "$H${US}P8${US}working${US}a8${US}Just started" "$H${US}P10${US}working${US}a10${US}" "$H${US}P11${US}working${US}a11${US}" "$H${US}P17${US}working${US}a17${US}" \
        "$H${US}P12${US}idle${US}a12${US}" "$H${US}P13${US}idle${US}a13${US}" "$H${US}P14${US}idle${US}a14${US}" "$H${US}P15${US}idle${US}a15${US}" "$H${US}P16${US}idle${US}a16${US}" > "$d/shared/agents.local"
    printf 'Mac%sP1%sidle%sm1%s\n' "$US" "$US" "$US" "$US" > "$d/shared/agents.remote"
    printf '%s%s%s\n' P15 "$US" "$d/repo" > "$d/shared/agents.cwd"
    for s in "" shadow/; do                                          # the state the two sides read: the same files
        for id in rf1 rf2 rf3; do : > "$d/shared/${s}revidle/$id"; done
        printf '%s\n' $(( N - 20 )) > "$d/shared/${s}nowidle/ni1"; printf '%s\n' $(( N - 20 )) > "$d/shared/${s}nowidle/ni2"
        printf '%s\n' $(( N - 40 )) > "$d/shared/${s}agentgone/ag1"; printf '%s\n' $(( N - 40 )) > "$d/shared/${s}agentgone/ag2"
        printf '%s\n' $(( N - 20 )) > "$d/shared/${s}qidle/aq1"
    done
    printf '%s\n' $(( N - 100 )) > "$d/run/cards/P7"; printf '%s\n' $(( N - 100 )) > "$d/shared/shadow/cards/P7"; printf '%s\n' $(( N - 100 )) > "$d/shared/cards/P7"

    # projects, the tests of a worktree, deployments
    printf 'url = git@github.com:acme/ims.git\n' > /dev/null
    printf '[core]\n\tbare = false\n[remote "origin"]\n\turl = git@github.com:Acme/IMS.git\n' > "$d/repo2/.git/config"; echo "ref: refs/heads/main" > "$d/repo2/.git/HEAD"
    printf 'id=proj-ims\nkind=project\ntitle=IMS\nrepos=github.com/acme/ims\nworkspaces=IMS (main)\ndeleted=0\nupdated=1\n' > "$d/topics/proj-ims.topic"
    printf 'id=proj-bm\nkind=project\ntitle=Bilendo Marketing\nrepos=github.com/acme/bm\ndeleted=0\nupdated=1\n' > "$d/topics/proj-bm.topic"
    printf '%s\n' "$H${US}P30${US}t1${US}a${US}${US}${US}$d/repo2" "$H${US}P31${US}t1${US}a${US}github.com/acme/ims${US}proj-ims${US}$d/repo2" "$H${US}P32${US}t2${US}a${US}${US}${US}$d/wt40" "$H${US}P33${US}t2${US}${US}${US}${US}/x" > "$d/shared/agents.meta.local"
    printf '%s\n' "$H${US}P30${US}idle${US}w1${US}" "$H${US}P31${US}idle${US}w1${US}" "$H${US}P32${US}idle${US}IMS (main)${US}" "$H${US}P40${US}working${US}a40${US}" "$H${US}P41${US}idle${US}a41${US}" "$H${US}P42${US}idle${US}a42${US}" "$H${US}P43${US}working${US}a43${US}" >> "$d/shared/agents.local"
    printf '%s\n' "P40${US}$d/wt40" "P41${US}$d/wt41" "P42${US}$d/wt41" "P43${US}$d/wt41" >> "$d/shared/agents.cwd"
    printf '{"tree":"wt40","suite":"server","startedAt":%s,"updatedAt":%s,"total":10,"done":3,"ok":null,"passed":3}\n' $(( N * 1000 - 5000 )) $(( N * 1000 )) > "$d/ts/wt40.server.json"
    UPD=$N tp "$d" tn1 title=Starts\ testing status=now agent_pane=P40 agent_machine=$H agent=a40
    UPD=$N tp "$d" tn2 title=Tests\ over\ working status=testing agent_pane=P43 agent_machine=$H agent=a43
    UPD=$N tp "$d" tn3 title=Tests\ over\ idle status=testing agent_pane=P41 agent_machine=$H agent=a41
    UPD=$N tp "$d" tn4 title=Own\ card status=testing auto_now=1 agent_pane=P42 agent_machine=$H agent=a42
    printf '%s\n' "$(( N - 30 ))${US}IMS${US}Backend${US}running${US}3${US}7${US}feat/ship${US}https://ci/9" "$(( N - 90 ))${US}IMS${US}Frontend${US}ok${US}5${US}5${US}feat/done${US}https://ci/8" > "$d/shared/deploys.list"
    UPD=$N tp "$d" dp1 title=Ships\ feat/ship status=now agent_pane=P44 agent_machine=$H agent=a44 agent_branch=feat/ship project=proj-ims
    mkdir -p "$d/mergerepo"; git -C "$d/mergerepo" init -q . && git -C "$d/mergerepo" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "Merge pull request #77 from acme/feat-q"
    printf '%s\n' "P46${US}$d/mergerepo" >> "$d/shared/agents.cwd"
    printf '%s\n' "$(( N - 20 ))${US}IMS${US}Backend${US}running${US}1${US}3${US}feat-q (#77)${US}https://ci/77" >> "$d/shared/deploys.list"
    UPD=$(( N - 3000 )) tp "$d" dp5 title=Finished\ long\ ago status=finished agent_pane=P46 agent_machine=$H agent=a46 agent_branch=wip/old project=proj-ims
    UPD=$N tp "$d" dq1 title=Same\ work status=now agent_pane=P48 agent_machine=$H agent=a48
    UPD=$N tp "$d" dq2 title=Same\ work status=finished agent_pane=P48 agent_machine=$H agent=a48
    UPD=$N tp "$d" dq4 title=Waiting\ for\ New\ Work status=review agent_pane=P49 agent_machine=$H agent=a49
    UPD=$(( N - 4 * 3600 )) tp "$d" dq5 title=Same\ work status=review agent_pane=P48 agent_machine=$H agent=a48
    UPD=$N tp "$d" dq3 title=Other\ work status=finished agent_pane=P48 agent_machine=$H agent=a48
    UPD=$N tp "$d" dp4 title=Went\ back\ to\ its\ branch status=finished agent_pane=P46 agent_machine=$H agent=a46 agent_branch=wip/own project=proj-ims
    UPD=$N tp "$d" dp3 title=Stopped\ but\ shipping status=finished agent_pane=P45 agent_machine=$H agent=a45 agent_branch=feat/ship project=proj-ims
    UPD=$N tp "$d" dp2 title=Waits\ and\ it\ went\ through status=waiting deploy_wait=1 project=proj-ims deploy_url=https://ci/8 waiting_for=Deployment waiting_since=$(( N - 500 ))
    printf 'id=an-%s-a2\nkind=agentnum\ntitle=%s/a2\nnum=7\ncreated=1\nupdated=1\ndeleted=0\n' "$H" "$H" > "$d/topics/an-${H}-a2.topic"
}
norm() {  # the log, comparable: new ids are NEW, times are T, sorted
    sed -E 's/\|@[0-9]+$//; s/\|(waiting_since|deploy_since|delegated)\|[0-9]+/|\1|T/; s/\|(shadow-new-[0-9]+|[0-9]{9,}-[0-9]+)\|/|NEW|/; s/(created|updated)=[0-9]+/\1=T/g' | sort
}
for fn in remind_waiting finished_expire agent_dedup agent_cards review_follow now_idle_follow agent_gone_follow auto_question_follow branch_sync agentnum_assign project_sync tests_sync deploy_sync; do
    B=$T/b-$fn; R=$T/r-$fn; fixture "$B"; fixture "$R"
    ( export BHOTE_CONFIG=$B/config BHOTE_DATA=$B/data BHOTE_SHARED=$B/shared BHOTE_INTENT_LOG=$B/intent.log; mkdir -p "$B/data"; : > "$BHOTE_INTENT_LOG"
      TOPIC_DIR=$B/topics; DATA_DIR=$B/data; SHARED_DIR=$B/shared; CONFIG_FILE=$B/config; RUN_DIR=$B/run; AGENT_LOCAL=$SHARED_DIR/agents.local; AGENT_REMOTE=$SHARED_DIR/agents.remote; AGENT_CWD=$SHARED_DIR/agents.cwd; AGENT_META_L=$SHARED_DIR/agents.meta.local; AGENT_NOW_L=$SHARED_DIR/agents.now.local
      HOST=$H; STORE_DIRTY=$SHARED_DIR/dirty; AN_AT=-999; $fn ) >/dev/null 2>&1
    mkdir -p "$R/data"
    "$BD" follow "$R/topics" --shared "$R/shared" --data "$R/data" --host "$H" --config "$R/config" --shadow --only "$fn" > "$R/intent.log" 2>/dev/null
    if [ -s "$B/intent.log" ] && diff <(norm < "$B/intent.log") <(norm < "$R/intent.log") >/dev/null; then ok "follow: $fn decides the same ($(wc -l < "$B/intent.log" | tr -d ' ') actions)"
    else bad "follow: $fn differs"; echo "--- bash"; norm < "$B/intent.log" | head -12; echo "--- rust"; norm < "$R/intent.log" | head -12; fi
done

# the same, with the real writes: the topic files that come out (topic_set drops the keys that belong to the old status, stamps `updated`, ...)
snap() {  # snap <dir>: every topic file, times as T, new files as NEW (their ids as they are made)
    local f; for f in "$1"/topics/*.topic; do
        case "${f##*/}" in shadow-*|[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]*-[0-9]*.topic) printf '== NEW\n' ;; *) printf '== %s\n' "${f##*/}" ;; esac
        sed -E 's/^(created|updated|waiting_since|deploy_since|delegated)=[0-9]+/\1=T/; s/^id=[0-9]{9,}-[0-9]+/id=NEW/' "$f"
    done | awk '/^== /{ if (blk != "") print blk; blk = $0; next } { blk = blk "\n" $0 } END { print blk }' | sort
}
for fn in remind_waiting finished_expire agent_dedup agent_cards review_follow now_idle_follow agent_gone_follow auto_question_follow branch_sync agentnum_assign project_sync tests_sync deploy_sync; do
    B=$T/lb-$fn; R=$T/lr-$fn; fixture "$B"; fixture "$R"
    ( export BHOTE_CONFIG=$B/config BHOTE_DATA=$B/data BHOTE_SHARED=$B/shared; mkdir -p "$B/data"
      TOPIC_DIR=$B/topics; DATA_DIR=$B/data; SHARED_DIR=$B/shared; CONFIG_FILE=$B/config; RUN_DIR=$B/run; AGENT_LOCAL=$SHARED_DIR/agents.local; AGENT_REMOTE=$SHARED_DIR/agents.remote; AGENT_CWD=$SHARED_DIR/agents.cwd; AGENT_META_L=$SHARED_DIR/agents.meta.local; AGENT_NOW_L=$SHARED_DIR/agents.now.local
      HOST=$H; STORE_DIRTY=$SHARED_DIR/dirty; AN_AT=-999; $fn ) >/dev/null 2>&1
    mkdir -p "$R/data"; cp -r "$R/shared/shadow/cards" "$R/shared/cards.tmp" 2>/dev/null; rm -rf "$R/shared/cards"; mv "$R/shared/cards.tmp" "$R/shared/cards" 2>/dev/null
    "$BD" follow "$R/topics" --shared "$R/shared" --data "$R/data" --host "$H" --config "$R/config" --only "$fn" >/dev/null 2>&1
    if diff <(snap "$B") <(snap "$R") >/dev/null; then ok "follow (written): $fn leaves the same topics"
    else bad "follow (written): $fn differs"; diff <(snap "$B") <(snap "$R") | head -14; fi
done
[ -n "${KEEP:-}" ] && echo "kept $T" || rm -rf "$T"; exit $fail
