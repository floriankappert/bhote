#!/usr/bin/env bash
# The merge with kharka in Rust (bhoted sync) against store_sync of the script: the same scenario in two throw-away setups (a hub and a
# daemon each), one merged by bash, one by the daemon; the topic folders and the entries in kharka must come out the same.
# Needs the kharka binary (KHARKA, else the PATH) and the daemon binary; skipped without. Usage: bash tests/sync.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID KHARKA_HUB KHARKA_SYNC KHARKA_DATA KHARKA_NODE KHARKA_CONFIG KHARKA_HUB_SOCKET KHARKA_HUB_DATA
KHARKA=${KHARKA:-$(command -v kharka)}
[ -n "$KHARKA" ] && [ -x "$KHARKA" ] || { echo "skipped: no kharka binary (set KHARKA)"; exit 0; }
T=$(mktemp -d); export T NO_COLOR=1 TMPDIR=$T
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
BD=$(BHOTE_SOURCE_ONLY=1 bash -c '. ./bhote; bhoted_bin_set b; printf %s "$b"')
[ -n "$BD" ] || { echo "skipped: no bhoted binary"; rm -rf "$T"; exit 0; }
PIDS=""
trap 'kill $PIDS 2>/dev/null; wait 2>/dev/null; rm -rf "$T"' EXIT
NOW=$(date +%s)
setup() {  # setup <name>: a hub, a daemon and a topic folder with the scenario
    local n=$1 d=$T/$1; mkdir -p "$d/topics" "$d/shared" "$d/data"
    "$KHARKA" serve --hub-socket "$d/h.sock" --socket "$d/hub.sock" --data "$d/hub" --config "$d/none.toml" >/dev/null 2>"$d/hub.log" & PIDS="$PIDS $!"
    local i; for (( i = 0; i < 100; i++ )); do [ -S "$d/h.sock" ] && break; sleep 0.05; done
    "$KHARKA" daemon --hub "$d/h.sock" --socket "$d/a.sock" --data "$d/kh" --node a >/dev/null 2>"$d/daemon.log" & PIDS="$PIDS $!"
    for (( i = 0; i < 100; i++ )); do "$KHARKA" --socket "$d/a.sock" whoami >/dev/null 2>&1 && break; sleep 0.05; done
    rput() { printf '%s' "$2" | "$KHARKA" --socket "$d/a.sock" put "$1" - >/dev/null; }
    lput() { printf '%s\n' "$@" > "$d/topics/${1#id=}.topic"; }
    local old=$(( NOW - 40 * 86400 )) recent=$(( NOW - 86400 ))
    lput id=t1 title=Local\ old updated=100 deleted=0;               rput bhote.topics/t1 '{"id":"t1","title":"Remote newer","updated":"200","deleted":"0"}'
    lput id=t2 title=Local\ newer updated=300 deleted=0;             rput bhote.topics/t2 '{"id":"t2","title":"Remote older","updated":"200","deleted":"0"}'
    lput id=t3 title=Only\ here updated=150 deleted=0
    rput bhote.topics/t4 '{"id":"t4","title":"Only there","status":"next","updated":"50","deleted":"0","description":"x"}'
    lput id=t5 title=Tie\ local updated=250 deleted=0;               rput bhote.topics/t5 '{"id":"t5","title":"Tie remote","updated":"250","deleted":"0"}'
    rput bhote.topics/t6 "{\"id\":\"t6\",\"title\":\"From the future\",\"updated\":\"$(( NOW + 99999 ))\",\"deleted\":\"0\"}"
    lput id=t7 title=Dead\ and\ old updated=$old deleted=1;           rput bhote.topics/t7 "{\"id\":\"t7\",\"title\":\"Dead and old\",\"updated\":\"$old\",\"deleted\":\"1\"}"
    lput id=t8 title=Dead\ old\ here\ only updated=$old deleted=1
    lput id=t9 title=Dead\ recently updated=$recent deleted=1;       rput bhote.topics/t9 "{\"id\":\"t9\",\"title\":\"Alive there\",\"updated\":\"$(( recent - 100 ))\",\"deleted\":\"0\"}"
    lput id=proj-x kind=project title=Project\ local updated=400 deleted=0; rput bhote.projects/proj-x '{"id":"proj-x","kind":"project","title":"Project remote","updated":"300","deleted":"0"}'
    rput bhote.topics/proj-bad '{"id":"proj-bad","title":"Wrong collection","updated":"500","deleted":"0"}'
    rput bhote.topics/t10 '{"id":"t10","title":"Ctrl char","updated":"60","deleted":"0","description":"a\u0001b"}'
    rput bhote.topics/t11 '{"id":"t12","title":"Another id","updated":"60","deleted":"0"}'
    lput id=t13 title=Same\ and\ equal updated=70 deleted=0 description=same; rput bhote.topics/t13 '{"id":"t13","title":"Same and equal","updated":"70","deleted":"0","description":"same"}'
}
state() {  # state <name>: the folder and the entries, comparable
    local d=$T/$1 f; for f in "$d"/topics/*.topic; do [ -e "$f" ] || continue; printf '== %s\n' "${f##*/}"; cat "$f"; done
    printf '== kharka\n'; "$KHARKA" --socket "$d/a.sock" ls bhote. --entries | jq -S -c '[.key, .value]' | sort
}
setup bash; setup rust
( export KHARKA_SOCKET=$T/bash/a.sock BHOTE_SOURCE_ONLY=1 BHOTE_CONFIG=$T/bash/config BHOTE_DATA=$T/bash/data BHOTE_SHARED=$T/bash/shared TMPDIR=$T
  printf 'STORE=kharka\n' > "$T/bash/config"
  bash -c '. ./bhote; TOPIC_DIR=$T/bash/topics; DATA_DIR=$T/bash/data; SHARED_DIR=$T/bash/shared; STORE_STATUS=$DATA_DIR/store.status; STORE_DIRTY=$SHARED_DIR/dirty; KHARKA=$KHARKA; store_sync; echo "bash rc=$?"' ) > "$T/bash.out" 2>&1; tail -1 "$T/bash.out"
KHARKA_SOCKET=$T/rust/a.sock "$BD" sync "$T/rust/topics" --shared "$T/rust/shared" --data "$T/rust/data"; echo "rust rc=$?"
if diff <(state bash) <(state rust) > "$T/state.diff"; then ok "bhoted sync: the topics and the kharka entries come out as store_sync leaves them ($(state rust | grep -c '^== ') files)"
else bad "bhoted sync differs from store_sync"; head -30 "$T/state.diff"; fi
b=$(grep -E '^state=' "$T/bash/data/store.status"); r=$(grep -E '^state=' "$T/rust/data/store.status")
[ "$b" = "$r" ] && [ -n "$b" ] && ok "bhoted sync: the same state in store.status ($b)" || bad "store.status: bash [$b] rust [$r]"
# a second round changes nothing
cp -r "$T/rust/topics" "$T/rust.before"; KHARKA_SOCKET=$T/rust/a.sock "$BD" sync "$T/rust/topics" --shared "$T/rust/shared" --data "$T/rust/data"
diff -r "$T/rust.before" "$T/rust/topics" >/dev/null && ok "bhoted sync: a second merge changes nothing" || bad "a second merge changed the folder"
[ -n "${KEEP:-}" ] && { trap - EXIT; echo "kept $T"; }; exit $fail
