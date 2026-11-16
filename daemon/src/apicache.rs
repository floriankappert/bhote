//! The shared API cache: every request to GitHub (gh) and CircleCI goes through `cached`, so that the daemon, the bash fallback
//! and the herdr-app mod do not ask the same thing again within a few seconds. One file per request in `api_dir()`
//! (BHOTE_API_CACHE, "off" = no cache, else ~/.cache/bhote/api), `<readable prefix>-<cksum crc>-<cksum length>.json`, written atomically (temp + rename):
//!   {key, fetched_at, tried_at, ttl, status (ok|stale|error), etag, body, error, fails, retry_at}
//! The same format and file names are written by the bash helper `api_cached` (bhote) and by `cachedGh` of the mod.
//! A request header (the CircleCI token) is never part of a key or an entry.
//!  - fresh (now < fetched_at + ttl): the body, no network.
//!  - the ttl is API_CACHE_EVERY (15 s) while something runs, and `QUIET` times that (at most 120 s) when nothing does.
//!  - an error keeps the last good body (status stale) and backs off: to the reset time of a rate limit when it is known, else
//!    15 s, 30 s, 60 s ... up to 15 minutes. The body of a stale entry is still what the callers get.
//!  - requests of one key in one process are one request (a lock per key, the second caller reads what the first fetched).
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const QUIET: u64 = 6;          // a quiet answer (nothing runs) is kept QUIET x the base interval ...
const QUIET_MAX: u64 = 120;    // ... but at most this many seconds
const BACKOFF_MAX: u64 = 900;  // the longest wait after errors without a known reset time
const RESET_MAX: u64 = 3600;   // a reset time further away than this is not believed

static BASE: AtomicU64 = AtomicU64::new(15);
/// API_CACHE_EVERY from the settings (seconds, at least 1).
pub fn set_base(v: &str) { if let Ok(n) = v.trim().parse::<u64>() { if n >= 1 { BASE.store(n, Ordering::Relaxed); } } }
pub fn base() -> u64 { BASE.load(Ordering::Relaxed) }

pub fn api_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("BHOTE_API_CACHE").filter(|v| !v.is_empty()) { return PathBuf::from(d); }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".cache")).join("bhote").join("api")
}

pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

/// What a fetch gives: a body (with its ETag, if there is one), or "not modified" (an If-None-Match was answered with 304).
pub enum Got { Body(String, String), NotModified }
/// Why a fetch failed: a rate limit (with the epoch second it ends, when known) or anything else.
pub enum Fail { Rate(Option<u64>), Other(String) }

/// POSIX `cksum` (CRC-32, then the length), so that the shell can name the same files: `printf %s "$key" | cksum`.
fn cksum(b: &[u8]) -> (u32, usize) {
    let mut crc: u32 = 0;
    let mut feed = |byte: u8| { crc ^= (byte as u32) << 24; for _ in 0..8 { crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04C1_1DB7 } else { crc << 1 }; } };
    for &x in b { feed(x); }
    let mut n = b.len(); while n > 0 { feed((n & 0xff) as u8); n >>= 8; }
    (!crc, b.len())
}
pub fn file_name(key: &str) -> String {
    let pre: String = key.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).take(32).collect();
    let (crc, len) = cksum(key.as_bytes());
    format!("{}-{}-{}.json", pre, crc, len)
}

/// True when the answer shows work in progress (the next look should be soon): a running or waiting run, check or pipeline.
/// A failure alone is not "active": it stays red until somebody acts, and polling it every 15 s would change nothing.
pub fn active(body: &str) -> bool {
    ["in_progress", "IN_PROGRESS", "queued", "QUEUED", "PENDING", "pending", "WAITING", "waiting", "\"running\"", "\"failing\"", "on_hold"].iter().any(|w| body.contains(w))
}
fn ttl_for(body: &str, base: u64) -> u64 { if active(body) { base } else { (base * QUIET).min(QUIET_MAX).max(base) } }

