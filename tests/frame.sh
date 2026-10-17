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
rm -rf "$T"; exit $fail
