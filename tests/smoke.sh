#!/usr/bin/env bash
# Smoke test: every view must draw (non-empty, framed or ruled, with its own marker text) without errors.
# Runs against a throw-away config and data directory. Usage: bash tests/smoke.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID   # the tests must not run as "an agent"
export NO_COLOR=1 BHOTE_ONCE=1 BHOTE_CONFIG=$(mktemp -d)/config BHOTE_DATA=$(mktemp -d) BHOTE_SHARED=$(mktemp -d)
fail=0
check() {  # check <view> <text that must appear>
    out=$(BHOTE_VIEW=$1 bash ./bhote 2>&1)
    if printf '%s' "$out" | grep -q "command not found\|unbound\|syntax error"; then echo "FAIL $1: shell error"; printf '%s\n' "$out" | head -3; fail=1
    elif printf '%s' "$out" | grep -q "$2"; then echo "ok   $1"
    else echo "FAIL $1: '$2' missing"; fail=1; fi
}
check splash   "PRESS ANY KEY TO START"
check splash   "Herd Management"
check main     "Herd Management"
check settings "Settings"
# no line may reach the panel's right edge, and the frame has exactly ROWS lines (main view)
fit() {  # fit <cols> <rows> <view>
    local t; t=$(mktemp -d); : > "$t/c"
    for i in 1 2 3 4 5 6 7 8 9 10 11 12; do BHOTE_CONFIG=$t/c BHOTE_DATA=$t/d bash ./bhote add "A fairly long topic title number $i" >/dev/null; done
    BHOTE_CONFIG=$t/c BHOTE_DATA=$t/d bash ./bhote add "日本語のとても長いタイトル 🎉 Grüße aus Köln und noch mehr Text" -d "説明 説明 説明 説明 説明 説明 説明 説明" >/dev/null
    BHOTE_CONFIG=$t/c BHOTE_DATA=$t/d BHOTE_ONCE=1 BHOTE_COLS=$1 BHOTE_ROWS=$2 BHOTE_VIEW=$3 LC_ALL=en_US.UTF-8 bash ./bhote \
      | sed 's/\x1b\[[0-9;]*[mK]//g' | python3 -c "
import sys, unicodedata
def w(l): return sum(0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in 'WF' else 1 for c in l)
L=sys.stdin.read().split('\n'); L=L[:-1] if L and L[-1]=='' else L
ok=max(w(l) for l in L)<$1-1 and ('$3'!='main' or len(L)==$2)
sys.exit(0 if ok else 1)" && echo "ok   fit $3 $1x$2" || { echo "FAIL fit $3 $1x$2"; fail=1; }
    rm -rf "$t"
}
for size in "44 40" "36 24" "44 87"; do set -- $size; fit "$1" "$2" main; fit "$1" "$2" settings; done
BHOTE_SOURCE_ONLY= bash ./bhote setup --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert "machines" in d and "plugin" in d' && echo "ok   setup --json" || { echo "FAIL setup --json"; fail=1; }
# the welcome screen at its narrowest: every line inside the frame (a wrapped line shifts the blinking prompt)
w=$(BHOTE_ONCE=1 BHOTE_VIEW=splash BHOTE_COLS=42 BHOTE_ROWS=39 LC_ALL=en_US.UTF-8 bash ./bhote | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | python3 -c 'import sys; print(max(len(l) for l in sys.stdin.read().split("\n")))')
[ "$w" -le 42 ] && echo "ok   splash fits 42 columns" || { echo "FAIL splash is $w wide at 42 columns"; fail=1; }
# nothing in review, now, next or waiting: the bored agents (on top, a done topic below them; low panel: one line only)
b=$(mktemp -d); : > "$b/c"; bv() { BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d BHOTE_ONCE=1 BHOTE_COLS=36 BHOTE_ROWS=$1 BHOTE_VIEW=main LC_ALL=en_US.UTF-8 bash ./bhote; }
bv 30 | grep -q "the agents are bored" && ok_b=1 || ok_b=0
BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d bash ./bhote add "Ship it" >/dev/null; BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d bash ./bhote done 1 >/dev/null
o=$(bv 30); case "$o" in *"zZ"*"the agents are bored"*"Done"*"Ship it"*) ;; *) ok_b=0 ;; esac
o=$(bv 15); case "$o" in *zZ*) ok_b=0 ;; esac                    # (never a cut-off picture)
BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d bash ./bhote add "Real work" -s now >/dev/null; bv 30 | grep -q "bored" && ok_b=0
[ "$ok_b" = 1 ] && echo "ok   main: bored agents when nothing is open, not cut off when low, gone with work" || { echo "FAIL main: bored agents"; fail=1; }
rm -rf "$b"
# the welcome screen counts the topics that are not done
b=$(mktemp -d); : > "$b/c"; for st in done done now later; do BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d bash ./bhote add "T $st" -s $st >/dev/null; done
BHOTE_CONFIG=$b/c BHOTE_DATA=$b/d BHOTE_VIEW=splash BHOTE_COLS=42 BHOTE_ROWS=39 LC_ALL=en_US.UTF-8 bash ./bhote | grep -q "Topics   2 " \
  && echo "ok   splash: topics without the done ones" || { echo "FAIL splash: topic count includes done"; fail=1; }
rm -rf "$b"
# in colour the news link is a terminal hyperlink (OSC 8) with a dotted underline, and the line still fits
s=$(BHOTE_FORCE_COLOR=1 BHOTE_ONCE=1 BHOTE_VIEW=splash BHOTE_COLS=42 BHOTE_ROWS=39 LC_ALL=en_US.UTF-8 bash ./bhote)
case "$s" in *$'\033[4:4m\033]8;;https://github.com/floriankappert/bhote/'*$'\033\\see GitHub\033]8;;\033\\\033[24m'*) echo "ok   splash: 'see GitHub' is a dotted link" ;; *) echo "FAIL splash: no news link"; fail=1 ;; esac
w=$(printf '%s' "$s" | python3 -c 'import re,sys; t=re.sub(r"\x1b\]8;;[^\x1b]*\x1b\\|\x1b\[[0-9;:]*[A-Za-z]", "", sys.stdin.read()); print(max(len(l) for l in t.split("\n")))')
[ "$w" -le 42 ] && echo "ok   splash fits 42 columns in colour" || { echo "FAIL splash is $w wide in colour"; fail=1; }
exit $fail
