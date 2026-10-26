#!/usr/bin/env bash
# Two machines on one (real ssh, kharka hub and daemons, two muxes) (tests/muxtwo.py); skipped without python3, sshd, kharka or herdr-kharka
cd "$(dirname "$0")" || exit 1
command -v python3 >/dev/null 2>&1 || { echo skipped; exit 0; }
exec python3 muxtwo.py
