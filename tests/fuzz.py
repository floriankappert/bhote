#!/usr/bin/env python3
"""Garbage in, no panic out: random and hostile data in the topics, the agent lists, the monitor lists and the settings, and every call of the
daemon that reads them (blocks, the whole frame, the screen, the follow-ups in shadow mode, the index, the merge validation).
A panic (exit 101) or a signal is a failure. Skipped without the binary. Usage: python3 tests/fuzz.py [iterations, 120] [seed]"""
import os, random, subprocess, sys, tempfile, shutil, time
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "daemon", "target", "release", "bhoted")
if not os.path.exists(BIN):
    print("skipped: no bhoted binary"); sys.exit(0)
N = int(sys.argv[1]) if len(sys.argv) > 1 else 120
SEED = int(sys.argv[2]) if len(sys.argv) > 2 else int(time.time())
rnd = random.Random(SEED)
US = "\x1f"
KEYS = ["id", "title", "status", "waiting_for", "waiting_since", "created", "updated", "deleted", "description", "agent", "agent_machine", "agent_pane", "agent_session", "delegated",
        "transfer_to", "transfer_to_name", "slack_label", "slack_msg", "slack_channel", "agent_branch", "project", "kind", "deploy_wait", "deploy_url", "dismissed", "auto_now", "auto", "review_src",
        "review_note", "review_ask", "review_reply", "repos", "workspaces", "ci", "deploy_branches", "deploy_names", "test_names", "num", "code"]
STATUS = ["now", "testing", "finished", "next", "waiting", "review", "later", "done", "", "bogus", "NOW", "waiting "]
NASTY = ["", " ", "=", "==", "a=b=c", "\x1b[31m", "\x1b]0;x\x07", "日本語のタイトル", "é" * 80, "x" * 5000, "​‍", "👍🏽 emoji", "\t\t", "../../etc/passwd", "a" + US + "b", "\x1d" * 3, "\x01\x02\x1c", "−" * 300, "-1", "99999999999999999999", "1e9", "0x10", "null", "'\"`$(x)", "%s%n", "\r", "́" * 40, "¦¦¦", "¶¶", "proj-x", "more:next", "!1!2", "w1:p1", "w1:", ":p3", "w:p1", "wZ:p"]
def junk():
    r = rnd.random()
    if r < 0.35: return rnd.choice(NASTY)
    if r < 0.6: return str(rnd.randint(-5, 2_000_000_000))
    if r < 0.8: return "".join(rnd.choice("abcdefghij kl-_/.:0123456789") for _ in range(rnd.randint(0, 40)))
    return "".join(chr(rnd.randint(1, 0x2fff)) for _ in range(rnd.randint(0, 20))).replace("\n", " ")
def topic_text(i):
    lines = []
    for _ in range(rnd.randint(0, 22)):
        k = rnd.choice(KEYS) if rnd.random() < 0.9 else junk()
        v = rnd.choice(STATUS) if k == "status" else (str(int(time.time()) - rnd.randint(-100, 10**7)) if k in ("updated", "created", "waiting_since", "delegated") else junk())
        lines.append(f"{k}={v}")
    if rnd.random() < 0.05: lines.append(junk())
    t = "\n".join(lines) + ("\n" if rnd.random() < 0.9 else "")
    if rnd.random() < 0.03: return os.urandom(rnd.randint(0, 300)).decode("latin-1")
    return t
