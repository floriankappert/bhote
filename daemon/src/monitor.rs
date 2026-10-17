//! The deployment and test monitors (step 3): what `monitor_fetch deploys|tests` writes, written here: `deploys.list` and
//! `tests.list` (time US project US label US state US done US total US note US url, newest first, at most 50), and `.at`.
//! GitHub Actions through `gh`, CircleCI through its API v2 (token from the settings or the environment), local test runs
//! and the live status files. Same rules as the bash functions (deploys_github, deploys_circleci, tests_*).
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The slow part is the network: the independent calls run side by side (the lists are sorted afterwards, so the order of the results does not matter).
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|sc| { let hs: Vec<_> = items.iter().map(|x| sc.spawn(|| f(x))).collect(); hs.into_iter().filter_map(|h| h.join().ok()).collect() })
}

const US: &str = "\u{1f}";

pub struct Ctx {
    pub shared: PathBuf,
    pub data: PathBuf,       // BHOTE_DATA: topics/, testruns/
    pub topics: PathBuf,
    pub cfg: HashMap<String, String>,
}
impl Ctx {
    fn get(&self, k: &str, d: &str) -> String { self.cfg.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string()) }
    fn on(&self, k: &str, d: &str) -> bool { self.get(k, d) == "on" }
}

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn s(v: &Value) -> String { match v { Value::String(x) => x.clone(), Value::Null => String::new(), o => o.to_string() } }
fn or_null<'a>(v: &'a Value, k: &str) -> &'a Value { v.get(k).unwrap_or(&Value::Null) }
/// mon_line: control characters in every field become a space.
fn line(f: &[&str]) -> String { f.iter().map(|x| x.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>()).collect::<Vec<_>>().join(US) }
fn cc_ctrl(x: &str) -> String { x.chars().map(|c| if (c as u32) < 0x20 { ' ' } else { c }).collect() }

/// mon_iso: "2026-10-17T12:34:56.789Z" -> Unix seconds (None when it is not an ISO time).
fn iso(t: &str) -> Option<u64> {
    let b = t.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' { return None; }
    let n = |a: usize, z: usize| t.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let era = y2.div_euclid(400); let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1; let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let v = days * 86400 + h * 3600 + mi * 60 + se;
    if v < 0 { None } else { Some(v as u64) }
}

// ── the commands: gh (with a time limit) and the CircleCI API ──────────────────────────────────────────────
fn run(cmd: &str, args: &[&str], secs: u64) -> Option<String> {
    let mut child = Command::new(cmd).args(args).stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || { let mut b = Vec::new(); let _ = out.read_to_end(&mut b); String::from_utf8_lossy(&b).into_owned() });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => { let o = reader.join().ok()?; return if st.success() { Some(o) } else { None }; }
            Ok(None) if t0.elapsed() < Duration::from_secs(secs) => std::thread::sleep(Duration::from_millis(10)),
            _ => { let _ = child.kill(); let _ = child.wait(); return None; }
        }
    }
}
fn gh(args: &[&str]) -> Option<String> { run("gh", args, 25) }
fn gh_json(args: &[&str]) -> Option<Value> { serde_json::from_str(&gh(args)?).ok() }
fn gh_ok() -> bool { run("gh", &["auth", "status"], 15).is_some() }

