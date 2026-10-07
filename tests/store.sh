#!/usr/bin/env bash
# Tests of the data location: local-first, offline fallback, merge when the machine is back. No network: a fake ssh runs
# the remote command in a throw-away "remote home". Usage: bash tests/store.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/local BHOTE_SSH=$T/fakessh BHOTE_SHARED=$T/shared
export FAKE_HOME=$T/remote-home; mkdir -p "$FAKE_HOME"
cat > "$T/fakessh" <<'SH'
#!/bin/sh
# fakessh -o X -o Y host command...   (offline when FAKE_OFFLINE=1)
while [ "$1" = "-o" ]; do shift 2; done; shift          # options, then the host
[ -n "$FAKE_OFFLINE" ] && exit 255
cd "$FAKE_HOME" && HOME=$FAKE_HOME exec ${FAKE_LOGIN_SHELL:-sh} -c "$*"      # the login shell of the machine (zsh on some machines)
SH
chmod +x "$T/fakessh"
# shellcheck disable=SC1091
. ./bhote
store_host() { echo fakehost; }                        # stands in for `herdr machine list`
cfg_set STORE remote; cfg_set STORE_MACHINE Fake; cfg_set SYNC_VIA ssh    # the ssh transport (herdr: see the end)
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
mk() {  # mk <dir> <id> <title> <updated> [deleted]
    mkdir -p "$1"; printf 'id=%s\ntitle=%s\nstatus=now\nwaiting_for=\nwaiting_since=\ncreated=%s\nupdated=%s\ndeleted=%s\n' "$2" "$3" "$4" "$4" "${5:-0}" > "$1/$2.topic"
}
R=$FAKE_HOME/$REMOTE_TOPIC_DIR
title() { topic_get "$1" title -; }

# 1) offline: the topic is written locally, the sync fails, the summary says so
mk "$TOPIC_DIR" a "Written while offline" 1000
FAKE_OFFLINE=1 store_sync && bad "offline sync should fail" || ok "offline: sync fails, nothing lost"
[ -f "$TOPIC_DIR/a.topic" ] && ok "offline: topic is still there locally" || bad "offline: topic lost"
store_summary | grep -q "OFFLINE.*1 change" && ok "offline: summary counts the waiting change" || { bad "offline summary: $(store_summary)"; }

# 2) back online: the local topic is pushed
store_sync && ok "online: sync succeeds" || bad "online sync failed"
[ "$(title "$R/a.topic")" = "Written while offline" ] && ok "online: offline topic reached the machine" || bad "push missing"
store_summary | grep -q "in sync with Fake" && ok "online: summary says in sync" || bad "summary: $(store_summary)"

# 3) the machine has a topic we do not know: it is pulled
mk "$R" b "Made on the other machine" 2000
store_sync; [ "$(title "$TOPIC_DIR/b.topic")" = "Made on the other machine" ] && ok "pull: remote-only topic arrives" || bad "pull missing"

# 4) both changed the same topic: the newer one wins, whichever side it is on
mk "$TOPIC_DIR" c "C local old" 3000;  mk "$R" c "C remote new" 4000
mk "$TOPIC_DIR" d "D local new" 6000;  mk "$R" d "D remote old" 5000
store_sync
[ "$(title "$TOPIC_DIR/c.topic")" = "C remote new" ] && ok "conflict: newer remote wins locally" || bad "c: $(title "$TOPIC_DIR/c.topic")"
[ "$(title "$R/d.topic")" = "D local new" ] && ok "conflict: newer local wins remotely" || bad "d: $(title "$R/d.topic")"

# 5) a deletion travels (tombstone) and the topic leaves the list
topic_set "$TOPIC_DIR/a.topic" deleted 1; sleep 1; touch "$TOPIC_DIR/a.topic"
store_sync
[ "$(topic_get "$R/a.topic" deleted 0)" = 1 ] && ok "delete: tombstone reached the machine" || bad "tombstone missing"
topic_files | grep -q "/a.topic" && bad "deleted topic still listed" || ok "delete: topic no longer listed"

