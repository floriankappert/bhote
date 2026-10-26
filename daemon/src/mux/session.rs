//! The session on disk: layout, names, folders, labels and the commands that start the agents again. Written when something changed,
//! read when the server starts cold; panes keep their names (w7:p5), because topics refer to them.
use super::layout::{Dir, Node};
use super::server::{Server, Spawn};
use super::world::Tab;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn state_path() -> PathBuf {
    if let Some(p) = std::env::var_os("BHOTE_MUX_STATE").filter(|p| !p.is_empty()) { return PathBuf::from(p); }
    let base = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state"))).unwrap_or_default();
    base.join("bhote").join("mux-session.json")
}

fn quote(w: &str) -> String { if !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c)) { w.to_string() } else { format!("'{}'", w.replace('\'', "'\\''")) } }

/// How an agent is started again: what its hook said, else the usual command of the agent for its session.
fn resume_of(agent: &str, session: &str, reported: &[String]) -> Vec<String> {
    if !reported.is_empty() { return reported.to_vec(); }
    if session.is_empty() { return Vec::new(); }
    match agent { "claude" => vec!["claude".into(), "--resume".into(), session.into()], "codex" => vec!["codex".into(), "resume".into(), session.into()], _ => Vec::new() }
}

fn node_json(s: &Server, n: &Node) -> Option<Value> {
    match n {
        Node::Leaf(p) => {
            let q = s.panes.get(p)?;
            if s.is_panel(*p) { return None; }
            let sess = q.session.as_ref().and_then(|v| v.get("value")).and_then(|v| v.as_str()).unwrap_or("");
            let resume = q.agent.as_deref().map(|a| resume_of(a, sess, &q.resume)).unwrap_or_default();
            Some(json!({"leaf": {"n": q.n, "label": q.label, "cwd": q.foreground().1, "cmd": q.cmd, "agent": q.agent, "session": sess, "resume": resume}}))
        }
        Node::Split { dir, ratio, a, b, .. } => match (node_json(s, a), node_json(s, b)) {
            (Some(x), Some(y)) => Some(json!({"split": if *dir == Dir::Right { "right" } else { "down" }, "ratio": ratio, "a": x, "b": y})),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        },
    }
}

pub fn snapshot(s: &Server) -> Value {
    let mut wss = Vec::new();
    for w in &s.world.ws {
        let mut tabs = Vec::new();
        for t in &w.tabs {
            let Some(root) = node_json(s, &t.root) else { continue };
            let n_of = |p: u32| s.panes.get(&p).map(|q| q.n);
            tabs.push(json!({"id": t.id, "label": t.label, "no_panels": t.no_panels, "focus": n_of(t.focus), "zoomed": t.zoomed.and_then(n_of), "root": root}));
        }
        if tabs.is_empty() || w.hidden { continue; }   // (the view of another machine, opened by a click: made again by the next click, not kept over a restart)
        wss.push(json!({"id": w.id, "label": w.label, "cwd": w.cwd, "next_tab": w.next_tab, "next_pane": w.next_pane, "hidden": w.hidden, "active_tab": w.tabs.get(w.active).map(|t| t.id), "tabs": tabs}));
    }
    json!({"version": 1, "sidebars": s.sb_sizes, "active_workspace": s.world.ws.get(s.world.active).map(|w| w.id), "workspaces": wss})
}

impl Server {
    /// Writes the session when it differs from the file (at most every 300 ms, unless `force`).
    pub fn save_state(&mut self, force: bool) {
        let Some(f) = self.state_file.clone() else { return };
        if !force && self.state_at.map_or(false, |t| t.elapsed() < Duration::from_millis(300)) { return; }
        self.state_at = Some(Instant::now());
        if self.world.ws.is_empty() { return; }
        let text = snapshot(self).to_string();
        if text == self.state_saved { return; }
        if let Some(d) = f.parent() { let _ = std::fs::create_dir_all(d); }
        let tmp = f.with_extension(format!("tmp.{}", std::process::id()));
        if std::fs::write(&tmp, &text).is_ok() && std::fs::rename(&tmp, &f).is_ok() { self.state_saved = text; }
    }