fn token(c: &Ctx) -> String {
    let t = c.cfg.get("CIRCLECI_TOKEN").filter(|v| !v.is_empty()).cloned()
        .or_else(|| std::env::var("CIRCLECI_TOKEN").ok().filter(|v| !v.is_empty()))
        .or_else(|| std::env::var("CIRCLE_TOKEN").ok()).unwrap_or_default();
    t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect()
}
fn circleci_get(c: &Ctx, path: &str) -> Option<Value> {
    let t = token(c); if t.is_empty() { return None; }
    if !path.chars().all(|x| x.is_ascii_alphanumeric() || "/_.?=&-".contains(x)) { return None; }
    if std::env::var("BHOTED_HTTP").map_or(false, |v| v == "curl") {   // the tests: the same fake curl the script's tests use (the token on stdin, never an argument)
        let url = format!("https://circleci.com/api/v2/{}", path);
        let mut ch = Command::new("curl").args(["-sf", "--max-time", "8", "-K", "-", &url]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        { use std::io::Write; ch.stdin.take()?.write_all(format!("header = \"Circle-Token: {}\"\n", t).as_bytes()).ok()?; }
        let o = ch.wait_with_output().ok()?; if !o.status.success() { return None; }
        return serde_json::from_slice(&o.stdout).ok();
    }
    let r = ureq::get(&format!("https://circleci.com/api/v2/{}", path)).set("Circle-Token", &t).timeout(Duration::from_secs(8)).call().ok()?;
    serde_json::from_reader(r.into_reader()).ok()
}

// ── small helpers of the bash script ───────────────────────────────────────────────────────────────────────
fn map_entries(m: &str) -> Vec<(String, Option<String>)> {
    m.split(',').filter(|e| !e.is_empty()).map(|e| match e.find('=') { Some(i) => (e[..i].to_string(), Some(e[i + 1..].to_string())), None => (e.to_string(), None) }).collect()
}
fn mon_label(name: &str, map: &str) -> String {
    for (k, v) in map_entries(map) { if k == name { return v.unwrap_or_else(|| name.to_string()); } }
    let mut n = name.to_string();
    for suf in [" Deployment", " deployment"] { if let Some(x) = n.strip_suffix(suf) { n = x.to_string(); break; } }
    if let Some(x) = n.strip_prefix("deploy-") { n = x.to_string(); }
    if let Some(x) = n.strip_prefix("Run ") { n = x.to_string(); }
    n
}
fn mon_match(name: &str, map: &str, word: &str) -> bool {
    if !map.is_empty() { return map_entries(map).iter().any(|(k, _)| k == name); }
    name.to_lowercase().contains(word)
}
fn mon_job_label(job: &str, map: &str) -> String {
    for (k, v) in map_entries(map) { if k == job { if let Some(v) = v { return v; } } }
    String::new()
}
fn mon_job_word(job: &str) -> String {
    match job.rfind('-') { Some(i) => { let w = &job[i + 1..]; let mut ch = w.chars(); match ch.next() { Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(), None => String::new() } } None => String::new() }
}
fn mon_shipped(t: &str) -> String {
    if let Some(r) = t.strip_prefix("Merge pull request #") {
        if let Some(i) = r.find(" from ") {
            let n = r[..r.find(' ').unwrap_or(r.len())].to_string();
            let b = r[i + 6..].split(' ').next().unwrap_or("");
            let b = b.split_once('/').map_or(b, |x| x.1);
            let _ = i; return format!("{} (#{})", b, n);
        }
    }
    if let Some(r) = t.strip_prefix("Merge branch '") { if let Some(i) = r.find('\'') { return r[..i].to_string(); } }
    t.to_string()
}

// ── GitHub Actions ──────────────────────────────────────────────────────────────────────────────────────────
fn deploys_github(proj: &str, slug: &str, branches: &str, names: &str) -> Option<Vec<String>> {
    let f = "databaseId,workflowName,status,conclusion,createdAt,headBranch,displayTitle,url";
    let nm: Vec<String> = map_entries(names).into_iter().map(|e| e.0).filter(|k| !k.is_empty()).collect();
    let brs: Vec<&str> = (if branches.is_empty() { "main" } else { branches }).split(',').filter(|b| !b.is_empty()).collect();
    let mut runs: BTreeMap<i64, Value> = BTreeMap::new(); let mut got = false;
    let mut add = |v: Value| { got = true; for r in v.as_array().cloned().unwrap_or_default() { if let Some(id) = r["databaseId"].as_i64() { runs.insert(id, r); } } };
    if !nm.is_empty() {
        for e in &nm { for b in &brs { if let Some(v) = gh_json(&["run", "list", "-R", slug, "--workflow", e, "--branch", b, "--limit", "5", "--json", f]) { add(v); } } }
    } else {
        for b in &brs { if let Some(v) = gh_json(&["run", "list", "-R", slug, "--branch", b, "--limit", "50", "--json", f]) { add(v); } }
    }
    if !got { return None; }
    let allowed = if branches.is_empty() { "main" } else { branches };
    let mut out = Vec::new();
    for (id, r) in runs {
        let br = s(&r["headBranch"]); if !allowed.split(',').any(|b| b == br) { continue; }
        let wf = s(&r["workflowName"]); if !mon_match(&wf, names, "deploy") { continue; }
        let (st, cc) = (s(&r["status"]), s(&r["conclusion"]));
        let state = match (st.as_str(), cc.as_str()) {
            ("completed", "success") => "ok", ("completed", "failure" | "timed_out" | "startup_failure") => "failed", ("completed", "cancelled") => "canceled",
            ("completed", _) => continue, ("waiting", _) => "approval", ("in_progress", _) => "running", _ => "queued",
        };
        let (mut d, mut tot) = (String::new(), String::new());
        if state == "running" {
            if let Some(v) = gh_json(&["run", "view", &id.to_string(), "-R", slug, "--json", "jobs"]) {
                let steps: Vec<&Value> = v["jobs"].as_array().map(|j| j.iter().flat_map(|x| x["steps"].as_array().into_iter().flatten()).collect()).unwrap_or_default();
                d = steps.iter().filter(|x| x["status"] == "completed").count().to_string(); tot = steps.len().to_string();
            }
        }
        let mut title = s(&r["displayTitle"]);
        if title.contains(" from ") && title.contains('/') { // `*" from "*/*`: after the last " from ", then after the first "/"
            let t = title.rsplit(" from ").next().unwrap_or("").to_string(); title = t.split_once('/').map_or(t.clone(), |x| x.1.to_string());
        }
        let Some(t) = iso(&s(&r["createdAt"])) else { continue };
        out.push(line(&[&t.to_string(), proj, &mon_label(&wf, names), state, &d, &tot, &title, &s(&r["url"])]));
    }
    Some(out)
}

fn tests_github(c: &Ctx, proj: &str, slug: &str, names: &str) -> Option<Vec<String>> {
    let runs = gh_json(&["run", "list", "-R", slug, "--limit", "8", "--json", "databaseId,workflowName,status,updatedAt,headBranch,url"])?;
    let dir = c.shared.join("ghjobs"); let _ = fs::create_dir_all(&dir);
    let strip = |n: &str| -> String { // " (3/4)" at the end
        if let Some(i) = n.rfind(" (") { let t = &n[i + 2..]; if let Some(t) = t.strip_suffix(')') { if let Some((a, b)) = t.split_once('/') { if !a.is_empty() && !b.is_empty() && a.bytes().all(|x| x.is_ascii_digit()) && b.bytes().all(|x| x.is_ascii_digit()) { return n[..i].to_string(); } } } }
        n.to_string()
    };
    let wanted: Vec<&str> = names.split(',').collect();
    let mut out = Vec::new();
    let rl: Vec<Value> = runs.as_array().cloned().unwrap_or_default();
    let files: Vec<PathBuf> = par_map(&rl, |r| {         // the jobs of each run, fetched side by side (a finished run is kept)
        let id = s(&r["databaseId"]); let up: String = s(&r["updatedAt"]).chars().filter(|c| c.is_ascii_digit()).collect();
        let jf = dir.join(format!("{}-{}", id, up));
        if fs::metadata(&jf).map_or(true, |m| m.len() == 0) {
            if let Some(o) = gh(&["run", "view", &id, "-R", slug, "--json", "jobs"]) { let tmp = jf.with_extension(format!("tmp{}", id)); if fs::write(&tmp, o).is_ok() { let _ = fs::rename(&tmp, &jf); } }
        }
        jf
    });
    for (r, jf) in rl.into_iter().zip(files) {
        let Ok(txt) = crate::proc::read_cap(&jf) else { continue }; let Ok(v) = serde_json::from_str::<Value>(&txt) else { continue };
        let jobs: Vec<&Value> = v["jobs"].as_array().into_iter().flatten().filter(|j| { let n = s(&j["name"]); if names.is_empty() { n.to_lowercase().contains("test") } else { wanted.contains(&strip(&n).as_str()) } }).collect();
        if jobs.is_empty() { continue; }
        let any = |f: &dyn Fn(&Value) -> bool| jobs.iter().any(|j| f(j));
        let state = if any(&|j| j["status"] != "completed") { "running" }
            else if any(&|j| j["conclusion"] == "failure" || j["conclusion"] == "timed_out") { "failed" }
            else if jobs.iter().all(|j| j["conclusion"] == "success" || j["conclusion"] == "skipped") { "ok" } else { "canceled" };
        let done = jobs.iter().filter(|j| j["status"] == "completed").count();
        let name = strip(&s(&jobs[0]["name"]));
        let start = jobs.iter().filter_map(|j| j["startedAt"].as_str()).min().map(|x| x.to_string());
        let Some(t) = start.and_then(|x| iso(&x)) else { continue };
        out.push(line(&[&t.to_string(), proj, &name, state, &done.to_string(), &jobs.len().to_string(), &format!("{}|~|", s(&r["headBranch"])), &s(&r["url"])]));
    }
    // old job files go after a day
    if let Ok(rd) = fs::read_dir(&dir) { for e in rd.flatten() { if e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map_or(false, |a| a > Duration::from_secs(86400)) { let _ = fs::remove_file(e.path()); } } }
    Some(out)
}

// ── CircleCI ────────────────────────────────────────────────────────────────────────────────────────────────
struct Wf { id: String, status: String, created: String, num: String, subj: String, branch: String }
fn circleci_workflows(c: &Ctx, slug: &str, branch: &str, k: usize) -> Vec<Wf> {
    let mut q = format!("project/{}/pipeline", slug); if !branch.is_empty() { q.push_str(&format!("?branch={}", branch)); }
    let mut out = Vec::new();
    let Some(p) = circleci_get(c, &q) else { return out };
    let items: Vec<&Value> = p["items"].as_array().into_iter().flatten().take(k).collect();
    let per = par_map(&items, |it| {
        let mut out = Vec::new();
        let pid = s(&it["id"]); if pid.is_empty() { return out; }
        let (num, subj, br) = (s(&it["number"]), cc_ctrl(&s(&it["vcs"]["commit"]["subject"])), s(&it["vcs"]["branch"]));
        let Some(w) = circleci_get(c, &format!("pipeline/{}/workflow", pid)) else { return out };
        let mut by: BTreeMap<String, Value> = BTreeMap::new();   // group_by(.name) | max_by(.created_at)
        for x in w["items"].as_array().into_iter().flatten() {
            let n = s(&x["name"]);
            if by.get(&n).map_or(true, |o| s(&x["created_at"]) >= s(&o["created_at"])) { by.insert(n, x.clone()); }
        }
        for (_, x) in by { out.push(Wf { id: s(&x["id"]), status: s(&x["status"]), created: s(&x["created_at"]), num: num.clone(), subj: subj.clone(), branch: br.clone() }); }
        out
    });
    for v in per { out.extend(v); }
    out
}
fn circleci_jobs(c: &Ctx, wid: &str, status: &str) -> Option<Value> {
    let dir = c.shared.join("ccjobs"); let f = dir.join(wid);
    let fin = matches!(status, "success" | "failed" | "error" | "canceled");
    if fin { if let Ok(t) = crate::proc::read_cap(&f) { if !t.is_empty() { if let Ok(v) = serde_json::from_str(&t) { return Some(v); } } } }
    let j = circleci_get(c, &format!("workflow/{}/job", wid))?;
    if fin {
        let _ = fs::create_dir_all(&dir); let tmp = dir.join(format!("{}.tmp", wid));
        if fs::write(&tmp, j.to_string()).is_ok() { let _ = fs::rename(&tmp, &f); }
        if let Ok(rd) = fs::read_dir(&dir) { for e in rd.flatten() { if e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map_or(false, |a| a > Duration::from_secs(2880 * 60)) { let _ = fs::remove_file(e.path()); } } }
    }
    Some(j)
}
fn circleci_state(st: &str) -> Option<&'static str> {
    Some(match st { "success" => "ok", "failed" | "error" | "unauthorized" => "failed", "failing" | "running" => "running", "on_hold" => "approval", "canceled" => "canceled", _ => return None })
}
const TERM_DEPLOY: [&str; 5] = ["success", "failed", "canceled", "infrastructure_fail", "timedout"];
const TERM_ALL: [&str; 9] = ["success", "failed", "canceled", "not_run", "infrastructure_fail", "timedout", "unauthorized", "terminated-unknown", "retried"];

