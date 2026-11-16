#!/usr/bin/env python3
"""bhote task sources: the adapters that mirror outside systems (Linear) into tasks.

Called by `bhote task sync` (the script reconciles the files; this module only talks to the outside and normalizes). Standard library only.

    tasksources.py list [--source linear|all] [--dry-run]   {"tasks": [...], "ok": ["linear"], "errors": {"linear": "..."}, "skipped": {"linear": "no key"},
                                                             "filtered": [{"source", "reason"}, ...]}  (--dry-run: also no API cache entry is written)
    tasksources.py fetch --source S <ref>               one normalized task (or null)
    tasksources.py set-status --source S <ref> <status> [--id ID]   exit 0 done, 3 not changeable (nothing is retried), 1 error
    tasksources.py open <url>                           prints the URL when it is http(s)

One adapter per source: list() -> normalized tasks, fetch(ref), open(url), optional set_status(). A normalized task:
    {source, source_ref, source_url, title, description, status, state}
(status is bhote's vocabulary: now next waiting review later done; state is the name in the outside system).

Linear mirrors ONLY the issues in triage (workflow state of type `triage`) that are assigned to nobody (kind `triage`, to decide), plus the open issues assigned to you (kind `mine`), of the teams in LINEAR_TEAMS (all teams the key sees when unset).
Each is status next. An issue that leaves triage drops out of the list; the caller then marks its task done.

Secrets: the key comes from the secret store through secretstore.read_value (an internal call: `bhote secret get --reveal` is refused in Claude
sessions on purpose), else from the environment. It goes to curl on stdin (-K -), never into an argument, a cache entry, a log or an error message.
Reads go through the shared API cache (the format and file names of daemon/src/apicache.rs); writes never do.
"""
import json, os, re, subprocess, sys, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import secretstore as ss  # noqa: E402

LINEAR_URL = "https://api.linear.app/graphql"
DESC_MAX = 140
STATUSES = {"now", "next", "waiting", "review", "later", "done"}
DRY = False                                 # --dry-run: nothing is written, not even the API cache
FILTERED = []                               # what the list rules left out: {source, reason} (never a title, never an id)


class Fail(Exception):
    code = 0


class NotChangeable(Exception):
    pass


# ── settings, secrets ─────────────────────────────────────────────────────────────────────────────────────────

def cfg(key, default=""):
    v = ss.cfg_get(key, default)
    return v if v != "" else default


def secret(name):
    """The value of a secret: the environment (tests, CI), else the store. None without one."""
    v = os.environ.get(name)
    if v: return v.strip()
    try:
        return (ss.read_value(name) or "").strip() or None
    except Exception:
        return None


def clean(s, n=500):
    return re.sub(r"[\x00-\x1f\x7f]", " ", str(s or "")).strip()[:n]


def short(s):
    """A description is a short note: the first line, cut."""
    t = clean(str(s or "").strip().split("\n", 1)[0] if s else "", DESC_MAX)
    return t


# ── the shared API cache (the same entries as bash api_cached and apicache.rs) ───────────────────────────────

def cksum(b):
    crc = 0
    def feed(x):
        nonlocal crc
        crc ^= x << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if crc & 0x80000000 else (crc << 1) & 0xFFFFFFFF
    for x in b: feed(x)
    n = len(b)
    while n > 0:
        feed(n & 0xFF); n >>= 8
    return (~crc) & 0xFFFFFFFF, len(b)


def cache_dir():
    d = os.environ.get("BHOTE_API_CACHE")
    if d: return d
    base = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
    return os.path.join(base, "bhote", "api")


def cache_file(key):
    pre = re.sub(r"[^A-Za-z0-9]", "_", key)[:32]
    crc, ln = cksum(key.encode())
    return os.path.join(cache_dir(), f"{pre}-{crc}-{ln}.json")


