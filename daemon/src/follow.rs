//! The follow-ups of the collector (step 4), ported one by one from the script: remind_waiting, finished_expire, agent_dedup,
//! agent_cards, review_follow, now_idle_follow, agent_gone_follow, auto_question_follow, branch_sync, agentnum_assign.
//! Each takes the same state the bash function reads (topics, the agent lists, the settings, small state files) and acts through a Sink.
use crate::store::{self, chars_prefix, first_value, nocntrl, now, num_or, topic_get, Env, Sink, Topic};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const US: char = '\u{1f}';

pub struct Row { pub m: String, pub id: String, pub st: String, pub name: String, pub task: String }
pub struct Ctx<'a> {
    pub env: &'a Env, pub host: String, pub cfg: HashMap<String, String>,
    pub local: Option<Vec<Row>>, pub remote: Vec<Row>,
    /// where the small state files live: the shared folder itself, or (shadow) a folder of its own, so that a shadow run never moves the real state
    pub state: PathBuf, pub reminded: PathBuf,
}
impl<'a> Ctx<'a> {
    fn cfg(&self, k: &str, d: &str) -> String { self.cfg.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string()) }
    fn num(&self, k: &str, d: u64) -> u64 { num_or(&self.cfg(k, ""), d) }
    fn dir(&self, name: &str) -> PathBuf { let d = self.state.join(name); let _ = fs::create_dir_all(&d); d }
    /// the state of an agent: the first row of the local, then the remote list
    fn status(&self, m: &str, p: &str) -> Option<String> {
        self.local.iter().flatten().chain(self.remote.iter()).find(|r| r.m == m && r.id == p).map(|r| r.st.clone())
    }
}

pub fn read_rows(path: &Path) -> Option<Vec<Row>> {
    let t = fs::read_to_string(path).ok()?;
    Some(t.lines().filter(|l| !l.is_empty()).map(|l| { let mut p = l.split(US).map(|x| x.to_string()); Row { m: p.next().unwrap_or_default(), id: p.next().unwrap_or_default(), st: p.next().unwrap_or_default(), name: p.next().unwrap_or_default(), task: p.next().unwrap_or_default() } }).collect())
}

