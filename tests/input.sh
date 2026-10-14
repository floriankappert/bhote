#!/usr/bin/env bash
# Tests of the panel's keyboard input: a terminal that is gone (hung up, closed) is the END of the input, not a timeout. A panel that read
# it as a timeout spun forever and used a core. No real herdr pane, no real terminal: setsid leaves the shell without a controlling tty
# (/dev/tty cannot be opened, as in a pane that is gone). Usage: bash tests/input.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID
T=$(mktemp -d); export NO_COLOR=1 BHOTE_SOURCE_ONLY=1 TMPDIR=$T BHOTE_CONFIG=$T/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
command -v setsid >/dev/null 2>&1 || { echo "skip setsid is missing"; exit 0; }
# the same file with the bash 4 branch switched off: the perl path and the plain path of bash 3.2 (macOS) run here, too
sed 's/"${BASH_VERSINFO\[0\]}" -ge 4/"${BASH_VERSINFO[0]}" -ge 99/' bhote > "$T/bhote3"
cat > "$T/run.sh" <<'SH'
. "$1"; HAVE_PERL=$2
SECONDS=0; in_fill 1; a=$?; in_fill wait; b=$?; INBUF=""; in_get k 1; c=$?
echo "$a $b $c $SECONDS"
SH
for v in bhote "$T/bhote3"; do for perl in 1 ""; do
    out=$(setsid bash "$T/run.sh" "$v" "$perl" </dev/null 2>/dev/null)
    set -- $out
    [ "$1 $2 $3" = "2 2 2" ] && [ "${4:-9}" -lt 3 ] && ok "no terminal is end of input (2), at once: $(basename "$v")${perl:+ with perl}" || bad "end of input: $v perl=$perl -> $out"
done; done
# a quiet terminal is still a timeout (1), not the end: a real pty (script) that nobody types into
if command -v script >/dev/null 2>&1; then
    cat > "$T/quiet.sh" <<'SH'
. "$1"; HAVE_PERL=$2; in_fill 1; echo "rc=$?"
SH
    for v in bhote "$T/bhote3"; do for perl in 1 ""; do
        out=$( (sleep 3 | script -qec "bash $T/quiet.sh $v $perl" /dev/null) 2>/dev/null | tr -d '\r')
        case "$out" in *rc=1*) ok "a quiet terminal is a timeout (1): $(basename "$v")${perl:+ with perl}" ;; *) bad "quiet terminal: $v perl=$perl -> $out" ;; esac
    done; done
fi
rm -rf "$T"; exit $fail
