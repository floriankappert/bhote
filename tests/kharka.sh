#!/usr/bin/env bash
# Tests of STORE=kharka: the topics merged with a kharka daemon. Two topic folders stand for two machines, each with its own
# daemon (a, b); both daemons sync through one hub, all in a throw-away folder. Needs the kharka binary (KHARKA, else kharka
# on the PATH): without it the suite is skipped. Usage: bash tests/kharka.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
unset KHARKA_HUB KHARKA_SYNC KHARKA_DATA KHARKA_NODE KHARKA_CONFIG KHARKA_HUB_SOCKET KHARKA_HUB_DATA
KHARKA=${KHARKA:-$(command -v kharka)}
if [ -z "$KHARKA" ] || [ ! -x "$KHARKA" ]; then echo "skipped: no kharka binary (set KHARKA)"; exit 0; fi
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/a BHOTE_SHARED=$T/shared
REAL_KHARKA=$KHARKA
export KHARKA KHARKA_SOCKET=$T/a.sock
HUB=""; PID_a=""; PID_b=""
hub_start() {
    "$REAL_KHARKA" serve --hub-socket "$T/h.sock" --socket "$T/hub.sock" --data "$T/hub" --config "$T/none.toml" > /dev/null 2> "$T/hub.log" &
    HUB=$!
    local i; for (( i = 0; i < 100; i++ )); do [ -S "$T/h.sock" ] && return 0; sleep 0.05; done
    echo "kharka hub did not start: $(cat "$T/hub.log")"; exit 1
}
hub_stop() { [ -n "$HUB" ] && kill "$HUB" 2>/dev/null; wait "$HUB" 2>/dev/null; HUB=""; rm -f "$T/h.sock"; }
daemon_start() {  # daemon_start a|b: the daemon of that machine, synced through the hub
    "$REAL_KHARKA" daemon --hub "$T/h.sock" --socket "$T/$1.sock" --data "$T/kharka-$1" --node "$1" > /dev/null 2> "$T/daemon-$1.log" &
    printf -v "PID_$1" '%s' "$!"
    local i; for (( i = 0; i < 100; i++ )); do "$REAL_KHARKA" --socket "$T/$1.sock" whoami >/dev/null 2>&1 && return 0; sleep 0.05; done
    echo "kharka daemon $1 did not start: $(cat "$T/daemon-$1.log")"; exit 1
}
daemon_stop() { local p; p="PID_$1"; [ -n "${!p}" ] && kill "${!p}" 2>/dev/null; wait "${!p}" 2>/dev/null; printf -v "PID_$1" '%s' ""; }
settle() {  # until both daemons are online, have nothing pending and the same last sequence number (each has the other's writes)
    local i a b
    for (( i = 0; i < 200; i++ )); do
        a=$("$REAL_KHARKA" --socket "$T/a.sock" status 2>/dev/null | jq -r '"\(.online) \(.pending) \(.seq)"')
        b=$("$REAL_KHARKA" --socket "$T/b.sock" status 2>/dev/null | jq -r '"\(.online) \(.pending) \(.seq)"')
        case "$a" in "true 0 "*) [ "$a" = "$b" ] && return 0 ;; esac
        sleep 0.05
    done
    echo "the daemons did not settle: a=$a b=$b"
}
# shellcheck disable=SC1091
. ./bhote
trap 'kharka_follow_stop; daemon_stop a; daemon_stop b; hub_stop; rm -rf "$RUN_DIR" "$T"' EXIT   # (after sourcing: bhote sets its own EXIT trap)
cfg_set STORE kharka
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
A=$TOPIC_DIR; B=$T/b/topics; mkdir -p "$A" "$B"
on() { TOPIC_DIR=$1; KHARKA_SOCKET=$T/$2.sock; }               # on <topic folder> a|b: the machine the next calls run on
mk() {  # mk <dir> <id> <title> <updated> [deleted]
    printf 'id=%s\ntitle=%s\nstatus=now\nwaiting_for=\nwaiting_since=\ncreated=%s\nupdated=%s\ndeleted=%s\n' "$2" "$3" "$4" "$4" "${5:-0}" > "$1/$2.topic"
}
title() { topic_get "$1" title -; }
entry() { "$REAL_KHARKA" get "$1" 2>/dev/null; }
stamp() { "$REAL_KHARKA" ls "$1" --entries 2>/dev/null | jq -r --arg k "$1" 'select(.key == $k) | .hlc'; }
gone() {  # gone <pid>...: none of them runs any more (waits up to 8 s)
    local i p alive
    for (( i = 0; i < 80; i++ )); do
        alive=0; for p in "$@"; do kill -0 "$p" 2>/dev/null && alive=1; done
        [ "$alive" = 0 ] && return 0; sleep 0.1
    done
    return 1
}

