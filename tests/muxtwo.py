#!/usr/bin/env python3
"""Two machines, built on one: each has its own mux server, its own kharka daemon (with the sidecar herdr-kharka the mux starts), its own sshd (with the environment of its
machine) and its own folders; one kharka hub joins them; ssh between them is real ssh (keys, ControlMaster). What crosses between the machines:
the agents (through kharka), a command to the other machine's agent (kharka_call), `herdr --machine` (ssh), a workspace of the other machine opened here (ssh + bhote view).
Skipped without sshd, kharka and herdr-kharka. Usage: python3 tests/muxtwo.py"""
import os, sys, json, time, shutil, subprocess, tempfile, signal
for tool in ("kharka", "herdr-kharka", "ssh", "ssh-keygen", "jq"):
    if not shutil.which(tool): print(f"skipped: no {tool}"); sys.exit(0)
SSHD = next((p for p in ("/usr/sbin/sshd", "/usr/bin/sshd", "/usr/local/sbin/sshd") if os.path.exists(p)), None)
if not SSHD: print("skipped: no sshd"); sys.exit(0)
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "daemon", "target", "release", "bhoted")
if not os.path.exists(BIN): print("skipped: no bhoted binary"); sys.exit(0)
BH = os.path.join(ROOT, "bhote")
KH = shutil.which("kharka")
W = tempfile.mkdtemp(prefix="two.")
USER = os.environ.get("USER") or subprocess.run(["id", "-un"], capture_output=True, text=True).stdout.strip()
fails = 0; procs = []
def ok(m): print("ok   " + m, flush=True)
def bad(m, extra=""):
    global fails; fails += 1; print("FAIL " + m + (("\n  " + str(extra)[:900]) if extra else ""), flush=True)
def check(c, m, extra=""): ok(m) if c else bad(m, extra)
def cleanup():
    for p in procs:
        try: p.terminate()
        except Exception: pass
    for M in "AB":
        try: subprocess.run([BIN, "mux-call", "server.stop"], env=envs[M], capture_output=True, timeout=10)
        except Exception: pass
    time.sleep(0.5)
    shutil.rmtree(W, ignore_errors=True)
def done():
    cleanup(); sys.exit(1 if fails else 0)
import atexit; atexit.register(lambda: os.path.exists(W) and cleanup())
signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
signal.signal(signal.SIGALRM, lambda *_: (print("FAIL ran out of time (240 s)", flush=True), done())); signal.alarm(240)

# keys and the two sshd (each sets the environment of ITS machine for the sessions it gives)
subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", f"{W}/id"], check=True)
subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", f"{W}/hostkey"], check=True)
shutil.copy(f"{W}/id.pub", f"{W}/authorized")
import socket
def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p
PORT = {"A": free_port(), "B": free_port()}
os.makedirs(f"{W}/abin"); open(f"{W}/abin/claude", "w").write("#!/bin/sh\necho \"esc to interrupt\"\nwhile read c; do echo \"you said: $c\"; done\n"); os.chmod(f"{W}/abin/claude", 0o755)
open(f"{W}/kharka.toml", "w").write("")
envs = {}
for M in "AB":
    d = f"{W}/{M}"; os.makedirs(f"{d}/shared"); os.makedirs(f"{d}/data/topics")
    open(f"{d}/config", "w").write(f"AUTOSTART=off\nSTORE=kharka\nAGENTS_VIA=kharka\nREMOTE_AGENTS=on\nKHARKA_LABEL={M}\nKHARKA={KH}\nNOTIFY=off\nUPDATE_CHECK=off\n"
        f"REMOTE_SSH=ssh -t -F {W}/sshconfig -o ControlMaster=auto -o ControlPath={W}/cm-%C -o ControlPersist=60\n")
    env = {"BHOTE_MUX_SOCK": f"{d}/mux.sock", "BHOTE_MUX_STATE": f"{d}/state.json", "BHOTE_CONFIG": f"{d}/config", "BHOTE_SHARED": f"{d}/shared", "BHOTE_DATA": f"{d}/data",
           "KHARKA_SOCKET": f"{W}/k{M}.sock", "KHARKA": KH, "BHOTED_BIN": BIN, "BHOTE_VIEW_GRACE": "60"}
    envs[M] = dict(os.environ, **env, PATH=f"{W}/abin:" + os.environ["PATH"], SHELL="/bin/sh", TERM="xterm-256color", NO_COLOR="1")
    for k in ("HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID", "HERDR_SOCKET_PATH", "HERDR_ENV", "HERDR_BIN_PATH", "BHOTE_MUX", "BHOTE_PANEL", "XDG_RUNTIME_DIR"): envs[M].pop(k, None)
    setenv = " ".join(f"{k}={v}" for k, v in {**env, "PATH": envs[M]["PATH"]}.items())
    open(f"{W}/sshd_{M}", "w").write(f"Port {PORT[M]}\nListenAddress 127.0.0.1\nHostKey {W}/hostkey\nAuthorizedKeysFile {W}/authorized\nPidFile {W}/sshd_{M}.pid\nStrictModes no\nUsePAM no\n"
        f"PasswordAuthentication no\nKbdInteractiveAuthentication no\nPubkeyAuthentication yes\nSetEnv {setenv}\nLogLevel ERROR\n")
