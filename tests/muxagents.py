#!/usr/bin/env python3
"""Agents in the native mux: the agent in front of a pane is found, its state comes from the screen or from a hook (report-agent),
finished work shows as done until the pane is shown, bhote hears of every change (bhote event), agent.read / agent.prompt.
The agent is a fake `claude` script; bhote is a fake that logs its events. Usage: python3 tests/muxagents.py"""
import os, json, time, subprocess
from muxlib import *
BHOTE = '#!/bin/sh\nif [ "$1" = event ]; then echo "$HERDR_PLUGIN_EVENT_JSON" >> "$(dirname "$0")/events.log"; exit 0; fi\nexec sleep 600\n'
E = Env(panel_bin=BHOTE)
check = E.check
os.makedirs(f"{E.T}/bin"); 
open(f"{E.T}/bin/claude", "w").write('''#!/bin/sh
# a fake agent: works until you press enter (the status line says so), then asks something, then waits for input
echo "✻ Thinking… (3s · esc to interrupt)"
read a
printf '\\n Do you want to proceed?\\n ❯ 1. Yes\\n   2. No\\n'
read b
printf '\\033[2J\\033[H all done.\\n\\n  ? for shortcuts\\n'
while read c; do echo "you said: $c"; done
''')
os.chmod(f"{E.T}/bin/claude", 0o755)
E.env["PATH"] = f"{E.T}/bin:" + E.env["PATH"]
E.start()
c = E.client(120, 40)
assert c.wait_for("$ ")
agent = lambda pid: next(p for p in E.panes() if p["pane_id"] == pid)
def wait_status(pid, st, t=6):
    end = time.time() + t
    while time.time() < end:
        if agent(pid).get("agent_status") == st: return True
        time.sleep(0.15)
    return False
check(agent("w1:p1")["agent_status"] == "unknown" and "agent" not in agent("w1:p1"), "a plain shell is no agent")
# a second pane takes the focus, the agent runs in the first
E.call("pane.split", {"pane_id": "w1:p1", "direction": "right", "focus": True}); E.settle()
check(E.focused() == "w1:p2", "the new pane is focused")
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"})
check(wait_status("w1:p1", "working"), "the agent is found and its status line means working", agent("w1:p1"))
check(agent("w1:p1").get("agent") == "claude", "agent: claude")
E.call("pane.send_text", {"pane_id": "w1:p1", "text": "\r"})
check(wait_status("w1:p1", "blocked"), "a question on the screen means blocked", E.result("pane.read", {"pane_id": "w1:p1", "source": "visible"}))
E.call("pane.send_text", {"pane_id": "w1:p1", "text": "\r"})
check(wait_status("w1:p1", "done", 8), "finished while nobody looks: done", agent("w1:p1"))
E.call("pane.focus", {"pane_id": "w1:p1"})
check(wait_status("w1:p1", "idle"), "showing the pane makes it idle")
# bhote heard of the changes (working, blocked, done, idle)
time.sleep(0.5)
evs = [json.loads(l) for l in open(f"{E.T}/events.log").read().splitlines() if l.strip()]
states = [e["data"]["agent_status"] for e in evs if e["data"]["pane"]["pane_id"] == "w1:p1"]
check(states[:4] == ["working", "blocked", "done", "idle"], "bhote event: one per change, with the pane", states)
check(evs[0]["event"] == "pane.agent_status_changed" and evs[0]["data"]["pane"]["agent"] == "claude", "the event carries the pane as `pane list` shows it")
# a hook reports state and session (herdr's integration speaks this to the socket; here through the herdr link of the CLI)
bin_herdr = os.path.join(E.T, "herdrlink"); os.makedirs(bin_herdr); os.symlink(BIN, f"{bin_herdr}/herdr")
env2 = dict(E.env, PATH=bin_herdr + ":" + E.env["PATH"])
subprocess.run(["herdr", "pane", "report-agent", "w1:p1", "--source", "test", "--agent", "claude", "--state", "blocked", "--agent-session-id", "sess-1", "--", "claude", "--resume", "sess-1"], env=env2)
check(wait_status("w1:p1", "blocked", 3) and agent("w1:p1")["agent_session"]["value"] == "sess-1", "report-agent: a reported state wins over the screen, and the session is kept", agent("w1:p1"))
subprocess.run(["herdr", "pane", "release-agent", "w1:p1", "--source", "test"], env=env2)
check(wait_status("w1:p1", "idle", 4), "release-agent: the screen decides again")
# read and prompt
check("all done." in E.result("agent.read", {"pane_id": "w1:p1", "lines": 30})["text"], "agent.read: the agent's screen text")
E.call("agent.prompt", {"pane_id": "w1:p1", "text": "hello agent"}); E.settle(0.8)
check("you said: hello agent" in E.result("agent.read", {"pane_id": "w1:p1"})["text"], "agent.prompt: text and Enter reach the agent")
check([a["pane_id"] for a in E.result("agent.list")["agents"]] == ["w1:p1"], "agent.list")
# ending the agent: no agent any more
E.call("pane.send_keys", {"pane_id": "w1:p1", "keys": ["ctrl+c"]}); E.settle(1.8)
check("agent" not in agent("w1:p1") and agent("w1:p1")["agent_status"] == "unknown", "when the agent ends the pane is a plain pane again", agent("w1:p1"))
E.done()
