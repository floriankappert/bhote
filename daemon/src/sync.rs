//! The merge of the local tasks with kharka (step 4b), `store_sync` and `store_sync_run` of the script for STORE=kharka: the newer
//! `updated` wins per task, in both directions, writes to kharka only while the entry is what the merge read (--if-hlc),
//! tombstones go after 30 days, what is not valid is not taken.
use crate::store::{self, num_or, topic_get};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};


pub struct Cfg { pub topics: PathBuf, pub shared: PathBuf, pub data: PathBuf, pub kharka: String }

/// POSIX cksum of the bytes: (crc, length)
pub fn cksum(data: &[u8]) -> (u32, usize) {
    let mut crc: u32 = 0;
    let mut feed = |b: u8| { crc ^= (b as u32) << 24; for _ in 0..8 { crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04C1_1DB7 } else { crc << 1 }; } };
    for &b in data { feed(b); }
    let mut n = data.len(); while n > 0 { feed((n & 0xff) as u8); n >>= 8; }
    (!crc, data.len())
}

fn key_of(file: &str) -> String {
    let id = file.trim_end_matches(".topic").trim_end_matches(".task");
    if id.starts_with("proj-") { format!("bhote.projects/{}", id) } else if id.starts_with("mach-") { format!("bhote.machines/{}", id) } else if id.starts_with("an-") { format!("bhote.agentnums/{}", id) } else { format!("bhote.topics/{}", id) }
}
fn name_ok(n: &str) -> bool { let b = n.as_bytes(); !b.is_empty() && b[0].is_ascii_alphanumeric() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') }
fn nocntrl01(s: &str) -> String { s.chars().filter(|c| (*c as u32) > 0x1f).collect() }

/// topic_valid: KEY=value lines only, no key twice, numbers are digits, not from the future, small
pub fn topic_valid(text: &str, now: u64) -> bool {
    let mut seen: Vec<&str> = Vec::new(); let mut nr = 0;
    for l in text.lines() {
        nr += 1;
        let f: Vec<&str> = l.split('=').collect();
        if f.len() < 2 || f[0].is_empty() || !f[0].starts_with(|c: char| c.is_ascii_lowercase() || c == '_') || !f[0].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') { return false; }
        if seen.contains(&f[0]) { return false; } seen.push(f[0]);
        if matches!(f[0], "updated" | "created" | "waiting_since") {
            if !f[1].bytes().all(|b| b.is_ascii_digit()) { return false; }
            if !f[1].is_empty() && f[1].parse::<f64>().unwrap_or(0.0) > (now + 300) as f64 { return false; }
        }
        if l.len() > 4000 || nr > 64 { return false; }
    }
    true
}

fn kharka(bin: &str, args: &[&str], stdin: Option<&[u8]>) -> (i32, Vec<u8>, String) {
    match crate::proc::run(bin, args, 30, stdin) {   // never longer than 30 s: a hung daemon of kharka must not hold the merge (and its lock) for ever
        Some((c, o, e)) => (c, o, String::from_utf8_lossy(&e).to_string()),
        None => (127, Vec::new(), "no kharka binary".into()),
    }
}
fn err_line(e: &str) -> String { let l = e.lines().next().unwrap_or(""); let l: String = l.chars().filter(|c| !c.is_control()).collect(); let l = l.strip_prefix("kharka: ").unwrap_or(&l).to_string(); l.chars().take(200).collect() }

/// store_note ok|offline|again [extra lines]
fn note(cfg: &Cfg, state: &str, extra: &str) {
    let f = cfg.data.join("store.status");
    let mut ok_at = crate::proc::read_cap(&f).unwrap_or_default().lines().find_map(|l| l.strip_prefix("ok_at=").map(|s| s.to_string())).unwrap_or_default();
    if state == "ok" { ok_at = store::now().to_string(); }
    let mut body = format!("state={}\nok_at={}\n", state, if ok_at.is_empty() { "0".into() } else { ok_at });
    if !extra.is_empty() { body.push_str(extra); body.push('\n'); }
    let tmp = PathBuf::from(format!("{}.tmp.{}", f.display(), std::process::id()));
    if fs::write(&tmp, body).is_ok() { let _ = fs::rename(&tmp, &f); }
}

struct Remote { files: HashMap<String, String>, hlc: HashMap<String, String>, keys: HashMap<String, String> }

/// kharka_dump: every bhote entry as the lines of a task file
fn dump(cfg: &Cfg) -> Result<Remote, String> {
    let (rc, out, err) = kharka(&cfg.kharka, &["ls", "bhote.", "--entries"], None);
    if rc != 0 { return Err(err_line(&err)); }
    if out.len() > 8_000_000 { return Err(String::new()); }
    let (mut files, mut hlc, mut keys): (HashMap<String, String>, HashMap<String, String>, HashMap<String, String>) = (HashMap::new(), HashMap::new(), HashMap::new());
    for l in String::from_utf8_lossy(&out).lines() {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        let key = e["key"].as_str().unwrap_or(""); let Some(v) = e["value"].as_object() else { continue };
        let Some((coll, id)) = key.split_once('/') else { continue };
        if !["bhote.topics", "bhote.tasks", "bhote.projects", "bhote.machines", "bhote.agentnums"].contains(&coll) || !name_ok(id) { continue; }
        let want = if id.starts_with("proj-") { "bhote.projects" } else if id.starts_with("mach-") { "bhote.machines" } else if id.starts_with("an-") { "bhote.agentnums" } else { "bhote.topics" };
        if coll != want && !(want == "bhote.topics" && coll == "bhote.tasks") { continue; }   // (dual-read: bhote.tasks, wave 2, counts like bhote.topics)
        let file = format!("{}.topic", id);
        let mut text = String::new();
        for (k, x) in v { if let Some(s) = x.as_str() { let line = format!("{}={}", k, nocntrl01(s)); if !line.is_empty() { text.push_str(&line); text.push('\n'); } } }
        // both collections for one id: the newer `updated` wins, a tie goes to bhote.tasks
        if let Some(old) = files.get(&file) { let (o, n) = (updated_of(old), updated_of(&text)); if o > n || (o == n && coll != "bhote.tasks") { continue; } }
        files.insert(file.clone(), text); hlc.insert(file.clone(), e["hlc"].as_str().unwrap_or("").to_string()); keys.insert(file, key.to_string());
    }
    Ok(Remote { files, hlc, keys })
}

/// The value of a task file as kharka gets it (the first value of each lowercase key; the line form it comes back in)
fn value_of(text: &str) -> (serde_json::Map<String, Value>, String) {
    let mut m = serde_json::Map::new();
    for l in text.lines() {
        let Some(i) = l.find('=') else { continue };
        let k = &l[..i]; if k.is_empty() || !k.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') || !k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') { continue; }
        if !m.contains_key(k) { m.insert(k.to_string(), Value::String(l[i + 1..].to_string())); }
    }
    let lines: String = m.iter().filter_map(|(k, v)| v.as_str().map(|s| format!("{}={}\n", k, nocntrl01(s)))).collect();
    (m, lines)
}