open(f"{W}/sshconfig", "w").write("".join(f"Host node{M}\n  HostName 127.0.0.1\n  Port {PORT[M]}\n  User {USER}\n  IdentityFile {W}/id\n  IdentitiesOnly yes\n  StrictHostKeyChecking no\n  UserKnownHostsFile /dev/null\n  LogLevel ERROR\n" for M in "AB"))
def start(args, env=None, **kw):
    p = subprocess.Popen(args, env=env or os.environ, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, **kw); procs.append(p); return p
for M in "AB": start([SSHD, "-D", "-f", f"{W}/sshd_{M}", "-E", f"{W}/sshd_{M}.log"])
time.sleep(1.5)
def ssh(M, cmd, timeout=20):
    return subprocess.run(["ssh", "-F", f"{W}/sshconfig", "-o", "BatchMode=yes", f"node{M}", cmd], capture_output=True, text=True, timeout=timeout)
r = ssh("B", 'echo "$BHOTE_MUX_SOCK"')
if r.returncode != 0 or W not in r.stdout: print("skipped: ssh to a private sshd does not work here:", r.stderr[:200], r.stdout[:100]); done()
check(r.stdout.strip() == f"{W}/B/mux.sock", "real ssh to the machine B gives B's environment (its mux socket)", r.stdout)
check(ssh("A", 'echo "$BHOTE_MUX_SOCK"').stdout.strip() == f"{W}/A/mux.sock", "and to A gives A's")

# kharka: one hub, a daemon per machine
def start(args, env=None, **kw):
    p = subprocess.Popen(args, env=env or os.environ, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, **kw); procs.append(p); return p
start([KH, "serve", "--data", f"{W}/hub", "--socket", f"{W}/khub-d.sock", "--hub-socket", f"{W}/hub.sock", "--config", f"{W}/kharka.toml"])
time.sleep(1.5)
for M in "AB": start([KH, "daemon", "--hub", f"{W}/hub.sock", "--node", f"node{M}", "--data", f"{W}/kd{M}", "--socket", f"{W}/k{M}.sock", "--config", f"{W}/kharka.toml"])
time.sleep(2)
# a mux per machine, with a fake agent in the first pane of each, and the collector of bhote (it reads the other machine's agents out of kharka)
def mux_call(M, method, params=None):
    r = subprocess.run([BIN, "mux-call", method, json.dumps(params or {})], capture_output=True, text=True, env=envs[M], timeout=15)
    try: return json.loads(r.stdout).get("result", {})
    except Exception: return {}
for M in "AB":
    start([BIN, "mux"], env=envs[M], start_new_session=True)
for M in "AB":
    for _ in range(50):
        if mux_call(M, "ping").get("pong"): break
        time.sleep(0.2)
    else: bad(f"the mux of {M} does not start")
os.makedirs(f"{W}/lnk"); os.symlink(BIN, f"{W}/lnk/herdr")
def herdr(M, *a, timeout=30):
    return subprocess.run(["herdr", *a], capture_output=True, text=True, env=dict(envs[M], PATH=f"{W}/lnk:" + envs[M]["PATH"]), timeout=timeout)
def wait(f, t=30):
    end = time.time() + t
    while time.time() < end:
        try:
            v = f()
            if v: return v
        except Exception: pass
        time.sleep(0.4)
    return None
# each machine starts a pane with an agent in it (the pane is started by the mux itself: no client needed)
for M in "AB":
    mux_call(M, "pane.run", {"pane_id": "w1:p1", "command": "claude"})
time.sleep(0.8)
for M in "AB":
    herdr(M, "pane", "report-agent", "w1:p1", "--source", "t", "--agent", "claude", "--state", "idle", "--agent-session-id", f"sess-{M}")
def entry(M, node):
    r = subprocess.run([KH, "get", f"herdr/{node}"], capture_output=True, text=True, env=envs[M]); return json.loads(r.stdout) if r.stdout.strip() else {}
check(wait(lambda: entry("A", "nodeB").get("herdr") == "online"), "the sidecar of B wrote its entry and A sees it through the hub", entry("A", "nodeB"))
check(wait(lambda: any(a.get("pane") == "w1:p1" for a in entry("A", "nodeB").get("agents", []))), "A sees the agent of B in kharka", entry("A", "nodeB").get("agents"))
check(wait(lambda: any(a.get("pane") == "w1:p1" for a in entry("B", "nodeA").get("agents", []))), "and B sees the agent of A")
# the collector of bhote on A: `bhote agents` lists both machines
col = {}
for M in "AB":
    col[M] = start([BIN, "run", f"{W}/{M}/data/topics", f"{W}/{M}/shared/topics.idx", "--agents", "--remote", "--host", M, "--config", f"{W}/{M}/config", "--data", f"{W}/{M}/data"], env=envs[M])