fn deploys_circleci(c: &Ctx, proj: &str, slug: &str, branches: &str, names: &str) -> Vec<String> {
    let map = if branches.is_empty() { "=" } else { branches };
    let entries: Vec<&str> = map.split(',').filter(|e| !e.is_empty()).collect();
    let per = par_map(&entries, |e| {
        let mut out = Vec::new();
        let b = e.split('=').next().unwrap_or(""); let mut lab = e.find('=').map_or(*e, |i| &e[i + 1..]).to_string();
        if lab == b { lab.clear(); }
        let wfs = circleci_workflows(c, slug, b, 4);
        let jobs = par_map(&wfs, |w| circleci_state(&w.status).and_then(|_| circleci_jobs(c, &w.id, &w.status)));
        for (w, jobs) in wfs.iter().zip(jobs) {
            let Some(state) = circleci_state(&w.status) else { continue };
            let Some(jobs) = jobs else { continue };
            let items: Vec<&Value> = jobs["items"].as_array().into_iter().flatten().collect();
            let all: Vec<String> = items.iter().map(|j| s(&j["name"])).collect();
            let dj: Option<String> = if names.is_empty() {
                all.iter().find(|n| n.to_lowercase().contains("deploy")).or_else(|| all.iter().find(|n| n.to_lowercase().contains("approv"))).cloned()
            } else { map_entries(names).into_iter().map(|e| e.0).find(|k| all.contains(k)) };
            let Some(dj) = dj else { continue };
            let djst = items.iter().find(|j| s(&j["name"]) == dj).map(|j| s(&j["status"])).unwrap_or_default();
            if djst == "not_run" { continue; }
            let mut l = mon_job_label(&dj, names); if l.is_empty() { l = lab.clone(); }
            if l.is_empty() { l = mon_job_word(&dj); } if l.is_empty() { l = w.branch.clone(); }
            let part: Vec<&&Value> = items.iter().filter(|j| !s(&j["name"]).to_lowercase().starts_with("notify") && s(&j["status"]) != "not_run").collect();
            let d = part.iter().filter(|j| TERM_DEPLOY.contains(&s(&j["status"]).as_str())).count();
            let Some(t) = iso(&w.created) else { continue };
            let shipped: String = mon_shipped(&w.subj).chars().take(70).collect();
            out.push(line(&[&t.to_string(), proj, &l, state, &d.to_string(), &part.len().to_string(), &shipped, &format!("https://app.circleci.com/pipelines/{}/{}/workflows/{}", slug, w.num, w.id)]));
        }
        out
    });
    per.into_iter().flatten().collect()
}
fn tests_circleci(c: &Ctx, proj: &str, slug: &str, names: &str) -> Vec<String> {
    let wanted: Vec<&str> = names.split(',').collect(); let mut out = Vec::new();
    let wfs = circleci_workflows(c, slug, "", 4);
    let all_jobs = par_map(&wfs, |w| circleci_get(c, &format!("workflow/{}/job", w.id)));
    for (w, jobs) in wfs.into_iter().zip(all_jobs) {
        let Some(jobs) = jobs else { continue };
        let sel: Vec<&Value> = jobs["items"].as_array().into_iter().flatten().filter(|j| { let n = s(&j["name"]); if names.is_empty() { let l = n.to_lowercase(); l.contains("test") || l.contains("build") } else { wanted.contains(&n.as_str()) } }).collect();
        if sel.is_empty() { continue; }
        let term = |j: &&Value| TERM_ALL.contains(&s(&j["status"]).as_str());
        let st = |j: &&Value| s(&j["status"]);
        let state = if sel.iter().any(|j| !term(j)) { "running" } else if sel.iter().any(|j| matches!(st(j).as_str(), "failed" | "infrastructure_fail" | "timedout")) { "failed" } else if sel.iter().all(|j| st(j) == "success") { "ok" } else { "canceled" };
        let Some(t) = iso(&w.created) else { continue };
        out.push(line(&[&t.to_string(), proj, &s(&sel[0]["name"]), state, &sel.iter().filter(|j| term(j)).count().to_string(), &sel.len().to_string(), &format!("{}|~|", w.branch), &format!("https://app.circleci.com/pipelines/{}/{}/workflows/{}", slug, w.num, w.id)]));
    }
    out
}

