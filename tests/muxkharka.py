#!/usr/bin/env python3
"""herdr-kharka (the program that writes a machine's agents into kharka) against the native mux's socket, with a throw-away kharka daemon:
what the other machines read is there, with the agent and its state. Skipped without kharka and herdr-kharka. Usage: python3 tests/muxkharka.py"""
import shutil, subprocess, time, json
if not (shutil.which("kharka") and shutil.which("herdr-kharka")): print("skipped: no kharka"); raise SystemExit(0)
from muxlib import *
E = Env()
check = E.check
import os
os.makedirs(f"{E.T}/abin"); open(f"{E.T}/abin/claude", "w").write("#!/bin/sh\nexec cat\n"); os.chmod(f"{E.T}/abin/claude", 0o755)
E.env["PATH"] = f"{E.T}/abin:" + E.env["PATH"]
E.start(); c = E.client(); c.wait_for("$ ")
E.call("pane.split", {"pane_id": "w1:p1", "direction": "right"})
lnk = f"{E.T}/lnk"; os.makedirs(lnk); os.symlink(BIN, f"{lnk}/herdr")
env = dict(E.env, KHARKA_SOCKET=f"{E.T}/k.sock", KHARKA_DATA=f"{E.T}/kd", KHARKA_NODE="testnode", PATH=f"{lnk}:" + E.env["PATH"])
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"}); time.sleep(0.5)
subprocess.run(["herdr", "pane", "report-agent", "w1:p1", "--source", "t", "--agent", "claude", "--state", "working", "--agent-session-id", "s1"], env=env, capture_output=True)
d = subprocess.Popen(["kharka", "daemon", "--hub", "local"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
r = subprocess.run(["herdr-kharka", "--once", "--herdr-socket", E.sock], capture_output=True, text=True, env=env)
check(r.returncode == 0, "herdr-kharka reads the mux and writes its entry", r.stderr)
v = json.loads(subprocess.run(["kharka", "get", "herdr/testnode"], capture_output=True, text=True, env=env).stdout or "{}")
check(v.get("herdr") == "online" and v.get("workspaces", [{}])[0].get("id") == "w1", "the entry has the workspaces", v)
check([a.get("pane") for a in v.get("agents", [])] == ["w1:p1"] and v["agents"][0].get("agent") == "claude", "and the agent of the pane", v.get("agents"))
d.kill()
E.done()
