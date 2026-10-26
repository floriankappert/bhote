#!/usr/bin/env bash
# Runs the test suites (all of them, or the ones named) and exits 1 when one of them fails. Usage: bash tests/run.sh [smoke cli ...]
# Needs jq, python3 and git. Each suite works in throw-away folders and never touches your config, data or herdr.
cd "$(dirname "$0")/.." || exit 1
[ $# -gt 0 ] || set -- smoke cli agents input store slack monitor follow frame fuzz panel mux muxcli muxkeys muxpanels muxagents muxchat muxsession muxhidden muxkharka muxview muxperf muxtwo muxhooks muxnoherdr muxlaunch sync kharka   # (kharka is skipped without the kharka binary)
fail=0
for s in "$@"; do
    printf '── %s\n' "$s"
    out=$(bash "tests/$s.sh" 2>&1); rc=$?
    # only what is not ok (FAIL lines, errors); not the printf noise of a `... | grep -q` that closed the pipe early while bhote still wrote
    # (a shell that ignores SIGPIPE, as in CI on macOS and in an agent's shell, says so): the checks themselves are not touched
    printf '%s\n' "$out" | grep -v '^ok ' | grep -v 'printf: write error: Broken pipe' || true
    printf '%s ok\n' "$(printf '%s\n' "$out" | grep -c '^ok ')"
    [ "$rc" = 0 ] || { echo "suite $s failed (exit $rc)"; fail=1; }
done
exit $fail
