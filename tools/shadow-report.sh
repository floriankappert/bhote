#!/usr/bin/env bash
# Compares what the collector's follow-ups did in bash (shadow/bash.log) with what the daemon decided (shadow/rust.log) while
# BHOTED_FOLLOW=shadow: every decision of one side that the other side did not make within a few seconds. Usage: tools/shadow-report.sh [seconds, 6]
dir=${BHOTE_SHARED:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/bhote-$(id -u)}/shadow
w=${1:-6}
[ -s "$dir/bash.log" ] || [ -s "$dir/rust.log" ] || { echo "no shadow logs in $dir (BHOTED_FOLLOW=shadow, and the daemon running)"; exit 0; }
awk -F'|@' -v w="$w" '
    function norm(s) { gsub(/\|(shadow-new-[0-9]+|[0-9]{9,}-[0-9]+)\|/, "|NEW|", s); gsub(/(created|updated)=[0-9]+/, "&", s); return s }
    FILENAME ~ /bash.log$/ { nb++; bt[nb] = $2 + 0; bl[nb] = norm($1); next }
    { x = norm($1); t = $2 + 0; if ((x in lastr) && t - lastr[x] <= w) { lastr[x] = t; next }   # (a shadow round that repeats within seconds: nothing was changed in between)
      lastr[x] = t; nr++; rt[nr] = t; rl[nr] = x }
    END {
        for (i = 1; i <= nr; i++) { hit = 0
            for (j = 1; j <= nb; j++) if (!used[j] && bl[j] == rl[i] && (bt[j] - rt[i] <= w) && (rt[i] - bt[j] <= w)) { used[j] = 1; hit = 1; break }
            if (hit) same++; else { onlyr++; print "only the daemon: " rl[i] "   (" rt[i] ")" } }
        for (j = 1; j <= nb; j++) if (!used[j]) { onlyb++; print "only bash:       " bl[j] "   (" bt[j] ")" }
        printf "%d decisions the same, %d only in the daemon, %d only in bash (window %d s)\n", same, onlyr, onlyb, w
    }' "$dir/bash.log" "$dir/rust.log" 2>/dev/null