# 1) no daemon: written locally, the sync fails, the summary says so and why
on "$A" a; mk "$A" a "Written without a daemon" 1000
store_sync && bad "sync without a daemon should fail" || ok "offline: sync fails, nothing lost"
store_summary | grep -q "OFFLINE.*1 change.*kharka" && ok "offline: summary counts the waiting change" || bad "summary: $(store_summary)"
store_summary | grep -q "no kharka daemon" && ok "offline: summary gives kharka's own complaint" || bad "summary: $(store_summary)"
grep -q '^error=.*no kharka daemon' "$STORE_STATUS" && ! grep -q "$(printf '\033')" "$STORE_STATUS" \
    && ok "offline: the complaint is kept in store.status, one clean line" || bad "store.status: $(cat "$STORE_STATUS")"

# 2) the daemons are there: the topic becomes an entry, its lines a JSON object
hub_start; daemon_start a; daemon_start b
store_sync && ok "online: sync succeeds" || bad "online sync failed"
[ "$(entry bhote.topics/a | jq -r .title)" = "Written without a daemon" ] && ok "push: the topic is the entry bhote.topics/a" || bad "entry: $(entry bhote.topics/a)"
store_summary | grep -q "in sync with kharka" && ok "online: summary says in sync" || bad "summary: $(store_summary)"
store_summary | grep -q "hub offline\|pending\|no hub" && bad "summary: $(store_summary)" || ok "online: nothing about the hub while all is well"

# 3) the other machine gets it through the hub, and records go to their own collections
settle; on "$B" b; store_sync
[ "$(title "$B/a.topic")" = "Written without a daemon" ] && ok "pull: the other machine has the topic" || bad "pull missing"
cmp -s "$A/a.topic" "$B/a.topic" && ok "pull: the file arrives as it was written" || bad "pulled file differs: $(diff "$A/a.topic" "$B/a.topic")"
printf 'id=proj-shop\nkind=project\ntitle=Shop\nrepos=github.com/x/shop\ncreated=1\nupdated=1500\ndeleted=0\n' > "$B/proj-shop.topic"
store_sync
[ "$(KHARKA_SOCKET=$T/b.sock entry bhote.projects/proj-shop | jq -r .title)" = Shop ] && ok "records: a project is bhote.projects/proj-…" || bad "project entry missing"
settle; on "$A" a; store_sync; [ -f "$A/proj-shop.topic" ] && ok "records: the project reaches the first machine" || bad "project not pulled"

# 4) a change on one machine reaches the other; the newer one wins
on "$B" b; topic_set "$B/a.topic" title "Changed on B"; store_sync
settle; on "$A" a; store_sync; [ "$(title "$A/a.topic")" = "Changed on B" ] && ok "change: B's edit reaches A" || bad "A has: $(title "$A/a.topic")"
mk "$A" c "C on A, newer" 4000; mk "$B" c "C on B, older" 3000
on "$A" a; store_sync; settle; on "$B" b; store_sync
[ "$(title "$B/c.topic")" = "C on A, newer" ] && ok "conflict: the newer topic wins on both" || bad "c on B: $(title "$B/c.topic")"
settle; on "$A" a; store_sync; [ "$(title "$A/c.topic")" = "C on A, newer" ] && ok "conflict: the older one does not come back" || bad "c on A: $(title "$A/c.topic")"

