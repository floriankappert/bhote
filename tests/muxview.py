#!/usr/bin/env python3
"""A pane of another machine in a tab of its own: `bhote view <pane>` shows one pane of a server (keys go to it, it is not resized, it ends with the pane),
and a click on an agent of another machine (remote_view of the script) opens such a tab over ssh. Here ssh is a fake that runs the command on this machine,
so the 'other machine' is the same server. Usage: python3 tests/muxview.py"""
import os, time, subprocess
from muxlib import *
E = Env()
check = E.check
os.makedirs(f"{E.T}/bin"); os.makedirs(f"{E.T}/shared")
open(f"{E.T}/fakessh", "w").write('#!/bin/sh\n# fake ssh: ssh [-t] [-o x]... <node> <command>: runs the command here\nwhile [ "$1" = "-t" ] || [ "$1" = "-o" ]; do if [ "$1" = "-o" ]; then shift 2; else shift; fi; done\nshift\nexec sh -c "$1"\n'); os.chmod(f"{E.T}/fakessh", 0o755)
os.symlink(os.path.join(ROOT, "bhote"), f"{E.T}/bin/bhote")
os.makedirs(f"{E.T}/.local/bin"); os.symlink(os.path.join(ROOT, "bhote"), f"{E.T}/.local/bin/bhote")   # (where remote_cmd looks on the other machine)
open(f"{E.T}/shared/agents.nodes", "w").write("Omarchy\x1fnode-omarchy\x1f0\x1fagent.prompt\n")
open(f"{E.T}/config", "a").write(f"REMOTE_SSH={E.T}/fakessh -t {{node}}\n")
E.env.update(BHOTE_VIEW_GRACE="2", PATH=f"{E.T}/bin:" + E.env["PATH"], BHOTED_BIN=BIN, BHOTE_SHARED=f"{E.T}/shared", BHOTE_DATA=f"{E.T}/data", SHELL="/bin/sh")
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
vr = E.rect(vt)
rows = next(p for p in E.panes() if p["pane_id"] == "w1:p2")["scroll"]["viewport_rows"]
check(rows == vr["height"], "a viewer gives the pane the size of its own window (a program there uses the whole window)", (rows, vr, size))
E.call("pane.close", {"pane_id": "w1:p2"}); E.settle(1.0)
check(vt not in [p["pane_id"] for p in E.panes()] or "pane closed" in E.result("pane.read", {"pane_id": vt, "source": "visible"}).get("text", "pane closed"), "the viewer ends with its pane")
E.call("tab.close", {"tab_id": "w1:t2"}); E.settle(0.3)
# a click on a workspace (or agent) of another machine: it is a workspace of its own here, named after the machine and the workspace, not in the list
E.call("pane.split", {"pane_id": "w1:p1", "direction": "right", "focus": False}); E.settle(0.4)
other = next(p["pane_id"] for p in E.panes() if p["pane_id"] != "w1:p1")
E.call("pane.run", {"pane_id": other, "command": "echo agent_text_here; cat"}); E.settle(0.5)
ws = other.split(":")[0]
open(f"{E.T}/shared/workspaces.remote", "w").write(f"Omarchy\x1f{ws}\x1fIMS (wt1)\x1f1\x1fidle\n")
open(f"{E.T}/shared/agents.meta.remote", "w").write(f"Omarchy\x1f{other}\x1fw1:t1\x1fa\x1f\x1f\x1f/tmp\n")
call = f"bash -c 'BHOTE_SOURCE_ONLY=1 . {ROOT}/bhote; HOST=Mac; BHOTE_MUX=native; remote_workspace Omarchy {ws}'"   # (bash: on Ubuntu /bin/sh is dash)
E.call("pane.run", {"pane_id": "w1:p1", "command": call}); E.settle(2.0)
check([w["label"] for w in E.workspaces()] == [E.workspaces()[0]["label"]], "the mounted workspace is not in the workspace list", E.workspaces())
allws = E.result("workspace.list", {"all": True})["workspaces"]
mount = next((w for w in allws if w["label"] == "Omarchy · IMS (wt1)"), None)
check(mount is not None and mount["focused"], "remote_workspace makes it and shows it", allws)
if mount:
    vp = next(p["pane_id"] for p in E.panes() if p["workspace_id"] == mount["workspace_id"])
    check("agent_text_here" in E.result("pane.read", {"pane_id": vp, "source": "visible"})["text"], "it shows that agent's pane", E.result("pane.read", {"pane_id": vp, "source": "visible"}))
    E.call("workspace.focus", {"workspace_id": "w1"})
    E.call("pane.run", {"pane_id": "w1:p1", "command": call}); E.settle(1.0)
    allws = E.result("workspace.list", {"all": True})["workspaces"]
    check(len([w for w in allws if w["label"] == "Omarchy · IMS (wt1)"]) == 1 and next(w for w in allws if w["label"] == "Omarchy · IMS (wt1)")["focused"], "a second click shows the same one (no second)")
    # the same without the script (bhoted mount: what a click in the panel does): the mount that is there is shown, none is made twice
    E.call("workspace.focus", {"workspace_id": "w1"})
    subprocess.run([BIN, "mount", "Omarchy", ws, other], env=E.env, capture_output=True, timeout=20); E.settle(1.0)
    allws = E.result("workspace.list", {"all": True})["workspaces"]
    check(len([w for w in allws if w["label"] == "Omarchy · IMS (wt1)"]) == 1 and next(w for w in allws if w["label"] == "Omarchy · IMS (wt1)")["focused"], "bhoted mount shows the mount that is there (no second one)", [(w["label"], w["focused"]) for w in allws])
    open(f"{E.T}/shared/workspaces.remote", "a").write("Omarchy\x1fw9\x1fSecond one\x1f2\x1fidle\n")
    subprocess.run([BIN, "mount", "Omarchy", "w9", other], env=E.env, capture_output=True, timeout=20); E.settle(1.2)
    allws = E.result("workspace.list", {"all": True})["workspaces"]
    sec = next((w for w in allws if w["label"] == "Omarchy · Second one"), None)
    check(sec is not None and "agent_text_here" in E.result("pane.read", {"pane_id": next(p["pane_id"] for p in E.panes() if p["workspace_id"] == sec["workspace_id"]), "source": "visible"}).get("text", ""), "bhoted mount makes a mount that shows the pane (ssh + view)", [w["label"] for w in allws])
    # nobody looks any more: the views end (the ssh connection and the size over there are let go)
    c.kill(); time.sleep(4)
    check(not any(w["label"] == "Omarchy · IMS (wt1)" for w in E.result("workspace.list", {"all": True})["workspaces"]), "without a client the view ends after the grace time")
