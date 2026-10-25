#!/usr/bin/env bash
# The Claude Code hook and its installer (tests/muxhooks.py); skipped without python3, jq or the daemon binary
cd "$(dirname "$0")" || exit 1
command -v python3 >/dev/null 2>&1 && command -v jq >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 muxhooks.py