def cached(key, fetch):
    """The body of fetch() (a str), through the API cache: fresh within its ttl, stale with backoff on an error. Raises Fail without any body."""
    if os.environ.get("BHOTE_API_CACHE") == "off": return fetch()
    try: base = max(1, int(cfg("API_CACHE_EVERY", "15")))
    except ValueError: base = 15
    f = cache_file(key); now = int(time.time()); ent = {}
    try:
        with open(f) as fh: ent = json.load(fh)
    except (OSError, ValueError):
        ent = {}
    fresh = ent.get("fetched_at", 0) + ent.get("ttl", 0) > now and ent.get("status") != "error"
    if (fresh or ent.get("retry_at", 0) > now) and ent.get("body"): return ent["body"]
    if ent.get("retry_at", 0) > now and not ent.get("body"): raise Fail("backing off after an error")
    try:
        body = fetch()
        ttl = min(base * 6, 120); ttl = max(ttl, base)
        save(f, {"key": key, "fetched_at": now, "tried_at": now, "ttl": ttl, "status": "ok", "etag": "", "body": body, "error": "", "fails": 0, "retry_at": 0})
        return body
    except Fail as e:
        fails = int(ent.get("fails", 0)); wait = min(base << min(fails, 10), 900)
        last = ent.get("body", "")
        save(f, {"key": key, "fetched_at": ent.get("fetched_at", 0), "tried_at": now, "ttl": ent.get("ttl", base), "status": "stale" if last else "error", "etag": "", "body": last, "error": "failed", "fails": fails + 1, "retry_at": now + wait})
        if last: return last
        raise e


def save(path, obj):
    if DRY: return
    try:
        os.makedirs(os.path.dirname(path), mode=0o700, exist_ok=True)
        fd, tmp = tempfile.mkstemp(prefix=os.path.basename(path) + ".tmp.", dir=os.path.dirname(path))
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w") as fh: json.dump(obj, fh)
        os.replace(tmp, path)
    except OSError:
        pass


# ── HTTP through curl: the key on stdin, never an argument ───────────────────────────────────────────────────

def http(method, url, headers, body=None, timeout=15):
    """Returns (status, text). The headers (with the key) go to curl as a config on stdin; a body goes through a private temp file."""
    for v in headers.values():
        if re.search(r'["\\\x00-\x1f]', v): raise Fail("a header value with a quote or control character")
    cfgtxt = "".join(f'header = "{k}: {v}"\n' for k, v in headers.items())
    args = ["curl", "-s", "--max-time", str(timeout), "-X", method, "-w", "\n%{http_code}", "-K", "-", url]
    tmp = None
    try:
        if body is not None:
            fd, tmp = tempfile.mkstemp(prefix="bhote-task-"); os.fchmod(fd, 0o600)
            with os.fdopen(fd, "w") as fh: fh.write(body)
            args[1:1] = ["--data-binary", "@" + tmp]
        try:
            r = subprocess.run(args, input=cfgtxt.encode(), capture_output=True, timeout=timeout + 5)
        except (OSError, subprocess.TimeoutExpired):
            raise Fail("curl failed")
    finally:
        if tmp:
            try: os.unlink(tmp)
            except OSError: pass
    if r.returncode != 0: raise Fail("curl failed")
    out = r.stdout.decode("utf-8", "replace")
    text, _, code = out.rpartition("\n")
    try: st = int(code.strip())
    except ValueError: raise Fail("no HTTP status")
    return st, text


def ok_body(st, text, what):
    if st >= 400:
        e = Fail(f"{what}: the key is not accepted (HTTP {st})" if st == 401 else f"{what}: HTTP {st}")
        e.code = st
        raise e
    return text


# ── Linear ───────────────────────────────────────────────────────────────────────────────────────────────────