# herdr --machine <label> ...: the command runs over there (here: the same server) and the answer comes back
lnk = f"{E.T}/lnk"; os.makedirs(lnk); os.symlink(BIN, f"{lnk}/herdr")
henv = dict(E.env, PATH=f"{lnk}:" + E.env["PATH"])
r = subprocess.run(["herdr", "--machine", "Omarchy", "pane", "list"], env=henv, capture_output=True, text=True)
check(r.returncode == 0 and '"panes"' in r.stdout, "herdr --machine <label> pane list is answered by the other machine", r.stdout[:200] + r.stderr)
pid0 = E.panes()[0]["pane_id"]
E.call("pane.send_text", {"pane_id": pid0, "text": "echo marker_$((6*7))\r"}); time.sleep(0.5)
r = subprocess.run(["herdr", "--machine", "Omarchy", "pane", "read", pid0, "--source", "visible"], env=henv, capture_output=True, text=True)
check(r.returncode == 0 and "marker_42" in r.stdout, "also with options and a text answer (pane read)", r.stdout[:200] + r.stderr)
r = subprocess.run(["herdr", "--machine", "Nowhere", "pane", "list"], env=henv, capture_output=True, text=True)
check(r.returncode == 1 and "no machine Nowhere" in r.stderr, "an unknown machine is an error")

E.done()
