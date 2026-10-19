#!/usr/bin/env python3
"""The Rust panel in a real terminal (a pty): the main view, the mouse, the keys, a line to type into, and the views that bash still draws.
Skipped without the daemon binary. Usage: python3 tests/panel.py"""
import os, pty, sys, time, select, fcntl, termios, struct, re, tempfile, shutil, glob, subprocess
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "daemon", "target", "release", "bhoted")
if not os.path.exists(BIN):
    print("skipped: no bhoted binary"); sys.exit(0)
import signal
CUR = [None]
LIVE = []
def _timeout(*_):   # a test that hangs must not hold the build: end it, and the panels it started
    print("FAIL the panel test ran out of time (240 s)", flush=True)
    p = CUR[0]
    if p is not None:
        tail = p.out[-900:].decode("utf8", "replace").replace("\x1b", "<E>")
        print("  last output (%d bytes, alive=%s): %s" % (len(p.out), p.alive, tail), flush=True)
    import traceback; traceback.print_stack(file=sys.stdout); sys.stdout.flush()
    for pid in LIVE:
        try: os.killpg(pid, 9)
        except OSError: pass
    os._exit(1)
signal.signal(signal.SIGALRM, _timeout); signal.alarm(240)
def _excepthook(tp, val, tb):   # an error in the test itself is a failure, with what the terminal showed last
    import traceback; print("FAIL the test broke: %s: %s" % (tp.__name__, val), flush=True); traceback.print_tb(tb, file=sys.stdout)
    p = CUR[0]
    if p is not None: print("  last output: " + p.out[-600:].decode("utf8", "replace").replace("\x1b", "<E>"), flush=True)
    for pid in LIVE:
        try: os.killpg(pid, 9)
        except OSError: pass
    os._exit(1)
sys.excepthook = _excepthook
fails = 0
def ok(m): print("ok   " + m, flush=True)
def bad(m):
    global fails; fails += 1; print("FAIL " + m, flush=True)
    p = CUR[0]
    if p is not None:   # what the terminal showed last (escape codes made visible), to see why
        tail = p.out[-700:].decode("utf8", "replace").replace("\x1b", "<E>")
        print("  last output (%d bytes, alive=%s): %s" % (len(p.out), p.alive, tail), flush=True)

