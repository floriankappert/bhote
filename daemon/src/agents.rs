//! The local agents (step 2): what `collect_local` / `collect_one` in the bash script write, written here from herdr's
//! `pane list` and `workspace list`: agents.local, agents.meta.local, agents.now.local, agents.cwd, focus.pane, agents.rev.
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const US: char = '\u{1f}';

/// jq's `gsub("[\u0001-\u001f\u007f-\u009f]"; "")`
fn clean(s: &str) -> String { s.chars().filter(|c| !matches!(*c as u32, 1..=0x1f | 0x7f..=0x9f)).collect() }
/// jq's `gsub("[\u0000-\u001f]"; "")`
fn clean_lo(s: &str) -> String { s.chars().filter(|c| (*c as u32) > 0x1f).collect() }

/// jq's `tostring` of a value (a string stays as it is, null is "null").
fn tostr(v: &Value) -> String { match v { Value::String(s) => s.clone(), other => other.to_string() } }
/// `(x // "")`: null and false give the default.
fn or_empty(v: Option<&Value>) -> Value { match v { None | Some(Value::Null) | Some(Value::Bool(false)) => Value::String(String::new()), Some(x) => x.clone() } }
fn truthy(v: Option<&Value>) -> bool { !matches!(v, None | Some(Value::Null) | Some(Value::Bool(false))) }

fn herdr(args: &[&str], secs: u64) -> Option<Value> {
    let mut child = Command::new("herdr").args(args).stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || { let mut s = String::new(); let _ = out.read_to_string(&mut s); s });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => { let s = reader.join().ok()?; return if st.success() { serde_json::from_str(&s).ok() } else { None }; }
            Ok(None) if t0.elapsed() < Duration::from_secs(secs) => std::thread::sleep(Duration::from_millis(5)),
            _ => { let _ = child.kill(); let _ = child.wait(); return None; }
        }
    }
}

pub struct Out { pub local: String, pub meta: String, pub now: String, pub cwd: String, pub focus: String }

/// The five files of one collection of this machine, from the two herdr answers.
pub fn build(host: &str, panes: &Value, workspaces: &Value, tests_busy: bool) -> Out {
    let empty = Vec::new();
    let pl = panes.pointer("/result/panes").and_then(|v| v.as_array()).unwrap_or(&empty);
    let wl = workspaces.pointer("/result/workspaces").and_then(|v| v.as_array()).unwrap_or(&empty);
    let mut names = std::collections::HashMap::new();
    let mut busy = std::collections::HashMap::new();
    for w in wl {
        let id = tostr(&or_empty(w.get("workspace_id")));
        names.insert(id.clone(), clean(&tostr(w.get("label").unwrap_or(&Value::Null))));
        let tests = tostr(&or_empty(w.pointer("/tokens/tests")));
        busy.insert(id, tests.starts_with('\u{25cc}'));
    }
    let (mut local, mut meta, mut now, mut cwd) = (String::new(), String::new(), String::new(), String::new());
    let mut focus = String::new();
    for p in pl {
        let id = tostr(p.get("pane_id").unwrap_or(&Value::Null));
        let has_agent = truthy(p.get("agent"));
        let tab = tostr(&or_empty(p.get("tab_id")));
        let folder = tostr(&if truthy(p.get("foreground_cwd")) { p["foreground_cwd"].clone() } else { or_empty(p.get("cwd")) });
        // meta: every pane
        let join = |parts: &[String]| parts.iter().map(|s| clean_lo(s)).collect::<Vec<_>>().join(&US.to_string());
        meta.push_str(&join(&[host.to_string(), id.clone(), tab.clone(), if has_agent { "a".into() } else { String::new() }, tostr(&or_empty(p.pointer("/tokens/repo"))), tostr(&or_empty(p.pointer("/tokens/project"))), folder.clone()]));
        meta.push('\n');
        if focus.is_empty() && p.get("focused") == Some(&Value::Bool(true)) { focus = format!("{} {}", id, tab); }
        if !has_agent { continue; }
        let tnow = or_empty(p.pointer("/tokens/now"));
        if tnow != Value::String(String::new()) { now.push_str(&join(&[host.to_string(), id.clone(), tostr(&tnow)])); now.push('\n'); }
        cwd.push_str(&join(&[id.clone(), folder.clone()])); cwd.push('\n');
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '_' || c == '-') { continue; }
        let wid = p.get("workspace_id");
        let status_raw = tostr(&or_empty(p.get("agent_status")));
        let ws_busy = wid.map_or(false, |w| *busy.get(&tostr(w)).unwrap_or(&false));
        let status = if tests_busy && ws_busy && status_raw != "working" { "testing".to_string() } else { clean(&status_raw) };
        let wname = match wid.and_then(|w| names.get(&tostr(w))) {
            Some(n) => n.clone(),
            None => clean(&tostr(&or_empty(wid))),
        };
        let task = clean(&tostr(&or_empty(p.pointer("/tokens/task"))));
        let task = task.strip_prefix("\u{221f} ").unwrap_or(&task).to_string();
        local.push_str(&[clean(host), id, status, wname, task].join(&US.to_string()));
        local.push('\n');
    }
    Out { local, meta, now, cwd, focus: format!("{}\n", focus) }
}

