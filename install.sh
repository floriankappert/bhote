#!/usr/bin/env bash
# Installs bhote for this user: a link ~/.local/bin/bhote to this checkout, the daemon (built when cargo is there), the Claude Code hook; herdr is not needed
# (when it is there, its panel plugin is linked for MUX=herdr).
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin"
ln -sfn "$DIR/bhote" "$HOME/.local/bin/bhote"
echo "✓ $HOME/.local/bin/bhote -> $DIR/bhote"
# the daemon (bhoted: the native mux, the panel, the chat): built here when cargo is there and it is not yet
if [ ! -x "$DIR/daemon/target/release/bhoted" ] && ! command -v bhoted >/dev/null 2>&1; then
  if command -v cargo >/dev/null 2>&1; then (cd "$DIR/daemon" && cargo build --release 2>&1 | tail -n 1) && echo "✓ bhoted built"
  else echo "! the native mux needs bhoted: install Rust (https://rustup.rs) and run: cargo build --release --manifest-path $DIR/daemon/Cargo.toml"; fi
fi
command -v jq >/dev/null 2>&1 || echo "! bhote needs jq (brew install jq / pacman -S jq / apt install jq)"
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "! add $HOME/.local/bin to your PATH" ;; esac
if command -v herdr >/dev/null 2>&1; then
  herdr plugin unlink flo.bhote-panel >/dev/null 2>&1 || true            # an older id of the same plugin
  herdr plugin list 2>/dev/null | grep -q "bhote.panel.*$DIR/herdr-plugin" \
    || { herdr plugin unlink bhote.panel >/dev/null 2>&1 || true; herdr plugin link "$DIR/herdr-plugin" >/dev/null; }
  echo "✓ herdr plugin bhote.panel (opens the panel next to the agent pane of every tab at herdr start)"
fi
"$DIR/bhote" integration install claude 2>/dev/null | sed 's/^/✓ /' || true    # Claude Code: the hook that reports a session's state exactly
if command -v omarchy >/dev/null 2>&1 && [ -d "$HOME/.config/omarchy" ]; then   # Omarchy: the bar widget (enable it yourself)
  mkdir -p "$HOME/.config/omarchy/plugins"
  t="$HOME/.config/omarchy/plugins/bhote.bar"
  if [ -d "$t" ] && [ ! -L "$t" ]; then echo "! $t is a real directory: move it away, then run install.sh again"
  else ln -sfn "$DIR/omarchy-plugin" "$t"
    echo "✓ Omarchy bar widget bhote.bar (show it: omarchy plugin enable bhote.bar --section right)"; fi
fi
