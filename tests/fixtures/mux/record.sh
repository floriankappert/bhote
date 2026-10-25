#!/bin/sh
# Records session1.raw: a real shell session in a 80x24 pty (colours, wide characters, scroll region, alt screen, cursor moves).
# Usage: record.sh  (rewrites session1.raw; the expectations in daemon/src/mux/replay test depend on its content)
cd "$(dirname "$0")"
script -qfec "stty cols 80 rows 24; sh -c '
printf \"\\033[1;31mred bold\\033[0m \\033[38;2;10;200;30mtruecolor\\033[0m \\033[48;5;27m256bg\\033[0m\\n\"
printf \"wide: 日本語 ok\\n\"
printf \"\\033[2J\\033[H\"
i=1; while [ \$i -le 30 ]; do echo line\$i; i=\$((i+1)); done
printf \"\\033[5;10r\\033[10;1H\\nscrolled\\n\\033[r\"
printf \"\\033[?1049h\\033[2J\\033[3;5Halt screen\\033[?1049l\"
printf \"\\033[24;1Hdone\"
'" /dev/null < /dev/null > session1.raw 2>/dev/null
wc -c session1.raw
