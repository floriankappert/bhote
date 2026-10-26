#!/usr/bin/env python3
"""Speed of the native mux: the echo of a key (typed in a client, shown in the client), a flood of output, control calls, a switch of workspace.
The numbers are printed; the limits are generous (a loaded CI machine) but would catch a mux that is 10 times slower than it should be.
Usage: python3 tests/muxperf.py"""
import os, time, statistics, subprocess, sys
from muxlib import *
E = Env(limit=180)
check = E.check
E.start(); c = E.client(200, 50); assert c.wait_for("$ ")
E.call("pane.run", {"command": "cat"}); time.sleep(0.5); c.pump(0.3)
# 1. the echo of a key
lat = []
for i in range(40):
    ch = "abcdefghijklmnopqrstuvwxyz"[i % 26]
    mark = len(c.out); t0 = time.perf_counter()
    os.write(c.fd, ch.encode())
    end = t0 + 2
    while time.perf_counter() < end:
        import select
        r, _, _ = select.select([c.fd], [], [], 0.002)
        if r:
            c.out += os.read(c.fd, 65536)
            if ch.encode() in c.out[mark:]: break
    lat.append((time.perf_counter() - t0) * 1000)
    time.sleep(0.02)
med, p95 = statistics.median(lat), sorted(lat)[int(len(lat) * 0.95) - 1]
print(f"     key echo: median {med:.1f} ms, p95 {p95:.1f} ms")
check(med < 25 and p95 < 80, "a key is echoed within 25 ms (median) / 80 ms (p95)", (med, p95))
os.write(c.fd, b"\x03"); time.sleep(0.3)
# 2. a flood of output
E.call("pane.run", {"command": "seq 1 300000; echo FLOOD_$((6*7))"})
t0 = time.perf_counter()
ok = c.wait_for("FLOOD_42", t=30)
dt = time.perf_counter() - t0
print(f"     flood: 300000 lines shown after {dt:.2f} s, {len(c.out) // 1024} KB sent to the client")
check(ok and dt < 8, "300000 lines of output are through in 8 s", dt)
check(len(c.out) < 40 * 1024 * 1024, "and the client got a bounded amount of bytes (a diff, not every line)", len(c.out))
# 3. control calls
t = []
for _ in range(30):
    t0 = time.perf_counter(); subprocess.run([BIN, "herdr", "pane", "list"], env=E.env, capture_output=True); t.append((time.perf_counter() - t0) * 1000)
print(f"     herdr pane list: median {statistics.median(t):.1f} ms")
check(statistics.median(t) < 40, "a herdr command answers within 40 ms", statistics.median(t))
# 4. a switch of workspace (what a click in the sidebar does) while a pane floods
E.call("workspace.create", {"label": "two", "focus": True}); time.sleep(0.3)
E.call("pane.run", {"pane_id": "w1:p1", "command": "yes | head -n 2000000 >/dev/null; while :; do echo busy; sleep 0.01; done"})
t = []
for i in range(20):
    t0 = time.perf_counter()
    try: E.call("workspace.focus", {"workspace_id": "w1" if i % 2 else "w2"})
    except subprocess.TimeoutExpired:   # (diagnosis for a machine that is not here: where is the server stuck?)
        pid = subprocess.run(["pgrep", "-n", "-f", "bhoted mux"], capture_output=True, text=True).stdout.split()
        print("SERVER STUCK, pids", pid, flush=True)
        if pid and sys.platform == "darwin":
            r = subprocess.run(["sample", pid[0], "2", "-file", "/tmp/sample.txt"], capture_output=True, text=True, timeout=30)
            print(open("/tmp/sample.txt").read()[:6000] if os.path.exists("/tmp/sample.txt") else r.stdout[:3000], flush=True)
        elif pid:
            print(subprocess.run(["sh", "-c", f"cat /proc/{pid[0]}/stack 2>/dev/null; ls /proc/{pid[0]}/task | wc -l; for t in /proc/{pid[0]}/task/*; do echo $t $(cat $t/wchan 2>/dev/null) $(cat $t/comm); done"], capture_output=True, text=True).stdout[:3000], flush=True)
        check(False, "the server answers under load", "no answer in 10 s"); E.done()
    t.append((time.perf_counter() - t0) * 1000)
print(f"     workspace.focus under load: median {statistics.median(t):.1f} ms")
check(statistics.median(t) < 60, "a switch of workspace answers within 60 ms even while a pane writes", statistics.median(t))
# 5. a client that does not read (a terminal that hangs, a slow ssh): the server must not wait for it, and it gets the right picture when it reads again
E.call("workspace.focus", {"workspace_id": "w1"}); E.call("pane.focus", {"pane_id": "w1:p1"})
E.call("pane.send_keys", {"pane_id": "w1:p1", "keys": ["ctrl+c"]}); time.sleep(0.3)
E.call("pane.send_text", {"pane_id": "w1:p1", "text": "clear; for i in $(seq 1 3000); do echo line_$i; done; echo STUCK_DONE_$((6*7))\r"})
t = []
for i in range(15):
    t0 = time.perf_counter(); E.call("ping"); t.append((time.perf_counter() - t0) * 1000); time.sleep(0.1)   # (the client is not read from meanwhile)
print(f"     ping while a client does not read: max {max(t):.1f} ms")
check(max(t) < 500, "the server answers while a client does not read", max(t))
ok_ = c.wait_for("STUCK_DONE_42", t=20)
check(ok_, "and the client that reads again gets the picture", "pane: " + E.result("pane.read", {"pane_id": "w1:p1", "source": "visible"}).get("text", "")[-400:] + "\nclient bytes: " + str(len(c.out)) + " tail: " + repr(c.out[-300:]))
E.done()
