#!/usr/bin/env bash
# The native mux in a pseudo terminal (tests/mux.py); skipped without python3 or the daemon binary
cd "$(dirname "$0")/.." || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 tests/mux.py
