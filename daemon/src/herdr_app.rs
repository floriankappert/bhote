//! herdr-app as the source of the agents. The daemon and the script were written against herdr's CLI (`herdr pane list`,
//! `herdr workspace list`); herdr-app has no such CLI, but `herdr-app ls` (spaces, tabs, terminals of every machine),
//! `herdr-app status` (the focus) and `hc ls --json` (the folder of each terminal) say the same. `snapshot` turns them into
//! the shapes `agents::build` already reads, per machine, so everything behind it stays as it is; `cli` answers the few
//! `herdr ...` calls of the script (`bhoted herdr pane list`), see `herdr()` in the script.
//!
//! Ids: a pane is the terminal's address without its machine (`t3`), a workspace is the space's id, a tab the tab's id.
//! A terminal is an agent when its program reports a status (herdr-app's own rule).
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// herdr-app is the source unless `BHOTE_HERDR_BACKEND=classic` asks for the old herdr (the tests with their fake herdr do).
pub fn enabled() -> bool { std::env::var("BHOTE_HERDR_BACKEND").map_or(true, |v| v != "classic") }

fn run_json(cmd: &str, args: &[&str], secs: u64) -> Option<Value> {
    let (code, out, _) = crate::proc::run(cmd, args, secs, None)?;
    if code != 0 { return None; }
    serde_json::from_slice(&out).ok()
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(|x| x.as_str()).unwrap_or("") }
/// `omarchy-flo/t3` -> `t3` (an id on the machine it lives on)
fn local_id(s: &str) -> &str { s.rsplit('/').next().unwrap_or(s) }
fn rank(s: &str) -> u8 { match s { "blocked" => 4, "done" => 3, "working" => 2, "idle" => 1, _ => 0 } }

pub struct Machine {
    /// None: this computer
    pub name: Option<String>,
    /// `{"result":{"panes":[..]}}` as `herdr pane list` prints it
    pub panes: Value,
    /// `{"result":{"workspaces":[..]}}` as `herdr workspace list` prints it
    pub workspaces: Value,
    /// the core of that machine does not answer
    pub down: bool,
}

/// Every machine herdr-app shows, this computer first. None when herdr-app does not answer (the old files stay).
pub fn snapshot() -> Option<Vec<Machine>> {
    let ls = run_json("herdr-app", &["ls"], 15)?;
    let hc = run_json("hc", &["ls", "--json"], 15).unwrap_or(Value::Null);
    let focused = run_json("herdr-app", &["status"], 10).and_then(|s| s.get("focused_pane").and_then(|p| p.as_str().map(|x| x.to_string()))).unwrap_or_default();
    let mut cwd: BTreeMap<String, String> = BTreeMap::new();
    for t in hc.get("terminals").and_then(|t| t.as_array()).into_iter().flatten() { cwd.insert(text(t, "address").to_string(), text(t, "cwd").to_string()); }
    let mut state: BTreeMap<String, String> = BTreeMap::new();
    for m in hc.get("machines").and_then(|t| t.as_array()).into_iter().flatten() { state.insert(text(m, "name").to_string(), text(m, "state").to_string()); }

    // this computer first, then the machines in the order herdr-app lists their spaces
    let mut order: Vec<Option<String>> = vec![None];
    let mut panes: BTreeMap<Option<String>, Vec<Value>> = BTreeMap::new();
    let mut spaces: BTreeMap<Option<String>, Vec<Value>> = BTreeMap::new();
    for s in ls.get("spaces").and_then(|s| s.as_array()).into_iter().flatten() {
        let m = s.get("machine").and_then(|m| m.as_str()).map(|m| m.to_string());
        if !order.contains(&m) { order.push(m.clone()); }
        let sid = local_id(text(s, "id")).to_string();
        let mut top = "unknown";
        for tab in s.get("tabs").and_then(|t| t.as_array()).into_iter().flatten() {
            let tid = local_id(text(tab, "id")).to_string();
            for t in tab.get("terminals").and_then(|t| t.as_array()).into_iter().flatten() {
                let addr = text(t, "terminal");
                let status = t.get("status").and_then(|x| x.as_str());
                if let Some(st) = status { if rank(st) > rank(top) { top = st; } }
                let folder = cwd.get(addr).cloned().unwrap_or_default();
                let agent = if status.is_some() { json!(t.get("app").and_then(|a| a.as_str()).unwrap_or("agent")) } else { Value::Null };
                panes.entry(m.clone()).or_default().push(json!({
                    "pane_id": local_id(addr), "tab_id": tid, "workspace_id": sid, "agent": agent,
                    "agent_status": status.unwrap_or("unknown"), "cwd": folder, "foreground_cwd": folder,
                    "focused": !focused.is_empty() && text(t, "pane") == focused,
                }));
            }
        }
        let list = spaces.entry(m.clone()).or_default();
        let number = list.len() + 1;
        list.push(json!({ "workspace_id": sid, "label": text(s, "name"), "number": number, "focused": s.get("active") == Some(&Value::Bool(true)), "agent_status": top }));
    }
    Some(order.into_iter().map(|m| {
        let down = m.as_ref().map_or(false, |n| state.get(n).map_or(false, |s| s != "connected"));
        Machine { panes: json!({"result": {"panes": panes.remove(&m).unwrap_or_default()}}), workspaces: json!({"result": {"workspaces": spaces.remove(&m).unwrap_or_default()}}), name: m, down }
    }).collect())
}