# 5) a deletion travels as a tombstone
on "$A" a; topic_set "$A/a.topic" deleted 1; store_sync
settle; on "$B" b; store_sync; [ "$(topic_get "$B/a.topic" deleted 0)" = 1 ] && ok "delete: the tombstone reaches the other machine" || bad "tombstone missing"
topic_files | grep -q "/a.topic" && bad "deleted topic still listed" || ok "delete: the topic leaves the list"

# 6) nothing changed: a second round writes nothing to kharka
settle; before=$("$REAL_KHARKA" ls bhote. --entries | jq -r .hlc | sort | tr '\n' ' ')
on "$A" a; store_sync; on "$B" b; store_sync
after=$("$REAL_KHARKA" ls bhote. --entries | jq -r .hlc | sort | tr '\n' ' ')
[ "$before" = "$after" ] && ok "idempotent: a round without changes writes nothing" || bad "a quiet round wrote to kharka"

# 7) what comes from kharka is checked like any replica: no escape codes, no repeated keys, nothing from the future
export KHARKA_SOCKET=$T/a.sock
"$REAL_KHARKA" put bhote.topics/evil '{"id":"evil","title":"x","updated":"99999999999","deleted":"0"}' >/dev/null
"$REAL_KHARKA" put 'bhote.topics/ctl' '{"id":"ctl","title":"Bad\u001b[2Jtitle\nstatus=done","status":"now","updated":"5000","deleted":"0"}' >/dev/null
"$REAL_KHARKA" put bhote.topics/notobj '"just a string"' >/dev/null
"$REAL_KHARKA" put bhote.other/x '{"id":"x","title":"other collection","updated":"5000"}' >/dev/null
settle; on "$B" b; store_sync 2>/dev/null
[ ! -f "$B/evil.topic" ] && ok "hardening: a topic from the future is not taken" || bad "future topic taken"
[ "$(topic_get "$B/ctl.topic" status)" = now ] && [ "$(grep -c . "$B/ctl.topic")" = 5 ] && ok "hardening: control characters cannot add keys" || bad "ctl: $(cat "$B/ctl.topic")"
[ ! -f "$B/notobj.topic" ] && [ ! -f "$B/x.topic" ] && ok "hardening: values that are no objects and other collections are left alone" || bad "odd entries became files"

# 8) an entry counts only in the collection of its id: bhote.topics/proj-z and bhote.projects/proj-z are not one file
"$REAL_KHARKA" put bhote.projects/proj-z '{"id":"proj-z","kind":"project","title":"Project Z","updated":"5000","deleted":"0"}' >/dev/null
"$REAL_KHARKA" put bhote.topics/proj-z '{"id":"proj-z","kind":"project","title":"In the wrong collection","updated":"6000","deleted":"0"}' >/dev/null
"$REAL_KHARKA" put bhote.projects/plain '{"id":"plain","title":"A topic among the projects","updated":"5000","deleted":"0"}' >/dev/null
settle; on "$B" b; store_sync 2>/dev/null
[ "$(title "$B/proj-z.topic")" = "Project Z" ] && ok "collections: a project comes from bhote.projects only" || bad "proj-z: $(title "$B/proj-z.topic")"
[ ! -f "$B/plain.topic" ] && ok "collections: a topic among the projects is left alone" || bad "plain.topic was taken"

# 9) the round trip: a file that kharka gives back in another form (no final newline, invalid UTF-8, a stray line, a key twice)
# is pushed once, then has kharka's form here too, and is not pushed again on every round
on "$A" a
printf 'id=rt\ntitle=Caf\351 \342\200\224 bytes\nnot a key line\nstatus=now\nstatus=done\ncreated=8000\nupdated=8000\ndeleted=0' > "$A/rt.topic"
store_sync; s1=$(stamp bhote.topics/rt)
[ -n "$s1" ] && [ "$(entry bhote.topics/rt | jq -r .status)" = now ] && ok "round trip: pushed (the first of two keys counts)" || bad "rt entry: $(entry bhote.topics/rt)"
[ "$(tail -c 1 "$A/rt.topic" | od -An -c | tr -d ' ')" = '\n' ] && ! grep -q 'not a key' "$A/rt.topic" && [ "$(grep -c '^status=' "$A/rt.topic")" = 1 ] \
    && ok "round trip: the local file has kharka's form now" || bad "rt.topic: $(od -c "$A/rt.topic" | head -5)"