/// kharka_push: 0 ok, 4 the entry changed since the merge read it, else an error (and its text)
fn push(cfg: &Cfg, file: &Path, stamp: &str, remote_lines: Option<&str>, key: Option<&str>) -> (i32, String) {
    let Ok(was) = crate::proc::read_cap(file) else { return (1, String::new()) };
    let (m, lines) = value_of(&was);
    let mut rc = 0; let mut err = String::new();
    if remote_lines.map_or(false, |r| r == lines) { /* kharka has it already: only the local file was not in its form */ }
    else {
        let key = key.map(|k| k.to_string()).unwrap_or_else(|| key_of(&file.file_name().unwrap_or_default().to_string_lossy()));
        let body = serde_json::to_vec(&Value::Object(m)).unwrap_or_default();
        let mut args = vec!["put", key.as_str(), "-"]; if !stamp.is_empty() { args.push("--if-hlc"); args.push(stamp); }
        let (r, _, e) = kharka(&cfg.kharka, &args, Some(&body)); rc = r; err = err_line(&e);
    }
    if rc == 0 && was != lines && crate::proc::read_cap(file).map_or(false, |now| now == was) {
        let tmp = PathBuf::from(format!("{}.{}x", file.display(), std::process::id()));
        if fs::write(&tmp, &lines).is_ok() { let _ = fs::rename(&tmp, file); }
    }
    (rc, err)
}

