"""Helpers for the pty tests of the native mux: a server on a throw-away socket, a client in a pseudo terminal, control calls."""
import os, pty, sys, time, select, fcntl, termios, struct, tempfile, shutil, subprocess, json, signal
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "daemon", "target", "release", "bhoted")
if not os.path.exists(BIN):
    print("skipped: no bhoted binary"); sys.exit(0)

class Env:
    def __init__(self, keys=None, limit=120, config="AUTOSTART=off\n", panel_bin=None):
        self.T = tempfile.mkdtemp()
        self.sock = f"{self.T}/mux.sock"
        if keys is not None: open(f"{self.T}/keys", "w").write(keys)
        open(f"{self.T}/config", "w").write(config)
        if panel_bin is not None:
            open(f"{self.T}/panel", "w").write(panel_bin); os.chmod(f"{self.T}/panel", 0o755)
        self.env = dict(os.environ, BHOTE_CONFIG=f"{self.T}/config", BHOTE_BIN=f"{self.T}/panel" if panel_bin is not None else "/nonexistent", BHOTE_MUX_SOCK=self.sock, SHELL="/bin/sh", PS1="$ ", TERM="xterm-256color", HOME=self.T, ENV="", BHOTE_KEYS=f"{self.T}/keys")
        for k in ("HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID", "XDG_RUNTIME_DIR"): self.env.pop(k, None)
        self.fails = 0; self.clients = []
        signal.signal(signal.SIGALRM, lambda *_: (print("FAIL the test ran out of time", flush=True), self.cleanup(), os._exit(1))); signal.alarm(limit)
    def ok(self, m): print("ok   " + m, flush=True)
    def bad(self, m, extra=""):
        self.fails += 1; print("FAIL " + m + (("\n  " + str(extra)) if extra else ""), flush=True)
    def check(self, c, m, extra=""): self.ok(m) if c else self.bad(m, extra)
    def call(self, method, params=None):
        r = subprocess.run([BIN, "mux-call", method, json.dumps(params or {})], capture_output=True, text=True, env=self.env, timeout=10)
        return json.loads(r.stdout) if r.stdout.strip() else {"error": r.stderr}
    def result(self, method, params=None): return self.call(method, params).get("result", {})
    def panes(self): return self.result("pane.list").get("panes", [])
    def focused(self): return next((p["pane_id"] for p in self.panes() if p["focused"]), None)
    def layout(self, pane=None): return self.result("pane.layout", {"pane_id": pane} if pane else {}).get("layout", {})
    def rect(self, pane):
        return next(p for p in self.layout(pane)['panes'] if p['pane_id'] == pane)['rect']
    def tabs(self): return self.result("tab.list").get("tabs", [])
    def workspaces(self): return self.result("workspace.list").get("workspaces", [])
    def start(self):
        """A server without a client (the first attach starts one by itself)."""
        subprocess.Popen([BIN, "mux"], env=self.env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        for _ in range(60):
            if self.call("ping").get("result"): return
            time.sleep(0.1)
    def client(self, cols=100, rows=30):
        c = Client(self, cols, rows); self.clients.append(c); return c
    def settle(self, t=0.25): time.sleep(t)
    def cleanup(self):
        for c in self.clients: c.kill()
        try: self.call("server.stop")
        except Exception: pass
        shutil.rmtree(self.T, ignore_errors=True)
    def done(self):
        self.cleanup(); sys.exit(1 if self.fails else 0)

class Client:
    def __init__(self, e, cols, rows):
        self.e = e
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            os.execve(BIN, [BIN, "attach"], e.env)
        self.out = b""; self.status = None
    def pump(self, t=0.1):
        end = time.time() + t
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.03)
            if r:
                try: d = os.read(self.fd, 65536)
                except OSError: return
                if not d: return
                self.out += d
    def wait_for(self, text, t=6, since=0):
        end = time.time() + t; text = text.encode() if isinstance(text, str) else text
        while time.time() < end:
            if text in self.out[since:]: return True
            self.pump(0.1)
        return False
    def send(self, s, wait=0.15):
        os.write(self.fd, s.encode() if isinstance(s, str) else s); self.pump(wait)
    def mouse(self, b, x, y, press=True, wait=0.1):   # 1-based like the terminal reports
        self.send(f"\x1b[<{b};{x};{y}{'M' if press else 'm'}", wait)
    def resize(self, cols, rows): fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    def wait_exit(self, t=6):
        end = time.time() + t
        while time.time() < end:
            self.pump(0.05)
            p, st = os.waitpid(self.pid, os.WNOHANG)
            if p: self.status = os.waitstatus_to_exitcode(st); return True
        return False
    def kill(self):
        try: os.kill(self.pid, 9); os.waitpid(self.pid, 0)
        except (OSError, ChildProcessError): pass
