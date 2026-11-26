#!/usr/bin/env python3
"""bhote secret manager: the store, the scan of Claude Code profiles, the import, the purge of the Claude context, named environments.

Called by `bhote secret ...` and `bhote env ...` (and copied to the other machines like monitors.py). Standard library only.

The rule of this file: a secret value lives in exactly three places, the backend (macOS Keychain, libsecret through secret-tool, an encrypted file, or an
op:// reference that is resolved at use time), the memory of this process, and the environment of a command that `env run` starts. It is never printed
(only masked: first 6 and last 3 characters and the length), never put into an argument of another program, never written into a cache or a log.
"""
import argparse, fcntl, getpass, glob, hashlib, json, os, re, shutil, subprocess, sys, tempfile, time

HOME = os.path.expanduser("~")
CFG = os.environ.get("BHOTE_CONFIG") or os.path.join(HOME, ".config", "bhote", "config")
DIR = os.path.dirname(CFG)
INDEX = os.path.join(DIR, "secrets.json")      # names, backend, masked form, fingerprint, dates: no value
ENVS = os.path.join(DIR, "envs.json")          # named sets: variable -> secret name (references)
FILEDIR = os.path.join(DIR, "secrets.d")       # the encrypted files of the file backend
KEYFILE = os.path.join(DIR, "secrets.key")     # openssl: the key of those files (0600)
AGEKEY = os.path.join(DIR, "secrets.age-key")  # age: the identity
SERVICE = "bhote"
NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,79}$")
CONFIG_SECRETS = ["CIRCLECI_TOKEN"]            # the settings of type secret (the registry in bhote)
MIN_AGE = int(os.environ.get("BHOTE_PURGE_MIN_AGE", "900"))   # purge skips files modified in the last 15 minutes
TIMEOUT = 25


class Fail(Exception):
    pass


# ── small helpers ──────────────────────────────────────────────────────────────────────────────────────────────

def cfg_get(key, default=""):
    try:
        with open(CFG, "r", errors="ignore") as f:
            val = default
            for line in f:
                if line.startswith(key + "="):
                    val = line.rstrip("\n").split("=", 1)[1]
            return val
    except OSError:
        return default


def fp_of(v):
    return hashlib.sha256(v.encode("utf-8", "surrogateescape")).hexdigest()[:10]