class Linear:
    name = "linear"

    def key(self):
        k = secret("LINEAR_API_KEY")
        if not k: raise Skip("no LINEAR_API_KEY in the secret store (bhote secret set LINEAR_API_KEY)")
        return k

    def gql(self, query, variables, cache=None):
        k = self.key()
        def go():
            st, text = http("POST", LINEAR_URL, {"Authorization": k, "Content-Type": "application/json"}, json.dumps({"query": query, "variables": variables}))
            if st in (400, 401, 403) and (st == 401 or "authentication" in text.lower()):
                e = Fail("Linear: the key is not accepted (HTTP %d)" % st); e.code = st; raise e
            ok_body(st, text, "Linear")
            try: d = json.loads(text)
            except ValueError: raise Fail("Linear: not JSON")
            if d.get("errors"): raise Fail("Linear: " + clean((d["errors"][0] or {}).get("message", "error"), 120))
            return text
        text = cached(cache, go) if cache else go()
        data = json.loads(text).get("data")
        if not isinstance(data, dict): raise Fail("Linear: no data in the answer")
        return data

    def norm(self, i, kind="triage"):
        st = i.get("state") or {}
        status = "next"
        if kind == "mine":      # the workflow state decides: started is now (a review state: review), backlog is later, the rest next
            status = "review" if "review" in (st.get("name") or "").lower() else {"started": "now", "backlog": "later"}.get(st.get("type"), "next")
        return {"source": "linear", "source_ref": clean(i.get("identifier")), "source_url": clean(i.get("url"), 300), "kind": kind,
                "title": clean(i.get("title")), "description": short(i.get("description")), "status": status, "state": clean(st.get("name"), 60)}

    def pages(self, flt, tag, kind, keep):
        q = ("query($after: String) { issues(first: 100, after: $after, filter: %s) { nodes { identifier title description url state { name type } assignee { isMe } team { key } } "
             "pageInfo { hasNextPage endCursor } } }" % flt)
        out, after = [], None
        for page in range(10):
            d = self.gql(q, {"after": after}, cache=f"linear {tag} p{page} {after or ''}")
            a = d["issues"]
            for i in a["nodes"]:
                if not keep(i):
                    FILTERED.append({"source": "linear", "reason": "not " + kind}); continue
                out.append(self.norm(i, kind))
            if not a["pageInfo"]["hasNextPage"]: break
            after = a["pageInfo"]["endCursor"]
            if not after: raise Fail("Linear: a next page without a cursor")
        else:
            raise Fail("Linear: more than 1000 issues (%s)" % tag)
        return out

    def list(self):
        teams = [x.strip().upper() for x in cfg("LINEAR_TEAMS").split(",") if x.strip()]
        tm = (', team: {key: {in: [%s]}}' % ", ".join(json.dumps(t) for t in teams)) if teams else ""
        inteam = lambda i: not teams or (i.get("team") or {}).get("key", "").upper() in teams
        # to decide: in triage and assigned to nobody
        out = self.pages('{state: {type: {eq: "triage"}}, assignee: {null: true}' + tm + "}", f"triage {','.join(teams)}", "triage",
                         lambda i: (i.get("state") or {}).get("type") == "triage" and not i.get("assignee") and inteam(i))
        # the others: open and assigned to you (its own filter in the panel)
        out += self.pages('{assignee: {isMe: {eq: true}}, state: {type: {in: ["backlog", "unstarted", "started"]}}' + tm + "}", f"mine {','.join(teams)}", "mine",
                          lambda i: (i.get("state") or {}).get("type") in ("backlog", "unstarted", "started") and (i.get("assignee") or {}).get("isMe") and inteam(i))
        return out

    def fetch(self, ref):
        q = "query($id: String!) { issue(id: $id) { identifier title description url state { name type } team { key } } }"
        i = self.gql(q, {"id": ref}).get("issue")
        return self.norm(i) if i else None

    def open(self, url):
        return url

    def set_status(self, ref, status, ident=""):
        """Only `done` is written back: the issue moves to the Done state of its team. Every other status stays local."""
        if status != "done": raise NotChangeable("only done is written back to Linear")
        q = "query($id: String!) { issue(id: $id) { id team { states { nodes { id name type position } } } } }"
        i = self.gql(q, {"id": ref}).get("issue")
        if not i: raise NotChangeable("the issue is gone")
        nodes = sorted(i["team"]["states"]["nodes"], key=lambda s: s.get("position", 0))
        pick = next((s for s in nodes if s["name"].lower() == "done" and s["type"] == "completed"), None) or next((s for s in nodes if s["type"] == "completed"), None)
        if not pick: raise NotChangeable("no Done state in the team")
        m = "mutation($id: String!, $s: String!) { issueUpdate(id: $id, input: {stateId: $s}) { success } }"
        d = self.gql(m, {"id": i["id"], "s": pick["id"]})
        if not d["issueUpdate"]["success"]: raise Fail("Linear refused the change")

    PRIO = {"none": 0, "urgent": 1, "high": 2, "medium": 3, "low": 4}

    def triage(self, ref, action, priority=""):
        """Accept (optionally with a priority) moves the issue out of triage into the default state of its team; decline moves it to the team's canceled state."""
        if action not in ("accept", "decline"): raise Fail("triage: accept or decline")
        if priority and priority not in self.PRIO: raise Fail("priority: urgent, high, medium, low or none")
        q = "query($id: String!) { issue(id: $id) { id state { type } team { defaultIssueState { id } states { nodes { id name type position } } } } }"
        i = self.gql(q, {"id": ref}).get("issue")
        if not i: raise NotChangeable("the issue is gone")
        if (i.get("state") or {}).get("type") != "triage": raise NotChangeable("not in triage any more")
        nodes = sorted(i["team"]["states"]["nodes"], key=lambda s: s.get("position", 0))
        if action == "accept":
            pick = (i["team"].get("defaultIssueState") or {}).get("id") or next((s["id"] for s in nodes if s["type"] in ("backlog", "unstarted")), None)
        else:
            pick = next((s["id"] for s in nodes if s["type"] == "canceled" and s["name"].lower() in ("canceled", "cancelled")), None) or next((s["id"] for s in nodes if s["type"] == "canceled"), None)
        if not pick: raise NotChangeable("the team has no state for this")
        inp = {"stateId": pick}
        if action == "accept" and priority: inp["priority"] = self.PRIO[priority]
        m = "mutation($id: String!, $in: IssueUpdateInput!) { issueUpdate(id: $id, input: $in) { success } }"
        if not self.gql(m, {"id": i["id"], "in": inp})["issueUpdate"]["success"]: raise Fail("Linear refused the change")