# 6) nothing changed: a second sync changes nothing
before=$(cat "$TOPIC_DIR"/*.topic | md5 2>/dev/null || cat "$TOPIC_DIR"/*.topic | md5sum); store_sync
after=$(cat "$TOPIC_DIR"/*.topic | md5 2>/dev/null || cat "$TOPIC_DIR"/*.topic | md5sum)
[ "$before" = "$after" ] && ok "idempotent: second sync changes nothing" || bad "second sync changed data"

# 8) hardening: what the other side sends is never trusted
cat > "$T/evilssh" <<'SH'
#!/bin/sh
case "$*" in *@@*) printf '@@../escape.topic\nid=x\ntitle=pwn\nupdated=99999\n@@bad name;touch pwned.topic\nupdated=99999\n@@good1.topic\nid=good1\ntitle=ok\nstatus=now\nupdated=99999\ndeleted=0\n\n@@END\n' ;; *) exec "$FAKE_SSH_REAL" "$@" ;; esac
SH
chmod +x "$T/evilssh"; cp -R "$TOPIC_DIR" "$T/local-before"
FAKE_SSH_REAL=$T/fakessh BHOTE_SSH=$T/evilssh store_sync
[ -z "$(find "$T" -name 'escape.topic' -o -name 'pwned.topic' -o -name 'bad name*' 2>/dev/null)" ] && ok "hardening: odd file names from the replica never become files" || bad "path traversal / odd name written"
[ -f "$TOPIC_DIR/good1.topic" ] && ok "hardening: a plain topic name is still accepted" || bad "good topic from replica missing"
rm -f "$TOPIC_DIR/good1.topic"
store_ssh "-oProxyCommand=touch$T/hit" true >/dev/null 2>&1; [ ! -e "$T/hit" ] && ok "hardening: a host that looks like an ssh option is refused" || bad "option injection through the host"

# 9) escape codes and newlines inside topics
f=$TOPIC_DIR/ctl.topic; printf 'id=ctl\ntitle=Bad\033[2Jtitle\nstatus=now\nupdated=5\ndeleted=0\n' > "$f"
topics_load; case "${T_TITLE[*]}" in *$'\033'*) bad "escape code reached the screen data" ;; *) ok "hardening: escape codes are stripped from titles" ;; esac
topic_set "$f" title $'x\nstatus=done'; [ "$(topic_get "$f" status)" = now ] && ok "hardening: a newline in a value cannot add a key" || bad "key injection via newline"
rm -f "$f"

# 10) private scratch dir, nothing predictable
[ "$(ls -ld "$RUN_DIR" | cut -c1-10)" = "drwx------" ] && ok "hardening: scratch dir is private (0700)" || bad "scratch dir is open: $(ls -ld "$RUN_DIR")"

# 11) hardening round 2: numbers from topic files never reach $(( )) as code, replica files are checked, dumps must be complete
f=$TOPIC_DIR/arith.topic; printf 'id=arith\ntitle=Arith\nstatus=waiting\nwaiting_for=x\nwaiting_since=a[$(touch %s/PWNED)]\nupdated=b[$(touch %s/PWNED2)]\ndeleted=0\n' "$T" "$T" > "$f"
topics_load; NOW=1000; for (( i = 0; i < T_N; i++ )); do age_text "${T_WSINCE[$i]:-0}" >/dev/null; age_text "${T_UPD[$i]:-0}" >/dev/null; done
age_text 'c[$(touch '"$T"'/PWNED3)]' >/dev/null
[ ! -e "$T/PWNED" ] && [ ! -e "$T/PWNED2" ] && [ ! -e "$T/PWNED3" ] && ok "hardening: a[\$(cmd)] in a timestamp never runs" || bad "arithmetic injection ran a command"
rm -f "$f"
printf 'id=d\ntitle=Shown\ndescription=first\ndescription=second\nupdated=5\n' > "$T/dup.topic"
topic_valid "$T/dup.topic" 1000 && bad "a replica file with a repeated key was accepted" || ok "hardening: a replica file with a repeated key is refused"
printf 'id=d\ntitle=x\nupdated=a[1]\n' > "$T/num.topic"; topic_valid "$T/num.topic" 1000 && bad "non-numeric updated accepted" || ok "hardening: a non-numeric timestamp in a replica file is refused"
printf 'id=d\ntitle=x\nupdated=99999999999\n' > "$T/fut.topic"; topic_valid "$T/fut.topic" 1000 && bad "a timestamp from the future accepted" || ok "hardening: a timestamp far in the future is refused"
printf 'id=d\ntitle=x\nstatus=now\nupdated=900\ndeleted=0\n' > "$T/okv.topic"; topic_valid "$T/okv.topic" 1000 && ok "hardening: a normal replica file is accepted" || bad "a normal file was refused"
f=$TOPIC_DIR/first.topic; printf 'id=first\ntitle=First\ndescription=hidden\ndescription=shown\nstatus=now\nupdated=5\ndeleted=0\n' > "$f"
topics_load; for (( i = 0; i < T_N; i++ )); do [ "${T_ID[$i]}" = first ] && d=${T_DESC[$i]}; done
[ "$d" = "$(topic_get "$f" description)" ] && ok "hardening: the list and the agent prompt read the same value of a repeated key" || bad "list shows '$d', prompt gets '$(topic_get "$f" description)'"
rm -f "$f"
# a dump that ends early (ssh died in the middle) changes nothing
mk "$R" half "Half" 99990; before=$(ls "$TOPIC_DIR" | sort)
cat > "$T/halfssh" <<'SH'
#!/bin/sh
case "$*" in *@@*) printf '\n@@half.topic\nid=half\ntitle=Half\nupdated=99990\n' ;; *) exec "$FAKE_SSH_REAL" "$@" ;; esac
SH
chmod +x "$T/halfssh"; FAKE_SSH_REAL=$T/fakessh BHOTE_SSH=$T/halfssh store_sync; rc=$?
[ "$rc" = 1 ] && [ "$before" = "$(ls "$TOPIC_DIR" | sort)" ] && ok "hardening: an incomplete dump is not merged (offline)" || bad "incomplete dump: rc=$rc"
rm -f "$R/half.topic"
# the sync lock: busy -> 2; a lock of a dead process is taken over
sleep 30 & held=$!; ln -s "$held" "$DATA_DIR/sync.lock"
store_sync; [ $? = 2 ] && ok "hardening: a running sync elsewhere returns 2 (busy)" || bad "busy lock not reported"
kill "$held"; wait "$held" 2>/dev/null
store_sync; [ $? = 0 ] && [ ! -e "$DATA_DIR/sync.lock" ] && [ ! -L "$DATA_DIR/sync.lock" ] && ok "hardening: the lock of a dead process is taken over and released" || bad "stale lock not taken over"
# config: several keys in one write, labels with regex characters
mkdir -p "$DATA_DIR/sync.lock"; echo 1 > "$DATA_DIR/sync.lock/pid"   # an old directory lock (older version) is replaced
store_sync; [ $? = 0 ] && [ ! -e "$DATA_DIR/sync.lock" ] && ok "hardening: an old directory lock is replaced" || bad "old dir lock blocks"
cfg_set A 1 B 2; [ "$(cfg_get A x)$(cfg_get B x)" = 12 ] && ok "hardening: cfg_set writes several keys at once" || bad "cfg_set multi"
cfg_set 'A.*' z; [ "$(cfg_get A x)" = 1 ] && ok "hardening: a key with regex characters does not delete other keys" || bad "regex key removed others"

# 12) the herdr transport: the other machine's plugin dumps its topics, herdr hands the output over; nothing is pushed
mkdir -p "$T/hbin" "$T/hremote"; mk "$T/hremote" hz "From the herdr side" 99990
cat > "$T/hbin/herdr" <<SH
#!/bin/sh
case "\$*" in
  *"machine list"*) printf 'id1\tFake\tfakehost\tdefault\tenabled\n' ;;
  *"plugin action invoke dump"*) echo invoked >> "$T/hlog" ;;
  *"plugin log list"*) n=\$(grep -c invoked "$T/hlog" 2>/dev/null); n=\${n:-0}   # an old dump (log 1, stale) and one per invoke
      TOPIC_DIR="$T/hremote" dump=\$(BHOTE_DATA="$T/hremote-data" BHOTE_SOURCE_ONLY=1 bash -c '. ./bhote; TOPIC_DIR="$T/hremote"; topics_dump')
      printf '%s' "\$dump" | python3 -c 'import json,sys; n=int(sys.argv[1]); d=sys.stdin.read()
logs=[{"log_id":"plugin-log-1","action_id":"dump","status":"succeeded","stdout":"\\n@@hz.topic\\nid=hz\\ntitle=Stale\\nupdated=99999\\n\\n@@END\\n"}]
logs+=[{"log_id":"plugin-log-%d"%(i+2),"action_id":"dump","status":"succeeded","stdout":d} for i in range(n)]
print(json.dumps({"result":{"logs":logs}}))' "\$n" ;;
esac
SH
chmod +x "$T/hbin/herdr"
store_host() { herdr machine list 2>/dev/null | awk -F'\t' -v l="$1" '$2 == l { print $3; exit }'; }   # the real one again
cfg_set SYNC_VIA herdr; PATH="$T/hbin:$PATH" store_sync; rc=$?
[ "$rc" = 0 ] && [ "$(title "$TOPIC_DIR/hz.topic")" = "From the herdr side" ] && ok "herdr transport: topics come over through the other side's plugin" || bad "herdr pull failed (rc $rc)"
mk "$TOPIC_DIR" onlyhere "Only local" 99995; PATH="$T/hbin:$PATH" store_sync
[ ! -f "$T/hremote/onlyhere.topic" ] && ok "herdr transport: nothing is written to the other side (it pulls itself)" || bad "herdr transport pushed"
[ "$(grep -c invoked "$T/hlog")" -ge 1 ] && ok "herdr transport: no ssh, the dump runs as a plugin action" || bad "dump action not invoked"
store_host() { echo fakehost; }; cfg_set SYNC_VIA ssh

# 7) data location local: no ssh at all
cfg_set STORE local; FAKE_OFFLINE=1 store_sync && ok "local mode: no connection needed" || bad "local mode tried to connect"
rm -rf "$T"; exit $fail
