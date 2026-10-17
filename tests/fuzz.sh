#!/usr/bin/env bash
# Garbage in, no panic out (tests/fuzz.py, a short run; FUZZ_N for more); skipped without python3 or the daemon binary
cd "$(dirname "$0")/.." || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
r=$(python3 tests/fuzz.py "${FUZZ_N:-40}" 2>&1); rc=$?; echo "$r"
[ "$r" = "${r#skipped}" ] && [ "$rc" = 0 ] && echo "ok   fuzz: $(printf %s "$r" | tail -1)"
exit $rc