class Skip(Exception):
    pass


ADAPTERS = {"linear": Linear}


def cmd_list(src):
    names = list(ADAPTERS) if src in ("", "all") else [src]
    res = {"tasks": [], "ok": [], "errors": {}, "skipped": {}, "filtered": []}
    for n in names:
        if n not in ADAPTERS: raise Fail("no source " + n)
        try:
            res["tasks"] += ADAPTERS[n]().list(); res["ok"].append(n)
        except Skip as e:
            res["skipped"][n] = str(e)
        except Fail as e:
            res["errors"][n] = str(e)
            FILTERED[:] = [f for f in FILTERED if f["source"] != n]    # (a failed source counts nothing)
        except (KeyError, TypeError, ValueError) as e:
            res["errors"][n] = "unexpected answer (" + type(e).__name__ + ")"
            FILTERED[:] = [f for f in FILTERED if f["source"] != n]
    res["filtered"] = FILTERED
    return res


def main(argv):
    if not argv: print(__doc__); return 2
    cmd, rest = argv[0], argv[1:]
    global DRY
    opts = {"--source": "", "--kind": "", "--id": "", "--priority": ""}; pos = []   # (--kind is read and ignored: older callers pass it)
    i = 0
    while i < len(rest):
        if rest[i] == "--dry-run": DRY = True; i += 1
        elif rest[i] in opts: opts[rest[i]] = rest[i + 1] if i + 1 < len(rest) else ""; i += 2
        else: pos.append(rest[i]); i += 1
    try:
        if cmd == "list":
            print(json.dumps(cmd_list(opts["--source"]))); return 0
        if cmd == "fetch":
            print(json.dumps(ADAPTERS[opts["--source"]]().fetch(pos[0]))); return 0
        if cmd == "set-status":
            ADAPTERS[opts["--source"]]().set_status(pos[0], pos[1], opts["--id"]); print(json.dumps({"ok": True})); return 0
        if cmd == "triage":      # triage <ref> accept|decline [--priority p]
            ADAPTERS[opts["--source"] or "linear"]().triage(pos[0], pos[1], opts["--priority"]); print(json.dumps({"ok": True})); return 0
        if cmd == "open":
            if pos and re.match(r"^https?://", pos[0]): print(pos[0]); return 0
            return 1
    except NotChangeable as e:
        print(json.dumps({"ok": False, "not_changeable": str(e)})); return 3
    except Skip as e:
        print(f"bhote: {e}", file=sys.stderr); return 4
    except (Fail, KeyError, IndexError) as e:
        print(f"bhote: {type(e).__name__ if not isinstance(e, Fail) else e}", file=sys.stderr); return 1
    print(__doc__, file=sys.stderr); return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
