//! The follow-ups of the collector (step 4), ported one by one from the script: remind_waiting, finished_expire, agent_dedup,
//! agent_cards, review_follow, now_idle_follow, agent_gone_follow, auto_question_follow, branch_sync, agentnum_assign.
//! Each takes the same state the bash function reads (topics, the agent lists, the settings, small state files) and acts through a Sink.
use crate::store::{self, chars_prefix, first_value, nocntrl, now, num_or, topic_get, Env, Sink, Topic};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};


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
    let t = crate::proc::read_cap(path).ok()?;
    Some(t.lines().filter(|l| !l.is_empty()).map(|l| { let mut p = l.split(US).map(|x| x.to_string()); Row { m: p.next().unwrap_or_default(), id: p.next().unwrap_or_default(), st: p.next().unwrap_or_default(), name: p.next().unwrap_or_default(), task: p.next().unwrap_or_default() } }).collect())
}

fn herdr_read(p: &str) -> String {
    crate::proc::run("herdr", &["pane", "read", p, "--source", "recent-unwrapped", "--lines", "80"], 20, None).map(|o| String::from_utf8_lossy(&o.1).into_owned()).unwrap_or_default()
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
fn read_state(d: &Path, id: &str) -> String { crate::proc::read_cap(d.join(id)).ok().and_then(|t| t.lines().next().map(|x| x.to_string())).unwrap_or_default() }
fn rm_state(d: &Path, id: &str) { let _ = fs::remove_file(d.join(id)); }

pub fn remind_waiting(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let h = c.num("WAIT_REMIND", 24); if h == 0 { return; }
    let n = now();
    for t in ts {
        if t.status != "waiting" { continue; }
        if (n as i128 - num_or(&t.wsince, n) as i128) < (h * 3600) as i128 { continue; }
        let id = format!("{}:{}", t.id, t.wsince);
        if crate::proc::read_cap(&c.reminded).unwrap_or_default().lines().any(|l| l == id) { continue; }
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
    // a recently finished card that says the same as a card of the same agent (same title, at work, in review, waiting, or a newer finished one) is the same work twice: put away
    for (i, t) in ts.iter().enumerate() {
        if !matches!(t.status.as_str(), "finished" | "review") || t.ap.is_empty() || t.an == "1" { continue; }
        let newer = |u: &Topic, j: usize| u.upd > t.upd || (u.upd == t.upd && j < i);
        let dup = ts.iter().enumerate().any(|(j, u)| j != i && u.ap == t.ap && am(u) == am(t) && u.title == t.title && match u.status.as_str() {
            "now" | "testing" => true,   // (the agent works on it again: the card of an earlier turn is superseded)
            "review" | "waiting" => t.status == "finished" || (u.status == "review" && newer(u, j)),
            "finished" => newer(u, j),
            _ => false });
        if dup { s.set(&t.file, "status", "later"); s.set(&t.file, "dismissed", "1"); }
    }
    // "Waiting for New Work" is no work: an agent that says so is idle (it has its card in the group IDLE), whatever status it put the topic in
    for t in ts {
        if matches!(t.status.as_str(), "now" | "testing" | "finished" | "review") && matches!(t.title.trim(), "Waiting for New Work" | "Waiting for new work" | "waiting for new work" | "Waiting For New Work") {
            s.set(&t.file, "status", "later"); s.set(&t.file, "dismissed", "1");
        }
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
    let owned: Vec<&str> = ts.iter().filter(|t| t.am == c.host && !t.ap.is_empty() && t.an != "1" && (matches!(t.status.as_str(), "now" | "testing" | "finished" | "review" | "next") || (t.status == "waiting" && t.dep == "1"))).map(|t| t.ap.as_str()).collect();   // (waiting for a deployment: a card, the agent has one: no second card that deploy_sync would take again)
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
fn herdr_answers() -> bool { crate::proc::run("herdr", &["pane", "list"], 15, None).map_or(false, |o| o.0 == 0) }

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
            let l = crate::proc::read_cap(&g).ok()?; let l = l.lines().next()?.trim_start_matches("gitdir: ").to_string();
            let p = if l.starts_with('/') { PathBuf::from(l) } else { Path::new(&d).join(l) };
            head = Some(p.join("HEAD")); break;
        }
        if d == "/" { break; }
        d = match d.rfind('/') { Some(0) => "/".into(), Some(i) => d[..i].to_string(), None => break };
    }
    let l = crate::proc::read_cap(head?).ok()?; let l = l.lines().next()?;
    if let Some(b) = l.strip_prefix("ref: refs/heads/") { Some(b.to_string()) }
    else if l.starts_with(|c: char| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) { Some(format!("@{}", chars_prefix(l, 7))) } else { None }
}

pub fn branch_sync(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    if c.cfg("GITHUB_BRANCHES", "on") != "on" { return; }
    let Ok(t) = crate::proc::read_cap(c.env.shared.join("agents.cwd")) else { return };
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
        let t = crate::proc::read_cap(f).unwrap_or_default();
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

// ── projects, the tests of a worktree, the deployments a topic waits for ─────────────────────────────────────

pub struct Proj { pub id: String, pub name: String, pub repos: String, pub ws: String }
/// projects_load: by name, with the repositories and the workspaces (the last value of a key counts, as the script's read loop does)
pub fn projects_full(topics: &Path) -> Vec<Proj> {
    let mut v = Vec::new();
    let mut files: Vec<PathBuf> = fs::read_dir(topics).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); n.starts_with("proj-") && n.ends_with(".topic") })).collect()).unwrap_or_default();
    files.sort();
    for f in files {
        let (mut name, mut repos, mut ws, mut del, mut kd) = (String::new(), String::new(), String::new(), "0".to_string(), String::new());
        for l in crate::proc::read_cap(&f).unwrap_or_default().lines() { if let Some(i) = l.find('=') { let (k, x) = (&l[..i], &l[i + 1..]); match k { "title" if name.is_empty() => name = x.to_string(), "repos" => repos = x.to_string(), "workspaces" => ws = x.to_string(), "deleted" => del = x.to_string(), "kind" => kd = x.to_string(), _ => {} } } }
        if kd == "project" && del != "1" && !name.is_empty() { v.push(Proj { id: f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), name: nocntrl(&name), repos, ws }); }
    }
    v.sort_by(|a, b| crate::ui::coll(&a.name, &b.name));
    v
}
fn list_has(list: &str, item: &str) -> bool { format!(",{},", list).contains(&format!(",{},", item)) }
fn project_for<'a>(ps: &'a [Proj], key: &str, ws: &str) -> Option<&'a Proj> {
    if !key.is_empty() { if let Some(p) = ps.iter().find(|p| list_has(&p.repos, key)) { return Some(p); } }
    if !ws.is_empty() { if let Some(p) = ps.iter().find(|p| list_has(&p.ws, ws)) { return Some(p); } }
    None
}
/// git_repo_key_set: the key of the repository in a folder (a worktree: its main one), empty outside one
pub fn git_repo_key(folder: &str) -> Option<String> {
    let mut d = folder.to_string(); let (mut cfgdir, mut main): (Option<PathBuf>, String) = (None, String::new());
    for _ in 0..40 {
        if d.is_empty() { break; }
        let g = Path::new(&d).join(".git");
        if g.is_dir() { cfgdir = Some(g); main = d.clone(); break; }
        if g.is_file() {
            let l = crate::proc::read_cap(&g).ok()?; let l = l.lines().next()?.trim_start_matches("gitdir: ").to_string();
            let p = if l.starts_with('/') { PathBuf::from(l) } else { Path::new(&d).join(l) };
            let ps = p.to_string_lossy().to_string();
            if let Some(i) = ps.find("/worktrees/") { let c = PathBuf::from(&ps[..i]); main = c.to_string_lossy().trim_end_matches("/.git").to_string(); cfgdir = Some(c); } else { cfgdir = Some(p); main = d.clone(); }
            break;
        }
        if d == "/" { break; }
        d = match d.rfind('/') { Some(0) => "/".into(), Some(i) => d[..i].to_string(), None => break };
    }
    let c = cfgdir?;
    let (mut url, mut inr) = (String::new(), false);
    for l in crate::proc::read_cap(c.join("config")).unwrap_or_default().lines() {
        if l.starts_with('[') { inr = l == "[remote \"origin\"]"; } else if inr && l.contains("url") && l.contains('=') { url = l[l.find('=').unwrap() + 1..].trim_start_matches(' ').to_string(); break; }
    }
    if !url.is_empty() {
        let mut u = url.as_str(); if let Some(i) = u.find("://") { u = &u[i + 3..]; } if let Some(i) = u.find('@') { u = &u[i + 1..]; }
        let u = u.trim_end_matches(".git").trim_end_matches('/'); let mut u = u.to_string();
        if let Some(i) = u.find(':') { let rest = u[i + 1..].to_string(); let first_digits = rest.starts_with(|c: char| c.is_ascii_digit()) && rest.contains('/'); u = if first_digits { format!("{}/{}", &u[..i], rest.splitn(2, '/').nth(1).unwrap_or("")) } else { format!("{}/{}", &u[..i], rest) }; }
        Some(u.to_lowercase())
    } else { Some(format!("dir:{}", main.rsplit('/').next().unwrap_or(""))) }
}