fn lock_take(path: &Path, me: u32) -> bool {
    let o = fs::read_link(path).ok().map(|p| p.to_string_lossy().to_string());
    if o.as_deref() == Some(&me.to_string()) { return true; }
    extern "C" { fn kill(pid: i32, sig: i32) -> i32; }
    if let Some(pid) = o.as_ref().and_then(|x| x.parse::<i32>().ok()) { if unsafe { kill(pid, 0) } != 0 { let _ = fs::remove_file(path); } }
    else if o.is_some() { let _ = fs::remove_file(path); }
    if path.is_dir() && fs::symlink_metadata(path).map_or(false, |m| !m.file_type().is_symlink()) { let _ = fs::remove_dir_all(path); }
    std::os::unix::fs::symlink(me.to_string(), path).is_ok()
}

fn updated_of(text: &str) -> u64 { text.lines().find_map(|l| l.strip_prefix("updated=")).map(|v| v.split('=').next().unwrap_or("").parse::<f64>().unwrap_or(0.0).max(0.0) as u64).unwrap_or(0) }

/// store_sync_run: 0 ok, 1 unreachable or refused, 3 kharka changed during the merge
fn run(cfg: &Cfg) -> i32 {
    let now = store::now(); let old = now.saturating_sub(30 * 86400);
    let remote = match dump(cfg) { Ok(r) => r, Err(e) => { note(cfg, "offline", &format!("error={}", if e.is_empty() { "the kharka daemon did not answer".to_string() } else { e })); return 1; } };
    let _ = fs::create_dir_all(&cfg.topics);
    // what is taken from the other side: valid, its own id, not too many
    let mut names: Vec<&String> = remote.files.keys().collect(); names.sort_by(|a, b| crate::ui::coll(a, b));
    let mut rfiles: HashMap<String, String> = HashMap::new(); let mut n = 0;
    for f in names {
        n += 1; let t = &remote.files[f];
        let id_ok = t.lines().find_map(|l| l.strip_prefix("id=")).map_or(false, |v| v == f.trim_end_matches(".topic"));
        if n > 2000 || !topic_valid(t, now) || !id_ok { continue; }
        rfiles.insert(f.clone(), t.clone());
    }
    let rejected: Vec<&String> = remote.files.keys().filter(|f| !rfiles.contains_key(*f)).collect();
    let mut local: HashMap<String, String> = HashMap::new();
    for p in store::task_files(&cfg.topics) { let n = p.file_name().unwrap_or_default().to_string_lossy().to_string(); let id = n.trim_end_matches(".topic").trim_end_matches(".task").to_string(); if name_ok(&id) { local.insert(format!("{}.topic", id), crate::proc::read_cap(&p).unwrap_or_default()); } }
    let mut ids: Vec<String> = rfiles.keys().chain(local.keys()).cloned().collect(); ids.sort(); ids.dedup();
    let (mut ok, mut again, mut err) = (true, false, String::new());
    for f in ids {
        if rejected.iter().any(|r| **r == f) && !local.contains_key(&f) { continue; }
        if remote.files.contains_key(&f) && !rfiles.contains_key(&f) { continue; }   // a name refused from the other side is neither pulled nor pushed
        let (hl, hr) = (local.contains_key(&f), rfiles.contains_key(&f));
        let (l0, r0) = (local.get(&f).map(|t| updated_of(t)).unwrap_or(0), rfiles.get(&f).map(|t| updated_of(t)).unwrap_or(0));
        let differ = if !hl || !hr || l0 != r0 { true } else if l0 != 0 { cksum(local[&f].as_bytes()) != cksum(rfiles[&f].as_bytes()) } else { false };
        if !differ { continue; }
        let path = store::task_path(&cfg.topics, f.trim_end_matches(".topic"));
        let l = if hl { num_or(&topic_get(&path, "updated"), 0) } else { 0 }; let r = rfiles.get(&f).map(|t| num_or(&store::first_value(t, "updated").map(|v| store::nocntrl(&v)).unwrap_or_default(), 0)).unwrap_or(0);
        if r > l {
            let pt = cfg.topics.join(format!(".pull.{}{}", std::process::id(), n)); n += 1;
            if fs::write(&pt, &rfiles[&f]).is_err() { continue; }
            if path.exists() && num_or(&topic_get(&path, "updated"), 0) != l { let _ = fs::remove_file(&pt); continue; }
            let _ = fs::rename(&pt, &path);
        } else if l > r {
            if l < old && topic_get(&path, "deleted") == "1" { continue; }
            let (rc, e) = push(cfg, &path, remote.hlc.get(&f).map(|s| s.as_str()).unwrap_or(""), None, remote.keys.get(&f).map(|s| s.as_str()));
            match rc { 0 => {} 4 => again = true, _ => { ok = false; err = e; } }
        } else if l == r && l != 0 && hr && fs::read(&path).ok().as_deref() != Some(rfiles[&f].as_bytes()) {
            let (cl, cr) = (cksum(&fs::read(&path).unwrap_or_default()).0, cksum(rfiles[&f].as_bytes()).0);
            if cr > cl { let tf = PathBuf::from(format!("{}.{}y", path.display(), std::process::id())); if fs::write(&tf, &rfiles[&f]).is_ok() { let _ = fs::rename(&tf, &path); } }
            else { let (rc, e) = push(cfg, &path, remote.hlc.get(&f).map(|s| s.as_str()).unwrap_or(""), Some(&rfiles[&f]), remote.keys.get(&f).map(|s| s.as_str())); match rc { 0 => {} 4 => again = true, _ => { ok = false; err = e; } } }
        }
    }
    // the tombstones that are old enough go, from kharka too (only while the entry is what the merge read)
    if let Ok(rd) = fs::read_dir(&cfg.topics) {
        let _ = rd; let mut tombs: Vec<PathBuf> = store::task_files(&cfg.topics); tombs.sort();
        for t in tombs {
            let Ok(text) = crate::proc::read_cap(&t) else { continue };
            if !text.lines().any(|l| l.starts_with("deleted=1")) { continue; }
            if num_or(&topic_get(&t, "updated"), 0) >= old { continue; }
            let name = format!("{}.topic", t.file_name().unwrap_or_default().to_string_lossy().trim_end_matches(".topic").trim_end_matches(".task"));
            if let Some(st) = remote.hlc.get(&name).filter(|s| !s.is_empty()) {
                let dk = remote.keys.get(&name).cloned().unwrap_or_else(|| key_of(&name));
                let (rc, _, e) = kharka(&cfg.kharka, &["del", &dk, "--if-hlc", st], None);
                match rc { 0 => {} 4 => { again = true; continue; } _ => { ok = false; err = err_line(&e); continue; } }
            }
            let _ = fs::remove_file(&t);
        }
    }
    if !ok { note(cfg, "offline", &format!("error={}", if err.is_empty() { "kharka refused a write".to_string() } else { err })); return 1; }
    let (_, so, _) = kharka(&cfg.kharka, &["status"], None);
    let v: Value = serde_json::from_slice(&so).unwrap_or(Value::Null);
    let hub = nocntrl01(v["hub"].as_str().unwrap_or(""));
    let pending: String = match &v["pending"] { Value::Null => "0".into(), x => x.to_string() }.chars().filter(|c| c.is_ascii_digit()).collect();
    let status = format!("hub={}\nhub_online={}\npending={}", hub, v["online"] == Value::Bool(true), pending);
    if again { note(cfg, "again", &status); return 3; }
    note(cfg, "ok", &status); 0
}

