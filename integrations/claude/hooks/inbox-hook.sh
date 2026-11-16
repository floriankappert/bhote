#!/usr/bin/env bash
# Claude Code hook (UserPromptSubmit and PostToolUse): messages that other agents sent to this agent (bhote send) reach it at its next tool call, also in a long
# turn, instead of when the turn ends. Not for Stop.
#
# The cost when nothing waits: one directory test, no process. The panel (and `bhote send` on this machine) put an empty file ~/.cache/bhote/inbox/<node>-<name>
# for every agent with a message waiting and remove it when the message is handed over. Only when such a file exists for THIS agent does the hook ask
# `bhote inbox --hook <event>`, which claims each message in kharka before it prints it (so the panel's delivery to a free agent and this hook never both hand it over).
#
# The contract of Claude Code (code.claude.com/docs/en/hooks): the event arrives as JSON on stdin ({"hook_event_name": ..., ...}). Exit 0 and a JSON object on stdout:
# {"systemMessage": "<shown to the person>", "hookSpecificOutput": {"hookEventName": "<event>", "additionalContext": "<the model reads it>"}}; at most 10000 characters.
# How to switch it on: docs/claude-mod.md ("Messages between agents", a UserPromptSubmit and a PostToolUse entry in your settings.json; bhote does not edit your settings).
bh=bhote; command -v bhote >/dev/null 2>&1 || bh=$(dirname "$0")/../../../bhote
dir=${BHOTE_INBOX:-${XDG_CACHE_HOME:-$HOME/.cache}/bhote/inbox}
set -- "$dir"/*
[ -e "$1" ] || exit 0                    # no message waits for any agent of this machine: done (the usual case)
[ -n "${HC_TERMINAL_ID:-}" ] || exit 0   # not an agent of herdr
# who am I: asked once a minute (a file per terminal), the node and the name of the space as the panel keys the messages
idf="$dir/.id/$HC_TERMINAL_ID"; me=""; at=0
if [ -r "$idf" ]; then read -r at me < "$idf"; fi
now=$(date +%s)
case "$at" in ''|*[!0-9]*) at=0 ;; esac
if [ -z "$me" ] || [ $(( now - at )) -ge 60 ]; then
    me=$("$bh" inbox --whoami 2>/dev/null)
    mkdir -p "$dir/.id" 2>/dev/null && printf '%s %s\n' "$now" "$me" > "$idf"
fi
[ -n "$me" ] && [ -e "$dir/$me" ] || exit 0   # something waits, but not for me
input=$(cat)
ev=$(printf '%s' "$input" | sed -n 's/.*"hook_event_name" *: *"\([A-Za-z]*\)".*/\1/p' | head -1)
case "$ev" in UserPromptSubmit|PostToolUse) ;; *) exit 0 ;; esac
exec "$bh" inbox --hook "$ev"