def mask(v):
    """First 6 and last 3 characters and the length: sk-ant…gAA (108). A short value shows less (never more than a third of it)."""
    n = len(v)
    a, b = (6, 3) if n >= 18 else (max(1, n // 6), max(1, n // 9))
    return f"{v[:a]}…{v[-b:]} ({n})"


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime())


def write_atomic(path, data, mode=0o600):
    os.makedirs(os.path.dirname(path), mode=0o700, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".tmp.", dir=os.path.dirname(path))
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb") as f:
            f.write(data if isinstance(data, bytes) else data.encode())
            f.flush(); os.fsync(f.fileno())
        os.replace(tmp, path)
    except BaseException:
        try: os.unlink(tmp)
        except OSError: pass
        raise


def run(cmd, inp=None, timeout=TIMEOUT):
    """Runs a program; its output is returned, never printed. A hang (a GUI permission prompt) ends as a Fail, it is not worked around."""
    try:
        return subprocess.run(cmd, input=inp, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise Fail(f"{os.path.basename(cmd[0])} did not answer within {timeout} s (a permission prompt on the screen? answer it, then run it again)")
    except OSError as e:
        raise Fail(f"{os.path.basename(cmd[0])}: {e.strerror}")


# ── the index ──────────────────────────────────────────────────────────────────────────────────────────────────

class Index:
    def __enter__(self):
        os.makedirs(DIR, mode=0o700, exist_ok=True)
        self.lock = open(INDEX + ".lock", "a"); fcntl.flock(self.lock, fcntl.LOCK_EX)
        try:
            with open(INDEX, "rb") as f: self.d = json.load(f)
        except (OSError, ValueError):
            self.d = {"version": 1, "secrets": {}}
        self.s = self.d.setdefault("secrets", {})
        return self

    def save(self):
        write_atomic(INDEX, json.dumps(self.d, indent=1, sort_keys=True))

    def __exit__(self, *a):
        fcntl.flock(self.lock, fcntl.LOCK_UN); self.lock.close()


def load_index():
    try:
        with open(INDEX, "rb") as f: return json.load(f).get("secrets", {})
    except (OSError, ValueError):
        return {}


# ── backends ───────────────────────────────────────────────────────────────────────────────────────────────────

def which(x): return shutil.which(x)


def b_keychain_set(name, value):
    esc = value.replace("\\", "\\\\").replace('"', '\\"')
    r = run(["security", "-i"], f'add-generic-password -U -s {SERVICE} -a "{name}" -w "{esc}"\n'.encode())   # (stdin: the value never is an argument)
    if r.returncode != 0: raise Fail("the keychain refused the entry (security exit %d; no permission or no GUI session?)" % r.returncode)


def b_keychain_get(name):
    r = run(["security", "find-generic-password", "-s", SERVICE, "-a", name, "-w"])
    if r.returncode != 0: return None
    return r.stdout.decode("utf-8", "surrogateescape").rstrip("\n")


def b_keychain_del(name):
    run(["security", "delete-generic-password", "-s", SERVICE, "-a", name])


def b_st_set(name, value):
    r = run(["secret-tool", "store", f"--label=bhote {name}", "service", SERVICE, "name", name], value.encode())
    if r.returncode != 0: raise Fail("secret-tool refused the entry (is the keyring unlocked and a D-Bus session there?)")


def b_st_get(name):
    r = run(["secret-tool", "lookup", "service", SERVICE, "name", name])
    return r.stdout.decode("utf-8", "surrogateescape") if r.returncode == 0 and r.stdout else None


def b_st_del(name):
    run(["secret-tool", "clear", "service", SERVICE, "name", name])


def file_mode():
    return "age" if which("age") and which("age-keygen") else "openssl"


def _file_path(name): return os.path.join(FILEDIR, name + ".enc")


def _openssl_key():
    if not os.path.exists(KEYFILE):
        write_atomic(KEYFILE, os.urandom(32).hex() + "\n", 0o600)
    return KEYFILE


def _age_recipient():
    if not os.path.exists(AGEKEY):
        r = run(["age-keygen"]);
        if r.returncode != 0: raise Fail("age-keygen failed")
        write_atomic(AGEKEY, r.stdout, 0o600)
    with open(AGEKEY, "r") as f:
        for line in f:
            m = re.search(r"(age1[a-z0-9]+)", line)
            if m and line.startswith("# public key"): return m.group(1)
    raise Fail("age identity without a public key")


def b_file_set(name, value):
    os.makedirs(FILEDIR, mode=0o700, exist_ok=True)
    if file_mode() == "age":
        r = run(["age", "-r", _age_recipient()], value.encode())
    else:
        r = run(["openssl", "enc", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-md", "sha256", "-salt", "-pass", "file:" + _openssl_key()], value.encode())
    if r.returncode != 0 or not r.stdout: raise Fail("encryption failed (%s)" % file_mode())
    write_atomic(_file_path(name), r.stdout, 0o600)


def b_file_get(name):
    try:
        with open(_file_path(name), "rb") as f: blob = f.read()
    except OSError:
        return None
    if blob.startswith(b"age-encryption.org"):
        r = run(["age", "-d", "-i", AGEKEY], blob)
    else:
        r = run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-md", "sha256", "-pass", "file:" + _openssl_key()], blob)
    return r.stdout.decode("utf-8", "surrogateescape") if r.returncode == 0 else None


def b_file_del(name):
    try: os.unlink(_file_path(name))
    except OSError: pass


def b_op_get(uri):
    r = run([os.environ.get("BHOTE_OP_BIN", "op"), "read", "--no-newline", uri], timeout=40)
    if r.returncode != 0: return None
    return r.stdout.decode("utf-8", "surrogateescape")


def pick_backend(forced=None):
    want = forced or os.environ.get("BHOTE_SECRET_BACKEND") or cfg_get("SECRET_BACKEND", "auto") or "auto"
    if want == "auto":
        if sys.platform == "darwin" and which("security"):
            if keychain_reachable(): return "keychain"
            raise Fail("the keychain is not reachable from this process (a background or sandboxed process may not use it: run this in your own terminal, "
                       "or choose the encrypted file on purpose: bhote config set SECRET_BACKEND file)")
        if sys.platform != "darwin" and which("secret-tool"):
            try:
                r = run(["secret-tool", "lookup", "service", SERVICE, "name", "__probe__"], timeout=8)
                if not r.stderr.strip(): return "secret-tool"    # (an error text: no D-Bus session or no unlocked keyring)
            except Fail:
                pass
        return "file"
    if want == "keychain" and not which("security"): raise Fail("no `security` here (macOS only)")
    if want == "secret-tool" and not which("secret-tool"): raise Fail("no secret-tool here (libsecret-tools)")
    if want not in ("keychain", "secret-tool", "file"): raise Fail(f"unknown backend {want}")
    return want


SET = {"keychain": b_keychain_set, "secret-tool": b_st_set, "file": b_file_set}
GET = {"keychain": b_keychain_get, "secret-tool": b_st_get, "file": b_file_get}
DEL = {"keychain": b_keychain_del, "secret-tool": b_st_del, "file": b_file_del}


def read_value(name, ent=None):
    """The value of a stored secret, or None. Only for env run, verify, import and purge: never printed by the callers."""
    ent = ent or load_index().get(name)
    if not ent: return None
    if ent["backend"] == "op": return b_op_get(ent["ref"])
    return GET[ent["backend"]](name)


def check_name(name):
    if not NAME_RE.match(name or ""): raise Fail("a name is letters, digits and _ (not starting with a digit), at most 80 characters")


def store(name, value, source="manual", backend=None, ref=None):
    """Stores a value (or an op:// reference) and records it in the index; returns the index entry. Verified by reading it back."""
    check_name(name)
    if ref is None:
        if not value or re.search(r"[\x00-\x08\x0a-\x1f\x7f]", value) or len(value) > 8192:
            raise Fail("the value is empty, has a control character or a line break, or is longer than 8192 characters")
        b = pick_backend(backend)
        SET[b](name, value)
        back = GET[b](name)
        if back != value:
            raise Fail(f"{name}: read back from {b} does not match: not recorded")
        ent = {"backend": b, "fp": fp_of(value), "masked": mask(value), "length": len(value)}
        if b == "file": ent["file"] = file_mode()
    else:
        if not re.match(r"^op://[^\s]+$", ref): raise Fail("a reference is op://vault/item/field")
        ent = {"backend": "op", "ref": ref}
        v = None
        try: v = b_op_get(ref)
        except Fail: pass
        if v: ent.update({"fp": fp_of(v), "masked": mask(v), "length": len(v)})
        else: ent.update({"fp": "", "masked": "(op://, not resolved yet)", "length": 0})
    with Index() as ix:
        old = ix.s.get(name)
        ent["created"] = old["created"] if old else now()
        ent["updated"] = now()
        ent["source"] = source
        ix.s[name] = ent; ix.save()
        if old and old["backend"] != ent["backend"] and old["backend"] != "op": DEL[old["backend"]](name)   # (moved to another backend: the old copy goes)
    return ent


def shred(path):
    try:
        n = os.path.getsize(path)
        with open(path, "r+b") as f:
            f.write(b"\0" * n); f.flush(); os.fsync(f.fileno())
    except OSError:
        pass
    try: os.unlink(path)
    except OSError: pass


def out(args, obj, text):
    if args.json: print(json.dumps(obj, ensure_ascii=False))
    else: print(text)


def machine_label():
    l = cfg_get("HOST_LABEL")
    return l or ("Mac" if sys.platform == "darwin" else os.uname().nodename)


# ── commands: the store ─────────────────────────────────────────────────────────────────────────────────────────

def cmd_set(a):
    check_name(a.name)
    if a.op:
        ent = store(a.name, None, a.source or "op", ref=a.op)
    else:
        if a.from_file:
            try:
                with open(a.from_file, "rb") as f: v = f.read().decode("utf-8", "surrogateescape")
            except OSError as e: raise Fail(f"{a.from_file}: {e.strerror}")
            v = re.sub(r"(\r?\n)+$", "", v)
        elif a.stdin:
            v = re.sub(r"(\r?\n)+$", "", sys.stdin.read())
        else:
            try: v = getpass.getpass(f"Value for {a.name} (hidden): ")
            except (EOFError, OSError): raise Fail("no terminal to type into: use --from-file F [--shred] or --stdin")
            v = v.strip("\r\n")
        ent = store(a.name, v, a.source or ("file" if a.from_file else "stdin" if a.stdin else "typed"), a.backend)
        if a.from_file and a.shred: shred(a.from_file)
    out(a, {"name": a.name, "stored": True, **public(ent)}, f"{a.name}: stored in {ent['backend']}  {ent['masked']}  fp {ent['fp']}")


def public(e):
    return {k: e[k] for k in ("backend", "masked", "length", "fp", "created", "updated", "source", "ref") if k in e}


def cmd_get(a):
    ent = load_index().get(a.name)
    if not ent: raise Fail(f"no secret {a.name} (bhote secret ls)")
    if not (a.reveal or a.clip):
        v = read_value(a.name, ent)
        out(a, {"name": a.name, "present": v is not None, "match": v is not None and (not ent.get("fp") or fp_of(v) == ent["fp"]), **public(ent)},
            f"{a.name}: {'present' if v is not None else 'UNREADABLE'}  {ent['masked']}  {ent['backend']}  fp {ent['fp']}")
        return 0 if v is not None else 1
    if a.reveal and (os.environ.get("CLAUDECODE") or os.environ.get("CLAUDE_CODE_SESSION_ID")) and os.environ.get("BHOTE_ALLOW_REVEAL") != "1":
        raise Fail("--reveal is not for agents: the value would end up in the chat. Use `bhote env run NAME -- cmd` (the command gets it as an environment variable).")
    v = read_value(a.name, ent)
    if v is None: raise Fail(f"{a.name} cannot be read from {ent['backend']}")
    if a.clip:
        clip(v)
        print(f"{a.name}: copied to the clipboard, cleared in {clip_seconds()} s")
    else:
        sys.stdout.write(v + ("\n" if sys.stdout.isatty() else ""))


def clip_seconds():
    try: return max(5, min(600, int(cfg_get("SECRET_CLIP_SECONDS", "30") or 30)))
    except ValueError: return 30


def clip_tools():
    if sys.platform == "darwin": return (["pbcopy"], ["pbpaste"])
    if os.environ.get("WAYLAND_DISPLAY") and which("wl-copy"): return (["wl-copy"], ["wl-paste", "--no-newline"])
    if which("xclip"): return (["xclip", "-selection", "clipboard"], ["xclip", "-selection", "clipboard", "-o"])
    raise Fail("no clipboard tool (pbcopy, wl-copy, xclip)")


def clip(v):
    cp, pp = clip_tools()
    r = run(cp, v.encode())
    if r.returncode != 0: raise Fail("the clipboard tool failed")
    code = ("import subprocess,sys,time,hashlib;time.sleep(%d);cp,pp=%r,%r;"
            "r=subprocess.run(pp,capture_output=True);"
            "h=hashlib.sha256(r.stdout.decode('utf-8','replace').rstrip('\\n').encode()).hexdigest()[:10];"
            "h==sys.argv[1] and subprocess.run(cp,input=b'')") % (clip_seconds(), cp, pp)
    subprocess.Popen([sys.executable, "-c", code, fp_of(v)], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)


def cmd_ls(a):
    if a.machine: return remote(a, ["ls", "--json"], a.machine)
    ix = load_index()
    rows = [{"name": n, **public(e)} for n, e in sorted(ix.items())]
    if a.json: print(json.dumps({"machine": machine_label(), "backend": safe_backend(), "secrets": rows}, ensure_ascii=False)); return
    if not rows: print("no secrets (bhote secret set NAME)"); return
    w = max(len(r["name"]) for r in rows)
    for r in rows:
        print(f"{r['name']:<{w}}  {r['masked']:<22}  {r['backend']:<11} fp {r['fp'] or '-':<10}  {r.get('updated','')[:10]}  {r.get('source','')}")


def safe_backend():
    try: return pick_backend()
    except Fail: return "none"


def cmd_rm(a):
    with Index() as ix:
        e = ix.s.pop(a.name, None)
        if not e: raise Fail(f"no secret {a.name}")
        if e["backend"] != "op": DEL[e["backend"]](a.name)
        ix.save()
    out(a, {"name": a.name, "removed": True}, f"{a.name}: removed")


def cmd_rename(a):
    check_name(a.new)
    ix = load_index(); e = ix.get(a.old)
    if not e: raise Fail(f"no secret {a.old}")
    if a.new in ix: raise Fail(f"{a.new} exists")
    if e["backend"] == "op":
        with Index() as x:
            x.s[a.new] = x.s.pop(a.old); x.save()
    else:
        v = read_value(a.old, e)
        if v is None or fp_of(v) != e["fp"]: raise Fail(f"{a.old} cannot be read back unchanged: not renamed")
        ne = store(a.new, v, e.get("source", "manual"), e["backend"])
        with Index() as x:
            x.s[a.new]["created"] = e.get("created", ne["created"])
            x.s.pop(a.old, None); x.save()
        DEL[e["backend"]](a.old)
    es = load_envs()
    for e in es.values():
        for var, sec in e["vars"].items():
            if sec == a.old: e["vars"][var] = a.new      # (the environments follow the name)
    save_envs(es)
    out(a, {"name": a.new, "renamed_from": a.old}, f"{a.old} → {a.new}")


def verify_one(name, ent):
    try: v = read_value(name, ent)
    except Fail as e: return False, str(e)
    if v is None: return False, "not readable"
    if ent.get("fp") and fp_of(v) != ent["fp"]: return False, "fingerprint differs"
    return True, "ok"


def cmd_verify(a):
    ix = load_index()
    names = sorted(ix) if (a.all or not a.name) else [a.name]
    if a.name and a.name not in ix: raise Fail(f"no secret {a.name}")
    res = []
    for n in names:
        ok, why = verify_one(n, ix[n]); res.append({"name": n, "ok": ok, "why": why, "fp": ix[n].get("fp", "")})
    if a.json: print(json.dumps(res))
    else:
        for r in res: print(f"{r['name']}: {'ok' if r['ok'] else 'FAIL ' + r['why']}  fp {r['fp']}")
    return 0 if all(r["ok"] for r in res) else 1


# ── the machine view (read-only: presence and fingerprint, never a value) ─────────────────────────────────────────

def remote(a, argv, machine):
    """Runs this very file on another machine (copied to its ~/.cache/bhote, as monitors.py is) and passes its output on. Only read-only commands go this way."""
    me = os.path.abspath(__file__)
    try:
        home = subprocess.run(["hc", "exec", "--on", machine, 'mkdir -p "$HOME/.cache/bhote"; printf %s "$HOME"'], capture_output=True, timeout=30).stdout.decode().strip()
        if not home.startswith("/"): raise Fail(f"{machine}: no answer")
        with open(me, "rb") as f:
            w = subprocess.run(["hc", "file", "write", "--on", machine, home + "/.cache/bhote/secretstore.py"], stdin=f, capture_output=True, timeout=30)
        if w.returncode != 0: raise Fail(f"{machine}: could not copy the scanner")
        r = subprocess.run(["hc", "exec", "--on", machine, "python3 \"$HOME/.cache/bhote/secretstore.py\" " + " ".join(argv)], capture_output=True, timeout=300)
    except (OSError, subprocess.TimeoutExpired) as e:
        raise Fail(f"{machine}: {e}")
    if r.returncode != 0: raise Fail(f"{machine}: {r.stderr.decode(errors='replace').strip()[:200] or 'failed'}")
    sys.stdout.write(r.stdout.decode()); return 0


def machine_names():
    try:
        r = subprocess.run(["hc", "machine", "ls", "--json"], capture_output=True, timeout=20)
        return [m["name"] for m in json.loads(r.stdout).get("machines", []) if m.get("state") == "connected"]
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return []


def alias(m):
    """What the panels call a machine: MACHINE_LABELS (name=Alias,...) of the bhote config, else its name."""
    for pair in cfg_get("MACHINE_LABELS").split(","):
        k, _, v = pair.partition("=")
        if k.strip() == m and v.strip(): return v.strip()
    return m


def cmd_machines(a):
    views = {machine_label(): {n: e for n, e in load_index().items()}}
    for m in ([a.machine] if a.machine else machine_names()):
        try:
            import io
            buf, old = io.StringIO(), sys.stdout
            sys.stdout = buf
            try: remote(a, ["ls", "--json"], m)
            finally: sys.stdout = old
            d = json.loads(buf.getvalue()); views[alias(m)] = {x["name"]: x for x in d["secrets"]}
        except (Fail, ValueError, KeyError) as e:
            views[alias(m)] = None
    names = sorted({n for v in views.values() if v for n in v})
    rows = []
    for n in names:
        cells = {}
        fps = {v[n].get("fp") for v in views.values() if v and n in v and v[n].get("fp")}
        for mach, v in views.items():
            if v is None: cells[mach] = {"state": "unreachable"}
            elif n not in v: cells[mach] = {"state": "missing"}
            else: cells[mach] = {"state": "present", "fp": v[n].get("fp", ""), "masked": v[n].get("masked", "")}
        if len(fps) > 1:
            for mach, c in cells.items():
                if c["state"] == "present": c["state"] = "different"
        rows.append({"name": n, "machines": cells})
    if a.json: print(json.dumps({"machines": list(views), "secrets": rows})); return
    ms = list(views); w = max([len(n) for n in names] + [6])
    print(f"{'':<{w}}  " + "  ".join(f"{m:<12}" for m in ms))
    for r in rows: print(f"{r['name']:<{w}}  " + "  ".join(f"{r['machines'][m]['state']:<12}" for m in ms))


# ── discovery ────────────────────────────────────────────────────────────────────────────────────────────────────

def profiles():
    env = os.environ.get("BHOTE_CLAUDE_DIRS")
    if env: return [p for p in env.split(":") if p]
    ds = sorted(set(glob.glob(os.path.join(HOME, ".claude")) + glob.glob(os.path.join(HOME, ".claude[0-9a-zA-Z_-]*"))))
    return [d for d in ds if os.path.isdir(d) and (os.path.isdir(os.path.join(d, "projects")) or os.path.exists(os.path.join(d, "history.jsonl")))]


def context_files():
    fs = []
    for d in profiles():
        for pat in ("projects/**/*", "shell-snapshots/*", "session-env/*/*", "history.jsonl", "paste-cache/*", "plans/*", "todos/*"):
            fs += [p for p in glob.glob(os.path.join(d, pat), recursive=True) if os.path.isfile(p) and not os.path.islink(p)]
    return sorted(set(fs))


ASSIGN = re.compile(r"\b([A-Z][A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|PAT|APIKEY)[A-Z0-9_]*)=[\"']?([^\s\"'\\$<>(){}]{12,})")
KNOWN = re.compile(r"(?<![A-Za-z0-9_-])(sk-ant-[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9_-]{24,}|ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|gho_[A-Za-z0-9]{30,}|"
                   r"lin_api_[A-Za-z0-9]{30,}|lin_oauth_[A-Za-z0-9]{30,}|xox[abprs]-[A-Za-z0-9-]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|glpat-[A-Za-z0-9_-]{20,}|"
                   r"CCIPAT_[A-Za-z0-9_-]{20,}|shpat_[A-Za-z0-9]{20,}|[sr]k_live_[A-Za-z0-9]{20,}|SG\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}|npm_[A-Za-z0-9]{30,})")
HEADER = re.compile(r"(Circle-Token|Private-Token|X-Api-Key|x-api-key|Authorization)[\"']?\s*[:=]\s*[\"']?(?:Bearer |token |Basic )?([A-Za-z0-9._~+/=-]{24,})")
SKIP = re.compile(r"(?i)^(x+|\.+|your|example|changeme|placeholder|<|\$|\*+|abc|test|dummy|fake|redacted|token|secret|password|insert|replace|xxx|null|none|undefined|process\.env|os\.environ|«secret:)")
SAFE_NAMES = {"CLAUDE_CODE_OAUTH_TOKEN"}
PUBLIC_NAME = re.compile(r"^CLAUDE_CODE_|PUBLISHABLE|NEXT_PUBLIC|PUBLIC_KEY|ANON_KEY|^HERDR_|_PATH$|_DIR$|_FILE$|_URL$|^PW_")
PREFIX_NAMES = [("sk-ant-", "ANTHROPIC_API_KEY"), ("AKIA", "AWS_ACCESS_KEY_ID"), ("github_pat_", "GITHUB_TOKEN"), ("ghp_", "GITHUB_TOKEN"), ("gho_", "GITHUB_TOKEN"),
                ("CCIPAT_", "CIRCLECI_TOKEN"), ("sk_live_", "STRIPE_LIVE_KEY"), ("rk_live_", "STRIPE_LIVE_KEY"), ("lin_", "LINEAR_API_KEY"), ("xox", "SLACK_TOKEN"),
                ("AIza", "GOOGLE_API_KEY"), ("glpat-", "GITLAB_TOKEN"), ("shpat_", "SHOPIFY_TOKEN"), ("SG.", "SENDGRID_API_KEY"), ("npm_", "NPM_TOKEN"),
                ("sk-", "OPENAI_API_KEY"), ("eyJ", "AUTH_BEARER")]
HEADER_NAMES = {"circle-token": "CIRCLECI_TOKEN", "private-token": "GITLAB_TOKEN", "x-api-key": "API_KEY"}
KNOWN_PREFIX = ("sk-", "ghp_", "github_pat_", "gho_", "lin_", "xox", "AKIA", "AIza", "glpat-", "CCIPAT_", "shpat_", "sk_live", "rk_live", "SG.", "npm_")


def accept(name, v, kind):
    """False for what is no secret: placeholders, paths, publishable keys, truncated values."""
    if len(v) < 12 or SKIP.match(v) or v.count(v[0]) == len(v) or re.fullmatch(r"[a-z]+", v): return False
    if re.fullmatch(r"[A-Za-z_]+", v) and ("_" in v and len(v) < 24 or v.isupper() or v.islower()): return False
    if "..." in v or "…" in v or v.startswith(("/", "~", "./", "http://", "https://", "pk_test", "pk_live", "pk_")): return False
    if name in SAFE_NAMES or (kind != "format" and PUBLIC_NAME.search(name)): return False
    if kind == "assign" and re.fullmatch(r"[0-9a-f]{40}", v) and "TOKEN" not in name and "KEY" not in name: return False
    if any(v.startswith(p) for p in KNOWN_PREFIX) and len(v) < 20: return False    # (cut off in a message)
    return True


def scan_collect(files=None):
    """{fingerprint: record with the value (memory only)}. Every caller prints masked forms only."""
    found = {}
    files = context_files() if files is None else files
    for p in files:
        try:
            if os.path.getsize(p) > 80_000_000: continue
            m = os.path.getmtime(p)
            with open(p, "r", errors="ignore") as f:
                for line in f:
                    if len(line) > 400_000: line = line[:400_000]
                    hits = [(g.group(1), g.group(2).rstrip(";,"), "assign") for g in ASSIGN.finditer(line)]
                    hits += [("", g.group(1), "format") for g in KNOWN.finditer(line)]
                    hits += [(g.group(1), g.group(2), "header") for g in HEADER.finditer(line)]
                    for name, v, kind in hits:
                        if not accept(name, v, kind): continue
                        fp = fp_of(v)
                        r = found.setdefault(fp, {"value": v, "names": set(), "kinds": set(), "where": {}, "count": 0, "last": 0, "files": set()})
                        if name: r["names"].add(name)
                        r["kinds"].add(kind); r["count"] += 1; r["last"] = max(r["last"], m); r["files"].add(p)
                        base = p.replace(HOME, "~")
                        key = re.sub(r"/[0-9a-f-]{36}(\.jsonl)?$", "/<session>", base)
                        r["where"][key] = r["where"].get(key, 0) + 1
        except OSError:
            pass
    return found


def name_hint(r):
    """A name for a found secret: the variable it was assigned to, else the kind of key; kind-based names carry 4 characters of the fingerprint."""
    for n in sorted(r["names"]):
        if n.lower() in HEADER_NAMES: return HEADER_NAMES[n.lower()], False
        if n != "Authorization" and re.match(r"^[A-Z][A-Z0-9_]*$", n): return n, False
    v = r["value"]
    for p, n in PREFIX_NAMES:
        if v.startswith(p): return n, True
    return "SECRET", True


def decide_names(found, existing):
    """{fp: name}: a name that is free, or the one this fingerprint already has; a clash gets _<fp4> (the most recently seen one keeps the plain name)."""
    by_fp = {e["fp"]: n for n, e in existing.items() if e.get("fp")}
    names, used = {}, set(existing)
    for fp, r in sorted(found.items(), key=lambda kv: -kv[1]["last"]):
        if fp in by_fp: names[fp] = by_fp[fp]; continue
        base, suffix = name_hint(r)
        n = f"{base}_{fp[:4]}" if suffix or base in used else base
        while n in used: n += "x"
        used.add(n); names[fp] = n
    return names


def scan_public(found, existing):
    by_fp = {e["fp"]: n for n, e in existing.items() if e.get("fp")}
    names = decide_names(found, existing)
    rows = []
    for fp, r in found.items():
        rows.append({"id": fp, "name": names[fp], "hint": sorted(r["names"])[:4], "kinds": sorted(r["kinds"]), "masked": mask(r["value"]), "length": len(r["value"]), "seen": r["count"],
                     "files": len(r["files"]), "last": time.strftime("%Y-%m-%d", time.localtime(r["last"])) if r["last"] else "",
                     "where": sorted(r["where"], key=lambda k: -r["where"][k])[:2], "in_store": by_fp.get(fp)})
    rows.sort(key=lambda x: (x["name"], x["last"]))
    return rows


def cmd_scan(a):
    if a.machine: return remote(a, ["scan"] + (["--json"] if a.json else []), a.machine)
    found = scan_collect(); ex = load_index()
    rows = scan_public(found, ex)
    if a.json: print(json.dumps({"host": machine_label(), "files": len(context_files()), "found": rows}, ensure_ascii=False)); return
    if not rows: print("nothing found"); return
    for r in rows:
        print(f"{r['id']}  {r['name']:<28} {r['masked']:<20} {','.join(r['kinds']):<14} seen {r['seen']:>3} in {r['files']} files  last {r['last']}  {'in store as ' + r['in_store'] if r['in_store'] else 'NOT in the store'}")
        for w in r["where"]: print(f"            {w}")


def cmd_import(a):
    if not (a.from_scan or a.config): raise Fail("import --from-scan [--dry-run]  |  import --config [--dry-run]")
    res = []
    if a.config:
        for k in CONFIG_SECRETS:
            v = cfg_get(k)
            if not v or v.startswith("secret:"): continue
            if a.dry_run: res.append({"name": k, "action": "would import", "masked": mask(v)}); continue
            e = store(k, v, "bhote config")
            ok = verify_one(k, e)[0]
            res.append({"name": k, "action": "imported", "masked": e["masked"], "fp": e["fp"], "verified": ok})
            if a.migrate and ok:                       # the config now points at the store (the daemon of this version reads secret:NAME)
                with open(CFG, "r") as f: lines = f.read().split("\n")
                lines = [f"{k}=secret:{k}" if l.startswith(k + "=") else l for l in lines]
                write_atomic(CFG, "\n".join(lines), 0o600); res[-1]["action"] = "imported, config points at the store"
    if a.from_scan:
        found = scan_collect(); ex = load_index()
        skip = [x for x in (a.skip or "").split(",") if x]       # fingerprints (or their start) that are no real secrets: left out
        found = {fp: r for fp, r in found.items() if not any(fp.startswith(x) for x in skip)}
        names = decide_names(found, ex)
        for fp, r in sorted(found.items(), key=lambda kv: names[kv[0]]):
            n = names[fp]
            if n in ex and ex[n].get("fp") == fp: res.append({"name": n, "action": "already stored", "masked": mask(r["value"]), "fp": fp, "verified": verify_one(n, ex[n])[0]}); continue
            if a.dry_run: res.append({"name": n, "action": "would import", "masked": mask(r["value"]), "fp": fp}); continue
            try:
                e = store(n, r["value"], "scan:" + ",".join(sorted(r["kinds"])))
                ok, _ = verify_one(n, e)
                res.append({"name": n, "action": "imported", "masked": e["masked"], "fp": e["fp"], "backend": e["backend"], "verified": ok})
            except Fail as ex_:
                res.append({"name": n, "action": "FAILED", "why": str(ex_), "masked": mask(r["value"]), "fp": fp})
    if a.json: print(json.dumps(res));
    else:
        for r in res: print(f"{r['name']:<30} {r['action']:<15} {r.get('masked',''):<20} fp {r.get('fp','-')}  {'verified' if r.get('verified') else ''}{r.get('why','')}")
    bad = [r for r in res if r["action"] == "FAILED" or r.get("verified") is False]
    return 1 if bad else 0


# ── purge: the values leave the Claude context ──────────────────────────────────────────────────────────────────

def verified_values():
    """[(name, fp, value bytes)] of the secrets that are readable in the store with the right fingerprint (longest first); op references are not resolved here."""
    vs, skipped = [], []
    for n, e in load_index().items():
        if e["backend"] == "op" or len(e.get("masked", "")) == 0: continue
        try: v = read_value(n, e)
        except Fail: v = None
        if v is None or fp_of(v) != e.get("fp") or len(v) < 12 or SKIP.match(v): skipped.append(n); continue
        vs.append((n, e["fp"], v.encode("utf-8", "surrogateescape")))
    vs.sort(key=lambda x: -len(x[2]))
    return vs, skipped


def rewrite(data, vals, is_jsonl):
    """(new bytes, {name#fp: count}, lines kept because the rewrite would not be valid JSON)."""
    counts, kept = {}, 0
    if not is_jsonl:
        for n, fp, v in vals:
            c = data.count(v)
            if c: counts[f"{n}#{fp}"] = c; data = data.replace(v, f"«secret:{n}#{fp}»".encode())
        return data, counts, kept
    outl = []
    for line in data.splitlines(keepends=True):
        new, lc = line, {}
        for n, fp, v in vals:
            c = new.count(v)
            if c: lc[f"{n}#{fp}"] = c; new = new.replace(v, f"«secret:{n}#{fp}»".encode())
        if lc:
            try: json.loads(new.decode("utf-8"));
            except (ValueError, UnicodeDecodeError): outl.append(line); kept += 1; continue
            for k, c in lc.items(): counts[k] = counts.get(k, 0) + c
        outl.append(new)
    return b"".join(outl), counts, kept


def cmd_purge(a):
    vals, skipped = verified_values()
    if not vals: raise Fail("no verified secrets in the store: nothing to remove (bhote secret import --from-scan first)")
    sid = os.environ.get("CLAUDE_CODE_SESSION_ID", "")
    rep = {"dry_run": a.dry_run, "secrets": len(vals), "unverified_skipped": sorted(skipped), "files": [], "skipped_fresh": 0, "skipped_active": 0, "changed_meanwhile": 0, "kept_lines": 0, "replaced": 0}
    t0 = time.time()
    for p in context_files():
        try:
            st = os.stat(p)
            if t0 - st.st_mtime < MIN_AGE: rep["skipped_fresh"] += 1; continue
            if sid and sid in p: rep["skipped_active"] += 1; continue
            if st.st_size > 200_000_000: continue
            with open(p, "rb") as f: data = f.read()
            if not any(v in data for _, _, v in vals): continue
            new, counts, kept = rewrite(data, vals, p.endswith(".jsonl"))
            rep["kept_lines"] += kept
            if not counts: continue
            if not a.dry_run:
                st2 = os.stat(p)
                if (st2.st_mtime_ns, st2.st_size) != (st.st_mtime_ns, st.st_size): rep["changed_meanwhile"] += 1; continue   # (a session wrote to it meanwhile: next run)
                tmp = os.path.join(os.path.dirname(p), f".{os.path.basename(p)}.purge.{os.getpid()}")
                try:
                    with open(tmp, "wb") as f:
                        f.write(new); f.flush(); os.fsync(f.fileno())
                    os.chmod(tmp, st.st_mode & 0o7777); os.utime(tmp, ns=(st.st_atime_ns, st.st_mtime_ns))
                    os.replace(tmp, p)
                finally:
                    if os.path.exists(tmp): os.unlink(tmp)
            rep["replaced"] += sum(counts.values())
            rep["files"].append({"file": p.replace(HOME, "~"), "counts": counts})
        except OSError as e:
            rep.setdefault("errors", []).append(f"{p.replace(HOME, '~')}: {e.strerror}")
    if a.json: print(json.dumps(rep, ensure_ascii=False)); return
    print(f"{'DRY RUN: ' if a.dry_run else ''}{len(vals)} verified secrets" + (f", not verified (left alone): {', '.join(sorted(skipped))}" if skipped else ""))
    for f in rep["files"]: print(f"  {f['file']}: " + ", ".join(f"{k} x{c}" for k, c in sorted(f["counts"].items())))
    print(f"{'would replace' if a.dry_run else 'replaced'} {rep['replaced']} occurrences in {len(rep['files'])} files; skipped as fresh (<{MIN_AGE // 60} min): {rep['skipped_fresh']}, active session: {rep['skipped_active']}, "
          f"changed meanwhile: {rep['changed_meanwhile']}, lines kept (not valid JSON after the change): {rep['kept_lines']}")


# ── environments ───────────────────────────────────────────────────────────────────────────────────────────────────

def load_envs():
    try:
        with open(ENVS, "rb") as f: return json.load(f)
    except (OSError, ValueError): return {}


def save_envs(d): write_atomic(ENVS, json.dumps(d, indent=1, sort_keys=True), 0o600)


def env_for_dir(path):
    path = os.path.realpath(path); best = None
    for n, e in load_envs().items():
        for d in e.get("dirs", []):
            d = os.path.realpath(os.path.expanduser(d))
            if path == d or path.startswith(d + os.sep):
                if best is None or len(d) > best[0]: best = (len(d), n)
    return best[1] if best else None


def env_vars(name):
    es = load_envs()
    if name not in es: raise Fail(f"no environment {name} (bhote env ls)")
    ix = load_index(); res, missing = {}, []
    for var, sec in es[name]["vars"].items():
        e = ix.get(sec); v = read_value(sec, e) if e else None
        if v is None: missing.append(sec)
        else: res[var] = v
    if missing: raise Fail(f"{name}: missing or unreadable secrets: {', '.join(sorted(set(missing)))} (bhote secret ls)")
    return res


DEFAULT_SKIP = {"ANTHROPIC_API_KEY"}          # (it would switch `claude` to API billing)
AUTO_NAME_RE = re.compile(r"_[0-9a-f]{4}$")     # (a name `bhote secret scan` made up for a key it could not name: GITHUB_TOKEN_66d2)


def default_vars():
    """The environment of every folder without an environment of its own: each secret with a name of its own (CIRCLECI_TOKEN) as the variable of that name.
    Unreadable ones are left out, never an error. Off with BHOTE_ENV_DEFAULT=off."""
    if os.environ.get("BHOTE_ENV_DEFAULT") == "off": return {}
    res = {}
    for sec, e in load_index().items():
        if not NAME_RE.match(sec) or sec in DEFAULT_SKIP or AUTO_NAME_RE.search(sec): continue
        try: v = read_value(sec, e)
        except Exception: v = None
        if v is not None: res[sec] = v
    return res


def cmd_env(a):
    es = load_envs(); sub = a.sub
    if sub == "ls":
        rows = [{"name": n, "vars": e["vars"], "dirs": e.get("dirs", [])} for n, e in sorted(es.items())]
        if a.json: print(json.dumps(rows)); return
        for r in rows: print(f"{r['name']}: " + ", ".join(f"{k}={v}" for k, v in r["vars"].items()) + (f"   in {', '.join(r['dirs'])}" if r["dirs"] else ""))
        if not rows: print("no environments (bhote env set NAME VAR=SECRET ...)")
    elif sub == "set":
        if not NAME_RE.match(a.name or ""): raise Fail("an environment name is letters, digits and _")
        e = es.setdefault(a.name, {"vars": {}, "dirs": []}); ix = load_index()
        for pair in a.rest:
            if "=" not in pair: pair = f"{pair}={pair}"          # NAME alone: the variable has the name of the secret
            var, sec = pair.split("=", 1)
            if not NAME_RE.match(var): raise Fail(f"{var}: not a variable name")
            if sec not in ix: raise Fail(f"{sec}: no such secret (bhote secret ls)")
            e["vars"][var] = sec
        save_envs(es); print(f"{a.name}: " + ", ".join(f"{k}={v}" for k, v in e["vars"].items()))
    elif sub == "unset":
        e = es.get(a.name) or (_ for _ in ()).throw(Fail(f"no environment {a.name}"))
        for v in a.rest: e["vars"].pop(v, None)
        save_envs(es); print(f"{a.name}: " + ", ".join(e["vars"]))
    elif sub == "rm":
        if es.pop(a.name, None) is None: raise Fail(f"no environment {a.name}")
        save_envs(es); print(f"{a.name}: removed")
    elif sub in ("bind", "unbind"):
        e = es.get(a.name) or (_ for _ in ()).throw(Fail(f"no environment {a.name}"))
        d = os.path.realpath(os.path.expanduser(a.rest[0] if a.rest else os.getcwd()))
        if sub == "bind" and d not in e.setdefault("dirs", []): e["dirs"].append(d)
        if sub == "unbind": e["dirs"] = [x for x in e.get("dirs", []) if x != d]
        save_envs(es); print(f"{a.name}: {sub} {d}")
    elif sub == "which":
        n = env_for_dir(a.name or os.getcwd())
        if n: print(n)
        else: return 1
    elif sub == "run":
        cmd = list(a.rest)
        if cmd and cmd[0] == "--": cmd = cmd[1:]
        name = a.name
        dflt = False
        if name == "--": name = None; cmd = list(a.rest)
        if name in (None, "--auto", "-"):
            name = env_for_dir(os.getcwd())
            if not name:                               # nothing bound to this folder: the secrets with a name of their own (default_vars)
                if not cmd: raise Fail("env run [NAME] -- command")
                dflt = True
        if not cmd: raise Fail("env run NAME -- command")
        if os.environ.get("BHOTE_BROKER") != "off" and not keychain_reachable():      # (outside the login session: the broker of the login session runs it)
            if broker_alive(): return broker_run(cmd, None if dflt else name, dflt)
            print("bhote: the keychain is not reachable from this process and no broker runs (in your own terminal: bhote secret broker install)", file=sys.stderr)
        env = dict(os.environ); env.update(default_vars() if dflt else env_vars(name))
        try: os.execvpe(cmd[0], cmd, env)
        except OSError as e: raise Fail(f"{cmd[0]}: {e.strerror}")
    else:
        raise Fail("env ls | set NAME VAR=SECRET... | unset NAME VAR... | rm NAME | bind NAME [dir] | unbind NAME [dir] | which [dir] | run NAME -- cmd")


# ── the broker: commands for processes that cannot reach the keychain ────────────────────────────────────────────────
# A process outside the login session of the user (a sandboxed agent, ssh, cron) cannot use the macOS Keychain. The broker is a small loop that runs in the
# login session (a LaunchAgent: `bhote secret broker install`, once, in your own terminal) and runs the commands such a process asks for, with the secrets
# in their environment, and hands the output back. The protocol is files in a folder of the user (0700): req/<id>.json (argv, cwd, environment), res/<id>.out,
# .err and, last, .rc. A command the broker runs is the user's own, but not under the sandbox of the one who asked: so only the programs in
# SECRET_BROKER_ALLOW run (default gh, git), by bare name, in a folder below the home, never with a value of a secret in the answer file except what the
# program itself prints. Every request is logged in bhote's data folder (secret-broker.log; `bhote secret broker log`): time, the first words of the command, folder, refusals, exit code.
BROKER_DIR = os.environ.get("BHOTE_BROKER_DIR") or os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.join(HOME, ".local", "share"), "bhote", "broker")
BROKER_ALLOW = "gh,git"
BROKER_LABEL = "dev.bhote.secret-broker"
BROKER_TIMEOUT = 600


def keychain_reachable():
    """False when this process cannot use the macOS Keychain (outside the login session). Other systems: True (their backends fail by themselves)."""
    if sys.platform != "darwin" or not which("security"): return True
    # (not by looking for an item: without the search list of the login session every item is "not found" (44), which looks like a working keychain)
    try:
        r = run(["security", "list-keychains"], timeout=15)
        return r.returncode == 0 and bool(r.stdout.strip())
    except Fail: return False


def broker_alive():
    try: return time.time() - os.stat(os.path.join(BROKER_DIR, "alive")).st_mtime < 20
    except OSError: return False


def broker_run(cmd, name, dflt):
    """env run through the broker: the answer's output is copied as it comes, the exit code of the command is returned."""
    import uuid
    rid = uuid.uuid4().hex
    req = {"id": rid, "argv": cmd, "cwd": os.getcwd(), "env": name, "default": bool(dflt)}
    write_atomic(os.path.join(BROKER_DIR, "req", rid + ".json"), json.dumps(req))
    res = os.path.join(BROKER_DIR, "res", rid)
    pos = {".out": 0, ".err": 0}; sinks = {".out": sys.stdout.buffer, ".err": sys.stderr.buffer}
    end = time.time() + BROKER_TIMEOUT + 60
    sent = time.time()
    reqf = os.path.join(BROKER_DIR, "req", rid + ".json")
    def drain():
        for ext in pos:
            try:
                with open(res + ext, "rb") as f:
                    f.seek(pos[ext]); b = f.read()
            except OSError: continue
            if b: pos[ext] += len(b); sinks[ext].write(b); sinks[ext].flush()
    try:
        while time.time() < end:
            drain()
            try: rc = int(open(res + ".rc").read().strip())
            except (OSError, ValueError):
                if os.path.exists(reqf) and time.time() - sent > 15:       # (the broker takes a request at once: nobody did, it has stopped)
                    raise Fail("the broker did not pick up the request (it stopped? bhote secret broker status; in your own terminal: bhote secret broker install)")
                time.sleep(0.1); continue
            drain(); return rc
        raise Fail("the broker did not answer (bhote secret broker status)")
    finally:
        for ext in (".out", ".err", ".rc"):
            try: os.unlink(res + ext)
            except OSError: pass
        try: os.unlink(os.path.join(BROKER_DIR, "req", rid + ".json"))
        except OSError: pass


BROKER_LOG = os.path.join(os.environ.get("BHOTE_DATA") or os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.join(HOME, ".local", "share"), "bhote"), "secret-broker.log")


def _broker_log(line):
    """What the broker did, in the data folder of bhote beside the other logs (kharka-watch.log ...): one line per request, never a secret or the output."""
    try:
        os.makedirs(os.path.dirname(BROKER_LOG), exist_ok=True)
        try:
            if os.path.getsize(BROKER_LOG) > 1_000_000: os.replace(BROKER_LOG, BROKER_LOG + ".1")
        except OSError: pass
        with open(BROKER_LOG, "a") as f: f.write(f"{now()} {line}\n")
    except OSError: pass


def _words(argv, n=3):
    """The first words of a command for the log (cut short: the rest could be anything somebody typed)."""
    return " ".join(x[:60] for x in argv[:n]) if isinstance(argv, list) else "?"


def _git_unsafe(args):
    """Options of git before its subcommand that run programs (-c core.sshCommand=..., a changed exec path)."""
    for a in args:
        if not a.startswith("-"): return False
        if a in ("-c", "--config-env", "--exec-path") or a.startswith(("--config-env=", "--exec-path=")) or (a.startswith("-c") and not a.startswith("--")): return True
    return False


def _broker_handle(path):
    import threading
    rid = os.path.basename(path)[:-5]
    res = os.path.join(BROKER_DIR, "res", rid)
    def finish(rc, msg=""):
        if msg:
            with open(res + ".err", "ab") as f: f.write((msg + "\n").encode())
        write_atomic(res + ".rc", str(rc))
    try:
        st = os.stat(path)
        if st.st_uid != os.getuid() or st.st_mode & 0o022:
            _broker_log(f"refused {rid}: the request file is not the user's own (owner or permissions)"); os.unlink(path); return
        req = json.load(open(path)); os.unlink(path)
        argv, cwd = req.get("argv"), os.path.realpath(req.get("cwd") or "")
        if not re.fullmatch(r"[0-9a-f]{32}", str(req.get("id", ""))) or req["id"] != rid: _broker_log(f"refused {rid}: bad request id"); return
        allow = [x.strip() for x in (cfg_get("SECRET_BROKER_ALLOW", BROKER_ALLOW) or BROKER_ALLOW).split(",") if x.strip()]
        if not isinstance(argv, list) or not argv or not all(isinstance(x, str) for x in argv): return finish(126, "bhote broker: no command")
        if "/" in argv[0] or argv[0] not in allow:
            _broker_log(f"refused {_words(argv)} in {cwd}: {argv[0][:30]} is not in SECRET_BROKER_ALLOW"); return finish(126, f"bhote broker: {argv[0]} is not allowed (SECRET_BROKER_ALLOW={','.join(allow)}; bhote config set SECRET_BROKER_ALLOW ...)")
        if argv[0] == "git" and _git_unsafe(argv[1:]): _broker_log(f"refused {_words(argv)} in {cwd}: git option that runs programs"); return finish(126, "bhote broker: git -c / --config-env / --exec-path before the subcommand are not allowed here")
        if not (cwd == HOME or cwd.startswith(HOME + os.sep)) or not os.path.isdir(cwd): _broker_log(f"refused {_words(argv)} in {cwd}: folder outside the home folder"); return finish(126, "bhote broker: the folder must be below the home folder")
        try:
            name = req.get("env") or (None if req.get("default") else env_for_dir(cwd))
            env = dict(os.environ); env["BHOTE_BROKER"] = "off"
            env.update(env_vars(name) if name else default_vars())
        except Fail as e: _broker_log(f"failed {_words(argv)} in {cwd}: {e}"); return finish(1, f"bhote: {e}")
        t0 = time.time(); _broker_log(f"run {_words(argv)} in {cwd} (env {name or 'default secrets'}: {len(env) - len(os.environ)} variables)")
        p = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def pump(src, ext):
            with open(res + ext, "ab", buffering=0) as f:
                for chunk in iter(lambda: src.read1(4096), b""): f.write(chunk)
        ts = [threading.Thread(target=pump, args=(p.stdout, ".out"), daemon=True), threading.Thread(target=pump, args=(p.stderr, ".err"), daemon=True)]
        for t in ts: t.start()
        try: rc = p.wait(timeout=BROKER_TIMEOUT)
        except subprocess.TimeoutExpired:
            p.kill(); rc = 124
        for t in ts: t.join(5)
        _broker_log(f"done {_words(argv, 2)} rc={rc} in {time.time() - t0:.1f}s")
        finish(rc)
    except FileNotFoundError as e:
        try: finish(127, f"bhote broker: {e.filename}: not found")
        except OSError: pass
    except Exception as e:
        try: finish(1, f"bhote broker: {type(e).__name__}")
        except OSError: pass


def broker_serve():
    import threading
    for d in ("req", "res"): os.makedirs(os.path.join(BROKER_DIR, d), mode=0o700, exist_ok=True)
    os.chmod(BROKER_DIR, 0o700)
    last = 0
    while True:
        if time.time() - last >= 5:
            write_atomic(os.path.join(BROKER_DIR, "alive"), str(os.getpid())); last = time.time()
            for f in glob.glob(os.path.join(BROKER_DIR, "res", "*")):      # (what nobody took within an hour)
                try:
                    if time.time() - os.stat(f).st_mtime > 3600: os.unlink(f)
                except OSError: pass
        for f in sorted(glob.glob(os.path.join(BROKER_DIR, "req", "*.json"))):
            threading.Thread(target=_broker_handle, args=(f,), daemon=True).start()
            time.sleep(0.05)
        time.sleep(0.2)


def cmd_broker(a):
    act = a.action or "status"
    plist = os.path.join(HOME, "Library", "LaunchAgents", BROKER_LABEL + ".plist")
    if act == "run":
        return broker_serve()
    if act == "log":
        try: lines = open(BROKER_LOG, errors="ignore").read().splitlines()
        except OSError: lines = []
        print("\n".join(lines[-40:]) if lines else f"no requests yet ({BROKER_LOG})"); return 0
    if act == "status":
        print(f"broker: {'running' if broker_alive() else 'not running'}  ·  keychain reachable from here: {'yes' if keychain_reachable() else 'no'}  ·  allowed: {cfg_get('SECRET_BROKER_ALLOW', BROKER_ALLOW)}")
        return 0 if broker_alive() else 1
    if sys.platform != "darwin": raise Fail("the broker is for macOS (the keychain of the login session); other systems have no use for it")
    if act == "install":
        if not keychain_reachable(): raise Fail("run this in your own terminal (the login session), not from a sandboxed or background process: the broker has to live there")
        me = os.path.abspath(__file__)
        body = ('<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict>\n'
                f'<key>Label</key><string>{BROKER_LABEL}</string>\n<key>ProgramArguments</key><array><string>{sys.executable}</string><string>{me}</string><string>broker</string><string>run</string></array>\n'
                f'<key>EnvironmentVariables</key><dict><key>PATH</key><string>{HOME}/.cargo/bin:{HOME}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string></dict>\n'
                '<key>RunAtLoad</key><true/><key>KeepAlive</key><true/>\n</dict></plist>\n')
        write_atomic(plist, body, 0o644)
        subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}/{BROKER_LABEL}"], capture_output=True)
        r = subprocess.run(["launchctl", "bootstrap", f"gui/{os.getuid()}", plist], capture_output=True, text=True)
        if r.returncode != 0: raise Fail(f"launchctl bootstrap failed: {r.stderr.strip()[:200]}")
        print(f"broker: installed ({BROKER_LABEL}); allowed programs: {cfg_get('SECRET_BROKER_ALLOW', BROKER_ALLOW)} (bhote config set SECRET_BROKER_ALLOW gh,git,curl to change)")
        return 0
    if act == "uninstall":
        subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}/{BROKER_LABEL}"], capture_output=True)
        try: os.unlink(plist)
        except OSError: pass
        print("broker: removed"); return 0
    raise Fail("broker run | status | log | install | uninstall")