fn put_tmp(path: &Path, text: &str) -> bool {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, text).is_ok() && fs::rename(&tmp, path).is_ok()
}

/// agents_put: only a changed list replaces the old one and counts the revision up; an unchanged one is only touched.
fn put_list(path: &Path, text: &str) -> bool {
    if fs::read_to_string(path).map_or(false, |old| old == text) {
        if let Ok(f) = fs::OpenOptions::new().write(true).open(path) { let _ = f.set_modified(std::time::SystemTime::now()); }
        return false;
    }
    put_tmp(path, text)
}

/// One collection of this machine: asks herdr, writes the files. false when herdr did not answer (the old files stay).
pub fn collect_local(shared: &Path, host: &str, tests_busy: bool) -> bool {
    let Some(panes) = herdr(&["pane", "list"], 20) else { return false };
    let ws = herdr(&["workspace", "list"], 20).unwrap_or(Value::Null);
    let o = build(host, &panes, &ws, tests_busy);
    let _ = put_tmp(&shared.join("agents.meta.local"), &o.meta);
    let a = put_list(&shared.join("agents.now.local"), &o.now);
    let b = put_list(&shared.join("agents.local"), &o.local);
    let _ = put_tmp(&shared.join("agents.cwd"), &o.cwd);
    let _ = put_tmp(&shared.join("focus.pane"), &o.focus);
    if a || b {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = fs::write(shared.join("agents.rev"), format!("{} {}{}\n", t, std::process::id(), t % 32768));
    }
    true
}

// ── the other machines through kharka (step 2b): collect_remote_kharka ───────────────────────────────────────

fn kharka_ls(bin: &str, prefix: &str) -> Option<Vec<Value>> {
    let mut child = Command::new(bin).args(["ls", prefix, "--entries"]).stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || { let mut b = Vec::new(); let _ = out.read_to_end(&mut b); String::from_utf8_lossy(&b).into_owned() });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => { let s = reader.join().ok()?; if !st.success() { return None; } return Some(s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()); }
            Ok(None) if t0.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(5)),
            _ => { let _ = child.kill(); let _ = child.wait(); return None; }
        }
    }
}

pub struct Remote { pub nodes: String, pub offline: String, pub meta: String, pub now: String, pub agents: String }

fn ctl_strip(s: &str) -> String { clean(s) }

/// The five outputs from the herdr/<node> and _nodes/ entries. `me` is this machine's node, `host` its label.
pub fn build_remote(h: &[Value], n: &[Value], me: &str, host: &str, off_list: &str, tests_busy: bool, now_s: f64) -> Remote {
    let mut on = std::collections::HashMap::new();
    for e in n { if let Some(k) = e["key"].as_str() { on.insert(k.trim_start_matches("_nodes/").to_string(), !matches!(e.pointer("/value/online"), Some(Value::Bool(false)))); } }
    let off = format!(",{},", off_list);
    let (mut nodes, mut offline, mut meta, mut now, mut agents) = (String::new(), String::new(), String::new(), String::new(), String::new());
    let us = US.to_string();
    for e in h {
        let key = e["key"].as_str().unwrap_or("");
        let v = &e["value"]; if !v.is_object() { continue; }
        let Some(node) = key.strip_prefix("herdr/") else { continue };
        if node.is_empty() || !node.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') { continue; }
        let lbl: String = tostr(&or_empty(v.get("label"))).chars().filter(|c| c.is_ascii_alphanumeric() || " ._-".contains(*c)).take(40).collect();
        let m = if lbl.is_empty() { node.to_string() } else { lbl };
        if node == me || m == host || off.contains(&format!(",{},", m)) { continue; }
        let stamp: f64 = e["hlc"].as_str().unwrap_or("0").split('-').next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let down = tostr(&or_empty(v.get("herdr"))) != "online" || on.get(node) == Some(&false) || now_s - stamp / 1000.0 > 60.0;
        let mut ws: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
        for w in v.get("workspaces").and_then(|x| x.as_array()).into_iter().flatten() { if w.is_object() { ws.insert(tostr(w.get("id").unwrap_or(&Value::Null)), w); } }
        let cmds: Vec<String> = v.get("commands").and_then(|x| x.as_array()).into_iter().flatten().map(|c| tostr(c).chars().filter(|c| c.is_ascii_lowercase() || *c == '.' || *c == '_').collect()).collect();
        nodes.push_str(&[m.clone(), node.to_string(), (if down { "1" } else { "0" }).to_string(), cmds.join(",")].join(&us)); nodes.push('\n');
        if down { offline.push_str(&m); offline.push('\n'); }
        for a in v.get("agents").and_then(|x| x.as_array()).into_iter().flatten() {
            if !a.is_object() { continue; }
            let pane = tostr(&or_empty(a.get("pane")));
            if pane.is_empty() || !pane.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '_' || c == '-') { continue; }
            let wk = tostr(&or_empty(a.get("workspace")));
            let w = ws.get(&wk);
            let mut st = ctl_strip(&tostr(&or_empty(a.get("status"))));
            let wtests = w.map(|w| tostr(&or_empty(w.pointer("/tokens/tests")))).unwrap_or_default();
            if tests_busy && wtests.starts_with('\u{25cc}') && st != "working" { st = "testing".into(); }
            let st_out = if down { format!("off-{}", st) } else { st };
            let wname = ctl_strip(&tostr(&match w.and_then(|w| w.get("label")).filter(|l| !matches!(l, Value::Null | Value::Bool(false))) { Some(l) => l.clone(), None => or_empty(a.get("workspace")) }));
            let task = ctl_strip(&tostr(&or_empty(a.pointer("/tokens/task"))));
            let task = task.strip_prefix("\u{221f} ").unwrap_or(&task).to_string();
            agents.push_str(&[m.clone(), pane.clone(), st_out, wname, task].join(&us)); agents.push('\n');
            meta.push_str(&[m.clone(), pane.clone(), ctl_strip(&tostr(&or_empty(a.get("tab")))), "a".into(), ctl_strip(&tostr(&or_empty(a.pointer("/tokens/repo")))), ctl_strip(&tostr(&or_empty(a.pointer("/tokens/project")))), ctl_strip(&tostr(&or_empty(a.get("cwd"))))].join(&us)); meta.push('\n');
            let tn = tostr(&or_empty(a.pointer("/tokens/now")));
            if !tn.is_empty() { now.push_str(&[m.clone(), pane.clone(), ctl_strip(&tn)].join(&us)); now.push('\n'); }
        }
    }
    Remote { nodes, offline, meta, now, agents }
}