class Panel:
    def __init__(self, cols=70, rows=40, cfg="", topics=None):
        self.T = tempfile.mkdtemp(); T = self.T
        for d in ("data/topics", "shared", "home"): os.makedirs(f"{T}/{d}")
        open(f"{T}/config", "w").write("BHOTED_PANEL=on\nCARD_GROUPING=status\nNOTIFY=off\nSETUP_DONE=1\nWIZARD_LEVEL=9\nUPDATE_CHECK=off\nSPLASH_AUTOCLOSE=off\n" + cfg)
        self.now = int(time.time())
        for t in (topics or DEFAULT):
            self.topic(**t)
        env = dict(os.environ, BHOTE_CONFIG=f"{T}/config", BHOTE_DATA=f"{T}/data", BHOTE_SHARED=f"{T}/shared", HOME=f"{T}/home", TMPDIR=T, TERM="xterm-256color", BHOTE_FORCE_COLOR="1")
        for k in ("NO_COLOR", "HERDR_PANE_ID", "HERDR_TAB_ID", "CLAUDECODE"): env.pop(k, None)
        self.pid, self.fd = pty.fork()
        if self.pid: LIVE.append(self.pid)
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))   # the size is there before the panel looks (a race on a slow machine otherwise)
            os.chdir(ROOT); os.execvpe("bash", ["bash", "./bhote"], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self.out = b""; self.alive = True; CUR[0] = self
        self.drain(2.5)
        if b"Bhote v" not in self.out:
            tail = self.out[-900:].decode("utf8", "replace").replace("\x1b", "<E>")
            print("  DIAG: no main view after start (%d bytes, alive=%s): %s" % (len(self.out), self.alive, tail), flush=True)
    def topic(self, id, **kw):
        d = {"id": id, "updated": self.now, "deleted": 0}; d.update(kw)
        open(f"{self.T}/data/topics/{id}.topic", "w").write("".join(f"{k}={v}\n" for k, v in d.items()))
    def file(self, id): return open(f"{self.T}/data/topics/{id}.topic").read()
    def get(self, id, key):
        for l in self.file(id).splitlines():
            if l.startswith(key + "="): return l[len(key) + 1:]
        return None
    def drain(self, t=0.8):
        end = time.time() + t
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if r:
                try: d = os.read(self.fd, 65536)
                except OSError: self.alive = False; return
                if not d: self.alive = False; return
                self.out += d
    def send(self, s, wait=0.9):
        self.out = b""
        os.write(self.fd, s.encode() if isinstance(s, str) else s)
        self.drain(wait)
    def expect(self, text, timeout=6.0):
        """Waits (the output is kept) until the text has been drawn; the screen is then the last frame."""
        end = time.time() + timeout
        while time.time() < end:
            if text in re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", self.out.decode("utf8", "replace")): return True
            self.drain(0.2)
        return False
    def screen(self):
        """The last full frame as lines of text (a frame starts with ESC [ H)."""
        s = self.out.decode("utf8", "replace")
        i = s.rfind("\x1b[H")
        s = s[i:] if i >= 0 else s
        s = re.sub(r"\x1b\][^\x07]*\x07", "", s)
        s = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", s)
        return s.split("\n")
    def row(self, text):
        for i, l in enumerate(self.screen()):
            if text in l: return i + 1
        return None
    def fresh_row(self, text, timeout=6.0):
        """The row of a text in a frame that is drawn now (a resize signal forces a redraw)."""
        self.out = b""; os.kill(self.pid, signal.SIGWINCH)
        self.expect(text, timeout)
        return self.row(text)
    def click(self, x, y, b=0):
        self.send(f"\x1b[<{b};{x};{y}M\x1b[<{b};{x};{y}m")
    def close(self):
        """Ends the panel: q, then SIGTERM, then SIGKILL on it and its group; never waits for ever."""
        def gone():
            try: return os.waitpid(self.pid, os.WNOHANG)[0] != 0
            except OSError: return True
        try: os.write(self.fd, b"q")
        except OSError: pass
        t0 = time.time()
        while time.time() - t0 < 1.5 and not gone(): self.drain(0.1)
        if not gone():
            try: os.kill(self.pid, 15)
            except OSError: pass
            t0 = time.time()
            while time.time() - t0 < 1.5 and not gone(): self.drain(0.1)
        if not gone():
            for f in (lambda: os.killpg(self.pid, 9), lambda: os.kill(self.pid, 9)):
                try: f()
                except OSError: pass
            t0 = time.time()
            while time.time() - t0 < 3 and not gone(): time.sleep(0.1)
        try: os.close(self.fd)
        except OSError: pass
        shutil.rmtree(self.T, ignore_errors=True)

DEFAULT = [
    dict(id="1000-1", title="First review", status="review", review_note="Check it", review_ask="Ok¦Stop"),
    dict(id="1001-1", title="Working topic", status="now"),
    dict(id="1002-1", title="A next topic", status="next"),
    dict(id="1003-1", title="Waiting for Anna", status="waiting", waiting_for="Anna", waiting_since=int(time.time()) - 100),
    dict(id="1004-1", title="Done thing", status="done"),
]

# 1) the main view comes up
p = Panel()
sc = "\n".join(p.screen())
for want in ("Bhote v", "REVIEW", "First review", "NOW", "Working topic", "A next topic", "Waiting for Anna", "Done thing", "Agents"):
    if want not in sc: bad(f"the main view shows {want!r}")
else:
    if all(w in sc for w in ("REVIEW", "NOW", "Agents")): ok("the main view comes up in the Rust panel (cards, lines, done, agents)")

# 1b) the collector runs for the panel (it holds the lead lock, and ends with the panel)
lock = os.path.join(p.T, "shared", "collector.lock"); t0 = time.time()
while not os.path.islink(lock) and time.time() - t0 < 6: time.sleep(0.2)
ok("the collector runs for the Rust panel (the lead lock is held)") if os.path.islink(lock) else bad("no collector lock: the collector is not running")

# 2) a click selects, x checks it off
y = p.fresh_row("A next topic")
p.click(10, y)
p.send("x")
st = p.get("1002-1", "status")
ok("click on a line, x: the topic is done") if st == "done" else bad(f"x: status of 1002-1 is {st}")

# 3) the card of a review topic has no question under it any more: the key 1 (the first answer) goes on with the topic
y = p.fresh_row("First review")
sc = "\n".join(p.screen())
bad("the review card still shows its question or buttons") if "Check it" in sc or "[1] Ok" in sc else ok("a review card has no question or buttons under it")
p.click(10, y)
p.send("1", 1.0)
time.sleep(1.0)
st = p.get("1000-1", "status")
ok("click on a review card, key 1: the topic goes on") if st == "now" else bad(f"review key 1: status is {st}")

# 4) a line to type into: n, then a title
p.send("n", 0.5)
p.send("Brand new topic; with a note\r", 1.0)
files = [f for f in glob.glob(p.T + "/data/topics/*.topic") if "Brand new topic" in open(f).read()]
if files and "description=with a note" in open(files[0]).read(): ok("n: a topic made from the typed line (title; description)")
else: bad("n: no new topic")

# 5) edit the title of the selected topic (e), Esc keeps the old one
y = p.fresh_row("Brand new topic"); p.click(10, y)
p.send("e", 0.5); p.send("\x1b", 0.8)
ok("e then Esc: no change") if "Brand new topic" in open(files[0]).read() else bad("e then Esc changed the topic")

# 6) the wheel and the hover must not break anything; q ends the panel
p.send("\x1b[<64;10;10M\x1b[<35;12;12M\x1b[<35;13;14M", 0.6)
sc = "\n".join(p.screen())
ok("hover and wheel: the panel still draws") if ("REVIEW" in sc or "NOW" in sc) else bad("hover/wheel: the screen is gone")
p.close()

# 7) the menu of a topic is the script's view: a double click opens it, Esc comes back
p = Panel()
y = p.fresh_row("Working topic"); Y_WORKING = y; p.out = b""
os.write(p.fd, (f"\x1b[<0;10;{y}M\x1b[<0;10;{y}m" * 2).encode())   # a double click: both in one write (a slow machine must not stretch it past 500 ms)
menu = p.expect("ACTIONS", 10)
p.out = b""; os.write(p.fd, b"\x1b"); back = p.expect("Agents", 8)
ok("double click: the topic's menu (the script's view), Esc: back on the list") if (menu and back) else bad(f"menu round trip (menu={menu}, back={back})")

# 7b) a review card too: two clicks a quarter of a second apart (the first one must not have taken the focus away yet) open its page
p.out = b""; p.expect("Agents", 2)
y = p.fresh_row("First review")
os.write(p.fd, f"\x1b[<0;10;{y}M\x1b[<0;10;{y}m".encode()); time.sleep(0.25); os.write(p.fd, f"\x1b[<0;10;{y}M\x1b[<0;10;{y}m".encode())
menu = p.expect("ACTIONS", 10)
p.out = b""; os.write(p.fd, b"\x1b"); back = p.expect("Agents", 8)
ok("double click on a review card: its page") if (menu and back) else bad(f"review double click (menu={menu}, back={back})")

# 8) settings (the script's view) and back
p.out = b""; os.write(p.fd, b","); sets = p.expect("ettings", 8) or p.expect("SETTINGS", 1)
p.out = b""; os.write(p.fd, b"\x1b"); back = p.expect("Agents", 8)
ok("comma: the settings (the script's view), Esc: back") if (sets and back) else bad(f"settings round trip (settings={sets}, back={back})")

# 9) the right button opens the menu as well; a click on the Done line selects it
y = p.fresh_row("Working topic") or Y_WORKING
os.write(p.fd, f"\x1b[<2;10;{y}M\x1b[<2;10;{y}m".encode())
menu = p.expect("ACTIONS", 8)
p.out = b""; os.write(p.fd, b"\x1b"); back = p.expect("Agents", 8)
ok("right button: the topic's menu") if (menu and back) else bad(f"right button menu (menu={menu}, back={back})")
# 10) the page is the panel's own: keys move, a number runs an action ("Park for later" on a now topic), Esc leaves
y = p.fresh_row("Working topic") or Y_WORKING
os.write(p.fd, f"\x1b[<2;10;{y}M\x1b[<2;10;{y}m".encode())
shown = p.expect("ACTIONS", 5)
lines = p.screen(); later = [l for l in lines if "Park for later" in l]
num = None
for l in lines:
    m = re.search(r"(\d+)\s+Park for later", l)
    if m: num = m.group(1)
if shown and num and int(num) < 10:
    p.send(num, 1.2)
    st = p.get("1001-1", "status")
    ok("the page: a number key runs the action (Park for later)") if st == "later" else bad(f"page action: status is {st}")
else: bad(f"the page did not show (shown={shown}, num={num})")

# 11) the chooser (s): no free agent here, so only "Just mark as started": Enter takes it
p.send(b"\x1b", 0.8)
p.topic(id="1500-1", title="Chooser candidate", status="next")
p.out = b""; p.expect("Chooser candidate", 8)
y = p.fresh_row("Chooser candidate")
if y:
    p.click(10, y); p.send("s", 1.0)
    sc = "\n".join(p.screen())
    if "Who takes it?" in sc and "Just mark as started" in sc:
        p.send("\r", 1.2)
        st = p.get("1500-1", "status")
        ok("s: the chooser; Enter on 'Just mark as started'") if st == "now" else bad(f"chooser: status is {st}")
    else: bad("s: the chooser did not show")
else: bad("the topic for the chooser did not show up")
p.close()

# 12) what dies comes back: the collector (the panel starts it again) and the daemon (the collector starts it again)
def children(pid):
    r = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True)
    return [int(x) for x in r.stdout.split()]
