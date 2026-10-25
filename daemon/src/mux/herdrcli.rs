//! `herdr ...` as the mux speaks it: the same words and the same JSON as herdr's command line, so that the scripts of bhote
//! (and the Rust code that runs `herdr`) work unchanged inside a native pane (the server puts a `herdr` link to this program in front of the PATH).
use super::client::call;
use serde_json::{json, Map, Value};

const VALUE_OPTS: &[&str] = &["pane", "direction", "ratio", "cwd", "env", "label", "source", "lines", "amount", "source-pane", "target-pane", "workspace", "tab", "token", "clear-token", "machine", "body", "sound", "plugin", "timeout", "state", "match", "title", "command", "agent", "agent-session-id", "agent-session-path", "message", "seq", "session-start-source"];

struct Args { pos: Vec<String>, opt: Vec<(String, String)>, flags: Vec<String> }

impl Args {
    fn parse(a: &[String]) -> Args {
        let (mut pos, mut opt, mut flags) = (Vec::new(), Vec::new(), Vec::new());
        let mut i = 0;
        while i < a.len() {
            if let Some(name) = a[i].strip_prefix("--") {
                if let Some((k, v)) = name.split_once('=') { opt.push((k.to_string(), v.to_string())); }
                else if VALUE_OPTS.contains(&name) && i + 1 < a.len() { opt.push((name.to_string(), a[i + 1].clone())); i += 1; }
                else { flags.push(name.to_string()); }
            } else { pos.push(a[i].clone()); }
            i += 1;
        }
        Args { pos, opt, flags }
    }
    fn get(&self, k: &str) -> Option<&str> { self.opt.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v.as_str()) }
    fn all(&self, k: &str) -> Vec<&str> { self.opt.iter().filter(|(n, _)| n == k).map(|(_, v)| v.as_str()).collect() }
    fn flag(&self, k: &str) -> bool { self.flags.iter().any(|f| f == k) }
}

fn env_map(a: &Args) -> Value {
    let mut m = Map::new();
    for kv in a.all("env") { if let Some((k, v)) = kv.split_once('=') { m.insert(k.to_string(), json!(v)); } }
    Value::Object(m)
}

/// The pane a command means: `--pane`, the first word, `--current` (and by default) the pane this runs in.
fn pane_param(a: &Args, positional: bool) -> Option<String> {
    a.get("pane").map(String::from)
        .or_else(|| if positional { a.pos.get(2).cloned() } else { None })
        .or_else(|| std::env::var("HERDR_PANE_ID").ok().filter(|s| !s.is_empty() && (a.flag("current") || a.get("pane").is_none())))
}

