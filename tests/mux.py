#!/usr/bin/env python3
"""The native mux in a real terminal (a pty): server start on demand, typing, detach and attach again, resize, modes, the control socket.
Skipped without the daemon binary. Usage: python3 tests/mux.py"""
import os, pty, sys, time, select, fcntl, termios, struct, tempfile, shutil, subprocess, json, signal
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "daemon", "target", "release", "bhoted")
if not os.path.exists(BIN):
    print("skipped: no bhoted binary"); sys.exit(0)
T = tempfile.mkdtemp()
SOCK = f"{T}/mux.sock"
open(f"{T}/config", "w").write("AUTOSTART=off\n")
ENV = dict(os.environ, BHOTE_CONFIG=f"{T}/config", BHOTE_MUX_SOCK=SOCK, BHOTE_MUX_STATE=f"{T}/state.json", SHELL="/bin/sh", PS1="$ ", TERM="xterm-256color", HOME=T, ENV="")
fails = 0
def ok(m): print("ok   " + m, flush=True)
def bad(m, extra=""):
    global fails; fails += 1; print("FAIL " + m + (("\n  " + extra) if extra else ""), flush=True)
def check(c, m, extra=""): ok(m) if c else bad(m, extra)
def _timeout(*_):
    print("FAIL the mux test ran out of time (90 s)", flush=True); cleanup(); os._exit(1)
signal.signal(signal.SIGALRM, _timeout); signal.alarm(90)

def call(method, params=None):
    r = subprocess.run([BIN, "mux-call", method, json.dumps(params or {})], capture_output=True, text=True, env=ENV, timeout=10)
    return json.loads(r.stdout) if r.stdout.strip() else {"error": r.stderr}

class Client:
    def __init__(self, cols=80, rows=24):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            os.execve(BIN, [BIN, "attach"], ENV)
        self.out = b""; self.status = None
    def pump(self, t=0.1):
        end = time.time() + t
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if r:
                try: d = os.read(self.fd, 65536)
                except OSError: return
                if not d: return
                self.out += d
    def wait_for(self, text, t=8):
        end = time.time() + t
        while time.time() < end:
            if text.encode() in self.out: return True
            self.pump(0.1)
        return False
    def send(self, s): os.write(self.fd, s.encode() if isinstance(s, str) else s)
    def resize(self, cols, rows): fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    def wait_exit(self, t=8):
        end = time.time() + t
        while time.time() < end:
            self.pump(0.05)
            p, st = os.waitpid(self.pid, os.WNOHANG)
            if p: self.status = os.waitstatus_to_exitcode(st); return True
        return False
    def kill(self):
        try: os.kill(self.pid, 9); os.waitpid(self.pid, 0)
        except OSError: pass

def cleanup():
    try: call("server.stop")
    except Exception: pass
    shutil.rmtree(T, ignore_errors=True)

# 1. first attach starts the server; type a command
c = Client()
check(c.wait_for("$ "), "attach starts the server and shows the shell prompt", repr(c.out[-300:]))
c.send("echo hello_$((20+1))\r")
check(c.wait_for("hello_21"), "typed command runs", repr(c.out[-300:]))
# 2. modes of the pane reach the terminal
c.send("printf '\\033[?2004h'\r")
check(c.wait_for("\x1b[?2004h"), "bracketed paste of the pane is switched on in the terminal")
c.send("printf '\\033[?2004l'\r")
# 3. detach
c.send("\x02d")
check(c.wait_exit() and c.status == 0, "ctrl+b d detaches (exit 0)", str(c.status))
check(b"[detached]" in c.out, "the client says it detached")
check(b"\x1b[?1049l" in c.out, "the client leaves the alternate screen")
# 4. the server survived, the pane has the line
r = call("pane.read")
check("hello_21" in r.get("result", {}).get("text", ""), "the server keeps the screen while detached", str(r))
# 5. attach again: same screen
c2 = Client(100, 30)
check(c2.wait_for("hello_21"), "attach again shows the same screen", repr(c2.out[-300:]))
def size():
    r = call("pane.layout")["result"]["layout"]["panes"][0]["rect"]   # (the bar takes the last row)
    return (r["width"], r["height"])
check(size() == (100, 29), "the pane has the size of the client (less the bar)", str(size()))
# 6. resize
c2.resize(90, 28)
time.sleep(0.5)
check(size() == (90, 27), "resize reaches the pane", str(size()))
c2.send("stty size\r")
check(c2.wait_for("27 90"), "the program in the pane sees the new size", repr(c2.out[-200:]))
# 7. literal prefix key and the control socket
r = call("pane.send_text", {"text": "echo via_socket\r"})
check(c2.wait_for("via_socket"), "pane.send_text types into the pane", str(r))
check(call("nonsense").get("error", {}).get("code") == "unknown_method", "an unknown method is an error")
# 7b. a full-screen program (alt screen, application cursor keys)
vi = shutil.which("vim") or shutil.which("vi")
if vi:
    n = len(c2.out)
    c2.send(f"{vi} -u NONE -N\r")
    time.sleep(1.0); c2.pump(0.3)
    check(b"\x1b[?1049h" in c2.out[n:] or b"\x1b[?47h" in c2.out[n:] or b"~" in c2.out[n:], "vim draws its screen", repr(c2.out[n:][-200:]))
    c2.send("ihello_vim\x1b")
    check(c2.wait_for("hello_vim"), "typing into vim works")
    c2.send(":q!\r")
    time.sleep(0.5); c2.pump(0.3)
    c2.send("echo back_$((1+2))\r")
    check(c2.wait_for("back_3"), "the shell is back after vim", repr(c2.out[-200:]))
# 8. stop
call("server.stop")
check(c2.wait_exit() and b"[stopped]" in c2.out, "server.stop ends the clients")
time.sleep(0.3)
check(not os.path.exists(SOCK), "the socket is gone")
try: os.remove(f"{T}/state.json")
except OSError: pass
# 9. the shell ending ends the server
c3 = Client()
check(c3.wait_for("$ "), "a fresh server starts again")
c3.send("exit\r")
check(c3.wait_exit() and b"[exited]" in c3.out, "the last pane ending ends the client", repr(c3.out[-200:]))
time.sleep(0.3)
check(not os.path.exists(SOCK), "and the server")
c.kill(); c3.kill()
cleanup()
sys.exit(1 if fails else 0)