fn kharka_self(shared: &Path, bin: &str) -> String {
    let f = shared.join("kharka.self");
    let fresh = fs::metadata(&f).ok().and_then(|m| m.modified().ok()).and_then(|m| m.elapsed().ok()).map_or(false, |a| a < Duration::from_secs(600));
    if let Ok(t) = fs::read_to_string(&f) { let t = t.trim().to_string(); if !t.is_empty() && fresh { return t; } }
    let o = Command::new(bin).arg("whoami").stdin(Stdio::null()).stderr(Stdio::null()).output().ok().filter(|o| o.status.success());
    let n: String = o.map(|o| String::from_utf8_lossy(&o.stdout).trim().chars().filter(|c| c.is_ascii_alphanumeric() || ".-_".contains(*c)).collect()).unwrap_or_default();
    if !n.is_empty() { let _ = fs::write(&f, format!("{}\n", n)); }
    n
}

/// collect_remote_kharka: false when the daemon of kharka did not answer (the machines keep their last agents, muted).
pub fn collect_remote_kharka(shared: &Path, host: &str, bin: &str, off_list: &str, tests_busy: bool) -> bool {
    let me = kharka_self(shared, bin);
    let (h, n) = (kharka_ls(bin, "herdr/"), kharka_ls(bin, "_nodes/"));
    let remote = shared.join("agents.remote");
    let (Some(h), Some(n)) = (h, n) else {
        if let Ok(t) = fs::read_to_string(shared.join("agents.nodes")) {
            let off: String = t.lines().filter_map(|l| l.split('\u{1f}').next()).map(|l| format!("{}\n", l)).collect(); let _ = put_tmp(&shared.join("machines.offline"), &off);
        }
        if let Ok(old) = fs::read_to_string(&remote) {
            let body: String = old.lines().map(|l| { let mut p: Vec<String> = l.split('\u{1f}').map(|x| x.to_string()).collect(); if p.len() >= 3 && !p[2].starts_with("off-") { p[2] = format!("off-{}", p[2]); } format!("{}\n", p.join(&US.to_string())) }).collect();
            put_list(&remote, &body);
        }
        return false;
    };
    let now_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    let r = build_remote(&h, &n, &me, host, off_list, tests_busy, now_s);
    let _ = put_tmp(&shared.join("agents.nodes"), &r.nodes);
    let _ = put_tmp(&shared.join("machines.offline"), &r.offline);
    let _ = put_tmp(&shared.join("agents.meta.remote"), &r.meta);
    let a = put_list(&shared.join("agents.now.remote"), &r.now);
    let b = put_list(&remote, &r.agents);
    if a || b {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = fs::write(shared.join("agents.rev"), format!("{} {}{}\n", t, std::process::id(), t % 32768));
    }
    true
}