// ── local runs and live status files ───────────────────────────────────────────────────────────────────────
extern "C" { fn kill(pid: i32, sig: i32) -> i32; }
/// worktree folder name -> the branch it is on now, from the folders of the agents (each folder asked once per round; the first agent of a worktree counts)
fn tree_branches(c: &Ctx) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(t) = crate::proc::read_cap(c.shared.join("agents.cwd")) else { return map };
    let folders: Vec<&str> = t.lines().filter_map(|l| l.split('\u{1f}').nth(1)).filter(|f| !f.is_empty() && Path::new(f).is_dir()).collect();
    let found = par_map(&folders, |f| {
        let top = run("git", &["-C", f, "rev-parse", "--show-toplevel"], 10)?; let top = top.trim().to_string();
        let br = run("git", &["-C", &top, "rev-parse", "--abbrev-ref", "HEAD"], 10).unwrap_or_default().trim().to_string();
        Some((Path::new(&top).file_name()?.to_string_lossy().into_owned(), br))
    });
    for (n, b) in found.into_iter().flatten() { map.entry(n).or_insert(b); }
    map
}
fn tests_local(c: &Ctx) -> Vec<String> {
    let mut out = Vec::new();
    let mut runs: Vec<PathBuf> = fs::read_dir(c.data.join("testruns")).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "run")).collect()).unwrap_or_default();
    runs.sort();
    for f in runs {
        let txt = crate::proc::read_cap(&f).unwrap_or_default(); let mut m: HashMap<&str, &str> = HashMap::new();
        for l in txt.lines() { if let Some(i) = l.find('=') { m.insert(&l[..i], &l[i + 1..]); } }   // the last value counts (the script's read loop)
        let g = |k: &str| m.get(k).copied().unwrap_or("");
        let state = if !g("ended").is_empty() { if g("exit") == "0" { "ok" } else { "failed" } }
            else if !g("pid").is_empty() && g("pid").parse::<i32>().map_or(true, |p| unsafe { kill(p, 0) } != 0) { "canceled" } else { "running" };
        let pj = if g("project").is_empty() { g("tree") } else { g("project") };
        out.push(line(&[g("started"), pj, g("name"), state, "", "", &format!("{}|~|{}", g("branch"), g("summary")), ""]));
    }
    let dir = c.get("TEST_STATUS_DIR", "~/.cache/bhote/test-status");
    let dir = if let Some(r) = dir.strip_prefix('~') { format!("{}{}", std::env::var("HOME").unwrap_or_default(), r) } else { dir };
    let mut files: Vec<PathBuf> = fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect()).unwrap_or_default();
    files.sort();
    let tnow = now() as f64;
    let branches = tree_branches(c);
    for f in files {
        let Ok(txt) = crate::proc::read_cap(&f) else { continue }; let Ok(v) = serde_json::from_str::<Value>(&txt) else { continue };
        let truthy = |k: &str| !matches!(v.get(k), None | Some(Value::Null) | Some(Value::Bool(false)));
        if !(truthy("tree") && truthy("suite") && truthy("startedAt")) { continue; }
        let num = |k: &str| v.get(k).and_then(|x| x.as_f64());
        let started = num("startedAt").unwrap_or(0.0);
        let up = (num("updatedAt").unwrap_or(started) / 1000.0).floor();
        let state = match v.get("ok") { Some(Value::Bool(true)) => "ok", Some(Value::Bool(false)) => "failed", _ => if tnow - up > 30.0 { "canceled" } else { "running" } };
        let cc = |x: &Value| -> String { let t = s(x); t.chars().map(|c| if (c as u32) < 0x20 || c as u32 == 0x7f { ' ' } else { c }).collect() };
        let n = |k: &str| -> String { let t = s(or_null(&v, k)); if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) { t } else { String::new() } };
        let failed = num("failed").unwrap_or(0.0);
        let passed = v.get("passed").filter(|x| !x.is_null()).map(|x| s(x)).unwrap_or_else(|| "0".into());
        let note = cc(&Value::String(format!("{}{} passed", if failed > 0.0 { format!("{} failed · ", failed) } else { String::new() }, passed)));
        let tree = cc(&v["tree"]); let bn = branches.get(&tree).cloned().unwrap_or_default();
        let startedat = (started / 1000.0).floor() as u64;
        out.push(format!("{}", [startedat.to_string(), tree, cc(&v["suite"]), state.to_string(), n("done"), n("total"), format!("{}|~|{}", bn, note), String::new()].join(US)));
    }
    out
}