pub fn project_sync(c: &Ctx, s: &mut dyn Sink) {
    let Ok(meta) = crate::proc::read_cap(c.env.shared.join("agents.meta.local")) else { return };
    let ps = projects_full(&c.env.topics);
    let cache_f = c.env.shared.join("repo.cache");
    let mut cache: HashMap<String, String> = crate::proc::read_cap(&cache_f).unwrap_or_default().lines().filter_map(|l| { let mut p = l.splitn(2, US); Some((p.next()?.to_string(), p.next().unwrap_or("").to_string())) }).collect();
    let local = c.local.as_deref().unwrap_or(&[]);
    for l in meta.lines() {
        let f: Vec<&str> = l.split(US).collect(); if f.len() < 4 || f[3] != "a" { continue; }
        let (pane, r, j, cwd) = (f[1], f.get(4).copied().unwrap_or(""), f.get(5).copied().unwrap_or(""), f.get(6).copied().unwrap_or(""));
        let key = if let Some(k) = cache.get(cwd) { k.clone() } else {
            let k = git_repo_key(cwd).unwrap_or_default();
            if !s.is_shadow() { use std::io::Write; if let Ok(mut o) = fs::OpenOptions::new().append(true).create(true).open(&cache_f) { let _ = writeln!(o, "{}{}{}", cwd, US, k); } }
            cache.insert(cwd.to_string(), k.clone()); k
        };
        let ws = local.iter().find(|x| x.id == pane).map(|x| x.name.clone()).unwrap_or_default();
        let pid = project_for(&ps, &key, &ws).map(|p| p.id.clone()).unwrap_or_default();
        if key == r && pid == j { continue; }
        let mut a: Vec<String> = vec!["pane".into(), "report-metadata".into(), pane.into(), "--source".into(), "bhote".into()];
        if !key.is_empty() { a.push("--token".into()); a.push(format!("repo={}", key)); } else { a.push("--clear-token".into()); a.push("repo".into()); }
        if !pid.is_empty() { a.push("--token".into()); a.push(format!("project={}", pid)); } else { a.push("--clear-token".into()); a.push("project".into()); }
        s.herdr(&a);
    }
}

