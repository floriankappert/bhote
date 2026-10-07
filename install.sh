#!/usr/bin/env bash
# Installs bhote for this user: a link ~/.local/bin/bhote to this checkout and, when herdr is there, the panel plugin.
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin"
ln -sfn "$DIR/bhote" "$HOME/.local/bin/bhote"
echo "✓ $HOME/.local/bin/bhote -> $DIR/bhote"
command -v jq >/dev/null 2>&1 || echo "! bhote needs jq (brew install jq / pacman -S jq / apt install jq)"
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "! add $HOME/.local/bin to your PATH" ;; esac
if command -v herdr >/dev/null 2>&1; then
  herdr plugin unlink flo.bhote-panel >/dev/null 2>&1 || true            # an older id of the same plugin
  herdr plugin list 2>/dev/null | grep -q "bhote.panel.*$DIR/herdr-plugin" \
    || { herdr plugin unlink bhote.panel >/dev/null 2>&1 || true; herdr plugin link "$DIR/herdr-plugin" >/dev/null; }
  echo "✓ herdr plugin bhote.panel (opens the panel next to the agent pane of every tab at herdr start)"
fi
