#!/usr/bin/env python3
"""The kharka sidecar of the native mux (STORE=kharka): the mux runs herdr-kharka against its own socket, with a throw-away kharka daemon:
what the other machines read is there (workspaces, the agent and its state), a killed sidecar comes back, it ends with the server.
Skipped without kharka and herdr-kharka. Usage: python3 tests/muxkharka.py"""
import shutil, subprocess, time, json, os
if not (shutil.which("kharka") and shutil.which("herdr-kharka")): print("skipped: no kharka"); raise SystemExit(0)
from muxlib import *
E = Env(config="AUTOSTART=off\nSTORE=kharka\n")
check = E.check
os.makedirs(f"{E.T}/abin"); open(f"{E.T}/abin/claude", "w").write("#!/bin/sh\nexec cat\n"); os.chmod(f"{E.T}/abin/claude", 0o755)
E.env.update(PATH=f"{E.T}/abin:" + E.env["PATH"], KHARKA_SOCKET=f"{E.T}/k.sock", KHARKA_DATA=f"{E.T}/kd", KHARKA_NODE="testnode")
d = subprocess.Popen(["kharka", "daemon", "--hub", "local"], env=E.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
def entry():
    r = subprocess.run(["kharka", "get", "herdr/testnode"], capture_output=True, text=True, env=E.env)
    return json.loads(r.stdout) if r.stdout.strip() else {}
def wait(f, t=12):
    end = time.time() + t
    while time.time() < end:
        if f(): return True
        time.sleep(0.4)
    return False
E.start(); c = E.client(); c.wait_for("$ ")
check(wait(lambda: entry().get("herdr") == "online"), "the mux starts herdr-kharka by itself: the entry is written", entry())
check(entry().get("workspaces", [{}])[0].get("id") == "w1", "the entry has the workspaces", entry())
lnk = f"{E.T}/lnk"; os.makedirs(lnk); os.symlink(BIN, f"{lnk}/herdr")
env2 = dict(E.env, PATH=f"{lnk}:" + E.env["PATH"])
E.call("pane.run", {"pane_id": "w1:p1", "command": "claude"}); time.sleep(0.5)
subprocess.run(["herdr", "pane", "report-agent", "w1:p1", "--source", "t", "--agent", "claude", "--state", "working", "--agent-session-id", "s1"], env=env2, capture_output=True)
check(wait(lambda: [a.get("pane") for a in entry().get("agents", [])] == ["w1:p1"] and entry()["agents"][0].get("status") == "working"), "the agent and its state follow", entry().get("agents"))
# a killed sidecar comes back
pid = int(open(os.path.join(os.path.dirname(E.sock), "kharka.pid")).read())
os.kill(pid, 9); time.sleep(0.5)
def newpid():
    try: return int(open(os.path.join(os.path.dirname(E.sock), "kharka.pid")).read())
    except OSError: return pid
check(wait(lambda: newpid() != pid, 15), "a sidecar that ends is started again")
# it ends with the server
pid = newpid()
E.call("server.stop"); time.sleep(1.0)
try: os.kill(pid, 0); alive = True
except OSError: alive = False
check(not alive, "the sidecar ends with the server")
d.kill()
E.done()
