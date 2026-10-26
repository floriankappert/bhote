#!/usr/bin/env python3
"""The view of another machine (a hidden workspace opened by a click) is not what a new terminal starts on: it is not kept over a restart,
and a client that attaches while it is shown gets a workspace of this machine. Usage: python3 tests/muxhidden.py"""
import os, signal, time
from muxlib import *
E = Env()
check = E.check
E.start()
c = E.client(100, 30)
assert c.wait_for("$ ")
r = E.call("workspace.create", {"label": "Omarchy · tmp", "hidden": True, "focus": True}); E.settle(0.5)

c2 = E.client(100, 30); E.settle(0.5)
foc = [w for w in E.call("workspace.list", {"all": True})["result"]["workspaces"] if w["focused"]]
check(foc and foc[0]["label"] != "Omarchy · tmp", "a new client starts on a workspace of this machine", foc)
E.call("workspace.focus", {"workspace_id": [w for w in E.call("workspace.list", {"all": True})["result"]["workspaces"] if w["label"] == "Omarchy · tmp"][0]["workspace_id"]})
time.sleep(1.2)
pid = E.result("ping")["pid"]; os.kill(pid, signal.SIGKILL); time.sleep(0.5)
E.start(); E.settle(0.8)
labels = [w["label"] for w in E.call("workspace.list", {"all": True})["result"]["workspaces"]]
check("Omarchy · tmp" not in labels, "the view of another machine is not restored", labels)
E.done()
