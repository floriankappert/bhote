#!/usr/bin/env python3
"""The usage of the Claude accounts of this machine (one per Claude Code profile: ~/.claude, ~/.claude2 …), as JSON, for the usage panel of bhote in herdr-app.

    python3 usage.py

Per profile with a ~/.claude*/.credentials.json: the 5-hour window and the weekly limit as the share used (0-100) and when it resets. An expired access
token is renewed with the profile's refresh token and the credentials file is written back at once (the refresh token rotates: a renewed token that is
not kept would log the profile out). Nothing is printed of the tokens.

Output: [{"profile", "email", "plan", "five_hour": {"used", "resets"}, "seven_day": {...}, "seven_day_opus": {...}, "error"}]
"""
import glob, json, os, sys, tempfile, time, urllib.request

HOME = os.path.expanduser("~")
CLIENT_ID = "9d1c250a-e61b-44d9-88ed-5944d1962f5e"
TOKEN_URL = "https://platform.claude.com/v1/oauth/token"
USAGE_URL = "https://api.anthropic.com/api/oauth/usage"


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


def one(d):
    path = os.path.join(d, ".credentials.json")
    out = {"profile": os.path.basename(d).lstrip("."), "email": email_of(d)}
    try:
        creds = json.load(open(path))
        o = creds["claudeAiOauth"]
        out["plan"] = o.get("subscriptionType")
        if o.get("expiresAt", 0) < (time.time() + 60) * 1000:
            refresh(path, creds)
        u = call(urllib.request.Request(USAGE_URL, headers={"Authorization": "Bearer " + o["accessToken"], "anthropic-beta": "oauth-2025-04-20", "User-Agent": "bhote"}))
        for k in ("five_hour", "seven_day", "seven_day_opus"):
            out[k] = window(u.get(k))
    except Exception as e:  # shown on the panel, never a token
        out["error"] = type(e).__name__ + (": " + str(e)[:80] if not isinstance(e, KeyError) else "")
    return out


dirs = sorted(set(glob.glob(os.path.join(HOME, ".claude")) + glob.glob(os.path.join(HOME, ".claude[0-9a-zA-Z_-]*"))))
dirs = [d for d in dirs if os.path.isfile(os.path.join(d, ".credentials.json"))]
json.dump([one(d) for d in dirs], sys.stdout)