    fn build(&mut self, wi: usize, tab: u32, v: &Value, made: &mut Vec<(u32, u32)>) -> Option<Node> {
        if let Some(l) = v.get("leaf") {
            let n = l.get("n").and_then(|n| n.as_u64()).map(|n| n as u32);
            let cmd: Vec<String> = l.get("cmd").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let cwd = l.get("cwd").and_then(|c| c.as_str()).filter(|c| std::path::Path::new(c).is_dir()).map(String::from);
            let sp = Spawn { cmd, cwd, env: Vec::new() };
            let pid = self.make_pane_n(wi, tab, n, &sp).ok()?;
            let q = self.panes.get_mut(&pid)?;
            q.label = l.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let resume: Vec<String> = l.get("resume").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            if !resume.is_empty() {
                // the agent is started again in the shell of the pane (when it ends, the shell is there)
                q.resume = resume.clone();
                let line = resume.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ");
                self.later.push((Instant::now() + Duration::from_millis(500), pid, format!("{line}\r").into_bytes()));
            }
            made.push((n.unwrap_or(0), pid));
            return Some(Node::Leaf(pid));
        }
        let dir = if v.get("split").and_then(|d| d.as_str()) == Some("down") { Dir::Down } else { Dir::Right };
        let ratio = v.get("ratio").and_then(|r| r.as_f64()).unwrap_or(0.5) as f32;
        let a = self.build(wi, tab, v.get("a")?, made);
        let b = self.build(wi, tab, v.get("b")?, made);
        match (a, b) { (Some(a), Some(b)) => Some(Node::Split { dir, ratio, fixed: super::layout::Fixed::None, a: Box::new(a), b: Box::new(b) }), (Some(x), None) | (None, Some(x)) => Some(x), _ => None }
    }
}

/// Starts the panes of the session file. False when there is none (or nothing in it): the caller makes a fresh workspace.
pub fn load(s: &mut Server) -> bool {
    let Some(f) = s.state_file.clone() else { return false };
    let Some(v) = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return false };
    let Some(wss) = v.get("workspaces").and_then(|w| w.as_array()) else { return false };
    let mut active = None;
    for w in wss {
        let u = |k: &str, d: u64| w.get(k).and_then(|x| x.as_u64()).unwrap_or(d) as u32;
        let id = u("id", 0); if id == 0 { continue; }
        let wi = s.world.restore_workspace(id, w.get("label").and_then(|x| x.as_str()).unwrap_or(""), w.get("cwd").and_then(|x| x.as_str()).unwrap_or(""), u("next_tab", 2), u("next_pane", 1));
        s.world.ws[wi].hidden = w.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false);
        for t in w.get("tabs").and_then(|t| t.as_array()).cloned().unwrap_or_default() {
            let tid = t.get("id").and_then(|x| x.as_u64()).unwrap_or(1) as u32;
            let mut made = Vec::new();
            let Some(root) = t.get("root").and_then(|r| s.build(wi, tid, r, &mut made)) else { continue };
            let by_n = |n: Option<u64>| n.and_then(|n| made.iter().find(|(m, _)| *m as u64 == n).map(|(_, p)| *p));
            let mut leaves = Vec::new(); root.leaves(&mut leaves);
            let mut tab = Tab::new(tid, t.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string(), leaves[0]);
            tab.root = root;
            tab.focus = by_n(t.get("focus").and_then(|x| x.as_u64())).unwrap_or(leaves[0]);
            tab.popup_prev = tab.focus;
            tab.zoomed = by_n(t.get("zoomed").and_then(|x| x.as_u64()));
            tab.no_panels = t.get("no_panels").and_then(|x| x.as_bool()).unwrap_or(false);
            s.world.ws[wi].tabs.push(tab);
        }
        if s.world.ws[wi].tabs.is_empty() { s.world.ws.remove(wi); continue; }
        let at = w.get("active_tab").and_then(|x| x.as_u64()).and_then(|id| s.world.ws[wi].tabs.iter().position(|t| t.id as u64 == id)).unwrap_or(0);
        s.world.ws[wi].active = at;
        if v.get("active_workspace").and_then(|x| x.as_u64()) == Some(id as u64) { active = Some(wi); }
    }
    if let Some(o) = v.get("sidebars").and_then(|x| x.as_object()) { for (k, n) in o { if let Some(n) = n.as_u64() { s.sb_sizes.insert(k.clone(), n as u16); } } }
    if s.world.ws.is_empty() { return false; }
    s.world.active = active.unwrap_or(0);
    s.after_change();
    true
}
