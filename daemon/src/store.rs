//! The topic store of the script in Rust (step 4): the arrays of `topics_load`, `topic_set` and `topic_new` with their side
//! effects, and the sink the follow-ups act through: `Live` writes the files and notifies, `Shadow` only records what it would do.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
/// [[:cntrl:]] removed (bash in a UTF-8 locale: C0, DEL and C1)
pub fn nocntrl(s: &str) -> String { s.chars().filter(|c| !c.is_control()).collect() }
/// num_or: digits only, no leading zeros ("08" would be octal), at most 18 digits
pub fn num_or(s: &str, d: u64) -> u64 { if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { return d; } let t = s.trim_start_matches('0'); if t.len() > 18 { d } else { t.parse().unwrap_or(0) } }
pub fn chars_prefix(s: &str, n: usize) -> String { s.chars().take(n).collect() }

pub const ORDER: [&str; 9] = ["review", "now", "testing", "deploying", "finished", "next", "waiting", "later", "done"];

#[derive(Clone, Default, Debug)]
pub struct Topic {
    pub file: PathBuf, pub id: String, pub title: String, pub status: String, pub wfor: String, pub wsince: String, pub upd: u64, pub desc: String,
    pub ag: String, pub am: String, pub ap: String, pub session: String, pub dl: String, pub tn: String, pub sl: String, pub sm: String, pub br: String,
    pub pj: String, pub dep: String, pub dm: String, pub an: String, pub au: String, pub rs: String,
}

/// The first value of a key counts (the index of the script does the same).
pub fn first_value(text: &str, key: &str) -> Option<String> {
    for l in text.lines() { if let Some(i) = l.find('=') { if &l[..i] == key { return Some(l[i + 1..].to_string()); } } }
    None
}
/// topic_get <file> <key> [default]: the first value, control characters removed
pub fn topic_get(file: &Path, key: &str) -> String { fs::read_to_string(file).ok().and_then(|t| first_value(&t, key)).map(|v| nocntrl(&v)).unwrap_or_default() }

/// topics_load: every topic, the order of the screen (review, now, testing, deploying, finished, next, waiting, later, done; in a group by file name).
pub fn load(topics: &Path) -> Vec<Topic> { load_files(topics) }

/// The same from the index of the daemon when it is current (shared/topics.idx), else from the files.
pub fn load_fast(topics: &Path, shared: &Path) -> Vec<Topic> {
    if let Some(v) = load_index(topics, shared) { return v; }
    load_files(topics)
}

/// The topics from the index the daemon keeps (topics.idx next to the other shared files): one file instead of hundreds. Only when it
/// is for this folder and no topic file is newer than it (a writer removes it, the daemon rebuilds it within moments).
fn load_index(topics: &Path, shared: &Path) -> Option<Vec<Topic>> {
    let idx = shared.join("topics.idx");
    let text = fs::read_to_string(&idx).ok()?;
    let mut lines = text.split('\n');
    let head = lines.next()?;
    if !head.starts_with('#') || !head.contains(&format!(" {} ", topics.display())) { return None; }
    let imt = fs::metadata(&idx).ok()?.modified().ok()?;
    if let Ok(rd) = fs::read_dir(topics) { for e in rd.flatten() { if e.path().extension().map_or(false, |x| x == "topic") && e.metadata().ok()?.modified().ok()? >= imt { return None; } } }
    const KEYS: [&str; 23] = ["title", "status", "waiting_for", "waiting_since", "updated", "deleted", "description", "agent", "agent_machine", "agent_pane", "agent_session", "delegated", "transfer_to_name", "slack_label", "slack_msg", "agent_branch", "project", "kind", "deploy_wait", "dismissed", "auto_now", "auto", "review_src"];
    let mut all = Vec::new();
    for l in lines {
        if l.is_empty() { continue; }
        let mut f = l.split('\u{1f}'); let file = PathBuf::from(f.next()?);
        let v: HashMap<&str, String> = KEYS.iter().map(|k| (*k, f.next().unwrap_or("").to_string())).collect();
        all.push((file, v));
    }
    Some(build(all.into_iter().map(|(f, v)| (f, Box::new(move |k: &str| v.get(k).cloned().unwrap_or_default()) as Box<dyn Fn(&str) -> String>)).collect()))
}

fn load_files(topics: &Path) -> Vec<Topic> {
    let mut files: Vec<PathBuf> = fs::read_dir(topics).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "topic")).collect()).unwrap_or_default();
    files.sort_by(|a, b| crate::ui::coll(&a.file_name().unwrap_or_default().to_string_lossy(), &b.file_name().unwrap_or_default().to_string_lossy()));   // (the shell's glob order: the locale's collation)
    let mut items: Vec<(PathBuf, Box<dyn Fn(&str) -> String>)> = Vec::new();
    for f in files {
        let text = fs::read_to_string(&f).unwrap_or_default();
        items.push((f, Box::new(move |k: &str| first_value(&text, k).unwrap_or_default())));
    }
    build(items)
}

