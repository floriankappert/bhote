#!/usr/bin/env bash
# Speed of the native mux (tests/muxperf.py); skipped without python3 or the daemon binary
cd "$(dirname "$0")" || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 muxperf.py