fn herdr_read(p: &str) -> String {
    Command::new("herdr").args(["pane", "read", p, "--source", "recent-unwrapped", "--lines", "80"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// pane_question_text: the agent's last line when it ends with a question (above the input box, footers skipped)
pub fn question_text(text: &str) -> String {
    let l: Vec<&str> = text.lines().collect();
    let n = l.len();
    let rule = |s: &str| !s.is_empty() && s.chars().all(|c| c == '─');
    let (mut box_, mut n2) = (0usize, 0);
    for i in (0..n).rev() { if rule(l[i]) { n2 += 1; if n2 == 2 { box_ = i + 1; break; } } }
    if box_ == 0 { box_ = n + 1; }
    for i in (0..box_.saturating_sub(1)).rev() {
        let t = l[i].trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
        let marker = ["✻", "✽", "✶", "✢", "※", "*", "·"].iter().any(|m| t.strip_prefix(m).map_or(false, |r| r.is_empty() || r.starts_with(' ')));
        if t.is_empty() || marker || t.starts_with('⏵') || t.starts_with('⏸') { continue; }
        let t = ["●", "⏺"].iter().find_map(|m| t.strip_prefix(m)).map_or(t, |r| r.trim_start_matches(' '));
        let core = t.trim_end_matches([')', '"', '\'', '”', '»']);
        return if core.ends_with('?') { t.to_string() } else { String::new() };
    }
    String::new()
}

fn touch_state(d: &Path, id: &str, v: &str) { let _ = fs::write(d.join(id), format!("{}\n", v)); }
fn read_state(d: &Path, id: &str) -> String { fs::read_to_string(d.join(id)).ok().and_then(|t| t.lines().next().map(|x| x.to_string())).unwrap_or_default() }
fn rm_state(d: &Path, id: &str) { let _ = fs::remove_file(d.join(id)); }

pub fn remind_waiting(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let h = c.num("WAIT_REMIND", 24); if h == 0 { return; }
    let n = now();
    for t in ts {
        if t.status != "waiting" { continue; }
        if (n as i128 - num_or(&t.wsince, n) as i128) < (h * 3600) as i128 { continue; }
        let id = format!("{}:{}", t.id, t.wsince);
        if fs::read_to_string(&c.reminded).unwrap_or_default().lines().any(|l| l == id) { continue; }
        if let Some(p) = c.reminded.parent() { let _ = fs::create_dir_all(p); }
        { use std::io::Write; if let Ok(mut f) = fs::OpenOptions::new().append(true).create(true).open(&c.reminded) { let _ = writeln!(f, "{}", id); } }
        s.notify("Still waiting", &format!("{} · {} for {}h+", t.title, if t.wfor.is_empty() { "?" } else { &t.wfor }, h), "request");
    }
}

pub fn finished_expire(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let m = c.num("FINISHED_MINUTES", 30); if m == 0 { return; }
    let n = now();
    for t in ts {
        if t.status != "finished" || (n as i128 - t.upd as i128) < (m * 60) as i128 { continue; }
        if topic_get(&t.file, "auto_now") == "1" { s.set(&t.file, "deleted", "1"); } else { s.set(&t.file, "status", "later"); s.set(&t.file, "dismissed", "1"); }
    }
}

pub fn agent_dedup(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let prefix = |id: &str| id.split('-').next().unwrap_or("").to_string();
    let am = |t: &Topic| if t.am.is_empty() { c.host.clone() } else { t.am.clone() };
    for (i, t) in ts.iter().enumerate() {
        if !(t.status == "now" || t.status == "testing") || t.ap.is_empty() || !t.tn.is_empty() || t.an == "1" { continue; }
        let mut keep = true;
        for (j, u) in ts.iter().enumerate() {
            if j == i || !(u.status == "now" || u.status == "testing") { continue; }
            if !(u.ap == t.ap && am(u) == am(t) && u.tn.is_empty()) || u.an == "1" { continue; }
            let (pj, pi) = (prefix(&u.id), prefix(&t.id));
            if let (Ok(a), Ok(b)) = (pj.parse::<i64>(), pi.parse::<i64>()) { if a > b { keep = false; break; } }
            if pj == pi && u.id > t.id { keep = false; break; }
        }
        if !keep { s.set(&t.file, "status", "finished"); s.set(&t.file, "auto_back", ""); }
    }
}

pub fn agent_cards(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.cfg("AGENT_CARDS", "on") != "on" { return; }
    let Some(local) = &c.local else { return };
    let cd = c.dir("cards"); let n = now();
    let key = |id: &str| id.chars().map(|x| if x.is_ascii_alphanumeric() || x == '_' || x == '-' { x } else { '_' }).collect::<String>();
    let mut work: Vec<&str> = Vec::new();
    for r in local.iter().filter(|r| r.m == c.host) {
        match r.st.as_str() { "working" | "testing" => { work.push(&r.id); if !cd.join(key(&r.id)).exists() { touch_state(&cd, &key(&r.id), &n.to_string()); } } _ => rm_state(&cd, &key(&r.id)) }
    }
    let owned: Vec<&str> = ts.iter().filter(|t| t.am == c.host && !t.ap.is_empty() && t.an != "1" && matches!(t.status.as_str(), "now" | "testing" | "finished" | "review" | "next")).map(|t| t.ap.as_str()).collect();
    let mut cards: Vec<String> = Vec::new();
    for t in ts {
        if !(t.am == c.host && !t.ap.is_empty()) || t.an != "1" { continue; }
        if !matches!(t.status.as_str(), "now" | "testing" | "finished") { s.set(&t.file, "auto_now", ""); continue; }
        if work.contains(&t.ap.as_str()) { if t.status == "finished" { s.set(&t.file, "status", "now"); } }
        else if t.status == "now" { s.set(&t.file, "status", "finished"); }
        if owned.contains(&t.ap.as_str()) { s.set(&t.file, "deleted", "1"); continue; }
        cards.push(t.ap.clone());
        for r in local.iter().filter(|r| r.m == c.host && r.id == t.ap) {
            let title = nocntrl(if r.task.is_empty() { &r.name } else { &r.task });
            if !title.is_empty() && title != t.title { s.set(&t.file, "title", &chars_prefix(&title, 500)); }
        }
    }
    for r in local.iter().filter(|r| r.m == c.host) {
        if !work.contains(&r.id.as_str()) || owned.contains(&r.id.as_str()) || cards.contains(&r.id) { continue; }
        if n.saturating_sub(num_or(&read_state(&cd, &key(&r.id)), n)) < 15 { continue; }
        let title = if r.task.is_empty() { &r.name } else { &r.task };
        let Some(f) = s.new_topic(title, "", "") else { continue };
        s.set(&f, "auto_now", "1"); s.set(&f, "agent", &r.name); s.set(&f, "agent_machine", &c.host); s.set(&f, "agent_pane", &r.id);
    }
}

/// agent_reopen_pick: the ONE finished or dismissed card of this agent that comes back when it works again (the newest); None when it has a card on now
pub fn reopen_pick(ts: &[Topic], session: &str, pane: &str, machine: &str, strict: bool) -> Option<usize> {
    let mut best: Option<usize> = None; let mut bu: i64 = -1;
    for (i, t) in ts.iter().enumerate() {
        let hit = (!session.is_empty() && t.session == session) || (!pane.is_empty() && t.ap == pane && (if t.am.is_empty() { machine } else { &t.am }) == machine && (!strict || t.session.is_empty()));
        if !hit || t.an == "1" { continue; }
        match t.status.as_str() {
            "now" | "testing" => { if t.tn.is_empty() { return None; } }
            "finished" => {}
            "later" => { if t.dm != "1" { continue; } }
            _ => continue,
        }
        if t.status == "now" { continue; }
        if t.upd as i64 >= bu { bu = t.upd as i64; best = Some(i); }
    }
    best
}

pub fn review_follow(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.local.is_none() { return; }
    let d = c.dir("revidle");
    for (i, t) in ts.iter().enumerate() {
        if t.ap.is_empty() { continue; }
        if !matches!(t.status.as_str(), "review" | "later" | "finished") { continue; }
        if t.status == "later" && t.dm != "1" { continue; }
        if t.status == "review" && !(t.sm.is_empty() && t.rs != "deploy") { continue; }
        let m = if t.am.is_empty() { c.host.clone() } else { t.am.clone() };
        match c.status(&m, &t.ap).unwrap_or_default().as_str() {
            "idle" | "done" | "blocked" => touch_state(&d, &t.id, ""),
            "working" => if d.join(&t.id).exists() {
                rm_state(&d, &t.id);
                if topic_get(&t.file, "auto") == "1" { s.set(&t.file, "deleted", "1"); }
                else if t.status == "review" { s.set(&t.file, "status", "now"); s.set(&t.file, "auto_back", ""); }
                else if reopen_pick(ts, "", &t.ap, &m, false) == Some(i) { s.set(&t.file, "status", "now"); s.set(&t.file, "auto_back", ""); }
            },
            _ => {}
        }
    }
}

pub fn now_idle_follow(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.local.is_none() { return; }
    let d = c.dir("nowidle"); let n = now();
    for t in ts {
        if !(t.status == "now" && !t.ap.is_empty() && t.tn.is_empty()) { rm_state(&d, &t.id); continue; }
        if t.an == "1" { continue; }
        if n.saturating_sub(num_or(&t.dl, 0)) < 30 { continue; }
        if n.saturating_sub(t.upd) < 60 { continue; }
        let m = if t.am.is_empty() { c.host.clone() } else { t.am.clone() };
        match c.status(&m, &t.ap).unwrap_or_default().as_str() {
            "idle" | "done" => {
                let t0 = read_state(&d, &t.id);
                if t0.is_empty() { touch_state(&d, &t.id, &n.to_string()); }
                else if n.saturating_sub(num_or(&t0, n)) >= 8 {
                    rm_state(&d, &t.id);
                    let fq = if m == c.host { question_text(&herdr_read(&t.ap)) } else { String::new() };
                    if !fq.is_empty() {
                        s.set(&t.file, "status", "review"); s.set(&t.file, "auto_back", "1");
                        s.notify("Ready for review", &format!("{} ({})", t.title, if t.ag.is_empty() { "agent" } else { &t.ag }), "request");
                    } else { s.set(&t.file, "status", "finished"); }
                }
            }
            _ => rm_state(&d, &t.id),
        }
    }
}

pub fn agent_gone_follow(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let Some(local) = &c.local else { return };
    let d = c.dir("agentgone"); let n = now();
    for t in ts {
        if !matches!(t.status.as_str(), "review" | "finished" | "now") { rm_state(&d, &t.id); continue; }
        if t.ap.is_empty() || !t.tn.is_empty() { rm_state(&d, &t.id); continue; }
        let m = if t.am.is_empty() { c.host.clone() } else { t.am.clone() };
        if m == c.host { if local.is_empty() { rm_state(&d, &t.id); continue; } }
        else if !c.remote.iter().any(|r| r.m == m && !r.st.starts_with("off-")) { rm_state(&d, &t.id); continue; }
        if c.status(&m, &t.ap).map_or(false, |x| !x.is_empty()) { rm_state(&d, &t.id); continue; }
        let t0 = read_state(&d, &t.id);
        if t0.is_empty() { touch_state(&d, &t.id, &n.to_string()); continue; }
        if n.saturating_sub(num_or(&t0, n)) < 30 { continue; }
        if !herdr_answers() { if let Ok(rd) = fs::read_dir(&d) { for e in rd.flatten() { let _ = fs::remove_file(e.path()); } } return; }
        rm_state(&d, &t.id);
        if topic_get(&t.file, "auto") == "1" || topic_get(&t.file, "auto_now") == "1" { s.set(&t.file, "deleted", "1"); }
        else if t.status == "now" { s.set(&t.file, "status", "finished"); }
        else { s.set(&t.file, "status", "later"); s.set(&t.file, "dismissed", "1"); s.set(&t.file, "auto_back", ""); }
    }
}
fn herdr_answers() -> bool { Command::new("herdr").args(["pane", "list"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |s| s.success()) }

pub fn auto_question_follow(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.local.is_none() { return; }
    let d = c.dir("qidle"); let n = now(); let mins = c.num("FINISHED_MINUTES", 30);
    for t in ts {
        if !(t.status == "review" && !t.ap.is_empty() && t.au == "1") { rm_state(&d, &t.id); continue; }
        let m = if t.am.is_empty() { c.host.clone() } else { t.am.clone() };
        match c.status(&m, &t.ap).unwrap_or_default().as_str() {
            "idle" | "done" => {
                if m != c.host { if mins > 0 && (n as i128 - t.upd as i128) >= (mins * 60) as i128 { s.set(&t.file, "deleted", "1"); } continue; }
                if !question_text(&herdr_read(&t.ap)).is_empty() { rm_state(&d, &t.id); continue; }
                let t0 = read_state(&d, &t.id);
                if t0.is_empty() { touch_state(&d, &t.id, &n.to_string()); }
                else if n.saturating_sub(num_or(&t0, n)) >= 8 { rm_state(&d, &t.id); s.set(&t.file, "deleted", "1"); }
            }
            _ => rm_state(&d, &t.id),
        }
    }
}

/// git_branch_set: the branch checked out in a folder (worktrees too), "@1234abc" when detached; read from .git, no git call
pub fn git_branch(folder: &str) -> Option<String> {
    let mut d = folder.to_string(); let mut head: Option<PathBuf> = None;
    for _ in 0..40 {
        if d.is_empty() { break; }
        let g = Path::new(&d).join(".git");
        if g.is_dir() { head = Some(g.join("HEAD")); break; }
        if g.is_file() {
            let l = fs::read_to_string(&g).ok()?; let l = l.lines().next()?.trim_start_matches("gitdir: ").to_string();
            let p = if l.starts_with('/') { PathBuf::from(l) } else { Path::new(&d).join(l) };
            head = Some(p.join("HEAD")); break;
        }
        if d == "/" { break; }
        d = match d.rfind('/') { Some(0) => "/".into(), Some(i) => d[..i].to_string(), None => break };
    }
    let l = fs::read_to_string(head?).ok()?; let l = l.lines().next()?;
    if let Some(b) = l.strip_prefix("ref: refs/heads/") { Some(b.to_string()) }
    else if l.starts_with(|c: char| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) { Some(format!("@{}", chars_prefix(l, 7))) } else { None }
}

pub fn branch_sync(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.cfg("GITHUB_BRANCHES", "on") != "on" { return; }
    let Ok(t) = fs::read_to_string(c.env.shared.join("agents.cwd")) else { return };
    let mut map: HashMap<String, String> = HashMap::new();
    for l in t.lines() { let mut p = l.splitn(2, US); if let (Some(a), Some(b)) = (p.next(), p.next()) { if !a.is_empty() { map.entry(a.to_string()).or_insert(b.to_string()); } } }
    for tp in ts {
        if !(tp.am == c.host && !tp.ap.is_empty()) || matches!(tp.status.as_str(), "done" | "later") { continue; }
        let Some(cwd) = map.get(&tp.ap) else { continue };
        if let Some(br) = git_branch(cwd) { if !br.is_empty() && br != tp.br { s.set(&tp.file, "agent_branch", &br); } }
    }
}

pub fn agentnum_assign(c: &Ctx, s: &mut dyn Sink) {
    let mut keys: Vec<(String, u64)> = Vec::new();
    let mut files: Vec<PathBuf> = fs::read_dir(&c.env.topics).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); n.starts_with("an-") && n.ends_with(".topic") })).collect()).unwrap_or_default();
    files.sort();
    for f in &files {
        let t = fs::read_to_string(f).unwrap_or_default();
        let (mut k, mut n, mut d) = (String::new(), String::new(), "0".to_string());
        for l in t.lines() { if let Some(v) = l.strip_prefix("title=") { if k.is_empty() { k = v.to_string(); } } else if let Some(v) = l.strip_prefix("num=") { n = v.to_string(); } else if let Some(v) = l.strip_prefix("deleted=") { d = v.to_string(); } }
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || d == "1" || k.is_empty() { continue; }
        keys.push((k, n.parse().unwrap_or(0)));
    }
    let pre = format!("{}/", c.host);
    let mut max = keys.iter().filter(|(k, _)| k.starts_with(&pre)).map(|(_, n)| *n).max().unwrap_or(0);
    let Some(local) = &c.local else { return };
    let n = now();
    for r in local.iter().filter(|r| r.m == c.host && !r.name.is_empty()) {
        let name = nocntrl(&r.name);
        let k = format!("{}/{}", r.m, name);
        if keys.iter().any(|(x, _)| *x == k) { continue; }
        let san = |x: &str| x.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>();
        let f = c.env.topics.join(format!("an-{}-{}.topic", chars_prefix(&san(&r.m), 20), chars_prefix(&san(&name), 60)));
        if f.exists() { continue; }
        max += 1;
        let id = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        s.write_file(&f, &format!("id={}\nkind=agentnum\ntitle={}/{}\nnum={}\ncreated={}\nupdated={}\ndeleted=0\n", id, r.m, name, max, n, n));
        keys.push((k, max));
    }
}