fn locks() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> { static L: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new(); L.get_or_init(Default::default) }

fn read_entry(path: &Path) -> Option<Value> { serde_json::from_str(&fs::read_to_string(path).ok()?).ok() }
fn write_entry(dir: &Path, path: &Path, e: &Value) {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let _ = fs::DirBuilder::new().recursive(true).mode(0o700).create(dir);
    let tmp = dir.join(format!(".tmp.{}.{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
    let ok = { use std::io::Write; fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).and_then(|mut f| f.write_all(e.to_string().as_bytes())).is_ok() };
    if ok { let _ = fs::rename(&tmp, path); } else { let _ = fs::remove_file(&tmp); }
}
fn u(v: &Value, k: &str) -> u64 { v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) }
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(|x| x.as_str()).unwrap_or("") }

/// The request `key`: the cached body while it is fresh, else the result of `fetch(etag)`. None when there is no body at all.
pub fn cached(key: &str, fetch: impl FnOnce(Option<&str>) -> Result<Got, Fail>) -> Option<String> {
    if std::env::var("BHOTE_API_CACHE").map_or(false, |v| v == "off") {   // no cache (the tests, debugging)
        return match fetch(None) { Ok(Got::Body(b, _)) => Some(b), _ => None };
    }
    cached_at(&api_dir(), now(), base(), key, fetch)
}