// ── one round ───────────────────────────────────────────────────────────────────────────────────────────────
struct Project { name: String, file: PathBuf }
fn projects(c: &Ctx) -> Vec<Project> {
    let mut v = Vec::new();
    let Ok(rd) = fs::read_dir(&c.topics) else { return v };
    for e in rd.flatten() {
        let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
        if !(n.starts_with("proj-") && n.ends_with(".topic")) { continue; }
        let txt = crate::proc::read_cap(&p).unwrap_or_default();
        let (mut name, mut del, mut kd) = (String::new(), "0".to_string(), String::new());
        for l in txt.lines() { if let Some(i) = l.find('=') { let (k, val) = (&l[..i], &l[i + 1..]); match k { "title" if name.is_empty() => name = val.to_string(), "deleted" => del = val.to_string(), "kind" => kd = val.to_string(), _ => {} } } }
        if kd == "project" && del != "1" && !name.is_empty() { v.push(Project { name: name.chars().filter(|c| !c.is_control()).collect(), file: p }); }
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}
fn first(file: &Path, key: &str) -> String {
    for l in crate::proc::read_cap(file).unwrap_or_default().lines() { if let Some(i) = l.find('=') { if &l[..i] == key { return l[i + 1..].to_string(); } } }
    String::new()
}

/// monitor_fetch deploys|tests
pub fn fetch(c: &Ctx, kind: &str) {
    let mut lines: Vec<String> = Vec::new();
    let ghok = std::sync::OnceLock::new();
    let ghok = || *ghok.get_or_init(gh_ok);
    let ps = projects(c);
    let per = par_map(&ps, |p| {
        let ci = first(&p.file, "ci"); if ci.is_empty() { return Vec::new(); }
        let (br, nm, tn) = (first(&p.file, "deploy_branches"), first(&p.file, "deploy_names"), first(&p.file, "test_names"));
        match (kind, ci.split_once(':')) {
            ("deploys", Some(("github", slug))) => if c.on("DEPLOY_GITHUB", "on") && ghok() { deploys_github(&p.name, slug, &br, &nm).unwrap_or_default() } else { Vec::new() },
            ("deploys", Some(("circleci", slug))) => if c.on("DEPLOY_CIRCLECI", "on") { deploys_circleci(c, &p.name, slug, &br, &nm) } else { Vec::new() },
            ("tests", Some(("github", slug))) => if c.on("TEST_GITHUB", "on") && ghok() { tests_github(c, &p.name, slug, &tn).unwrap_or_default() } else { Vec::new() },
            ("tests", Some(("circleci", slug))) => if c.on("TEST_CIRCLECI", "on") { tests_circleci(c, &p.name, slug, &tn) } else { Vec::new() },
            _ => Vec::new(),
        }
    });
    for l in per { lines.extend(l); }
    if kind == "tests" && c.on("TEST_LOCAL", "on") { lines.extend(tests_local(c)); }
    // grep -E "^[0-9]+US", sort -t US -k1,1nr (a tie: the whole line, bytes), head -50
    lines.retain(|l| { let t = l.split(US).next().unwrap_or(""); !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) && l.contains(US) });
    lines.sort_by(|a, b| { let ka = a.split(US).next().unwrap().parse::<u128>().unwrap_or(0); let kb = b.split(US).next().unwrap().parse::<u128>().unwrap_or(0); kb.cmp(&ka).then_with(|| a.as_bytes().cmp(b.as_bytes())) });
    lines.truncate(50);
    let list = c.shared.join(format!("{}.list", kind));
    let had = fs::metadata(&list).map_or(false, |m| m.len() > 0);
    if !lines.is_empty() || !had {
        let tmp = c.shared.join(format!("{}.tmp.{}", kind, std::process::id()));
        let body: String = lines.iter().map(|l| format!("{}\n", l)).collect();
        if fs::write(&tmp, body).is_ok() { let _ = fs::rename(&tmp, &list); }
    }
    let _ = fs::write(c.shared.join(format!("{}.at", kind)), format!("{}\n", now()));
    let _ = fs::write(c.shared.join("poke"), "");
}