/// One round of all of them, in the order of the collector (agentnum_assign first, then branch_sync, remind_waiting, agent_cards, agent_dedup,
/// review_follow, now_idle_follow, agent_gone_follow, finished_expire, auto_question_follow); every function reads the topics anew.
pub fn round(c: &Ctx, s: &mut dyn Sink, only: Option<&str>) {
    let want = |n: &str| only.map_or(true, |o| o == n);
    let mut ts = store::load_fast(&c.env.topics, &c.env.shared); let mut seen = s.writes();
    macro_rules! step { ($name:expr, $call:expr) => { if want($name) { $call; if s.writes() != seen { ts = store::load_fast(&c.env.topics, &c.env.shared); seen = s.writes(); } } } }
    step!("agentnum_assign", agentnum_assign(c, s));
    step!("branch_sync", branch_sync(c, &ts, s));
    step!("remind_waiting", remind_waiting(c, &ts, s));
    step!("agent_cards", agent_cards(c, &ts, s));
    step!("agent_dedup", agent_dedup(c, &ts, s));
    step!("review_follow", review_follow(c, &ts, s));
    step!("now_idle_follow", now_idle_follow(c, &ts, s));
    step!("agent_gone_follow", agent_gone_follow(c, &ts, s));
    step!("finished_expire", finished_expire(c, &ts, s));
    step!("auto_question_follow", auto_question_follow(c, &ts, s));
}

/// The context of a round from the files of the script (a missing local list: None, as the script's `[ -r "$AGENT_LOCAL" ]` fails).
pub fn context<'a>(env: &'a Env, host: &str, cfg: HashMap<String, String>, shadow: bool) -> Ctx<'a> {
    let state = if shadow { env.shared.join("shadow") } else { env.shared.clone() };
    let reminded = if shadow { env.shared.join("shadow").join("reminded") } else { env.data.join("reminded") };
    Ctx { env, host: host.to_string(), cfg, local: read_rows(&env.shared.join("agents.local")), remote: read_rows(&env.shared.join("agents.remote")).unwrap_or_default(), state, reminded }
}
#[allow(dead_code)]
fn unused(_: Option<String>) { let _ = first_value; }
