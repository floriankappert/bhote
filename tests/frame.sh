#!/usr/bin/env bash
# The blocks of the frame that the daemon draws (bhoted block <name>) against the bash functions: the same bytes, with and without colour, at
# three widths, with the settings that change the picture. Skipped without the binary (cargo build --release in daemon/). Usage: bash tests/frame.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID NO_COLOR
T=$(mktemp -d); export BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared
mkdir -p "$T/data/topics" "$T/shared"
# shellcheck disable=SC1091
. ./bhote
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
BD=""; bhoted_bin_set BD
[ -n "$BD" ] || { echo "skipped: no bhoted binary"; rm -rf "$T"; exit 0; }
NOW=$(date +%s); US=$'\037'
proj() { printf 'id=proj-%s\nkind=project\ntitle=%s\nci=%s\ndeleted=0\nupdated=1\n' "$1" "$2" "$3" > "$T/data/topics/proj-$1.topic"; }
proj ims IMS circleci:gh/acme/ims; proj bilendo Bilendo circleci:gh/acme/bilendo; proj bm "Bilendo Marketing" circleci:gh/acme/bm; proj cli "Bhote CLI" github:me/bhote
proj plain Plain ""; proj rails Rails github:acme/rails; proj ab "Alpha Beta" github:acme/ab; proj om "Omega 9" github:acme/om; proj ima "Ima Zed" github:acme/ima; proj dup "Imaginary Zoo" github:acme/dup
line() { printf '%s\n' "$*" | sed "s/ ${US} /${US}/g"; }
mk() { printf '%s\n' "$(( NOW - $1 ))$US$2$US$3$US$4$US$5$US$6$US$7$US$8"; }
{ mk 5 IMS Backend running 3 7 "feat/cancel (#12)" https://ci/1; mk 70 IMS Frontend running "" "" "Add credits (#13)" https://ci/2; mk 130 Bilendo Staging queued "" "" "release 42" https://ci/3
  mk 200 "Bilendo Marketing" Production approval "" "" "Merge pull request #5 from acme/feat/x" https://ci/4; mk 3700 IMS Backend ok 9 9 "fix/x (#10)" https://ci/5; mk 4000 IMS Portal failed 2 5 "oops" https://ci/6
  mk 90000 "Bhote CLI" main canceled "" "" "main" https://ci/7; mk 100000 "Alpha Beta" web ok 1 1 "ünïcode — wide 日本語 title" https://ci/8; mk 100500 Plain none ok 1 1 "n" ""; } > "$T/shared/deploys.list"
{ mk 8 IMS server running 24 79 "feat/calc|~|2031 passed" https://ci/1; mk 300 IMS client ok 3 3 "feat/calc|~|48 passed" https://ci/2; mk 600 IMS portal failed 1 2 "wip/x|~|2 failed · 10 passed" https://ci/3
  mk 900 IMS portal ok 2 2 "wip/x|~|12 passed" https://ci/4; mk 1200 Bilendo build canceled 0 0 "main|~|" https://ci/5; mk 5000 Rails test ok 9 9 "old list without a branch" https://ci/6
  mk 7000 "Omega 9" suite failed 0 4 "wip/y|~|4 failed" https://ci/7; mk 7100 "Ima Zed" suite queued "" "" "main|~|" https://ci/8; } > "$T/shared/tests.list"
cfg_clear() { : > "$T/config"; }
run_case() {  # run_case <name> <colour 0|1> <cols> [config lines…]
    local name=$1 col=$2 cols=$3; shift 3
    cfg_clear; printf '%s\n' DEPLOY_MONITOR=on TEST_MONITOR=on "$@" > "$T/config"
    local b r
    b=$( if [ "$col" = 1 ]; then export BHOTE_FORCE_COLOR=1; fi; unset NO_COLOR
         [ "$col" = 0 ] && export NO_COLOR=1
         bash -c '
            BHOTE_SOURCE_ONLY=1; . ./bhote; bhote_info >/dev/null 2>&1; theme_apply
            COLS=$1; NOW=$2; RULE_LINE=$(hline $(( COLS - 5 ))); DASH_LINE=$(dline $(( COLS - 5 )))
            colors_dump "$3"; monitors_block; printf @' _ "$cols" "$NOW" "$T/colors.env" 2>/dev/null )
    r=$( "$BD" block monitors --cols "$cols" --now "$NOW" --config "$T/config" --shared "$T/shared" --topics "$T/data/topics" --colors "$T/colors.env"; printf @ )
    if [ "$b" = "$r" ] && [ "${#b}" -gt 20 ]; then ok "frame: monitors, $name (colour $col, $cols columns)"
    else bad "frame: monitors, $name (colour $col, $cols columns)"; diff <(printf '%s' "$b" | command cat -v) <(printf '%s' "$r" | command cat -v) | head -8; fi
}
for col in 1 0; do for cols in 44 66 100; do
    run_case "all of it" $col $cols
    run_case "four rows" $col $cols MONITOR_ROWS=2
    run_case "projects off" $col $cols DEPLOY_PROJECTS_OFF=IMS,Rails TEST_PROJECTS_OFF=Bilendo,Omega\ 9
    run_case "everything off" $col $cols DEPLOY_PROJECTS_OFF=IMS,Bilendo,Bilendo\ Marketing,Bhote\ CLI,Alpha\ Beta,Plain
    run_case "compact" $col $cols DEPLOYS_COMPACT=on TESTS_COMPACT=on
    run_case "only tests" $col $cols DEPLOY_MONITOR=off
done; done

# ── the cards and lines (topics_block cards | lists): a fixture with every kind of card ─────────────────────────────────────────────
H=$HOST; mkdir -p "$T/shared/testprog"
tp() { local id=$1 upd=$2; shift 2; printf '%s\n' "id=$id" "$@" "updated=$upd" "deleted=0" > "$T/data/topics/$id.topic"; }
tp 1000-1 $((NOW-30)) title="Contract sheet: the credits" status=review agent=a1 agent_machine=$H agent_pane=w1:p1 description="Add the credit lines to the sheet" review_note="The sheet now has the credit lines. Please check the totals in the second table and the rounding of the last row, then tell me whether to go on with the export." review_ask="Looks good¦Fix the rounding¦Stop"
tp 1001-1 $((NOW-90)) title="Short review" status=review agent=a2 agent_machine=$H agent_pane=w1:p2 review_note="Done." review_ask="Ok"
tp 1002-1 $((NOW-20)) title="Annual package change in the contract flow, a very long title that has to be cut somewhere" status=now agent=a3 agent_machine=$H agent_pane=w2:p1 description="Rounding rule for SevDesk" agent_branch=feat/annual-package-change-with-a-long-branch-name
tp 1003-1 $((NOW-200)) title="Idle agent topic" status=now agent=a4 agent_machine=$H agent_pane=w2:p2
tp 1004-1 $((NOW-300)) title="Handing over" status=now agent=a5 agent_machine=$H agent_pane=w2:p3 transfer_to_name=a9
tp 1005-1 $((NOW-400)) title="No agent yet" status=now description="Add the invoice export"
tp 1006-1 $((NOW-500)) title="Tests are running" status=testing agent=a6 agent_machine=$H agent_pane=w3:p1 agent_branch=wip/tests
tp 1007-1 $((NOW-600)) title="Finished work" status=finished agent=a7 agent_machine=$H agent_pane=w3:p2 description="All done here"
tp 1008-1 $((NOW-700)) title="Remote agent" status=now agent=a8 agent_machine=Mac agent_pane=w4:p1
tp 1009-1 $((NOW-800)) title="Waits for a deployment" status=waiting deploy_wait=1 project=proj-ims agent_branch=feat/cancel waiting_for="Deployment" waiting_since=$((NOW-800))
tp 1010-1 $((NOW-800)) title="Deployment under way" status=waiting deploy_wait=1 project=proj-ims waiting_for="Deployment" waiting_since=$((NOW-800)) deploy_url=https://ci/1
tp 1011-1 $((NOW-800)) title="Deployment failed" status=waiting deploy_wait=1 project=proj-ims waiting_for="Deployment" waiting_since=$((NOW-800)) deploy_url=https://ci/6
tp 1012-1 $((NOW-800)) title="Deployment done" status=waiting deploy_wait=1 project=proj-ims waiting_for="Deployment" waiting_since=$((NOW-800)) deploy_url=https://ci/5
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13; do tp 2000-$i $((NOW-1000*i)) title="Next topic number $i with ünïcode 日本" status=next; done
for i in 1 2 3 4 5; do tp 2100-$i $((NOW-2000*i)) title="Waiting for somebody $i" status=waiting waiting_for="Katharina and Florian" waiting_since=$((NOW-3600*i)); done
tp 2101-1 $((NOW-100)) title="Waiting with agent" status=waiting waiting_for=Jakob agent=a1 waiting_since=$((NOW-100))
tp 2102-1 $((NOW-100)) title="Slack watch" status=waiting waiting_for=Anna slack_label="#team" waiting_since=$((NOW-100))
for i in 1 2 3; do tp 2200-$i $((NOW-5000*i)) title="Later topic $i" status=later; done
tp 2201-1 $((NOW-9000)) title="Dismissed card" status=later dismissed=1
tp 2300-1 $((NOW-9*86400)) title="Archived next" status=next
tp 2301-1 $((NOW-500)) title="A done one" status=done
US=$'\037'
printf '%s\n' "$H${US}w1:p1${US}idle${US}a1${US}Credit lines" "$H${US}w1:p2${US}done${US}a2${US}" "$H${US}w2:p1${US}working${US}a3${US}Do the rounding" "$H${US}w2:p2${US}idle${US}a4${US}" \
    "$H${US}w2:p3${US}working${US}a5${US}" "$H${US}w3:p1${US}testing${US}a6${US}" "$H${US}w3:p2${US}idle${US}a7${US}" > "$T/shared/agents.local"
printf 'Mac%sw4:p1%sblocked%sa8%sRemote task\n' "$US" "$US" "$US" "$US" > "$T/shared/agents.remote"
printf '%s\n' "$H${US}w2:p1${US}✳ Editing the rounding module" > "$T/shared/agents.now.local"
printf '%s\n' "$H${US}w1:p1${US}t1${US}a${US}repo${US}proj-ims${US}/x" "Mac${US}w4:p1${US}t2${US}a${US}repo${US}proj-bm${US}/y" > "$T/shared/agents.meta.local"
printf '3 7 server\n9 9 client\n' > "$T/shared/testprog/${H}_w3_p1"
mk2() { printf '%s\n' "$(( NOW - $1 ))$US$2$US$3$US$4$US$5$US$6$US$7$US$8"; }
{ mk2 5 IMS Backend running 3 7 "feat/cancel (#12)" https://ci/1; mk2 3700 IMS Backend ok 9 9 "feat/cancel (#10)" https://ci/5; mk2 4000 IMS Portal failed 2 5 "oops" https://ci/6; } > "$T/shared/deploys.list"
card_case() {  # card_case <name> <colour 0|1> <cols> <view lines…>
    local name=$1 col=$2 cols=$3; shift 3
    local b r
    printf '%s\n' "$@" > "$T/view.in"
    local data=${CASE_DATA:-$T/data}
    for mode in cards lists; do
        b=$( export BHOTE_DATA=$data; [ "$col" = 1 ] && export BHOTE_FORCE_COLOR=1; [ "$col" = 0 ] && export NO_COLOR=1
             bash -c '
                BHOTE_SOURCE_ONLY=1; . ./bhote; bhote_info >/dev/null 2>&1; theme_apply
                COLS=$1; NOW=$2; ROWS=60; RULE_LINE=$(hline $(( COLS - 5 ))); DASH_LINE=$(dline $(( COLS - 5 )))
                ui_load >/dev/null 2>&1; NOW=$2
                while IFS== read -r k v; do [ -n "$k" ] && printf -v "$k" %s "$v"; done < "$4"
                [ "${EXPANDED:-}" = "" ] && EXPANDED=" "
                CUR_PROJ=${CUR_PROJ:-}; U_SCOPE=${U_SCOPE:-all}
                colors_dump "$3"; view_dump "$5"; topics_block "$6"; printf @' _ "$cols" "$NOW" "$T/colors.env" "$T/view.in" "$T/view.env" "$mode" 2>/dev/null )
        r=$( "$BD" block "$mode" --cols "$cols" --now "$NOW" --config "$T/config" --shared "$T/shared" --topics "$data/topics" --colors "$T/colors.env" --view "$T/view.env"; printf @ )
        if [ "$b" = "$r" ] && { [ "${#b}" -gt 20 ] || [ -n "${CASE_DATA:-}" ]; }; then ok "frame: $mode, $name (colour $col, $cols columns)"
        else bad "frame: $mode, $name (colour $col, $cols columns)"; diff <(printf '%s' "$b" | command cat -v) <(printf '%s' "$r" | command cat -v) | head -10; fi
    done
}
: > "$T/config"
for col in 1 0; do for cols in 44 66 100; do
    card_case "plain" $col $cols "SEL_ID=1002-1" "EXPANDED= "
    card_case "selected review" $col $cols "SEL_ID=1000-1" "EXPANDED= "
    card_case "hover" $col $cols "SEL_ID=1005-1" "HOVER_ID=1002-1"
    card_case "line selected, groups open" $col $cols "SEL_ID=2000-3" "EXPANDED= next waiting later "
    card_case "more row selected" $col $cols "SEL_ID=more:next" "EXPANDED= "
    card_case "finished selected" $col $cols "SEL_ID=1007-1" "SPIN_CH=✢"
    card_case "hover a line" $col $cols "SEL_ID=1002-1" "HOVER_ID=2100-2"
    card_case "hover the selected" $col $cols "SEL_ID=1002-1" "HOVER_ID=1002-1"
    card_case "three lines a group" $col $cols "SEL_ID=2000-2" "LINE_MAX=3" "EXPANDED= later "
    card_case "no archive" $col $cols "SEL_ID=1002-1" "U_ARCHIVE_DAYS=0"
    card_case "project scope" $col $cols "SEL_ID=1002-1" "U_SCOPE=project" "CUR_PROJ=proj-ims"
    card_case "no branches" $col $cols "SEL_ID=1002-1" "U_GITHUB=off"
    mkdir -p "$T/empty/topics"; CASE_DATA=$T/empty card_case "empty: the bored agents" $col $cols "SEL_ID=x"
    CASE_DATA=$T/empty card_case "empty, small" $col $cols "SEL_ID=x" "BORED_SMALL=1"
done; done
rm -rf "$T"; exit $fail