/// The command: its exit code, what it prints, what it says on stderr.
pub fn exec(args: &[String]) -> (i32, String, String) {
    let (mut so, mut se) = (String::new(), String::new());
    macro_rules! out { ($($a:tt)*) => { { so.push_str(&format!($($a)*)); so.push('\n'); } } }
    macro_rules! err { ($($a:tt)*) => { { se.push_str(&format!($($a)*)); se.push('\n'); } } }
    // `--machine X` reaches another machine through kharka (M8); here only this one
    // after a lone `--` come the words that start the agent again (report-agent)
    let (args, resume): (&[String], Vec<String>) = match args.iter().position(|x| x == "--") { Some(i) => (&args[..i], args[i + 1..].to_vec()), None => (args, Vec::new()) };
    let a = Args::parse(args);
    if a.get("machine").is_some() { err!("herdr: --machine is not supported by the native mux"); return (1, so, se); }
    let noun = a.pos.first().map(|s| s.as_str()).unwrap_or("");
    let verb = a.pos.get(1).map(|s| s.as_str()).unwrap_or("");
    if noun == "--version" || a.flag("version") || noun == "version" { out!("bhote-mux {}", env!("CARGO_PKG_VERSION")); return (0, so, se); }
    let pane = |positional: bool| pane_param(&a, positional);
    let rest = |from: usize| a.pos.iter().skip(from).cloned().collect::<Vec<_>>();
    let mut p = Map::new();
    let mut put = |k: &str, v: Value| { p.insert(k.to_string(), v); };
    let method: String;
    let mut raw_text = false;
    match (noun, verb) {
        ("machine", "list") => return (0, so, se),
        ("server", "reload-config") | ("config", "check") => { out!("{}", json!({"id": "cli:server:reload-config", "result": {"status": "applied", "diagnostics": []}})); return (0, so, se); }
        ("notification", "show") => method = "notification.show".into(),
        ("api", "snapshot") => method = "api.snapshot".into(),
        ("pane", "list") => { method = "pane.list".into(); if let Some(w) = a.get("workspace") { put("workspace_id", json!(w)); } if let Some(t) = a.get("tab") { put("tab_id", json!(t)); } }
        ("pane", "get") => { method = "pane.get".into(); put("pane_id", json!(pane(true))); }
        ("pane", "current") => { method = "pane.get".into(); put("pane_id", json!(pane(false))); }
        ("pane", "layout") => { method = "pane.layout".into(); if let Some(x) = pane(true).filter(|_| a.get("tab").is_none()) { put("pane_id", json!(x)); } if let Some(t) = a.get("tab") { put("tab_id", json!(t)); } }
        ("pane", "process-info") => { method = "pane.process_info".into(); put("pane_id", json!(pane(true))); }
        ("pane", "split") => {
            method = "pane.split".into(); put("pane_id", json!(pane(true)));
            put("direction", json!(a.get("direction").unwrap_or("right")));
            if let Some(r) = a.get("ratio").and_then(|r| r.parse::<f64>().ok()) { put("ratio", json!(r)); }
            if let Some(c) = a.get("cwd") { put("cwd", json!(c)); }
            put("env", env_map(&a)); put("focus", json!(!a.flag("no-focus")));
        }
        ("pane", "swap") => {
            method = "pane.swap".into();
            if let Some(s) = a.get("source-pane") { put("source_pane", json!(s)); }
            if let Some(t) = a.get("target-pane") { put("target_pane", json!(t)); }
            if a.get("source-pane").is_none() { put("pane_id", json!(pane(false))); }
            if let Some(d) = a.get("direction") { put("direction", json!(d)); }
        }
        ("pane", "resize") => { method = "pane.resize".into(); put("pane_id", json!(pane(false))); put("direction", json!(a.get("direction"))); if let Some(x) = a.get("amount").and_then(|x| x.parse::<f64>().ok()) { put("amount", json!(x)); } }
        ("pane", "rename") => { method = "pane.rename".into(); put("pane_id", json!(pane(true))); put("label", json!(rest(3).join(" "))); }
        ("pane", "run") => { method = "pane.run".into(); put("pane_id", json!(pane(true))); put("command", json!(rest(3).join(" "))); }
        ("pane", "read") => { method = "pane.read".into(); raw_text = true; put("pane_id", json!(pane(true))); if let Some(s) = a.get("source") { put("source", json!(s)); } if let Some(l) = a.get("lines").and_then(|l| l.parse::<u64>().ok()) { put("lines", json!(l)); } }
        ("pane", "close") => { method = "pane.close".into(); put("pane_id", json!(pane(true))); }
        ("pane", "send-text") => { method = "pane.send_text".into(); put("pane_id", json!(pane(true))); put("text", json!(rest(3).join(" "))); }
        ("pane", "send-keys") => { method = "pane.send_keys".into(); put("pane_id", json!(pane(true))); put("keys", json!(rest(3))); }
        ("pane", "focus") => { method = "pane.focus".into(); put("pane_id", json!(pane(false))); if let Some(d) = a.get("direction") { put("direction", json!(d)); } }
        ("pane", "zoom") => { method = "pane.zoom".into(); put("pane_id", json!(pane(false))); }
        ("pane", "report-metadata") => {
            method = "pane.report_metadata".into(); put("pane_id", json!(pane(true)));
            let mut t = Map::new();
            for kv in a.all("token") { if let Some((k, v)) = kv.split_once('=') { t.insert(k.to_string(), json!(v)); } }
            put("tokens", Value::Object(t)); put("clear_tokens", json!(a.all("clear-token")));
        }
        ("pane", "report-agent") | ("pane", "report-agent-session") => {
            method = if verb == "report-agent" { "pane.report_agent".into() } else { "pane.report_agent_session".into() };
            put("pane_id", json!(pane(true)));
            for (k, key) in [("source", "source"), ("agent", "agent"), ("state", "state"), ("message", "message"), ("agent-session-id", "agent_session_id"), ("agent-session-path", "agent_session_path")] { if let Some(v) = a.get(k) { put(key, json!(v)); } }
            if let Some(n) = a.get("seq").and_then(|n| n.parse::<u64>().ok()) { put("seq", json!(n)); }
            if !resume.is_empty() { put("resume", json!(resume)); }
        }
        ("pane", "release-agent") => { method = "pane.release_agent".into(); put("pane_id", json!(pane(true))); if let Some(v) = a.get("source") { put("source", json!(v)); } }
        ("workspace", "list") => { method = "workspace.list".into(); if a.flag("all") { put("all", json!(true)); } }
        ("workspace", "get") | ("workspace", "focus") | ("workspace", "close") => { method = format!("workspace.{verb}"); if let Some(w) = a.pos.get(2) { put("workspace_id", json!(w)); } }
        ("workspace", "rename") => { method = "workspace.rename".into(); put("workspace_id", json!(a.pos.get(2))); put("label", json!(rest(3).join(" "))); }
        ("workspace", "create") => { method = "workspace.create".into(); if let Some(c) = a.get("cwd") { put("cwd", json!(c)); } if let Some(l) = a.get("label") { put("label", json!(l)); } put("env", env_map(&a)); put("focus", json!(a.flag("focus"))); if a.flag("hidden") { put("hidden", json!(true)); } }
        ("tab", "list") => { method = "tab.list".into(); if let Some(w) = a.get("workspace") { put("workspace_id", json!(w)); } }
        ("tab", "get") | ("tab", "focus") | ("tab", "close") => { method = format!("tab.{verb}"); if let Some(t) = a.pos.get(2) { put("tab_id", json!(t)); } }
        ("tab", "rename") => { method = "tab.rename".into(); put("tab_id", json!(a.pos.get(2))); put("label", json!(rest(3).join(" "))); }
        ("tab", "create") => { method = "tab.create".into(); if let Some(w) = a.get("workspace") { put("workspace_id", json!(w)); } if let Some(c) = a.get("cwd") { put("cwd", json!(c)); } if let Some(l) = a.get("label") { put("label", json!(l)); } put("env", env_map(&a)); put("focus", json!(!a.flag("no-focus"))); }
        ("agent", "focus") | ("agent", "get") | ("agent", "list") | ("agent", "rename") => { method = format!("agent.{verb}"); if let Some(x) = pane(true) { put("pane_id", json!(x)); } if verb == "rename" { put("label", json!(rest(3).join(" "))); } }
        ("agent", "prompt") => { method = "agent.prompt".into(); put("pane_id", json!(pane(true))); put("text", json!(rest(3).join(" "))); }
        ("agent", "read") => { method = "agent.read".into(); raw_text = true; put("pane_id", json!(pane(true))); if let Some(l) = a.get("lines").and_then(|l| l.parse::<u64>().ok()) { put("lines", json!(l)); } if let Some(s) = a.get("source") { put("source", json!(s)); } }
        ("agent", "send-keys") => { method = "pane.send_keys".into(); put("pane_id", json!(pane(true))); put("keys", json!(rest(3))); }
        _ => { err!("herdr: the native mux has no `{noun} {verb}`"); return (2, so, se); }
    }
    if !resume.is_empty() && matches!((noun, verb), ("tab", "create") | ("pane", "split") | ("workspace", "create")) { p.insert("command".into(), json!(resume)); }   // after `--`: what the new pane runs
    let code = match call(&method, Value::Object(p)) {
        Ok(v) => {
            if let Some(e) = v.get("error") { err!("{}", json!({"id": format!("cli:{noun}:{verb}"), "error": e})); return (1, so, se); }
            let result = v.get("result").cloned().unwrap_or(Value::Null);
            if raw_text { out!("{}", result.get("text").and_then(|t| t.as_str()).unwrap_or("")); }
            else { out!("{}", json!({"id": format!("cli:{noun}:{verb}"), "result": result})); }
            0
        }
        Err(e) => { err!("herdr: {e}"); 1 }
    };
    (code, so, se)
}

/// `herdr ...` from the command line (also through the `herdr` link of the mux, and `bhoted herdr`): the native mux, or herdr itself when that is what is wanted and there.
/// The `herdr` link of the mux: always the mux (whoever calls the link means it).
pub fn run_native(args: &[String]) -> i32 {
    let (code, so, se) = exec(args);
    use std::io::Write;
    let _ = std::io::stdout().write_all(so.as_bytes());
    let _ = std::io::stderr().write_all(se.as_bytes());
    code
}

pub fn run(args: &[String]) -> i32 {
    // `herdr --real-path`: where a herdr program is (exit 1 when there is none): the script asks
    if args == ["--real-path".to_string()] { return match super::backend::real_herdr() { Some(p) => { println!("{}", p.display()); 0 } None => 1 }; }
    if !super::backend::native() { super::backend::exec_real(args); }
    let (code, so, se) = exec(args);
    use std::io::Write;
    let _ = std::io::stdout().write_all(so.as_bytes());
    let _ = std::io::stderr().write_all(se.as_bytes());
    code
}