def bagents(M): return subprocess.run(["bash", BH, "agents"], capture_output=True, text=True, env=envs[M], timeout=60).stdout
out = wait(lambda: (lambda o: o if "B/w1:a1" in o and len(o.strip().splitlines()) >= 2 else None)(bagents("A")), 40)
check(out is not None and len(out.strip().splitlines()) >= 2, "`bhote agents` on A lists its own agent and the one of B", (out, bagents("A"), subprocess.run(["bash", "-c", "ls " + W + "/A/shared; cat " + W + "/A/shared/agents.remote " + W + "/A/shared/agents.local"], capture_output=True, text=True).stdout[:600]))
# a command to the agent of the other machine through kharka (what a click on its card and a prompt from the panel do)
def kcall(M, other, method, params):
    script = f'. {BH}; bhote_info; kharka_call {other} {method} \'{json.dumps(params)}\' 20; echo "rc=$?"; printf "%s\\n" "$KC_RESULT"; echo "err=$KC_ERR"'
    return subprocess.run(["bash", "-c", f"BHOTE_SOURCE_ONLY=1; {script}"], capture_output=True, text=True, env=envs[M], timeout=60)
wait(lambda: os.path.exists(f"{W}/A/shared/agents.nodes") and "B" in open(f"{W}/A/shared/agents.nodes").read(), 30)
r = kcall("A", "B", "agent.prompt", {"pane_id": "w1:p1", "text": "hello from A over kharka"})
check("rc=0" in r.stdout, "A prompts the agent of B through kharka (hub, sidecar of B, mux of B)", r.stdout + r.stderr)
check(wait(lambda: "you said: hello from A over kharka" in mux_call("B", "pane.read", {"pane_id": "w1:p1", "source": "visible"}).get("text", "")), "and the text arrives in B's agent, on B", mux_call("B", "pane.read", {"pane_id": "w1:p1"}))
check("you said: hello from A" not in mux_call("A", "pane.read", {"pane_id": "w1:p1", "source": "visible"}).get("text", ""), "(not in A's pane)")
r = kcall("B", "A", "agent.prompt", {"pane_id": "w1:p1", "text": "hello from B over kharka"})
check(wait(lambda: "you said: hello from B over kharka" in mux_call("A", "pane.read", {"pane_id": "w1:p1", "source": "visible"}).get("text", "")), "the other way, too: B prompts the agent of A", r.stdout + r.stderr)
# herdr --machine: the same command, over real ssh, on the other machine
mux_call("B", "workspace.create", {"label": "only-on-B", "focus": False})
r = herdr("A", "--machine", "B", "workspace", "list")
check(r.returncode == 0 and "only-on-B" in r.stdout and "only-on-B" not in herdr("A", "workspace", "list").stdout, "herdr --machine B workspace list (real ssh) shows B's workspaces, not A's", r.stdout[:300] + r.stderr[:300])
r = herdr("B", "--machine", "A", "pane", "read", "w1:p1", "--source", "visible")
check(r.returncode == 0 and "hello from B over kharka" in r.stdout, "and from B to A: the pane of A is read over ssh", r.stdout[:300] + r.stderr[:300])
# a workspace of the other machine opened here: a hidden workspace on A that shows B's agent (ssh + bhote view)
sc = f'BHOTE_SOURCE_ONLY=1; . {BH}; bhote_info; BHOTE_MUX=native; HERDR_PANE_ID=w1:p1; remote_workspace B w1 w1:p1; echo rc=$?'
r = subprocess.run(["bash", "-c", sc], capture_output=True, text=True, env=dict(envs["A"], PATH=f"{W}/lnk:" + envs["A"]["PATH"], HERDR_PANE_ID="w1:p1", BHOTE_MUX="native"), timeout=60)
allws = mux_call("A", "workspace.list", {"all": True}).get("workspaces", [])
mount = next((w for w in allws if w["label"].startswith("B · ")), None)
check(mount is not None and not any(w["label"].startswith("B · ") for w in mux_call("A", "workspace.list").get("workspaces", [])), "B's workspace is a hidden workspace on A", (allws, r.stdout, r.stderr[:300]))
if mount:
    vp = next((p["pane_id"] for p in mux_call("A", "pane.list", {"all": True}).get("panes", []) if p["workspace_id"] == mount["workspace_id"]), None)
    check(wait(lambda: "hello from A over kharka" in mux_call("A", "pane.read", {"pane_id": vp, "source": "visible"}).get("text", "")), "it shows B's agent pane (what B's agent printed), through ssh and bhote view", mux_call("A", "pane.read", {"pane_id": vp, "source": "visible"}))
    mux_call("A", "pane.send_text", {"pane_id": vp, "text": "typed on A into B\r"})
    check(wait(lambda: "you said: typed on A into B" in mux_call("B", "pane.read", {"pane_id": "w1:p1", "source": "visible"}).get("text", "")), "and what is typed there arrives in B's agent")
done()