def build(T):
    for d in ("data/topics", "shared", "shared/testprog", "ts"): os.makedirs(f"{T}/{d}", exist_ok=True)
    n = rnd.randint(0, 40)
    for i in range(n):
        name = rnd.choice([f"{1700000000 + rnd.randint(0, 99999)}-{rnd.randint(1, 99999)}", f"proj-{i}", f"mach-{i}", f"an-x-{i}", f"{i}"])
        open(f"{T}/data/topics/{name}.topic", "w", errors="replace").write(topic_text(i))
    rows = []
    for _ in range(rnd.randint(0, 12)):
        rows.append(US.join([rnd.choice(["Linux", "Mac", junk()]), rnd.choice(["w1:p1", "w2:p3", "wA:p10", junk()]), rnd.choice(["idle", "working", "blocked", "done", "testing", "off-idle", junk()]), junk(), junk()][:rnd.randint(1, 5)]))
    open(f"{T}/shared/agents.local", "w", errors="replace").write("\n".join(rows) + "\n")
    open(f"{T}/shared/agents.remote", "w", errors="replace").write("\n".join(rows[: len(rows) // 2]) + "\n")
    for f in ("agents.meta.local", "agents.meta.remote", "agents.now.local", "agents.cwd"):
        open(f"{T}/shared/{f}", "w", errors="replace").write("\n".join(US.join(junk() for _ in range(rnd.randint(1, 8))) for _ in range(rnd.randint(0, 8))) + "\n")
    for f in ("deploys.list", "tests.list"):
        open(f"{T}/shared/{f}", "w", errors="replace").write("\n".join(US.join([str(int(time.time()) - rnd.randint(-50, 10**6)), junk(), junk(), rnd.choice(["running", "ok", "failed", "queued", "approval", "canceled", junk()]), junk(), junk(), junk(), junk()][:rnd.randint(1, 8)]) for _ in range(rnd.randint(0, 20))) + "\n")
    cfg = [f"{rnd.choice(['DEPLOY_MONITOR', 'TEST_MONITOR', 'THEME', 'COLOR_TEXT', 'COLOR_DIM', 'MONITOR_ROWS', 'DONE_MAX', 'ARCHIVE_DAYS', 'AGENT_ROWS', 'FINISHED_MINUTES', 'WAIT_REMIND', 'DEPLOY_PROJECTS_OFF', 'PROJECT_COLORS', 'AGENTS_COMPACT', 'DONE_COMPACT', 'TESTS_COMPACT', 'TEST_STATUS_DIR', junk()])}={rnd.choice(['on', 'off', 'catppuccin', 'dracula', '#zzz', '#00ff00', junk()])}" for _ in range(rnd.randint(0, 14))]
    open(f"{T}/config", "w", errors="replace").write("\n".join(cfg) + "\n")
    colors = "".join(f"{k}={rnd.choice(['', chr(27) + '[31m', junk()])}\n" for k in ["X", "R", "NN", "S", "C", "L", "D", "BOLD", "B", "Y", "GREEN", "BLUE", "ORANGE", "MUTED", "PINK", "TEAL", "SELBG", "CARDBG", "CARDSEL", "CARDHOV", "CARDSUB", "CARDBTN", "LSEL", "DSEL", "DESC", "DESCSEL", "TITLE", "TITLESEL"])
    open(f"{T}/colors.env", "w", errors="replace").write(colors)
    view = {"SEL_ID": rnd.choice(["", "more:next", "more:done", junk()]), "HOVER_ID": junk(), "EXPANDED": rnd.choice([" ", " next waiting later done ", junk()]), "U_SCOPE": rnd.choice(["all", "project"]), "CUR_PROJ": rnd.choice(["", "proj-1", junk()]),
            "SPIN_CH": rnd.choice(["✶", "", junk()]), "ROWS": str(rnd.randint(-3, 80)), "HEAD_ROWS": str(rnd.randint(-2, 5)), "U_DONE_MAX": str(rnd.randint(-2, 30)), "U_ARCHIVE_DAYS": str(rnd.randint(-2, 30)), "LINE_MAX": str(rnd.randint(-2, 20)),
            "MCODES": rnd.choice(["", "Linux" + US + "LIN\x1e" + "Mac" + US + "MAC\x1e", junk()]), "HOST": "Linux", "PANE_FOCUS": rnd.choice(["true", "false", ""]), "FLASH": rnd.choice(["", junk()]), "HEAD_NOTE": rnd.choice(["", junk()]),
            "VERSION_LABEL": junk(), "HEAD_GRAD": junk(), "U_REMOTE": rnd.choice(["on", "off"]), "HAS_MACHINES": rnd.choice(["0", "1"]), "U_AGENT_ROWS": rnd.choice(["", "3", junk()]), "NEWFEAT": rnd.choice(["0", "1"]), "HOVER_BTN": rnd.choice(["0", "1"])}
    open(f"{T}/view.env", "w", errors="replace").write("".join(f"{k}={v}\n" for k, v in view.items()))
fails = 0; calls = 0
def run(args, T, stdin=None):
    global fails, calls
    calls += 1
    try:
        p = subprocess.run([BIN] + args, input=stdin, capture_output=True, timeout=20, env=dict(os.environ, KHARKA="/nonexistent", PATH="/usr/bin:/bin"))
    except subprocess.TimeoutExpired:
        fails += 1; print(f"FAIL (timeout, seed {SEED}): {args[:3]}"); return
    if p.returncode == 101 or p.returncode < 0 or b"panicked at" in p.stderr:
        fails += 1; print(f"FAIL (seed {SEED}, exit {p.returncode}): {' '.join(args[:3])}\n  {p.stderr.decode('utf8', 'replace')[:300]}")
        keep = f"/tmp/fuzz-fail-{SEED}-{calls}"; shutil.copytree(T, keep, dirs_exist_ok=True); print("  data kept in", keep)
for it in range(N):
    T = tempfile.mkdtemp(); build(T)
    common = ["--cols", str(rnd.randint(10, 160)), "--now", str(int(time.time())), "--config", f"{T}/config", "--shared", f"{T}/shared", "--topics", f"{T}/data/topics", "--colors", f"{T}/colors.env", "--view", f"{T}/view.env"]
    for blk in ("monitors", "cards", "lists", "done", "head", "bottom", "frame"):
        extra = []
        if blk == "done": extra = ["--max", str(rnd.randint(-1, 20))]
        if blk == "bottom" or blk == "frame": extra = ["--run", T]
        if blk == "frame": open(f"{T}/h", "w").write(junk()); open(f"{T}/b", "w").write(junk()); extra += ["--rows", str(rnd.randint(-2, 90)), "--head", f"{T}/h", "--bottom", f"{T}/b"]
        run(["block", blk] + common + extra, T)
    tid = next(iter(os.listdir(f"{T}/data/topics")), None)
    if tid:
        f = f"{T}/data/topics/{tid}"
        run(["block", "menu", "--file", f, "--idx", str(rnd.randint(-3, 30)), "--flash", junk()] + common, T)
        run(["block", "pick", "--file", f, "--title", junk(), "--mode", rnd.choice(["start", "transfer"]), "--idx", str(rnd.randint(-1, 5))] + common, T)
    run(["follow", f"{T}/data/topics", "--shared", f"{T}/shared", "--data", f"{T}/data", "--host", "Linux", "--config", f"{T}/config", "--shadow"], T)
    run(["index", f"{T}/data/topics", f"{T}/idx"], T)
    run(["collect", f"{T}/shared", "Linux", "--config", f"{T}/config"], T)
    run(["paint", "--rows", str(rnd.randint(0, 60)), "--run", T], T, stdin=("".join(junk() + "\n" for _ in range(rnd.randint(0, 30)))).encode("utf8", "replace"))
    run(["theme", "--config", f"{T}/config", "--head", junk()], T)
    run(["branch", junk(), "x"], T)
    shutil.rmtree(T, ignore_errors=True)
print(f"{calls} calls on {N} random setups (seed {SEED}): {'no panic' if not fails else str(fails) + ' failures'}")
sys.exit(1 if fails else 0)