def alive_pid(pid):
    try: os.kill(pid, 0); return True
    except OSError: return False
p = Panel()
pidf = os.path.join(p.T, "shared", "bhoted.pid"); t0 = time.time()
while not os.path.exists(pidf) and time.time() - t0 < 10: time.sleep(0.3)
d1 = int(open(pidf).read().split()[0]) if os.path.exists(pidf) else 0
if d1:
    os.kill(d1, 9); t0 = time.time(); d2 = d1
    while time.time() - t0 < 14:
        p.drain(0.5)   # (the terminal is read meanwhile: a panel that cannot write its screen waits)
        try: d2 = int(open(pidf).read().split()[0])
        except (OSError, ValueError): continue
        if d2 != d1 and alive_pid(d2): break
    ok("the daemon is started again after a kill") if (d2 != d1 and alive_pid(d2)) else bad("the daemon did not come back")
else: bad("the daemon did not start")
kids = children(p.pid)
if kids:
    for k in kids:
        try: os.kill(k, 9)
        except OSError: pass
    t0 = time.time(); back = []
    while time.time() - t0 < 12:
        p.drain(0.5); back = [k for k in children(p.pid) if k not in kids]
        if back: break
    ok("the collector is started again after a kill") if back else bad("the collector did not come back")
else: bad("the panel has no collector child")
p.close()

# 13) a window that is far too small: a note, no crash, and the list comes back
p = Panel()
def resize(cols, rows):
    fcntl.ioctl(p.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0)); os.kill(p.pid, signal.SIGWINCH)
p.out = b""; resize(12, 6); small = p.expect("Too small", 6)
p.out = b""; resize(70, 40); back = p.expect("REVIEW", 6)
ok("a tiny window: 'Too small', and the list is back at normal size") if (small and back and p.alive) else bad(f"resize (small={small}, back={back}, alive={p.alive})")
p.close()
sys.exit(1 if fails else 0)
