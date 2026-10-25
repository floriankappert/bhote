#!/usr/bin/env bash
# herdr-kharka against the native mux (tests/muxkharka.py); skipped without python3, kharka or herdr-kharka
cd "$(dirname "$0")" || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 muxkharka.py
