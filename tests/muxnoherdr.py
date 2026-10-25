#!/usr/bin/env python3
"""bhote without herdr: no herdr program on the PATH at all. The mux starts, the REAL panels (the left one with the workspaces and agents) draw in it,
the command line (bhote agents, bhote doctor, bhote reload) talks to the mux from a plain terminal, notifications and agent focus work.
Usage: python3 tests/muxnoherdr.py"""
import os, shutil, subprocess, time, json
from muxlib import *
E = Env(config="NOTIFY=off\nSETUP_DONE=1\nWIZARD_LEVEL=9\nUPDATE_CHECK=off\nSPLASH_AUTOCLOSE=off\nPANEL_LEFT=workspaces,agents,overview\nPANEL_RIGHT=topics\n")
check = E.check
# a PATH without herdr: the system's, nothing of the user's
sysbin = "/usr/bin:/bin"
assert not any(os.path.exists(os.path.join(d, "herdr")) for d in sysbin.split(":")), "herdr in the system path: this test needs a machine where it is not"
for d in ("data/topics", "shared"): os.makedirs(f"{E.T}/{d}")
os.makedirs(f"{E.T}/abin"); open(f"{E.T}/abin/claude", "w").write("#!/bin/sh\nexec cat\n"); os.chmod(f"{E.T}/abin/claude", 0o755)
tools = {os.path.dirname(shutil.which(x)) for x in ("jq", "git", "python3") if shutil.which(x)}   # (where the tools are, but no herdr)
sysbin = ":".join([sysbin] + sorted(d for d in tools if d not in sysbin.split(":") and not os.path.exists(os.path.join(d, "herdr"))))
E.env.update(PATH=f"{E.T}/abin:{sysbin}", BHOTE_BIN=os.path.join(ROOT, "bhote"), BHOTED_BIN=BIN, BHOTE_DATA=f"{E.T}/data", BHOTE_SHARED=f"{E.T}/shared", NO_COLOR="1")
E.env.pop("BHOTE_MUX", None)
E.start(); c = E.client(220, 50)
assert c.wait_for("$ ")
time.sleep(6)
sbs = {s["label"]: s["pane_id"] for s in E.layout().get("sidebars", [])}
check(set(sbs) == {"bhote-left", "bhote-right"}, "both real panels are there", sbs)
left = E.result("pane.read", {"pane_id": sbs["bhote-left"], "source": "visible"})["text"]
check("Workspaces" in left and ("bhote (" in left or "tests" in left), "the left panel lists the workspaces (it asked the mux, there is no herdr)", left[:300])
# an agent in the main pane: it shows in the panel and on the command line
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"}); time.sleep(0.5)
lnk = f"{E.T}/lnk"; os.makedirs(lnk); os.symlink(BIN, f"{lnk}/herdr")
subprocess.run(["herdr", "pane", "report-agent", "w1:p1", "--source", "t", "--agent", "claude", "--state", "working", "--agent-session-id", "s1"], env=dict(E.env, PATH=f"{lnk}:" + E.env["PATH"]), capture_output=True)
time.sleep(4)
left = E.result("pane.read", {"pane_id": sbs["bhote-left"], "source": "visible"})["text"]
check("working" in left or "✶" in left, "the left panel shows the agent", left[:500])
def bhote(*a): return subprocess.run(["bash", os.path.join(ROOT, "bhote"), *a], capture_output=True, text=True, env=E.env, timeout=60)
r = bhote("agents")
check(r.returncode == 0 and "working" in r.stdout, "`bhote agents` in a plain terminal lists it", r.stdout + r.stderr)
r = bhote("doctor")
check("herdr is not needed (and not installed)" in r.stdout and "typing bhote in this terminal: start the native mux" in r.stdout, "`bhote doctor` knows it needs no herdr", r.stdout)
r = bhote("reload")
check(r.returncode == 0 and "panels restarted" in r.stdout, "`bhote reload` restarts the panels of the mux", r.stdout + r.stderr)
time.sleep(5)
check({s["label"] for s in E.layout().get("sidebars", [])} == {"bhote-left", "bhote-right"}, "and they are back")
r = bhote("add", "A topic without herdr")
check(r.returncode == 0, "`bhote add` works", r.stdout + r.stderr)
E.done()