fn build(items: Vec<(PathBuf, Box<dyn Fn(&str) -> String>)>) -> Vec<Topic> {
    let mut all = Vec::new();
    for (f, g) in items {
        if g("deleted") == "1" || !g("kind").is_empty() { continue; }
        let mut st = g("status"); if st.is_empty() { st = "now".into(); }
        let st = nocntrl(&st);
        let st = if ["now", "testing", "finished", "next", "waiting", "review", "later", "done"].contains(&st.as_str()) { st } else { "now".into() };
        let ws = g("waiting_since"); let ws = if ws.bytes().all(|b| b.is_ascii_digit()) { ws } else { String::new() };
        let up = g("updated"); let up = if !up.is_empty() && up.bytes().all(|b| b.is_ascii_digit()) { up.parse().unwrap_or(0) } else { 0 };
        let id = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        all.push(Topic { file: f.clone(), id, title: nocntrl(&g("title")), status: st, wfor: nocntrl(&g("waiting_for")), wsince: ws, upd: up, desc: nocntrl(&g("description")),
            ag: nocntrl(&g("agent")), am: nocntrl(&g("agent_machine")), ap: nocntrl(&g("agent_pane")), session: g("agent_session").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect(),
            dl: g("delegated"), tn: g("transfer_to_name"), sl: g("slack_label"), sm: g("slack_msg"), br: g("agent_branch"), pj: g("project"), dep: g("deploy_wait"),
            dm: g("dismissed"), an: g("auto_now"), au: g("auto"), rs: g("review_src") });
    }
    let mut out = Vec::new();
    for s in ORDER {
        for t in &all {
            let ok = match s { "deploying" => t.status == "waiting" && t.dep == "1", "waiting" => t.status == "waiting" && t.dep != "1", _ => t.status == s };
            if ok { out.push(t.clone()); }
        }
    }
    out
}

/// What the follow-ups do. Live: files and notifications. Shadow: a log line for each, nothing else.
pub trait Sink {
    fn set(&mut self, file: &Path, key: &str, value: &str);
    fn new_topic(&mut self, title: &str, wfor: &str, desc: &str) -> Option<PathBuf>;
    fn write_file(&mut self, file: &Path, content: &str);
    fn notify(&mut self, title: &str, body: &str, sound: &str);
    /// a call of herdr that changes something (report-metadata)
    fn herdr(&mut self, args: &[String]);
    fn is_shadow(&self) -> bool;
    /// how many writes were done so far (a round reloads the topics only after a write)
    fn writes(&self) -> usize;
}

pub struct Env { pub topics: PathBuf, pub shared: PathBuf, pub data: PathBuf }

