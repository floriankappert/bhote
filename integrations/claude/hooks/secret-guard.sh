#!/usr/bin/env bash
# Claude Code hook (UserPromptSubmit): a key typed or pasted into the chat ends up as plaintext in the transcript under ~/.claude, in history.jsonl and in
# every later request. This hook looks at the prompt before it is sent. It prints only the kind of key it found, never the key.
#
#   SECRET_GUARD=off | warn | block      (environment, else `bhote config set SECRET_GUARD ...`; default warn)
#
# The contract of Claude Code for this event: the prompt arrives as JSON on stdin ({"prompt": "...", ...}). Exit 0 lets it through (what is printed as
# JSON {"systemMessage": ...} is shown to the person only). Exit 2 stops the prompt, erases it from the chat and shows what goes to stderr, to the person only.
# How to switch it on: docs/secrets.md (a UserPromptSubmit entry in your settings.json; bhote does not edit your settings).
mode=${SECRET_GUARD:-}
if [ -z "$mode" ]; then
    cfg=${BHOTE_CONFIG:-$HOME/.config/bhote/config}
    [ -r "$cfg" ] && mode=$(grep '^SECRET_GUARD=' "$cfg" | tail -1 | cut -d= -f2-)
fi
case "${mode:-warn}" in off) exit 0 ;; block) mode=block ;; *) mode=warn ;; esac

input=$(cat)
kind=""
hit() { printf '%s' "$input" | grep -Eq -- "$1" && kind="${kind:+$kind, }$2"; }
hit '(^|[^A-Za-z0-9_-])sk-ant-[A-Za-z0-9_-]{20,}' "Anthropic key"
hit '(^|[^A-Za-z0-9_-])sk-[A-Za-z0-9_-]{32,}' "API key (sk-)"
hit '(^|[^A-Za-z0-9_-])(ghp_|gho_)[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}' "GitHub token"
hit '(^|[^A-Za-z0-9_-])AKIA[0-9A-Z]{16}' "AWS access key"
hit '(^|[^A-Za-z0-9_-])[sr]k_live_[A-Za-z0-9]{20,}' "Stripe live key"
hit '(^|[^A-Za-z0-9_-])CCIPAT_[A-Za-z0-9_-]{20,}' "CircleCI token"
hit '(^|[^A-Za-z0-9_-])(lin_api_|lin_oauth_)[A-Za-z0-9]{30,}' "Linear key"
hit '(^|[^A-Za-z0-9_-])xox[abprs]-[A-Za-z0-9-]{20,}' "Slack token"
hit '(^|[^A-Za-z0-9_-])AIza[0-9A-Za-z_-]{30,}' "Google API key"
hit '(^|[^A-Za-z0-9_-])glpat-[A-Za-z0-9_-]{20,}|shpat_[A-Za-z0-9]{20,}|npm_[A-Za-z0-9]{30,}' "token"
hit 'BEGIN [A-Z ]*PRIVATE KEY' "private key"
# VARIABLE=value for names that say key/token/secret/password, with a value that is no placeholder
hit '[A-Z][A-Z0-9_]*(KEY|TOKEN|SECRET|PASSWORD|PASSWD)[A-Z0-9_]*=[\"'"'"']?[A-Za-z0-9._~+/-]{16,}' "variable with a key" 
if [ -n "$kind" ] && printf '%s' "$input" | grep -Eq '[A-Z][A-Z0-9_]*(KEY|TOKEN|SECRET|PASSWORD|PASSWD)[A-Z0-9_]*=[\"'"'"']?(your|example|xxx|<|\$|placeholder|changeme|\.\.\.)' && [ "$kind" = "variable with a key" ]; then exit 0; fi
[ -n "$kind" ] || exit 0

msg="Bitte nicht in den Chat ($kind erkannt): $ bhote secret set NAME  (Eingabe verdeckt im Terminal oder im Tab Env), danach in Befehlen nur den Namen benutzen: bhote env run ENV -- befehl"
if [ "$mode" = block ]; then
    printf '%s\n' "$msg. Der Prompt wurde nicht gesendet." >&2
    exit 2
fi
printf '{"systemMessage":"%s. Der Prompt wird trotzdem gesendet (SECRET_GUARD=block stoppt ihn)."}\n' "${msg//\"/\\\"}"
exit 0
