#!/usr/bin/env python3
"""The Claude Code hook of bhote (integrations/claude/hooks/agent-state.sh) and its installer: `bhote integration install claude` puts it into every profile's
settings.json once (the hooks that are there stay), uninstall takes only it away; the hook tells the mux what a session does: working, blocked, idle, the
session, and the end. Usage: python3 tests/muxhooks.py"""
import os, json, subprocess, time
from muxlib import *
E = Env()
check = E.check
BH = os.path.join(ROOT, "bhote")
prof = f"{E.T}/.claude2"; os.makedirs(prof)
os.makedirs(f"{E.T}/repo")
json.dump({"theme": "dark", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo mine"}]}]}}, open(f"{E.T}/repo/settings.json", "w"))
os.symlink(f"{E.T}/repo/settings.json", f"{prof}/settings.json")   # (as in a config repo: settings.json is a link)
env = dict(E.env, CLAUDE_CONFIG_DIR=prof, HOME=E.T, BHOTED_BIN=BIN)
def bhote(*a): return subprocess.run(["bash", BH, *a], capture_output=True, text=True, env=env, timeout=60)
r = bhote("integration", "install", "claude")
st = json.load(open(f"{prof}/settings.json"))
mine = lambda e: [h for g in st["hooks"].get(e, []) for h in g["hooks"] if "agent-state.sh" in h["command"]]
check(r.returncode == 0 and all(len(mine(e)) == 1 for e in ("SessionStart", "UserPromptSubmit", "PostToolUse", "Notification", "Stop", "SessionEnd")), "install puts the hook on the six events", r.stdout + r.stderr)
check(any(h["command"] == "echo mine" for g in st["hooks"]["Stop"] for h in g["hooks"]) and st["theme"] == "dark", "the hooks and settings that were there stay")
bhote("integration", "install", "claude"); st = json.load(open(f"{prof}/settings.json"))
check(all(len(mine(e)) == 1 for e in st["hooks"] if mine(e)), "a second install changes nothing (no doubles)")
check(os.access(f"{E.T}/.local/share/bhote/hooks/agent-state.sh", os.X_OK), "the script is copied to a place of its own")
check(os.path.islink(f"{prof}/settings.json") and "agent-state.sh" in open(f"{E.T}/repo/settings.json").read(), "settings.json stays a link into the config repo, and the repo file has the hook")
check("ok" in bhote("integration", "status", "claude").stdout, "status")
# the hook at work
os.makedirs(f"{E.T}/abin"); open(f"{E.T}/abin/claude", "w").write("#!/bin/sh\nexec cat\n"); os.chmod(f"{E.T}/abin/claude", 0o755)
E.env["PATH"] = f"{E.T}/abin:" + E.env["PATH"]
E.start(); c = E.client(); c.wait_for("$ ")
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"}); time.sleep(0.6)
lnk = f"{E.T}/lnk"; os.makedirs(lnk); os.symlink(BIN, f"{lnk}/herdr")
henv = dict(E.env, PATH=f"{lnk}:" + E.env["PATH"], HERDR_PANE_ID="w1:p1")
hook = f"{E.T}/.local/share/bhote/hooks/agent-state.sh"
def fire(**j): subprocess.run(["sh", hook], input=json.dumps(j), text=True, env=henv, capture_output=True)
pane = lambda: next(p for p in E.panes() if p["pane_id"] == "w1:p1")
def wait(f, t=4):
    end = time.time() + t
    while time.time() < end:
        if f(): return True
        time.sleep(0.1)
    return False
fire(hook_event_name="SessionStart", session_id="sess-9")
check(wait(lambda: pane().get("agent") == "claude" and pane()["agent_session"]["value"] == "sess-9"), "SessionStart: the agent and its session", pane())
fire(hook_event_name="UserPromptSubmit", session_id="sess-9")
check(wait(lambda: pane()["agent_status"] == "working"), "UserPromptSubmit: working")
fire(hook_event_name="Notification", session_id="sess-9", message="Claude needs your permission to use Bash")
check(wait(lambda: pane()["agent_status"] == "blocked"), "Notification (a permission): blocked")
fire(hook_event_name="PostToolUse", session_id="sess-9")
check(wait(lambda: pane()["agent_status"] == "working"), "PostToolUse: working again")
fire(hook_event_name="Stop", session_id="sess-9")
check(wait(lambda: pane()["agent_status"] in ("idle", "done")), "Stop: idle (done when nobody looks)", pane()["agent_status"])
fire(hook_event_name="UserPromptSubmit", session_id="sess-9", agent_id="sub-1")
time.sleep(0.5)
check(pane()["agent_status"] in ("idle", "done"), "a sub-agent's event is not the session's")
fire(hook_event_name="Notification", session_id="sess-9", message="Claude is waiting for your input")
time.sleep(0.5)
check(pane()["agent_status"] in ("idle", "done"), "a plain waiting notice is idle, not blocked")
# the resume command is kept for a restart of the mux
snap = open(f"{E.T}/state.json").read() if os.path.exists(f"{E.T}/state.json") else ""
time.sleep(1.0); snap = open(f"{E.T}/state.json").read()
check("--resume" in snap and "sess-9" in snap, "the session and its resume command are in the saved session", snap[:200])
# uninstall
bhote("integration", "uninstall", "claude"); st = json.load(open(f"{prof}/settings.json"))
check(not any(mine(e) for e in st.get("hooks", {})) and any(h["command"] == "echo mine" for g in st["hooks"]["Stop"] for h in g["hooks"]), "uninstall takes only its own hooks away")
E.done()