/// The tree (folder name of the worktree) of an agent's pane, and the test status files of it that are live: (done, total, suite)
fn live_tests(c: &Ctx, pane: &str) -> Vec<(String, String, String)> {
    if c.cfg("TESTS_BUSY", "on") != "on" { return Vec::new(); }
    let cwd = crate::proc::read_cap(c.env.shared.join("agents.cwd")).unwrap_or_default().lines().find_map(|l| { let mut p = l.splitn(2, US); if p.next() == Some(pane) { p.next().map(|x| x.to_string()) } else { None } }).unwrap_or_default();
    if cwd.is_empty() { return Vec::new(); }
    let top = crate::proc::run("git", &["-C", &cwd, "rev-parse", "--show-toplevel"], 10, None).filter(|o| o.0 == 0).map(|o| String::from_utf8_lossy(&o.1).trim().to_string()).unwrap_or(cwd);
    let tree = top.rsplit('/').next().unwrap_or("").to_string();
    let dir = c.cfg("TEST_STATUS_DIR", "~/.cache/bhote/test-status"); let dir = if let Some(r) = dir.strip_prefix('~') { format!("{}{}", std::env::var("HOME").unwrap_or_default(), r) } else { dir };
    let mut files: Vec<PathBuf> = fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); n.starts_with(&format!("{}.", tree)) && n.ends_with(".json") })).collect()).unwrap_or_default();
    files.sort();
    let now_f = now() as f64; let mut out = Vec::new();
    for f in files {
        let Ok(t) = crate::proc::read_cap(&f) else { continue }; let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else { continue };
        let finished = v.get("finished") == Some(&serde_json::Value::Bool(true));
        let okn = v.get("ok").map_or(true, |x| x.is_null());
        let total = v.get("total").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let upd = v.get("updatedAt").and_then(|x| x.as_f64()).unwrap_or(0.0) / 1000.0;
        if !finished && okn && total > 0.0 && upd > now_f - 30.0 {
            let suite: String = v.get("suite").map(|x| match x { serde_json::Value::String(s) => s.clone(), o => o.to_string() }).unwrap_or_default().chars().map(|c| if (c as u32) < 0x20 || c as u32 == 0x7f { ' ' } else { c }).collect();
            let fmt = |k: &str| v.get(k).filter(|x| !x.is_null()).map(|x| match x.as_f64() { Some(n) if n.fract() == 0.0 => (n as i64).to_string(), _ => x.to_string() }).unwrap_or_else(|| "0".into());
            out.push((suite.clone() + &US.to_string() + &fmt("done") + &US.to_string() + &fmt("total"), String::new(), String::new()));
        }
    }
    out
}
fn tests_running(c: &Ctx, pane: &str) -> bool { !live_tests(c, pane).is_empty() }
fn tests_progress_write(c: &Ctx, s: &mut dyn Sink, machine: &str, pane: &str, run: bool) {
    let name = format!("{}/testprog/{}_{}", c.env.shared.display(), machine, pane);
    let f: String = name.chars().map(|ch| if ch.is_ascii_alphanumeric() || "_/.-".contains(ch) { ch } else { '_' }).collect();
    if s.is_shadow() { return; }
    if !run { let _ = fs::remove_file(&f); return; }
    let mut lines: Vec<String> = live_tests(c, pane).into_iter().map(|t| t.0).collect(); lines.sort();
    let body: String = lines.iter().take(2).map(|l| { let p: Vec<&str> = l.split(US).collect(); format!("{} {} {}\n", p[1], p[2], p[0]) }).collect();
    if let Some(d) = Path::new(&f).parent() { let _ = fs::create_dir_all(d); }
    let tmp = format!("{}.tmp", f); if fs::write(&tmp, body).is_ok() { let _ = fs::rename(&tmp, &f); }
}

