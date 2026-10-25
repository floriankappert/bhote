#!/usr/bin/env python3
"""The chat panel (bhoted chat) in the native mux: the history from the agent's transcript, a message typed in the chat reaches the agent,
@ picks another agent, and CHAT_PANEL=bottom makes the pane by itself. Usage: python3 tests/muxchat.py"""
import os, json, time, subprocess
from muxlib import *
E = Env(config="CHAT_PANEL=bottom\nCHAT_HEIGHT=12\nAUTOSTART=on\nPANEL_LEFT=off\nPANEL_RIGHT=off\n", panel_bin="#!/bin/sh\nexec sleep 600\n")
check = E.check
sess = "11111111-2222-3333-4444-555555555555"
os.makedirs(f"{E.T}/.claude/projects/proj", exist_ok=True)
open(f"{E.T}/.claude/projects/proj/{sess}.jsonl", "w").write("\n".join(json.dumps(x) for x in [
    {"type": "user", "message": {"role": "user", "content": "please fix the build"}},
    {"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Looking at the failing test now."}, {"type": "tool_use", "name": "Bash", "input": {"command": "cargo test"}}]}}]) + "\n")
os.makedirs(f"{E.T}/bin"); open(f"{E.T}/bin/claude", "w").write('#!/bin/sh\necho "esc to interrupt"\nwhile read c; do echo "you said: $c"; done\n'); os.chmod(f"{E.T}/bin/claude", 0o755)
E.env["PATH"] = f"{E.T}/bin:" + E.env["PATH"]
E.start()
c = E.client(160, 50)
assert c.wait_for("$ ")
E.settle(1.0)
sbs = {s["label"]: s for s in E.layout().get("sidebars", [])}
check("bhote-chat" in sbs, "CHAT_PANEL=bottom: the chat panel is made (for all workspaces)", sbs)
chat = sbs["bhote-chat"]["pane_id"]; main = "w1:p1"
check(sbs["bhote-chat"]["rect"]["height"] == 12 and sbs["bhote-chat"]["rect"]["y"] > E.rect(main)["y"], "under the main area, 12 rows high", (sbs, E.rect(main)))
# the agent runs in the main pane; it reports its session
E.call("pane.run", {"pane_id": main, "command": "claude"}); E.settle(0.8)
env2 = dict(E.env); os.makedirs(f"{E.T}/lnk"); os.symlink(BIN, f"{E.T}/lnk/herdr"); env2["PATH"] = f"{E.T}/lnk:" + env2["PATH"]
subprocess.run(["herdr", "pane", "report-agent", main, "--source", "t", "--agent", "claude", "--state", "idle", "--agent-session-id", sess], env=env2)
def chat_text(): return E.result("pane.read", {"pane_id": chat, "source": "visible"}).get("text", "")
end = time.time() + 8
while time.time() < end and "fix the build" not in chat_text(): time.sleep(0.3)
t = chat_text()
check("please fix the build" in t and "Looking at the failing test now." in t and "Bash cargo test" in t, "the history shows the transcript (you, the agent, a tool line)", t)
check("claude" in t.split("\n")[0], "the header names the agent", t.split("\n")[0])
# type into the chat and send
for ch in "hello from chat": E.call("pane.send_text", {"pane_id": chat, "text": ch})
E.settle(0.4)
check("hello from chat" in chat_text(), "typed text shows in the input box")
E.call("pane.send_text", {"pane_id": chat, "text": "\r"}); E.settle(1.2)
check("you said: hello from chat" in E.result("pane.read", {"pane_id": main})["text"], "Enter sends it to the agent's prompt", E.result("pane.read", {"pane_id": main}))
check("hello from chat" in chat_text() and "please fix the build" in chat_text(), "the sent text is in the history")
# a second agent and @ to pick it
E.call("pane.split", {"pane_id": main, "direction": "right", "focus": False}); E.settle(0.5)
other = next(p["pane_id"] for p in E.panes() if p["pane_id"] not in (main, chat) and not p.get("label"))
E.call("workspace.rename", {"workspace_id": "w1", "label": "api"})
E.call("pane.run", {"pane_id": other, "command": "claude"}); E.settle(0.8)
end = time.time() + 6
while time.time() < end and len(E.result("agent.list")["agents"]) < 2: time.sleep(0.3)
check(len(E.result("agent.list")["agents"]) == 2, "two agents now")
for ch in "@": E.call("pane.send_text", {"pane_id": chat, "text": ch})
E.settle(0.8)
check(other in chat_text() or "claude" in chat_text(), "@ opens the list of agents", chat_text())
E.done()
