#!/usr/bin/env bash
# Runs the test suites (all of them, or the ones named) and exits 1 when one of them fails. Usage: bash tests/run.sh [smoke cli ...]
# Needs jq, python3 and git. Each suite works in throw-away folders and never touches your config, data or herdr.
cd "$(dirname "$0")/.." || exit 1
[ $# -gt 0 ] || set -- smoke cli agents store slack monitor kharka   # (kharka is skipped without the kharka binary)
fail=0
for s in "$@"; do
    printf '── %s\n' "$s"
    out=$(bash "tests/$s.sh" 2>&1); rc=$?
    printf '%s\n' "$out" | grep -v '^ok ' || true                # only what is not ok (FAIL lines, errors)
    printf '%s ok\n' "$(printf '%s\n' "$out" | grep -c '^ok ')"
    [ "$rc" = 0 ] || { echo "suite $s failed (exit $rc)"; fail=1; }
done
exit $fail