/// store_sync: under the lock; 0 ok, 1 offline, 2 another merge runs
pub fn store_sync(cfg: &Cfg) -> i32 {
    let _ = fs::create_dir_all(&cfg.data);
    let lock = cfg.shared.join("sync.lock"); let me = std::process::id();
    if !lock_take(&lock, me) { return 2; }
    let mut rc = run(cfg);
    if rc == 3 { rc = run(cfg); }
    if fs::read_link(&lock).ok().map(|p| p.to_string_lossy().to_string()).as_deref() == Some(&me.to_string()) { let _ = fs::remove_file(&lock); }
    if rc == 3 { let _ = fs::write(cfg.shared.join("dirty"), ""); let _ = fs::write(cfg.shared.join("poke"), ""); rc = 2; }
    rc
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_fields_survive_the_value_and_validity() {
        let t = "id=a\ntitle=T\nupdated=5\nv2_flag=1\nfuture_field=x\nsource_ref=IMS-9\n";
        assert!(topic_valid(t, 100));
        let (m, lines) = value_of(t);
        assert_eq!(m.get("v2_flag").and_then(|v| v.as_str()), Some("1"));
        assert!(lines.contains("future_field=x\n") && lines.contains("source_ref=IMS-9\n"));
        assert_eq!(key_of("a.task"), "bhote.topics/a");
        assert_eq!(key_of("proj-x.task"), "bhote.projects/proj-x");
    }
}
