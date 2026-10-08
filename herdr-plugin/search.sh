#!/bin/sh
# The "bhote search" popup (the plugin pane "search"): bhote find in it. Opened by the action "search" (settings › Find key).
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
[ -x "$bhote" ] || { echo "bhote not found"; sleep 2; exit 1; }
exec "$bhote" find
