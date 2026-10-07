#!/usr/bin/env bash
# Tests of the data location: local-first, offline fallback, merge when the machine is back. No network: a fake ssh runs
# the remote command in a throw-away "remote home". Usage: bash tests/store.sh
cd "$(dirname "$0")/.." || exit 1
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
cfg_set STORE remote; cfg_set STORE_MACHINE Fake
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
case "$*" in *@@*) printf '@@../escape.topic\nid=x\ntitle=pwn\nupdated=99999\n@@bad name;touch pwned.topic\nupdated=99999\n@@good1.topic\nid=good1\ntitle=ok\nstatus=now\nupdated=99999\ndeleted=0\n' ;; *) exec "$FAKE_SSH_REAL" "$@" ;; esac
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

# 7) data location local: no ssh at all
cfg_set STORE local; FAKE_OFFLINE=1 store_sync && ok "local mode: no connection needed" || bad "local mode tried to connect"
rm -rf "$T"; exit $fail