grep -q "Caf$(printf '\357\277\275')" "$A/rt.topic" && ok "round trip: invalid UTF-8 is what kharka holds (U+FFFD)" || bad "rt title: $(title "$A/rt.topic")"
store_sync; store_sync
[ "$(stamp bhote.topics/rt)" = "$s1" ] && ok "round trip: not pushed again" || bad "rt pushed again: $s1 -> $(stamp bhote.topics/rt)"
settle; on "$B" b; store_sync; cmp -s "$A/rt.topic" "$B/rt.topic" && ok "round trip: both machines have the same file" || bad "rt differs: $(diff "$A/rt.topic" "$B/rt.topic")"
# the same stamp, only the final newline gone (the larger checksum would win and push on every round before)
on "$A" a; for n in 1 2 3 4 5 6; do mk "$A" nl$n "Newline $n" 8100; done; store_sync
for n in 1 2 3 4 5 6; do printf '%s' "$(cat "$A/nl$n.topic")" > "$A/nl$n.topic"; done
before=$("$REAL_KHARKA" ls bhote.topics/nl --entries | jq -r .hlc | tr '\n' ' ')
store_sync; store_sync
after=$("$REAL_KHARKA" ls bhote.topics/nl --entries | jq -r .hlc | tr '\n' ' ')
[ "$before" = "$after" ] && ok "round trip: a file without its final newline is not pushed" || bad "pushed: $before -> $after"
[ "$(tail -c 1 "$A/nl1.topic" | od -An -c | tr -d ' ')" = '\n' ] && [ "$(tail -c 1 "$A/nl6.topic" | od -An -c | tr -d ' ')" = '\n' ] \
    && ok "round trip: and it has its newline back" || bad "nl: $(od -c "$A/nl1.topic" | tail -2)"

# 10) kharka changes during the merge (exit 4 of put --if-hlc): no "in sync", the merge runs again at once
cat > "$T/racing-kharka" <<'EOF'
#!/usr/bin/env bash
# kharka, but a conditional put meets another machine's write first, as long as $RACES counts
if [ "$1" = put ] && [ -s "$RACES" ] && case " $* " in *" --if-hlc "*) true ;; *) false ;; esac; then
    n=$(cat "$RACES"); if [ "$n" -gt 0 ]; then
        echo $(( n - 1 )) > "$RACES"
        "$REAL_KHARKA" put "$2" '{"id":"race","title":"Another machine","status":"now","created":"1","updated":"9000","deleted":"0"}' >/dev/null
    fi
fi
exec "$REAL_KHARKA" "$@"
EOF
chmod +x "$T/racing-kharka"; export REAL_KHARKA RACES=$T/races
on "$A" a; mk "$A" race "Race, first push" 9000; store_sync
mk "$A" race "Race, changed here" 9500; echo 1 > "$RACES"; rm -f "$STORE_DIRTY"
KHARKA=$T/racing-kharka store_sync; rc=$?
[ "$rc" = 0 ] && [ "$(entry bhote.topics/race | jq -r .title)" = "Race, changed here" ] && ok "exit 4: merged once more at once, the change here is in kharka" || bad "race rc=$rc: $(entry bhote.topics/race)"
store_summary | grep -q "in sync with kharka" && ok "exit 4: in sync after the second round" || bad "summary: $(store_summary)"
mk "$A" race "Race, changed again" 9600; echo 5 > "$RACES"; rm -f "$STORE_DIRTY" "$SHARED_DIR/poke"
KHARKA=$T/racing-kharka store_sync; rc=$?
[ "$rc" = 2 ] && [ -e "$STORE_DIRTY" ] && [ -e "$SHARED_DIR/poke" ] && ok "exit 4 twice: another round is asked for, and the collector woken" || bad "race rc=$rc dirty=$([ -e "$STORE_DIRTY" ] && echo yes)"
store_summary | grep -q "in sync" && bad "summary says in sync: $(store_summary)" || ok "exit 4 twice: the summary does not say in sync ($(store_summary))"
echo 0 > "$RACES"; store_sync; [ "$(entry bhote.topics/race | jq -r .title)" = "Race, changed again" ] && ok "exit 4: the next round brings the change" || bad "race: $(entry bhote.topics/race)"