pub fn cached_at(dir: &Path, now: u64, base: u64, key: &str, fetch: impl FnOnce(Option<&str>) -> Result<Got, Fail>) -> Option<String> {
    let lock = locks().lock().ok()?.entry(key.to_string()).or_default().clone();
    let _g = lock.lock().ok()?;                       // (a second caller waits here, then finds the entry the first one wrote)
    let path = dir.join(file_name(key));
    let old = read_entry(&path);
    let body_of = |e: &Option<Value>| e.as_ref().map(|e| st(e, "body").to_string()).filter(|b| !b.is_empty());
    if let Some(e) = &old {
        let fresh = u(e, "fetched_at") > 0 && now < u(e, "fetched_at") + u(e, "ttl");
        let waiting = now < u(e, "retry_at");        // after an error: no new try before this
        if (fresh && st(e, "status") != "error") || waiting { return body_of(&old); }
    }
    let etag = old.as_ref().map(|e| st(e, "etag").to_string()).filter(|t| !t.is_empty() && body_of(&old).is_some());
    let fails = old.as_ref().map_or(0, |e| u(e, "fails"));
    let e = match fetch(etag.as_deref()) {
        Ok(got) => {
            let (body, tag) = match got { Got::Body(b, t) => (b, t), Got::NotModified => (body_of(&old).unwrap_or_default(), etag.clone().unwrap_or_default()) };
            json!({"key": key, "fetched_at": now, "tried_at": now, "ttl": ttl_for(&body, base), "status": "ok", "etag": tag, "body": body, "error": "", "fails": 0, "retry_at": 0})
        }
        Err(f) => {
            let (msg, wait) = match f {
                Fail::Rate(Some(reset)) if reset > now && reset <= now + RESET_MAX => ("rate limit".to_string(), reset - now),
                Fail::Rate(_) => ("rate limit".to_string(), (base << fails.min(10)).min(BACKOFF_MAX)),
                Fail::Other(m) => (m, (base << fails.min(10)).min(BACKOFF_MAX)),
            };
            let (b, at, tag) = old.as_ref().map(|e| (st(e, "body").to_string(), u(e, "fetched_at"), st(e, "etag").to_string())).unwrap_or_default();
            json!({"key": key, "fetched_at": at, "tried_at": now, "ttl": old.as_ref().map_or(base, |e| u(e, "ttl")), "status": if b.is_empty() { "error" } else { "stale" },
                   "etag": tag, "body": b, "error": msg, "fails": fails + 1, "retry_at": now + wait})
        }
    };
    write_entry(dir, &path, &e);
    Some(st(&e, "body").to_string()).filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bhote-apicache-{}-{}", std::process::id(), n)); let _ = fs::remove_dir_all(&d); d }
    fn ok(b: &str) -> Result<Got, Fail> { Ok(Got::Body(b.to_string(), String::new())) }

    #[test] fn fresh_hit_makes_no_call() {
        let d = tmp("hit"); let calls = Cell::new(0);
        let f = |b: &'static str| { let calls = &calls; move |_: Option<&str>| { calls.set(calls.get() + 1); ok(b) } };
        assert_eq!(cached_at(&d, 1000, 15, "k", f("[]")).as_deref(), Some("[]"));
        assert_eq!(cached_at(&d, 1005, 15, "k", f("other")).as_deref(), Some("[]"));
        assert_eq!(calls.get(), 1);
    }
    #[test] fn ttl_expires_and_quiet_lasts_longer() {
        let d = tmp("ttl");
        cached_at(&d, 1000, 15, "busy", |_| ok("[{\"status\":\"in_progress\"}]"));
        cached_at(&d, 1000, 15, "calm", |_| ok("[{\"status\":\"completed\"}]"));
        assert_eq!(cached_at(&d, 1016, 15, "busy", |_| ok("new")).as_deref(), Some("new"));   // 15 s: expired
        assert_eq!(cached_at(&d, 1016, 15, "calm", |_| ok("new")).as_deref(), Some("[{\"status\":\"completed\"}]"));   // quiet: 90 s
        assert_eq!(cached_at(&d, 1091, 15, "calm", |_| ok("new")).as_deref(), Some("new"));
    }
    #[test] fn error_keeps_the_last_body_and_backs_off() {
        let d = tmp("err"); let calls = Cell::new(0);
        cached_at(&d, 1000, 15, "k", |_| ok("good"));
        let bad = |_: Option<&str>| { calls.set(calls.get() + 1); Err(Fail::Other("offline".into())) };
        assert_eq!(cached_at(&d, 1200, 15, "k", bad).as_deref(), Some("good"));   // stale, and still served
        assert_eq!(cached_at(&d, 1205, 15, "k", |_| { calls.set(calls.get() + 1); ok("never") }).as_deref(), Some("good"));   // inside the back-off: no call
        assert_eq!(calls.get(), 1);
        let e = read_entry(&d.join(file_name("k"))).unwrap();
        assert_eq!(st(&e, "status"), "stale"); assert_eq!(st(&e, "error"), "offline");
        assert_eq!(cached_at(&d, 1216, 15, "k", |_| ok("fresh")).as_deref(), Some("fresh"));   // after the back-off it recovers
    }
    #[test] fn rate_limit_waits_until_the_reset() {
        let d = tmp("rate");
        assert_eq!(cached_at(&d, 1000, 15, "k", |_| Err(Fail::Rate(Some(1300)))), None);
        assert_eq!(cached_at(&d, 1299, 15, "k", |_| ok("x")), None);                       // not asked before the reset
        assert_eq!(cached_at(&d, 1300, 15, "k", |_| ok("x")).as_deref(), Some("x"));
    }
    #[test] fn not_modified_keeps_the_body_and_renews_it() {
        let d = tmp("etag");
        cached_at(&d, 1000, 15, "k", |_| Ok(Got::Body("[1]".into(), "\"abc\"".into())));
        let seen = Cell::new(false);
        let r = cached_at(&d, 1200, 15, "k", |t| { seen.set(t == Some("\"abc\"")); Ok(Got::NotModified) });
        assert!(seen.get()); assert_eq!(r.as_deref(), Some("[1]"));
        assert_eq!(read_entry(&d.join(file_name("k"))).map(|e| u(&e, "fetched_at")), Some(1200));
    }
    #[test] fn cksum_matches_the_posix_tool() { assert_eq!(cksum(b"hello"), (3287646509, 5)); }
}
