#!/usr/bin/env python3
"""`bhote` typed in a terminal starts the native mux (and attaches); MUX=herdr starts herdr instead; bhote doctor runs. Usage: python3 tests/muxlaunch.py"""
import os, sys
from muxlib import *
E = Env()
check = E.check
env = dict(E.env, BHOTED_BIN=BIN, BHOTE_DATA=f"{E.T}/data", BHOTE_SHARED=f"{E.T}/shared", NO_COLOR="1")
os.makedirs(f"{E.T}/fb"); open(f"{E.T}/fb/herdr", "w").write("#!/bin/sh\necho FAKE_HERDR_STARTED\nsleep 1\n"); os.chmod(f"{E.T}/fb/herdr", 0o755)
BH = os.path.join(ROOT, "bhote")
def run(extra):
    e2 = dict(env, **extra); e2["PATH"] = f"{E.T}/fb:" + e2["PATH"]
    import pty, fcntl, termios, struct, select, time
    pid, fd = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0)); os.execve("/bin/bash", ["bash", BH], e2)
    return pid, fd
def drain(fd, t):
    import select, time
    out = b""; end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try: d = os.read(fd, 65536)
            except OSError: break
            if not d: break
            out += d
    return out
pid, fd = run({})
out = drain(fd, 4)
check(E.result("ping").get("pid") is not None, "bhote in a terminal starts the mux server", out[-200:])
os.write(fd, b"\x02d"); out += drain(fd, 1.5)
check(b"[detached]" in out, "and attaches (ctrl+b d detaches)", out[-200:])
E.call("server.stop")
pid2, fd2 = run({"BHOTE_MUX": "herdr"})
out2 = drain(fd2, 3)
check(b"FAKE_HERDR_STARTED" in out2, "MUX=herdr starts herdr instead", out2[-200:])
import subprocess
r = subprocess.run(["bash", BH, "doctor"], capture_output=True, text=True, env=dict(env, PATH=f"{E.T}/fb:" + env["PATH"]))
check("bhoted:" in r.stdout and r.returncode == 0, "bhote doctor", r.stdout + r.stderr)
E.done()