/// This computer's panes and workspaces, in the shapes of `herdr pane list` and `herdr workspace list`.
pub fn local() -> Option<(Value, Value)> {
    snapshot()?.into_iter().find(|m| m.name.is_none()).map(|m| (m.panes, m.workspaces))
}

/// `MACHINE_LABELS=omarchy-flo=Omarchy,other=Other`: bhote's name of a machine (what its topics say in `agent_machine`).
pub fn label(labels: &str, name: &str) -> String {
    labels.split(',').filter_map(|p| p.split_once('=')).find(|(k, _)| k.trim() == name).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| name.to_string())
}

/// bhote's label of a machine ("Omarchy") back to herdr-app's name ("omarchy-flo"), by MACHINE_LABELS; a name stays what it is.
pub fn unlabel(labels: &str, label: &str) -> String {
    labels.split(',').filter_map(|p| p.split_once('=')).find(|(_, v)| v.trim() == label).map(|(k, _)| k.trim().to_string()).unwrap_or_else(|| label.to_string())
}

/// A terminal as herdr-app and hc address it: `t3` here, `omarchy-flo/t3` on a machine.
fn addr(machine: &Option<String>, id: &str) -> String { match machine { Some(m) => format!("{}/{}", m, id), None => id.to_string() } }

/// One machine's panes and workspaces (None: this computer).
fn machine_data(machine: &Option<String>) -> Option<(Value, Value)> {
    snapshot()?.into_iter().find(|m| &m.name == machine).map(|m| (m.panes, m.workspaces))
}

/// The `herdr ...` calls of the script, answered from herdr-app: `bhoted herdr [--machine <label>] <args>`. The machine is bhote's
/// label (MACHINE_LABELS in the environment turns it back into herdr-app's name). Exit 1 for what herdr-app does not have (the
/// script treats it as "herdr did not answer").
pub fn cli(args: &[String]) -> i32 {
    let mut a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let mut machine: Option<String> = None;
    if a.first() == Some(&"--machine") && a.len() >= 2 {
        let labels = std::env::var("MACHINE_LABELS").unwrap_or_default();
        machine = Some(unlabel(&labels, a[1]));
        a = a[2..].to_vec();
    }
    let print = |v: Value| { println!("{}", v); 0 };
    match a.as_slice() {
        ["pane", "list", ..] => match machine_data(&machine) { Some((p, _)) => print(p), None => 1 },
        ["workspace", "list", ..] => match machine_data(&machine) { Some((_, w)) => print(w), None => 1 },
        ["pane", "get", id, ..] => match machine_data(&machine) {
            Some((p, _)) => match p["result"]["panes"].as_array().and_then(|l| l.iter().find(|x| text(x, "pane_id") == *id)) { Some(x) => print(json!({"result": {"pane": x}})), None => 1 },
            None => 1,
        },
        ["pane", "read", id, rest @ ..] => {
            let lines = rest.iter().position(|x| *x == "--lines").and_then(|i| rest.get(i + 1)).copied().unwrap_or("40");
            match crate::proc::run("hc", &["read", &addr(&machine, id), "--lines", lines], 15, None) { Some((0, out, _)) => { print!("{}", String::from_utf8_lossy(&out)); 0 } _ => 1 }
        }
        // a prompt to an agent, and a command typed into a pane's shell: both are text pasted into the terminal and sent with Enter
        ["agent", "prompt", id, text, ..] | ["pane", "run", id, text, ..] => ok(crate::proc::run("hc", &["send", &addr(&machine, id), text], 20, None)),
        // the other machines herdr-app shows, as `herdr machine list` prints them: id, label, host, profile, enabled (tab separated)
        ["machine", "list", ..] => match snapshot() {
            Some(ms) => {
                let labels = std::env::var("MACHINE_LABELS").unwrap_or_default();
                for m in ms { if let Some(n) = m.name { println!("{}\t{}\t{}\tdefault\tenabled", n, label(&labels, &n), n); } }
                0
            }
            None => 1,
        },
        ["agent", "focus", id, ..] => ok(crate::proc::run("herdr-app", &["do", "agent.focus", "--terminal", &addr(&machine, id)], 15, None)),
        ["workspace", "focus", id, ..] => ok(crate::proc::run("herdr-app", &["do", "space.select", "--id", &addr(&machine, id)], 15, None)),
        ["workspace", "create", ..] => ok(crate::proc::run("herdr-app", &["do", "space.new"], 15, None)),
        _ => 1,
    }
}

fn ok(r: Option<(i32, Vec<u8>, Vec<u8>)>) -> i32 { match r { Some((0, _, _)) => 0, _ => 1 } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels() {
        assert_eq!(label("omarchy-flo=Omarchy, x=Y", "omarchy-flo"), "Omarchy");
        assert_eq!(label("omarchy-flo=Omarchy", "other"), "other");
        assert_eq!(label("", "m"), "m");
    }
    #[test]
    fn unlabels() {
        assert_eq!(unlabel("omarchy-flo=Omarchy", "Omarchy"), "omarchy-flo");
        assert_eq!(unlabel("omarchy-flo=Omarchy", "other"), "other");
        assert_eq!(unlabel("", "Omarchy"), "Omarchy");
        assert_eq!(addr(&Some("omarchy-flo".into()), "t3"), "omarchy-flo/t3");
        assert_eq!(addr(&None, "t3"), "t3");
    }
    #[test]
    fn ids() {
        assert_eq!(local_id("omarchy-flo/t3"), "t3");
        assert_eq!(local_id("t3"), "t3");
        assert!(rank("blocked") > rank("working") && rank("working") > rank("idle") && rank("idle") > rank("unknown"));
    }
}
