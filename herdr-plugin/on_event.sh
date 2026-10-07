#!/bin/sh
# An agent's status changed: hand the event to bhote (it updates the topic of that agent, notifies, and pokes the panels).
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
[ -x "$bhote" ] || exit 0
exec "$bhote" event
