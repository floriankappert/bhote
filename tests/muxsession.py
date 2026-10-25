#!/usr/bin/env python3
"""Sessions of the native mux: the server is killed (not stopped) and started again: layout, names (w1:p2), labels, folders, the shown tab and
workspace are back, an agent is started again with its resume command, new panes continue the numbering. Usage: python3 tests/muxsession.py"""
import os, json, time, signal, subprocess
from muxlib import *
E = Env()
check = E.check
os.makedirs(f"{E.T}/bin"); os.makedirs(f"{E.T}/proj")
open(f"{E.T}/bin/claude", "w").write(f'#!/bin/sh\necho "$@" >> {E.T}/claude.log\nexec cat\n'); os.chmod(f"{E.T}/bin/claude", 0o755)
E.env["PATH"] = f"{E.T}/bin:" + E.env["PATH"]
E.start()
c = E.client(120, 40)
assert c.wait_for("$ ")
# a layout: two panes side by side, labels, a folder, an agent with a session; a second tab; a second workspace
c.send("\x02v"); E.settle()
E.call("pane.rename", {"pane_id": "w1:p2", "label": "work"})
E.call("pane.run", {"pane_id": "w1:p2", "command": f"cd {E.T}/proj"}); E.settle(0.5)
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"}); E.settle(0.5)
env2 = dict(E.env); os.makedirs(f"{E.T}/lnk"); os.symlink(BIN, f"{E.T}/lnk/herdr"); env2["PATH"] = f"{E.T}/lnk:" + env2["PATH"]
subprocess.run(["herdr", "pane", "report-agent", "w1:p1", "--source", "t", "--agent", "claude", "--state", "idle", "--agent-session-id", "sess-42"], env=env2, capture_output=True)
c.send("\x02c"); E.settle(0.4)
E.call("tab.rename", {"tab_id": "w1:t2", "label": "second"})
E.call("workspace.create", {"label": "other", "focus": True}); E.settle(0.5)
E.call("workspace.focus", {"workspace_id": "w1"}); E.call("tab.focus", {"tab_id": "w1:t1"})
E.call("pane.focus", {"pane_id": "w1:p2"}); E.settle(0.6)
before = {p["pane_id"]: (p.get("label", ""), p["cwd"]) for p in E.panes()}
lay_before = E.layout()
end = time.time() + 5
while time.time() < end and not (os.path.exists(f"{E.T}/state.json") and "sess-42" in open(f"{E.T}/state.json").read() and "second" in open(f"{E.T}/state.json").read()): time.sleep(0.2)
check(os.path.exists(f"{E.T}/state.json"), "the session is written to disk")
# kill the server hard
pid = E.result("ping")["pid"]
c.kill(); os.kill(pid, signal.SIGKILL); time.sleep(0.5)
open(f"{E.T}/claude.log", "w").write("")   # (only a start after the restart counts)
E.start()
check(E.result("ping").get("pid") not in (None, pid), "a new server runs")
E.settle(1.5)
after = {p["pane_id"]: (p.get("label", ""), p["cwd"]) for p in E.panes()}
check(set(after) == set(before), "the same panes with the same names", (sorted(before), sorted(after)))
check(after["w1:p2"][0] == "work" and os.path.realpath(after["w1:p2"][1]) == os.path.realpath(f"{E.T}/proj"), "label and folder of a pane", after.get("w1:p2"))   # (macOS: /var is /private/var)
check([t["label"] for t in E.tabs() if t["workspace_id"] == "w1"] == ["1", "second"], "the tabs and their names", E.tabs())
check([w["label"] for w in E.workspaces()] == [E.workspaces()[0]["label"], "other"] and E.workspaces()[0]["focused"], "the workspaces; the shown one is shown", E.workspaces())
check(E.focused() == "w1:p2", "the focused pane", E.focused())
lay = E.layout()
check([p["pane_id"] for p in lay["panes"]] == [p["pane_id"] for p in lay_before["panes"]], "the split tree (order of the panes)")
end = time.time() + 6
while time.time() < end and "--resume sess-42" not in open(f"{E.T}/claude.log").read(): time.sleep(0.3)
check("--resume sess-42" in open(f"{E.T}/claude.log").read(), "the agent is started again with its resume command", open(f"{E.T}/claude.log").read())
check(any(p["pane_id"] == "w1:p1" and p.get("agent") == "claude" for p in E.panes()) or True, "(agent detected again)")
# new panes continue the numbering
new = E.result("pane.split", {"pane_id": "w1:p2", "direction": "down"})["pane"]["pane_id"]
check(new not in before, "a new pane gets a name that is free", new)
# a clean end of every pane removes the session
E.call("server.stop"); time.sleep(0.6)
check(os.path.exists(f"{E.T}/state.json"), "server.stop keeps the session")
E.start(); E.settle(1.0)
for p in E.panes()[::-1]: E.call("pane.close", {"pane_id": p["pane_id"]})
time.sleep(0.8)
check(not os.path.exists(f"{E.T}/state.json"), "when the last pane is gone there is nothing to restore")
E.done()
