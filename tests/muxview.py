#!/usr/bin/env python3
"""A pane of another machine in a tab of its own: `bhote view <pane>` shows one pane of a server (keys go to it, it is not resized, it ends with the pane),
and a click on an agent of another machine (remote_view of the script) opens such a tab over ssh. Here ssh is a fake that runs the command on this machine,
so the 'other machine' is the same server. Usage: python3 tests/muxview.py"""
import os, time
from muxlib import *
E = Env()
check = E.check
os.makedirs(f"{E.T}/bin"); os.makedirs(f"{E.T}/shared")
open(f"{E.T}/fakessh", "w").write('#!/bin/sh\n# fake ssh: ssh -t <node> <command>: runs the command here\nshift; [ "$1" = "-t" ] && shift; shift\nexec sh -c "$1"\n'); os.chmod(f"{E.T}/fakessh", 0o755)
os.symlink(os.path.join(ROOT, "bhote"), f"{E.T}/bin/bhote")
open(f"{E.T}/shared/agents.nodes", "w").write("Omarchy\x1fnode-omarchy\x1f0\x1fagent.prompt\n")
open(f"{E.T}/config", "a").write(f"REMOTE_SSH={E.T}/fakessh -t {{node}}\n")
E.env.update(PATH=f"{E.T}/bin:" + E.env["PATH"], BHOTED_BIN=BIN, BHOTE_SHARED=f"{E.T}/shared", BHOTE_DATA=f"{E.T}/data", SHELL="/bin/sh")
E.start(); c = E.client(120, 40); assert c.wait_for("$ ")
E.call("pane.split", {"pane_id": "w1:p1", "direction": "right", "focus": False}); E.settle(0.4)
E.call("pane.run", {"pane_id": "w1:p2", "command": "echo hello_from_p2; cat"}); E.settle(0.5)
size = E.rect("w1:p2")
# a viewer (a tab running bhote view)
E.call("tab.create", {"label": "viewer", "command": [BIN, "view", "w1:p2"], "focus": True}); E.settle(1.0)
vt = next(p for p in E.panes() if p["tab_id"] == "w1:t2")["pane_id"]
txt = E.result("pane.read", {"pane_id": vt, "source": "visible"})["text"]
check("hello_from_p2" in txt, "bhote view shows the pane", txt)
E.call("pane.send_text", {"pane_id": vt, "text": "typed_in_viewer\r"}); E.settle(0.6)
check("typed_in_viewer" in E.result("pane.read", {"pane_id": "w1:p2", "source": "visible"})["text"], "keys of the viewer go to the viewed pane")
check(E.rect("w1:p2") == size, "a viewer does not change the size of the pane", (E.rect("w1:p2"), size))
E.call("pane.close", {"pane_id": "w1:p2"}); E.settle(1.0)
check(vt not in [p["pane_id"] for p in E.panes()] or "pane closed" in E.result("pane.read", {"pane_id": vt, "source": "visible"}).get("text", "pane closed"), "the viewer ends with its pane")
E.call("tab.close", {"tab_id": "w1:t2"}); E.settle(0.3)
# the click on an agent of another machine
E.call("pane.split", {"pane_id": "w1:p1", "direction": "right", "focus": False}); E.settle(0.4)
other = next(p["pane_id"] for p in E.panes() if p["pane_id"] != "w1:p1")
E.call("pane.run", {"pane_id": other, "command": "echo agent_text_here; cat"}); E.settle(0.5)
E.call("pane.run", {"pane_id": "w1:p1", "command": f"BHOTE_SOURCE_ONLY=1 . {ROOT}/bhote; HOST=Mac; BHOTE_MUX=native; remote_view Omarchy {other}; echo rv=$?"}); E.settle(2.0)
tabs = E.tabs()
check(any(t["label"] == f"Omarchy {other}" for t in tabs), "remote_view makes a tab named after the machine and the pane", [t["label"] for t in tabs])
tab = next((t for t in tabs if t["label"] == f"Omarchy {other}"), None)
if tab:
    vp = next(p["pane_id"] for p in E.panes() if p["tab_id"] == tab["tab_id"])
    check("agent_text_here" in E.result("pane.read", {"pane_id": vp, "source": "visible"})["text"], "and it shows that agent's pane", E.result("pane.read", {"pane_id": vp, "source": "visible"}))
    E.call("tab.focus", {"tab_id": "w1:t1"})
    E.call("pane.run", {"pane_id": "w1:p1", "command": f"BHOTE_SOURCE_ONLY=1 . {ROOT}/bhote; HOST=Mac; BHOTE_MUX=native; remote_view Omarchy {other}"}); E.settle(1.0)
    check(len([t for t in E.tabs() if t["label"] == f"Omarchy {other}"]) == 1 and next(t for t in E.tabs() if t["label"] == f"Omarchy {other}")["focused"], "a second click shows the same tab (no second one)")
E.done()
