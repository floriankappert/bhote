#!/usr/bin/env python3
"""The usage of the Claude accounts of this machine (one per Claude Code profile: ~/.claude, ~/.claude2 …), as JSON, for the usage panel of bhote in herdr-app.

    python3 usage.py

Per profile with a ~/.claude*/.credentials.json: the 5-hour window and the weekly limit as the share used (0-100) and when it resets. An expired access
token is renewed with the profile's refresh token and the credentials file is written back at once (the refresh token rotates: a renewed token that is
not kept would log the profile out). Nothing is printed of the tokens.

An answer is kept for 5 minutes (~/.cache/bhote/usage.json): the panel may ask as often as it likes, the API is asked once per 5 minutes per account. After a
failure (HTTP 429: Too Many Requests) it waits for Retry-After, else 5, 10, 20 … at most 30 minutes, and shows the last answer marked "stale".

Output: [{"profile", "email", "plan", "five_hour": {"used", "resets"}, "seven_day": {...}, "seven_day_opus": {...}, "stale", "error"}]
"""
import glob, json, os, sys, tempfile, time, urllib.error, urllib.request

HOME = os.path.expanduser("~")
CLIENT_ID = "9d1c250a-e61b-44d9-88ed-5944d1962f5e"
TOKEN_URL = "https://platform.claude.com/v1/oauth/token"
USAGE_URL = "https://api.anthropic.com/api/oauth/usage"
CACHE = os.path.join(HOME, ".cache", "bhote", "usage.json")   # per profile: {at, data, retry_at, fails}
FRESH = 300                                                    # seconds an answer is kept: the panel asks often, the API is asked rarely


def call(req):
    with urllib.request.urlopen(req, timeout=15) as r:
        return json.load(r)


def refresh(path, creds):
    o = creds["claudeAiOauth"]
    req = urllib.request.Request(TOKEN_URL, json.dumps({"grant_type": "refresh_token", "refresh_token": o["refreshToken"], "client_id": CLIENT_ID}).encode(),
                                 {"Content-Type": "application/json", "User-Agent": "bhote"})
    r = call(req)
    o["accessToken"] = r["access_token"]
    if r.get("refresh_token"):
        o["refreshToken"] = r["refresh_token"]
    o["expiresAt"] = int((time.time() + r.get("expires_in", 3600)) * 1000)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".credentials.")
    with os.fdopen(fd, "w") as f:
        json.dump(creds, f)
    os.chmod(tmp, 0o600)
    os.replace(tmp, path)


def email_of(d):
    for p in (os.path.join(d, ".claude.json"), os.path.join(HOME, ".claude.json") if d == os.path.join(HOME, ".claude") else None):
        try:
            e = json.load(open(p)).get("oauthAccount", {}).get("emailAddress")
            if e:
                return e
        except (OSError, ValueError, TypeError):
            pass
    return None


def window(w):
    if not isinstance(w, dict) or w.get("utilization") is None:
        return None
    return {"used": w["utilization"], "resets": w.get("resets_at")}


def load_cache():
    try:
        return json.load(open(CACHE))
    except (OSError, ValueError):
        return {}


def save_cache(c):
    os.makedirs(os.path.dirname(CACHE), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(CACHE), prefix=".usage.")
    with os.fdopen(fd, "w") as f:
        json.dump(c, f)
    os.chmod(tmp, 0o600)
    os.replace(tmp, CACHE)


def backoff(fails, retry_after):
    """Seconds to wait after a failure: what the API says (Retry-After), else 5, 10, 20 … at most 30 minutes."""
    if retry_after:
        return min(max(retry_after, 60), 3600)
    return min(300 * 2 ** max(0, fails - 1), 1800)


def one(d, cache):
    path = os.path.join(d, ".credentials.json")
    key = os.path.basename(d)
    out = {"profile": os.path.basename(d).lstrip("."), "email": email_of(d)}
    ent = cache.get(key, {})
    now = time.time()
    if ent.get("data") and (now - ent.get("at", 0) < FRESH or now < ent.get("retry_at", 0)):
        out.update(ent["data"])   # (fresh enough, or the API asked us to wait: no call)
        if now - ent.get("at", 0) >= FRESH:
            out["stale"] = True
        return out
    if not ent.get("data") and now < ent.get("retry_at", 0):
        out["error"] = "Rate limited, next try in %d min" % max(1, int((ent["retry_at"] - now) / 60 + 0.5))
        return out
    try:
        creds = json.load(open(path))
        o = creds["claudeAiOauth"]
        out["plan"] = o.get("subscriptionType")
        if o.get("expiresAt", 0) < (time.time() + 60) * 1000:
            refresh(path, creds)
        u = call(urllib.request.Request(USAGE_URL, headers={"Authorization": "Bearer " + o["accessToken"], "anthropic-beta": "oauth-2025-04-20", "User-Agent": "bhote"}))
        data = {"plan": out.get("plan")}
        for k in ("five_hour", "seven_day", "seven_day_opus"):
            data[k] = window(u.get(k))
        out.update(data)
        cache[key] = {"at": time.time(), "data": data, "fails": 0, "retry_at": 0}
    except Exception as e:  # shown on the panel, never a token
        fails = ent.get("fails", 0) + 1
        ra = 0
        if isinstance(e, urllib.error.HTTPError) and e.code == 429:
            try:
                ra = int(e.headers.get("Retry-After", 0))
            except (TypeError, ValueError):
                ra = 0
        cache[key] = {**ent, "fails": fails, "retry_at": time.time() + backoff(fails, ra)}
        if ent.get("data"):
            out.update(ent["data"])   # the last good answer stays, marked stale
            out["stale"] = True
        else:
            out["error"] = type(e).__name__ + (": " + str(e)[:80] if not isinstance(e, KeyError) else "")
    return out


dirs = sorted(set(glob.glob(os.path.join(HOME, ".claude")) + glob.glob(os.path.join(HOME, ".claude[0-9a-zA-Z_-]*"))))
dirs = [d for d in dirs if os.path.isfile(os.path.join(d, ".credentials.json"))]
cache = load_cache()
result = [one(d, cache) for d in dirs]
save_cache(cache)
json.dump(result, sys.stdout)
