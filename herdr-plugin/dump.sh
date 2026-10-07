#!/bin/sh
# Another machine reads this one's topics through herdr: it invokes this action and reads the output from the plugin log.
bhote=$(command -v bhote 2>/dev/null)
for c in "$HOME/.local/bin/bhote" /opt/homebrew/bin/bhote /usr/local/bin/bhote /usr/bin/bhote; do [ -n "$bhote" ] && break; [ -x "$c" ] && bhote=$c; done
[ -x "$bhote" ] || exit 1
exec "$bhote" dump --packed
