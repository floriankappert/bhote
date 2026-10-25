#!/bin/sh
# bhote's Claude Code hook: tells the multiplexer (the native mux, or herdr) what this Claude Code session does, exactly (not guessed from the screen):
#   SessionStart -> the session and "idle"      UserPromptSubmit, PostToolUse -> working      Notification -> blocked (a permission prompt) or idle
#   Stop -> idle      SessionEnd -> the report ends (the screen decides again)
# `bhote integration install claude` wires it into every Claude Code profile. Without a multiplexer pane (no HERDR_PANE_ID) it does nothing.
[ -n "${HERDR_PANE_ID:-}" ] || exit 0
command -v herdr >/dev/null 2>&1 || exit 0
input=$(cat 2>/dev/null)
field() { printf '%s' "$input" | sed -n "s/.*\"$1\" *: *\"\([^\"]*\)\".*/\1/p" | head -n 1; }
# a sub-agent's events are not this session's
case $input in *'"agent_id"'*) exit 0 ;; esac
ev=$(field hook_event_name); sid=$(field session_id)
case $ev in
    SessionStart) state=idle ;;
    UserPromptSubmit|PostToolUse|PreToolUse) state=working ;;
    Stop|SubagentStop) state=idle ;;
    Notification)
        case "$(field notification_type)$(field message)" in
            *permission*|*Permission*|*approve*|*"needs your"*) state=blocked ;;
            *) state=idle ;;
        esac ;;
    SessionEnd) herdr pane release-agent "$HERDR_PANE_ID" --source bhote:claude >/dev/null 2>&1; exit 0 ;;
    *) exit 0 ;;
esac
if [ -n "$sid" ]; then
    herdr pane report-agent "$HERDR_PANE_ID" --source bhote:claude --agent claude --state "$state" --agent-session-id "$sid" -- claude --resume "$sid" >/dev/null 2>&1
else
    herdr pane report-agent "$HERDR_PANE_ID" --source bhote:claude --agent claude --state "$state" >/dev/null 2>&1
fi
exit 0
