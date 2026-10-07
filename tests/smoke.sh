#!/usr/bin/env bash
# Smoke test: every view must draw (non-empty, framed or ruled, with its own marker text) without errors.
# Runs against a throw-away config and data directory. Usage: bash tests/smoke.sh
cd "$(dirname "$0")/.." || exit 1
export NO_COLOR=1 BHOTE_ONCE=1 BHOTE_CONFIG=$(mktemp -d)/config BHOTE_DATA=$(mktemp -d) BHOTE_SHARED=$(mktemp -d)
fail=0
check() {  # check <view> <text that must appear>
    out=$(BHOTE_VIEW=$1 bash ./bhote 2>&1)
    if printf '%s' "$out" | grep -q "command not found\|unbound\|syntax error"; then echo "FAIL $1: shell error"; printf '%s\n' "$out" | head -3; fail=1
    elif printf '%s' "$out" | grep -q "$2"; then echo "ok   $1"
    else echo "FAIL $1: '$2' missing"; fail=1; fi
}
check splash   "PRESS ANY KEY TO START"
check main     "work & agent sync"
check settings "Settings"
# no line may reach the panel's right edge, and the frame has exactly ROWS lines (main view)
fit() {  # fit <cols> <rows> <view>
    local t; t=$(mktemp -d); : > "$t/c"
    for i in 1 2 3 4 5 6 7 8 9 10 11 12; do BHOTE_CONFIG=$t/c BHOTE_DATA=$t/d bash ./bhote add "A fairly long topic title number $i" >/dev/null; done
    BHOTE_CONFIG=$t/c BHOTE_DATA=$t/d BHOTE_ONCE=1 BHOTE_COLS=$1 BHOTE_ROWS=$2 BHOTE_VIEW=$3 LC_ALL=en_US.UTF-8 bash ./bhote \
      | sed 's/\x1b\[[0-9;]*[mK]//g' | python3 -c "
import sys
L=sys.stdin.read().split('\n'); L=L[:-1] if L and L[-1]=='' else L
ok=max(len(l) for l in L)<$1-1 and ('$3'!='main' or len(L)==$2)
sys.exit(0 if ok else 1)" && echo "ok   fit $3 $1x$2" || { echo "FAIL fit $3 $1x$2"; fail=1; }
    rm -rf "$t"
}
for size in "44 40" "36 24" "44 87"; do set -- $size; fit "$1" "$2" main; fit "$1" "$2" settings; done
exit $fail