pub fn tests_sync(c: &Ctx, ts: &[Topic], s: &mut dyn Sink) {
    let Some(local) = &c.local else { return };
    for t in ts {
        if !matches!(t.status.as_str(), "now" | "testing") || !(t.am == c.host && !t.ap.is_empty()) { continue; }
        let run = tests_running(c, &t.ap);
        tests_progress_write(c, s, if t.am.is_empty() { &c.host } else { &t.am }, &t.ap, run);
        if t.status == "now" { if run && t.tn.is_empty() { s.set(&t.file, "status", "testing"); } continue; }
        if run { continue; }
        let st = local.iter().find(|r| r.m == c.host && r.id == t.ap).map(|r| r.st.clone()).unwrap_or_else(|| "gone".into());
        if st.starts_with("off-") || st == "unknown" { continue; }
        if st == "working" || st == "testing" { s.set(&t.file, "status", "now"); s.set(&t.file, "auto_back", ""); }
        else if topic_get(&t.file, "auto_now") == "1" { s.set(&t.file, "status", "now"); }
        else {
            s.set(&t.file, "status", "review");
            if topic_get(&t.file, "review_note").is_empty() && topic_get(&t.file, "review_ask").is_empty() { s.set(&t.file, "auto_back", "1"); }
            s.notify("Ready for review", &format!("{} ({})", t.title, if t.ag.is_empty() { "agent" } else { &t.ag }), "request");
        }
    }
}

