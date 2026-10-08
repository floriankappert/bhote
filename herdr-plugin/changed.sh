#!/bin/sh
# Action "changed": another machine says one of its agents changed its state; bhote asks it again at once (not after
# REMOTE_EVERY). Invoked from there through herdr (bhote event, setting PUSH_STATES).
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
[ -x "$bhote" ] || exit 0
exec "$bhote" remote-changed