pub struct Live<'a> { pub env: &'a Env, pub notify_on: bool, pub n: usize }
pub struct Shadow<'a> { pub env: &'a Env, pub log: Vec<String>, n: u32 }
impl<'a> Shadow<'a> { pub fn new(env: &'a Env) -> Self { Shadow { env, log: Vec::new(), n: 0 } } }

fn lock(f: &Path) -> bool {
    let l = PathBuf::from(format!("{}.lock", f.display()));
    for _ in 0..100 { if fs::create_dir(&l).is_ok() { return true; } std::thread::sleep(std::time::Duration::from_millis(50)); }
    let _ = fs::remove_dir_all(&l); fs::create_dir(&l).is_ok()
}
fn unlock(f: &Path) { let _ = fs::remove_dir(format!("{}.lock", f.display())); }

/// The value as topic_set stores it, and the lines that go with a change of status.
fn sanitize(key: &str, v: &str) -> String { let v = nocntrl(v); if key == "description" { chars_prefix(&v, 140) } else if key == "title" { chars_prefix(&v, 500) } else { v } }

fn today() -> String {
    // YYYY-MM-DD from the clock, local time through `date` (the script uses date +%Y-%m-%d)
    Command::new("date").arg("+%Y-%m-%d").stdin(Stdio::null()).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// The text of the description when a done topic is taken up again ("Reopened 2026-10-17. <the old one>").
fn reopened(old: &str) -> String {
    let b = old.as_bytes();
    let mut dsc = old.to_string();
    let dig = |i: usize| b.get(i).map_or(false, |c| c.is_ascii_digit());
    if old.starts_with("Reopened ") && (9..13).all(dig) && b.get(13) == Some(&b'-') && dig(14) && dig(15) && b.get(16) == Some(&b'-') && dig(17) && dig(18) {
        dsc = old[19..].to_string(); if let Some(x) = dsc.strip_prefix('.') { dsc = x.to_string(); } if let Some(x) = dsc.strip_prefix(' ') { dsc = x.to_string(); }
    }
    format!("Reopened {}.{}", today(), if dsc.is_empty() { String::new() } else { format!(" {}", dsc) })
}

fn real_set(env: &Env, f: &Path, key: &str, value: &str) {
    let v = sanitize(key, value);
    let cur = fs::read_to_string(f).unwrap_or_default();
    if key == "status" && v != "done" && first_value(&cur, "status").map(|x| nocntrl(&x)).as_deref() == Some("done") {
        let d = first_value(&cur, "description").map(|x| nocntrl(&x)).unwrap_or_default();
        real_set(env, f, "description", &reopened(&d));
    }
    let locked = lock(f);
    let cur = fs::read_to_string(f).unwrap_or_default();   // (after the wait: what the other writer left)
    let old = num_or(&first_value(&cur, "updated").map(|x| nocntrl(&x)).unwrap_or_default(), 0);
    let n = now().max(old + 1);
    let mut drop: Vec<&str> = vec!["updated"];
    if key == "status" && v != "now" { drop.extend(["transfer_to", "transfer_to_name", "transfer_from"]); }
    if key == "status" && v != "review" { drop.extend(["review_note", "review_ask", "review_reply", "review_src"]); }
    if key == "status" && v != "later" { drop.push("dismissed"); }
    if key == "status" && v != "waiting" { drop.extend(["deploy_wait", "deploy_match", "deploy_url", "deploy_since"]); }
    let mut out = String::new();
    for l in cur.lines() {
        let k = l.find('=').map(|i| &l[..i]);
        if let Some(k) = k { if k == key || drop.contains(&k) { continue; } }
        out.push_str(l); out.push('\n');
    }
    out.push_str(&format!("{}={}\nupdated={}\n", key, v, n));
    let tmp = PathBuf::from(format!("{}.{}{}", f.display(), std::process::id(), n % 100000));
    if fs::write(&tmp, out).is_ok() { let _ = fs::rename(&tmp, f); }
    if locked { unlock(f); }
    let _ = fs::write(env.shared.join("dirty"), "");
    let _ = fs::remove_file(env.shared.join("topics.idx"));
}

fn rand5() -> u32 { (SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos()) ^ std::process::id()) % 32768 }

impl<'a> Sink for Live<'a> {
    fn set(&mut self, file: &Path, key: &str, value: &str) { self.n += 1; real_set(self.env, file, key, value); }
    fn new_topic(&mut self, title: &str, wfor: &str, desc: &str) -> Option<PathBuf> {
        self.n += 1;
        let _ = fs::create_dir_all(&self.env.topics);
        let n = now(); let id = format!("{}-{}", n, rand5());
        let title = chars_prefix(nocntrl(title).trim_matches(' '), 500); if title.is_empty() { return None; }
        let wf = nocntrl(wfor); let desc = chars_prefix(&nocntrl(desc), 140);
        let (status, since) = if wf.is_empty() { ("now", String::new()) } else { ("waiting", n.to_string()) };
        let f = self.env.topics.join(format!("{}.topic", id));
        fs::write(&f, format!("id={}\ntitle={}\nstatus={}\nwaiting_for={}\nwaiting_since={}\ncreated={}\nupdated={}\ndeleted=0\ndescription={}\n", id, title, status, wf, since, n, n, desc)).ok()?;
        let _ = fs::write(self.env.shared.join("dirty"), ""); let _ = fs::remove_file(self.env.shared.join("topics.idx"));
        Some(f)
    }
    fn write_file(&mut self, file: &Path, content: &str) { self.n += 1; if fs::write(file, content).is_ok() { let _ = fs::write(self.env.shared.join("dirty"), ""); } }
    fn notify(&mut self, title: &str, body: &str, sound: &str) {
        if !self.notify_on { return; }
        let _ = Command::new("herdr").args(["notification", "show", &nocntrl(title), "--body", &nocntrl(body), "--sound", sound]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
    fn herdr(&mut self, args: &[String]) { self.n += 1; let _ = Command::new("herdr").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
    fn is_shadow(&self) -> bool { false }
    fn writes(&self) -> usize { self.n }
}

impl<'a> Sink for Shadow<'a> {
    fn set(&mut self, file: &Path, key: &str, value: &str) {
        let id = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let v = sanitize(key, value);
        // the nested description set of a reopened topic is logged as the script's topic_set logs it (before the status)
        if key == "status" && v != "done" && topic_get(file, "status") == "done" { self.log.push(format!("set|{}|description|{}", id, reopened(&topic_get(file, "description")))); }
        self.log.push(format!("set|{}|{}|{}", id, key, v));
    }
    fn new_topic(&mut self, title: &str, wfor: &str, desc: &str) -> Option<PathBuf> {
        let t = chars_prefix(nocntrl(title).trim_matches(' '), 500); if t.is_empty() { return None; }
        self.n += 1; self.log.push(format!("new|{}|{}|{}", t, nocntrl(wfor), chars_prefix(&nocntrl(desc), 140)));
        Some(self.env.topics.join(format!("shadow-new-{}.topic", self.n)))
    }
    fn write_file(&mut self, file: &Path, content: &str) { self.log.push(format!("file|{}|{}", file.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), content.replace('\n', "~"))); }
    fn notify(&mut self, title: &str, body: &str, sound: &str) { self.log.push(format!("notify|{}|{}|{}", nocntrl(title), nocntrl(body), sound)); }
    fn herdr(&mut self, args: &[String]) { self.log.push(format!("herdr|{}", args.join(" "))); }
    fn is_shadow(&self) -> bool { true }
    fn writes(&self) -> usize { 0 }
}

pub fn ids(topics: &[Topic]) -> HashMap<String, usize> { topics.iter().enumerate().map(|(i, t)| (t.id.clone(), i)).collect() }