/// branch_pr_number: the number of the merged pull request of a branch (kept for 10 minutes); empty: none
fn branch_pr_number(c: &Ctx, slug: &str, branch: &str) -> String {
    let san = |x: &str| -> String { x.chars().map(|ch| if ch.is_ascii_alphanumeric() || "._-".contains(ch) { ch } else { '_' }).collect() };
    let f = c.env.shared.join("prnum").join(format!("{}__{}", san(slug), san(branch)));
    if let Ok(m) = fs::metadata(&f) { if m.modified().ok().and_then(|t| t.elapsed().ok()).map_or(false, |a| a < std::time::Duration::from_secs(600)) { return crate::proc::read_cap(&f).unwrap_or_default(); } }
    let out = crate::proc::run("gh", &["pr", "list", "-R", slug, "--head", branch, "--state", "merged", "--limit", "1", "--json", "number", "--jq", ".[0].number // empty"], 25, None).filter(|o| o.0 == 0).map(|o| String::from_utf8_lossy(&o.1).trim().to_string()).unwrap_or_default();
    let n = if !out.is_empty() && out.bytes().all(|b| b.is_ascii_digit()) { out } else { String::new() };
    let _ = fs::create_dir_all(f.parent().unwrap()); let _ = fs::write(&f, &n); n
}

/// The pull request number in the title of the last commit in the folder of this machine's agent (empty: none)
fn agent_head_pr(c: &Ctx, pane: &str) -> String {
    let cwd = crate::proc::read_cap(c.env.shared.join("agents.cwd")).unwrap_or_default().lines().find_map(|l| { let mut p = l.splitn(2, US); if p.next() == Some(pane) { p.next().map(|x| x.to_string()) } else { None } }).unwrap_or_default();
    if cwd.is_empty() { return String::new(); }
    let subj = crate::proc::run("git", &["-C", &cwd, "log", "-1", "--format=%s"], 10, None).filter(|o| o.0 == 0).map(|o| String::from_utf8_lossy(&o.1).trim().to_string()).unwrap_or_default();
    let n: String = if subj.ends_with(')') && subj.contains("(#") { subj[subj.rfind("(#").unwrap() + 2..].trim_end_matches(')').to_string() }
        else if let Some(r) = subj.strip_prefix("Merge pull request #") { r.chars().take_while(|ch| ch.is_ascii_digit()).collect() } else { String::new() };
    if n.bytes().all(|b| b.is_ascii_digit()) { n } else { String::new() }
}

