#!/usr/bin/env bash
# bhote without herdr (tests/muxnoherdr.py); skipped without python3 or the daemon binary, or on a machine that has herdr in the system path
cd "$(dirname "$0")" || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 muxnoherdr.py