# 11) an old tombstone goes, here and in kharka; it is not pushed first, and not deleted over a newer write
on "$A" a; mk "$A" old "Long gone" 1000 1; store_sync
[ ! -f "$A/old.topic" ] && [ -z "$(entry bhote.topics/old)" ] && ok "tombstones: one older than 30 days goes, without being pushed" || bad "old tombstone stayed: $(entry bhote.topics/old)"
mk "$A" old2 "Alive once" 1500; store_sync; mk "$A" old2 "Gone long ago" 2000 1
store_sync
[ ! -f "$A/old2.topic" ] && [ -z "$(entry bhote.topics/old2)" ] && ok "tombstones: the entry in kharka is deleted (del --if-hlc)" || bad "old2: $(entry bhote.topics/old2)"
mk "$A" old3 "Alive once" 1500; store_sync; mk "$A" old3 "Gone long ago" 2000 1
cat > "$T/del-racing-kharka" <<'EOF'
#!/usr/bin/env bash
# kharka, but before a conditional del the entry is written again (another machine brings the topic back)
[ "$1" = del ] && "$REAL_KHARKA" put "$2" '{"id":"old3","title":"Brought back","status":"now","created":"1","updated":"3000","deleted":"0"}' >/dev/null
exec "$REAL_KHARKA" "$@"
EOF
chmod +x "$T/del-racing-kharka"
KHARKA=$T/del-racing-kharka store_sync
[ -f "$A/old3.topic" ] && [ "$(entry bhote.topics/old3 | jq -r .title)" = "Brought back" ] && ok "tombstones: an entry written again meanwhile is not deleted" || bad "old3: $(entry bhote.topics/old3)"
store_sync; [ "$(title "$A/old3.topic")" = "Brought back" ] && ok "tombstones: the next round merges it" || bad "old3 here: $(title "$A/old3.topic")"

# 12) the hub goes away: in step with the daemon here, the summary says the hub is away and what waits for it
hub_stop; sleep 0.3
on "$A" a; mk "$A" h "Made while the hub was away" 9700; store_sync
store_summary | grep -q "in sync with kharka.*hub offline, 1 pending" && ok "hub away: summary says hub offline, 1 pending" || bad "summary: $(store_summary)"
U_STORE=kharka; U_STORE_STATE=ok; U_STORE_HUB=x; U_STORE_HUB_ONLINE=false
[ "$(store_tag)" = " · hub offline" ] && ok "hub away: the status line says so" || bad "store_tag: $(store_tag)"
hub_start; settle; store_sync
store_summary | grep -q "hub offline\|pending" && bad "summary: $(store_summary)" || ok "hub back: nothing pending"
on "$B" b; store_sync; [ "$(title "$B/h.topic")" = "Made while the hub was away" ] && ok "hub back: the change reaches the other machine" || bad "h missing on B"