pub fn deploy_sync(c: &Ctx, s: &mut dyn Sink) {
    let Ok(list) = crate::proc::read_cap(c.env.shared.join("deploys.list")) else { return }; if list.is_empty() { return; }
    let n = now().to_string(); let ps = projects_full(&c.env.topics);
    let mut ts = store::load_fast(&c.env.topics, &c.env.shared);
    for l in list.lines() {
        let f: Vec<&str> = l.split(US).collect(); let at = |i: usize| f.get(i).copied().unwrap_or("");
        let (p, lb, st, nt, u) = (at(1), at(2), at(3), at(6), at(7));
        if !matches!(st, "running" | "queued" | "approval") { continue; }
        if nt.is_empty() || nt == "main" || nt == "master" || nt == "develop" || nt.starts_with('@') { continue; }
        let mut prn = String::new();
        if nt.ends_with(')') && nt.contains("(#") { let r = &nt[nt.rfind("(#").unwrap() + 2..]; prn = r.trim_end_matches(')').to_string(); if !prn.bytes().all(|b| b.is_ascii_digit()) { prn.clear(); } }
        else if let Some(r) = nt.strip_prefix("Merge pull request #") { prn = r.chars().take_while(|ch| ch.is_ascii_digit()).collect(); }
        for i in 0..ts.len() {
            let t = ts[i].clone();
            if !matches!(t.status.as_str(), "now" | "review" | "testing" | "finished") || t.ap.is_empty() || t.br.is_empty() { continue; }
            if t.status == "finished" && t.upd + 600 < at(0).parse::<u64>().unwrap_or(0) { continue; }
            // one topic per agent: its newest (by its last change; at the same time the first one wins); the older ones of that agent are not what is deployed
            let am = |x: &Topic| if x.am.is_empty() { c.host.clone() } else { x.am.clone() };
            if ts.iter().enumerate().any(|(j, o)| j != i && o.ap == t.ap && am(o) == am(&t) && (matches!(o.status.as_str(), "now" | "review" | "testing" | "finished") || (o.status == "waiting" && o.dep == "1")) && (o.upd > t.upd || (o.upd == t.upd && j < i))) { continue; }   // (finished long before this deployment began: an old topic of that agent, not its work)
            let pn = if t.pj.is_empty() { String::new() } else { ps.iter().find(|x| x.id == t.pj).map(|x| x.name.clone()).unwrap_or_default() };
            if !pn.is_empty() && pn != p { continue; }
            if t.br != nt {
                let mut ok = false;
                if !prn.is_empty() && !t.pj.is_empty() {
                    let slug = ps.iter().find(|x| x.id == t.pj).and_then(|x| x.repos.split(',').find_map(|k| k.strip_prefix("github.com/").filter(|r| r.contains('/')).map(|r| r.to_string())));
                    if let Some(slug) = slug { ok = branch_pr_number(c, &slug, &t.br) == prn; }
                }
                // or the last commit in the agent's folder (the merge of that pull request: the agent went back to its own branch)
                if !ok && !prn.is_empty() && (t.am.is_empty() || t.am == c.host) { ok = agent_head_pr(c, &t.ap) == prn; }
                if !ok { continue; }
            }
            s.set(&t.file, "status", "waiting"); s.set(&t.file, "waiting_for", &format!("Deployment {}", lb)); s.set(&t.file, "waiting_since", &n);
            s.set(&t.file, "deploy_wait", "1"); s.set(&t.file, "deploy_url", u); s.set(&t.file, "deploy_since", &n);
        }
        if s.writes() > 0 { ts = store::load_fast(&c.env.topics, &c.env.shared); }
    }
    let ts = if s.is_shadow() { store::load(&c.env.topics) } else { store::load(&c.env.topics) };
    for t in &ts {
        if !(t.status == "waiting" && t.dep == "1") { continue; }
        let Some((st, _d, _tot, _note, url, pj, lb)) = crate::cards::deploy_lookup_pub(&c.env.shared, &c.env.topics, &ps_names(&ps), t) else { continue };
        if store::topic_get(&t.file, "deploy_url").is_empty() { s.set(&t.file, "deploy_url", &url); }
        let (note, ic) = match st.as_str() {
            "ok" => (format!("Deployed: {} · {} went through. Check it in production.", pj, lb), "ok"),
            "failed" => (format!("The deployment of {} · {} failed. Look at it before you go on.", pj, lb), "bad"),
            "canceled" => (format!("The deployment of {} · {} was canceled. Look at it before you go on.", pj, lb), "bad"),
            _ => continue,
        };
        s.set(&t.file, "status", "review"); s.set(&t.file, "review_src", "deploy");
        s.set(&t.file, "review_note", &chars_prefix(&note, 240)); s.set(&t.file, "review_ask", "Review ansehen"); s.set(&t.file, "review_reply", "@jump");
        s.notify(if ic == "ok" { "Deployed" } else { "Deployment failed" }, &format!("{} ({} · {})", t.title, pj, lb), "request");
    }
}
fn ps_names(ps: &[Proj]) -> HashMap<String, String> { ps.iter().map(|p| (p.id.clone(), p.name.clone())).collect() }

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
    step!("project_sync", project_sync(c, s));
    step!("tests_sync", tests_sync(c, &ts, s));
    step!("deploy_sync", deploy_sync(c, s));
}

/// The context of a round from the files of the script (a missing local list: None, as the script's `[ -r "$AGENT_LOCAL" ]` fails).
pub fn context<'a>(env: &'a Env, host: &str, cfg: HashMap<String, String>, shadow: bool) -> Ctx<'a> {
    let state = if shadow { env.shared.join("shadow") } else { env.shared.clone() };
    let reminded = if shadow { env.shared.join("shadow").join("reminded") } else { env.data.join("reminded") };
    Ctx { env, host: host.to_string(), cfg, local: read_rows(&env.shared.join("agents.local")), remote: read_rows(&env.shared.join("agents.remote")).unwrap_or_default(), state, reminded }
}
#[allow(dead_code)]
fn unused(_: Option<String>) { let _ = first_value; }
