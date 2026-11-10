#!/usr/bin/env python3
"""The monitors that run inside Claude Code sessions of this machine, as JSON, for the agent cards of the bhote panel in herdr-app.

    python3 monitors.py

A monitor is a recurring job of a session (`/loop 5m ...`, CronCreate). It lives only in the memory of that session and is not written to disk,
so it is read from the session's transcript: a job counts from its CronCreate until a CronDelete (or 7 days, when Claude drops it) while the
session's process is alive. Read only; the transcripts are read incrementally (a cache in ~/.cache/bhote/monitors.json keeps where it stopped).

Output: [{"cwd", "session", "pid", "jobs": [{"id", "every", "prompt", "created"}]}], only for sessions that have a job.
"""
import glob, json, os, re, sys, time

HOME = os.path.expanduser("~")
CACHE = os.path.join(HOME, ".cache", "bhote", "monitors.json")
WEEK = 7 * 86400
WATCH = re.compile(r"\b(until|while|sleep|watch)\b")   # a background shell with a loop waits for something: it counts as a watcher


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def project_dir(cfg, cwd):
    return os.path.join(cfg, "projects", re.sub(r"[^A-Za-z0-9]", "-", cwd))


def scan(path, st):
    """Reads what was appended to the transcript since the last look; st = {offset, calls, jobs}."""
    try:
        size = os.path.getsize(path)
    except OSError:
        return st
    if size < st["offset"]:
        st.update(offset=0, calls={}, jobs={})
    with open(path, "rb") as f:
        f.seek(st["offset"])
        data = f.read()
    cut = data.rfind(b"\n")
    if cut < 0:
        return st
    st["offset"] += cut + 1
    for raw in data[: cut + 1].split(b"\n"):
        if not any(k in raw for k in (b"Cron", b"job ", b"Monitor", b"task-id", b"TaskStop", b"run_in_background", b"in background")):
            continue
        # a watcher of the session ends with a notification of its task that has a status (completed, failed, killed ...)
        if b"<task-id>" in raw:
            for tid, status in re.findall(r"<task-id>(\w+)</task-id>.*?<status>(\w+)</status>", raw.decode("utf8", "replace")):
                if status != "running":
                    st["jobs"].pop(tid, None)
        try:
            o = json.loads(raw)
        except ValueError:
            continue
        content = (o.get("message") or {}).get("content")
        if not isinstance(content, list):
            continue
        for b in content:
            if b.get("type") == "tool_use" and b.get("name") in ("CronCreate", "CronDelete"):
                st["calls"][b["id"]] = {"name": b["name"], "input": b.get("input") or {}, "t": o.get("timestamp", "")}
            elif b.get("type") == "tool_use" and b.get("name") == "TaskStop":
                i = b.get("input") or {}
                st["jobs"].pop(str(i.get("task_id") or i.get("shell_id") or ""), None)
            elif b.get("type") == "tool_use" and (b.get("name") == "Monitor" or (b.get("name") == "Bash" and (b.get("input") or {}).get("run_in_background") and WATCH.search((b.get("input") or {}).get("command") or ""))):
                st["calls"][b["id"]] = {"name": "Monitor", "input": b.get("input") or {}, "t": o.get("timestamp", "")}
            elif b.get("type") == "tool_result" and b.get("tool_use_id") in st["calls"]:
                call = st["calls"].pop(b["tool_use_id"])
                text = json.dumps(b.get("content"))
                if call["name"] == "Monitor":   # a watcher of the session (the Monitor tool, or a background shell that waits in a loop): a monitor as long as it runs
                    m = re.search(r"(?:task|ID:?) ([A-Za-z0-9]+)", text)
                    if m:
                        inp = call["input"]
                        n = re.search(r"sleep (\d+)", inp.get("command") or "")
                        st["jobs"][m.group(1)] = {"id": m.group(1), "every": "Every %s seconds" % n.group(1) if n else "watch", "prompt": (inp.get("description") or inp.get("command") or "")[:160], "created": call["t"]}
                elif call["name"] == "CronCreate":
                    m = re.search(r"job ([0-9a-f]+) \((.*?)\)", text)
                    if m and "recurring" in text:
                        st["jobs"][m.group(1)] = {"id": m.group(1), "every": m.group(2), "prompt": (call["input"].get("prompt") or "")[:160], "created": call["t"]}
                elif "Cancelled" in text:
                    st["jobs"].pop(str(call["input"].get("id")), None)
    return st


def main():
    try:
        cache = json.load(open(CACHE))
    except (OSError, ValueError):
        cache = {}
    out, keep = [], {}
    cfgs = [d for d in glob.glob(os.path.join(HOME, ".claude*")) if os.path.isdir(os.path.join(d, "sessions"))]
    for cfg in cfgs:
        for sf in glob.glob(os.path.join(cfg, "sessions", "*.json")):
            try:
                s = json.load(open(sf))
            except (OSError, ValueError):
                continue
            pid, cwd, sid = s.get("pid"), s.get("cwd"), s.get("sessionId")
            if not (pid and cwd and sid and alive(pid)):
                continue
            path = os.path.join(project_dir(cfg, cwd), sid + ".jsonl")
            st = cache.get(path)
            if not st or st.get("v") != 2:   # (a cache of an older version did not know the watchers: read the transcript again)
                st = {"v": 2, "offset": 0, "calls": {}, "jobs": {}}
            st = scan(path, st)
            keep[path] = st
            jobs = [j for j in st["jobs"].values() if _fresh(j["created"])]
            if jobs:
                out.append({"cwd": cwd, "session": sid, "pid": pid, "jobs": jobs})
    try:
        os.makedirs(os.path.dirname(CACHE), exist_ok=True)
        json.dump(keep, open(CACHE, "w"))
    except OSError:
        pass
    json.dump(out, sys.stdout)
    sys.stdout.write("\n")


def _fresh(created):
    try:
        t = time.mktime(time.strptime(created[:19], "%Y-%m-%dT%H:%M:%S")) - time.timezone
        return time.time() - t < WEEK
    except ValueError:
        return True


main()
