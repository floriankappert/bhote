#!/usr/bin/env bash
# Tests of the Slack pins: links are read without Claude, #channel and @person through it, and the watch sets a waiting topic
# to review when someone else writes. A fake claude answers (BHOTE_CLAUDE); no network. Usage: bash tests/slack.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
T=$(mktemp -d); export NO_COLOR=1 BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data TMPDIR=$T BHOTE_SHARED=$T/shared
export BHOTE_CLAUDE=$T/claude FAKE_ANSWER=$T/answer FAKE_LOG=$T/claude.log
echo NOTIFY=off > "$T/config"
# the fake: logs its prompt and what it was started with, answers like `claude -p --output-format json`
cat > "$T/claude" <<'SH'
#!/bin/sh
prompt=""; while [ $# -gt 0 ]; do [ "$1" = -p ] && { prompt=$2; shift; }; shift; done
{ printf 'PROMPT %s\n' "$prompt"; printf 'ENV herdr=%s pane=%s cc=%s cwd=%s\n' "${HERDR_ENV:-}" "${HERDR_PANE_ID:-}" "${CLAUDECODE:-}" "$PWD"; } >> "$FAKE_LOG"
case "$prompt" in *"slack_read_user_profile once"*) echo '{"is_error":false,"result":"{\"id\":\"U1\",\"name\":\"Flo\"}"}'; exit 0 ;; esac
[ -s "$FAKE_ANSWER" ] || { echo '{"is_error":true,"result":"no connector"}'; exit 1; }
jq -n --rawfile r "$FAKE_ANSWER" '{is_error: false, result: $r}'
SH
chmod +x "$T/claude"
B() { bash ./bhote "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
# shellcheck disable=SC1091
BHOTE_SOURCE_ONLY=1 . ./bhote

# links: channel, message (its thread), reply in a thread, DM; names need Claude
slack_pin_parse "https://acme.slack.com/archives/C0123ABC" && [ "$SP_CH" = C0123ABC ] && [ -z "$SP_TS" ] && [ "$SP_LABEL" = channel ] && ok "link: a channel" || bad "channel link: $SP_CH $SP_TS"
slack_pin_parse "https://acme.slack.com/archives/C0123ABC/p1712345678123456" && [ "$SP_TS" = 1712345678.123456 ] && [ "$SP_LABEL" = thread ] && ok "link: a message is its thread" || bad "message link: $SP_TS"
slack_pin_parse "https://acme.slack.com/archives/C0123ABC/p1712345999000100?thread_ts=1712345678.123456&cid=C0123ABC" && [ "$SP_TS" = 1712345678.123456 ] && ok "link: a reply points to its thread" || bad "reply link: $SP_TS"
slack_pin_parse "https://acme.slack.com/archives/D0456DEF" && [ "$SP_LABEL" = DM ] && ok "link: a DM" || bad "dm link"
slack_pin_parse "#sales-ops" && [ "$SP_NAME" = "#sales-ops" ] && [ -z "$SP_CH" ] && ok "name: #channel is looked up" || bad "#name"
slack_pin_parse "hello" && bad "plain text accepted" || ok "plain text is refused"
slack_pin_parse 'https://acme.slack.com/archives/C01;rm' && bad "odd channel id accepted" || ok "an odd channel id is refused"

# add/wait with a pin; the JSON carries it
out=$(B add "Contract review" -w Legal --slack "https://acme.slack.com/archives/C0123ABC" --json)
[ "$(echo "$out" | jq -r .slack.channel)" = C0123ABC ] && [ "$(echo "$out" | jq -r .status)" = waiting ] && ok "add --slack: pinned and waiting" || bad "add --slack: $out"
id=$(echo "$out" | jq -r .id); f="$TOPIC_DIR/$id.topic"
B show "$id" | grep -q "slack: channel (watched)" && ok "show: the pin, watched while it waits" || bad "show: $(B show "$id")"
echo '{"id":"C0999XYZ"}' > "$FAKE_ANSWER"
B add "Price list" >/dev/null; B slack "Price list" "#sales-ops" >/dev/null 2>&1
[ "$(B show "Price list" --json | jq -r .slack.channel)" = C0999XYZ ] && ok "bhote slack #channel: looked up through Claude" || bad "slack #channel: $(B show "Price list" --json)"
grep -q "ENV herdr= pane= cc= cwd=$BHOTE_DATA/slack" "$FAKE_LOG" && ok "claude runs outside herdr and any session, in its own folder" || bad "claude env: $(grep ENV "$FAKE_LOG" | tail -1)"
echo '{"id":null}' > "$FAKE_ANSWER"; B slack "Price list" "@nobody" >/dev/null 2>&1 && bad "an unknown person was pinned" || ok "an unknown #channel/@person is refused"
B slack "Price list" off >/dev/null; [ "$(B show "Price list" --json | jq -r .slack)" = null ] && ok "bhote slack off: unpinned" || bad "unpin"

# the watch: a message from someone else → review; my own only moves on; older ones are ignored
seen=$(topic_get "$f" slack_seen); new=$(( ${seen%.*} + 60 )).000100; mine=$(( ${seen%.*} + 30 )).000100
printf '{"me":"U1","pins":[{"topic":"%s","error":null,"messages":[{"ts":"%s","user":"U1","name":"Flo","text":"ping"}]}]}' "$id" "$mine" > "$FAKE_ANSWER"
: > "$FAKE_LOG"; slack_check
[ "$(topic_get "$f" status)" = waiting ] && [ "$(topic_get "$f" slack_seen)" = "$mine" ] && ok "watch: my own message only moves on" || bad "own message: $(topic_get "$f" status) $(topic_get "$f" slack_seen)"
[ "$(cat "$BHOTE_DATA/slack/me")" = U1 ] && ok "watch: who I am is asked once and kept" || bad "me: $(cat "$BHOTE_DATA/slack/me" 2>&1)"
grep -q "C0123ABC" "$FAKE_LOG" && ! grep -q "C0999XYZ" "$FAKE_LOG" && ok "watch: only pinned topics that wait are asked" || bad "pins in prompt: $(cat "$FAKE_LOG")"
printf '{"me":"U1","pins":[{"topic":"%s","messages":[{"ts":"%s","user":"U2","name":"Alex","text":"Hi <@U1|Flo>,  Clause 7 is *fine* <https://x.io|see>\\nfrom our side"},{"ts":"%s","user":"U9","name":"Old","text":"old"}]}]}' "$id" "$new" "$mine" > "$FAKE_ANSWER"
: > "$FAKE_LOG"; slack_check
grep -q "slack_read_user_profile once" "$FAKE_LOG" && bad "asked for me again" || ok "watch: the kept user is reused"
[ "$(topic_get "$f" status)" = review ] && [ "$(topic_get "$f" slack_msg)" = "Alex: Hi @Flo, Clause 7 is fine see from our side" ] && [ "$(topic_get "$f" slack_seen)" = "$new" ] \
  && ok "watch: someone else wrote → review, with who and what" || bad "review: $(topic_get "$f" status) '$(topic_get "$f" slack_msg)'"
B show "$id" | grep -q "slack message: Alex: Hi @Flo, Clause 7" && ok "show: the Slack message" || bad "show message"
frame=$(BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=40 bash ./bhote </dev/null)
echo "$frame" | grep -q "slack · Alex: Hi @Flo" && ok "panel: the answer under the topic" || bad "panel review line"
# waits again: the message goes, from now on; the same message again changes nothing
B wait "$id" Legal >/dev/null; [ "$(topic_get "$f" status)" = waiting ] && [ -z "$(topic_get "$f" slack_msg)" ] && ok "wait again: the message goes, the pin stays" || bad "rewait"
frame=$(BHOTE_ONCE=1 BHOTE_VIEW=main BHOTE_COLS=60 BHOTE_ROWS=40 bash ./bhote </dev/null)
echo "$frame" | grep -q "Legal · slack channel" && ok "panel: a waiting topic shows its pin" || bad "panel waiting line: $(echo "$frame" | grep -A2 Contract)"
slack_check; [ "$(topic_get "$f" status)" = waiting ] && ok "watch: an old message does not come back" || bad "old message re-triggered"
# Claude fails: nothing changes, the state says so
: > "$FAKE_ANSWER"; before=$(cat "$f"); slack_check
[ "$(cat "$f")" = "$before" ] && grep -q "^error" "$SHARED_DIR/slack.status" && ok "watch: a failing call changes nothing and is noted" || bad "failure handling"
# garbage from Claude: an odd topic id and a ts that is no number are dropped
printf '{"me":"U1","pins":[{"topic":"../x","messages":[{"ts":"9999999999.1","user":"U2"}]},{"topic":"%s","messages":[{"ts":"soon","user":"U2"}]}]}' "$id" > "$FAKE_ANSWER"
slack_check; [ "$(topic_get "$f" status)" = waiting ] && [ ! -e "$TOPIC_DIR/../x.topic" ] && ok "watch: odd answers are dropped" || bad "garbage accepted"
rm -rf "$T"; exit $fail