# 13) the follower: kharka watch wakes the collector at once, and nothing of it outlives a stop
on "$A" a; KW_READ=""; KW_WATCH=""
kharka_follow_start $$; sleep 0.5; rm -f "$STORE_DIRTY" "$SHARED_DIR/poke"
kill -0 "$KW_WATCH" 2>/dev/null && kill -0 "$KW_READ" 2>/dev/null && ok "follower: the watch and its reader run" || bad "follower did not start"
"$REAL_KHARKA" --socket "$T/b.sock" put bhote.topics/f '{"id":"f","title":"From B","updated":"9800","deleted":"0"}' >/dev/null
for (( i = 0; i < 40; i++ )); do [ -e "$SHARED_DIR/poke" ] && break; sleep 0.05; done
[ -e "$STORE_DIRTY" ] && [ -e "$SHARED_DIR/poke" ] && ok "follower: another machine's write asks for a merge and pokes the collector" || bad "no dirty/poke after B's write"
w=$KW_WATCH; r=$KW_READ; kharka_follow_stop
gone "$w" "$r" && ok "follower: stopped, the watch and the reader are gone" || bad "follower left $w $r behind"
kharka_follow_start $$; w=$KW_WATCH; r=$KW_READ; sleep 0.3; daemon_stop a
gone "$w" "$r" && ok "follower: ends with the daemon" || bad "follower outlived the daemon"
( me=""; self_pid_set me; kharka_follow_start "$me"; echo "$KW_WATCH $KW_READ" > "$T/kw"; while :; do sleep 1; done ) & sub=$!
daemon_start a; for (( i = 0; i < 40; i++ )); do [ -s "$T/kw" ] && break; sleep 0.05; done; read -r w r < "$T/kw"
kill -9 "$sub"; wait "$sub" 2>/dev/null
gone "$w" "$r" && ok "follower: ends when its collector was killed (kill -9)" || bad "follower outlived its collector: $w $r"
# without a daemon it is not started on every round, but after a growing pause
daemon_stop a; KW_READ=""; KW_WATCH=""; KW_AT=0; KW_WAIT=0; KW_NEXT=0; SECONDS=1000
kharka_follow_keep $$; first=$KW_READ; sleep 0.5
kharka_follow_keep $$; kharka_follow_keep $$
[ -n "$first" ] && [ -z "$KW_READ" ] && [ "$KW_WAIT" = 5 ] && ok "follower: no daemon, the next try waits 5 s" || bad "follower: read=$KW_READ wait=$KW_WAIT"
SECONDS=1006; kharka_follow_keep $$; sleep 0.5; kharka_follow_keep $$
[ "$KW_WAIT" = 10 ] && ok "follower: then 10 s" || bad "follower wait=$KW_WAIT"
daemon_start a; SECONDS=1020; kharka_follow_keep $$; w=$KW_WATCH; r=$KW_READ
cfg_set STORE local; kharka_follow_keep $$
[ -z "$KW_READ" ] && gone "$w" "$r" && ok "follower: STORE switched away, it stops" || bad "follower kept running after STORE=local"
cfg_set STORE kharka

# 14) settings: the kharka binary can be set; "Sync through" is not kharka's
SET_PAGE=connections settings_rows
case " ${SET_ROWS[*]} " in *"KHARKA|"*) ok "settings: a row for the kharka binary" ;; *) bad "rows: ${SET_ROWS[*]}" ;; esac
case " ${SET_ROWS[*]} " in *"SYNC_VIA"*) bad "settings: Sync through shown with kharka" ;; *) ok "settings: no Sync through with kharka" ;; esac
cp "$REAL_KHARKA" "$T/kharka-bin"; cfg_set KHARKA "$T/kharka-bin"
( unset KHARKA; k=""; kharka_bin_set k; [ "$k" = "$T/kharka-bin" ] ) && ok "settings: KHARKA names the binary" || bad "KHARKA setting not used"
cfg_set KHARKA ""

# 15) the daemon goes away and comes back: changes made meanwhile are merged then
daemon_stop a
on "$A" a; mk "$A" d "Made while kharka was down" 9900
store_sync && bad "sync should fail without the daemon" || ok "offline again: saved locally"
daemon_start a; store_sync; settle; on "$B" b; store_sync
[ "$(title "$B/d.topic")" = "Made while kharka was down" ] && ok "back: the change made meanwhile arrives" || bad "d missing on B"

exit $fail