# ── main ───────────────────────────────────────────────────────────────────────────────────────────────────────────

def main(argv):
    p = argparse.ArgumentParser(prog="bhote secret", description="the secret manager of bhote")
    p.add_argument("--json", action="store_true")
    sp = p.add_subparsers(dest="cmd", required=True)

    def add(name, fn, *flags, pos=()):
        q = sp.add_parser(name); q.set_defaults(fn=fn); q.add_argument("--json", action="store_true", default=argparse.SUPPRESS)
        for x in pos: q.add_argument(x[0], **x[1])
        for f in flags: q.add_argument(f[0], **f[1])
        return q
    name = [("name", {})]
    add("set", cmd_set, ("--from-file", {}), ("--stdin", {"action": "store_true"}), ("--shred", {"action": "store_true"}), ("--op", {}), ("--source", {}), ("--backend", {}), pos=name)
    add("get", cmd_get, ("--reveal", {"action": "store_true"}), ("--clip", {"action": "store_true"}), pos=name)
    add("ls", cmd_ls, ("--machine", {}))
    add("rm", cmd_rm, pos=name)
    add("rename", cmd_rename, pos=[("old", {}), ("new", {})])
    add("verify", cmd_verify, ("--all", {"action": "store_true"}), pos=[("name", {"nargs": "?"})])
    add("scan", cmd_scan, ("--machine", {}))
    add("import", cmd_import, ("--from-scan", {"action": "store_true"}), ("--config", {"action": "store_true"}), ("--migrate", {"action": "store_true"}), ("--skip", {}), ("--dry-run", {"action": "store_true"}))
    add("purge-context", cmd_purge, ("--dry-run", {"action": "store_true"}))
    add("machines", cmd_machines, ("--machine", {}))
    add("env", cmd_env)
    add("broker", cmd_broker, pos=[("action", {"nargs": "?"})])
    argv = list(argv)
    if argv[:1] == ["env"]:       # (by hand: argparse mangles the -- before the command of env run)
        rest = argv[1:]; post = []
        if "--" in rest: i = rest.index("--"); rest, post = rest[:i], rest[i:]
        js = "--json" in rest; rest = [x for x in rest if x != "--json"]
        a = argparse.Namespace(fn=cmd_env, json=js, sub=rest[0] if rest else "", name=rest[1] if len(rest) > 1 else None, rest=rest[2:] + post)
    else:
        a = p.parse_args(argv)
    if not hasattr(a, "json"): a.json = False
    try:
        return a.fn(a) or 0
    except Fail as e:
        print(f"bhote: {e}", file=sys.stderr); return 1
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
