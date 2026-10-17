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
