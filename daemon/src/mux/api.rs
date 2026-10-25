//! The control API of the mux (newline JSON on the socket): the verbs of herdr, with the shapes of its answers.
use super::layout::{self, Dir, Rect, Side};
use super::pane::Pane;
use super::server::{Server, Spawn};
use super::world::{self, World};
use serde_json::{json, Value};

type Res = Result<Value, (&'static str, String)>;
fn nf(what: &str) -> (&'static str, String) { ("not_found", format!("no such {what}")) }
fn bad(m: impl Into<String>) -> (&'static str, String) { ("invalid_params", m.into()) }

fn s<'a>(p: &'a Value, k: &str) -> Option<&'a str> { p.get(k).and_then(|v| v.as_str()) }
fn flag(p: &Value, k: &str) -> Option<bool> { p.get(k).and_then(|v| v.as_bool()) }

fn spawn_of(p: &Value) -> Spawn {
    let cmd = p.get("command").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let env = p.get("env").and_then(|e| e.as_object()).map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect()).unwrap_or_default();
    Spawn { cmd, cwd: s(p, "cwd").map(String::from), env }
}

fn key_bytes(k: &str) -> Vec<u8> {
    let named: &[(&str, &str)] = &[("Enter", "\r"), ("Return", "\r"), ("Escape", "\x1b"), ("Esc", "\x1b"), ("Tab", "\t"), ("Space", " "), ("Backspace", "\x7f"), ("Up", "\x1b[A"), ("Down", "\x1b[B"), ("Right", "\x1b[C"), ("Left", "\x1b[D"), ("Home", "\x1b[H"), ("End", "\x1b[F"), ("PageUp", "\x1b[5~"), ("PageDown", "\x1b[6~"), ("Delete", "\x1b[3~")];
    if let Some((_, v)) = named.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)) { return v.as_bytes().to_vec(); }
    let low = k.to_ascii_lowercase();
    for pre in ["ctrl+", "ctrl-", "c-"] {
        if let Some(r) = low.strip_prefix(pre) { if let Some(c) = r.chars().next().filter(|_| r.chars().count() == 1) { return vec![(c as u8) & 0x1f]; } }
    }
    for pre in ["alt+", "alt-", "m-"] { if let Some(r) = k.get(pre.len()..).filter(|_| low.starts_with(pre)) { let mut v = vec![0x1b]; v.extend(key_bytes(r)); return v; } }
    k.as_bytes().to_vec()
}

impl Server {
    pub fn pane_by_name(&self, name: &str) -> Option<u32> { self.panes.values().find(|p| p.name() == name).map(|p| p.id) }

    /// The pane a request means: `pane_id` (a name), else the focused one.
    fn pane_of(&self, p: &Value) -> Result<u32, (&'static str, String)> {
        match s(p, "pane_id").or_else(|| s(p, "pane")) {
            Some(n) => self.pane_by_name(n).ok_or_else(|| nf(&format!("pane {n}"))),
            None => self.world.focused_pane().ok_or_else(|| nf("pane")),
        }
    }
    fn ws_of(&self, p: &Value) -> Result<usize, (&'static str, String)> {
        match s(p, "workspace_id").or_else(|| s(p, "workspace")) {
            Some(n) => world::parse_name(n, 'w').and_then(|(w, _)| self.world.ws_index(w)).ok_or_else(|| nf(&format!("workspace {n}"))),
            None => Ok(self.world.active),
        }
    }
    fn tab_of(&self, p: &Value) -> Result<(usize, usize), (&'static str, String)> {
        match s(p, "tab_id").or_else(|| s(p, "tab")) {
            Some(n) => world::parse_name(n, 't').and_then(|(w, t)| self.world.find_tab(w, t)).ok_or_else(|| nf(&format!("tab {n}"))),
            None => { let w = self.world.active; self.world.ws.get(w).map(|x| (w, x.active)).ok_or_else(|| nf("tab")) }
        }
    }

    pub fn pane_json(&self, p: &Pane) -> Value {
        let (g, fcwd) = p.foreground();
        let _ = g;
        let sc = p.screen();
        let (rows, _) = sc.size();
        let mut o = json!({
            "pane_id": p.name(), "tab_id": world::tab_name(p.ws, p.tab), "workspace_id": world::ws_name(p.ws),
            "focused": self.world.focused_pane() == Some(p.id), "cwd": p.cwd, "foreground_cwd": fcwd,
            "terminal_id": format!("term_{:x}", p.id), "terminal_title": p.title(), "terminal_title_stripped": p.title().trim_start_matches(|c: char| !c.is_alphanumeric()).trim(),
            "agent_status": p.agent_status(), "revision": p.rev,
            "scroll": {"max_offset_from_bottom": sc.scrollback_len(), "offset_from_bottom": sc.scrollback(), "viewport_rows": rows.get()},
        });
        let m = o.as_object_mut().unwrap();
        if !p.label.is_empty() { m.insert("label".into(), json!(p.label)); }
        if !p.tokens.is_empty() { m.insert("tokens".into(), Value::Object(p.tokens.clone())); }
        if let Some(a) = p.agent_name() { m.insert("agent".into(), json!(a)); }
        if let Some(sess) = p.agent_session() { m.insert("agent_session".into(), sess); }
        o
    }

    fn tab_json(&self, wi: usize, ti: usize) -> Value {
        let w = &self.world.ws[wi]; let t = &w.tabs[ti];
        let mut l = Vec::new(); t.root.leaves(&mut l);
        json!({"tab_id": world::tab_name(w.id, t.id), "workspace_id": world::ws_name(w.id), "label": t.label, "number": ti + 1, "focused": wi == self.world.active && ti == w.active, "pane_count": l.len(), "agent_status": self.status_of(&l)})
    }
    fn ws_json(&self, wi: usize) -> Value {
        let w = &self.world.ws[wi];
        let mut l = Vec::new(); for t in &w.tabs { t.root.leaves(&mut l); }
        json!({"workspace_id": world::ws_name(w.id), "label": w.label, "number": wi + 1, "focused": wi == self.world.active, "active_tab_id": w.tabs.get(w.active).map(|t| world::tab_name(w.id, t.id)), "pane_count": l.len(), "tab_count": w.tabs.len(), "agent_status": self.status_of(&l)})
    }
    /// The most urgent status of the agents among these panes (blocked, working, done, idle), "unknown" when there are none.
    fn status_of(&self, panes: &[u32]) -> String {
        let rank = |s: &str| match s { "blocked" => 4, "working" => 3, "done" => 2, "idle" => 1, _ => 0 };
        let mut best = "unknown".to_string();
        for id in panes { if let Some(p) = self.panes.get(id) { let st = p.agent_status(); if rank(st) > rank(&best) { best = st.to_string(); } } }
        best
    }

    fn layout_json(&self, wi: usize, ti: usize) -> Value {
        let w = &self.world.ws[wi]; let t = &w.tabs[ti];
        let area = self.area();
        let (rects, divs) = World::rects(t, area);
        let rj = |r: &Rect| json!({"x": r.x, "y": r.y, "width": r.w, "height": r.h});
        let splits: Vec<Value> = divs.iter().map(|d| {
            let ratio = { let mut n = &t.root; for &sec in &d.path { if let layout::Node::Split { a, b, .. } = n { n = if sec { b } else { a }; } } if let layout::Node::Split { ratio, .. } = n { *ratio } else { 0.5 } };
            json!({"id": format!("split_{}", d.path.iter().map(|b| if *b { '1' } else { '0' }).collect::<String>() + if d.path.is_empty() { "root" } else { "" }), "direction": if d.dir == Dir::Right { "right" } else { "down" }, "ratio": ratio, "rect": rj(&d.rect)})
        }).collect();
        let name = |id: &u32| self.panes.get(id).map(|p| p.name()).unwrap_or_default();
        json!({"layout": {"area": rj(&area), "focused_pane_id": name(&t.focus), "panes": rects.iter().map(|(id, r)| json!({"pane_id": name(id), "focused": *id == t.focus, "rect": rj(r)})).collect::<Vec<_>>(), "splits": splits, "tab_id": world::tab_name(w.id, t.id), "workspace_id": world::ws_name(w.id), "zoomed": t.zoomed.is_some()}, "type": "pane_layout"})
    }

    /// The text of a pane: the last `lines` lines of scrollback plus screen (`unwrapped` joins the lines the terminal wrapped).
    pub fn read_text(&mut self, id: u32, source: &str, lines: usize, unwrapped: bool) -> String {
        let p = self.panes.get_mut(&id).unwrap();
        let (rows, cols) = { let (r, c) = p.screen().size(); (r.get() as usize, c.get()) };
        let mut all: Vec<(String, bool)> = Vec::new();
        if source == "visible" {
            let sc = p.screen();
            for (i, t) in sc.rows(0, cols).enumerate() { all.push((t, sc.row_wrapped(i as u16))); }
        } else {
            let total = p.screen().scrollback_len();
            let mut idx = 0;
            while idx < total + rows {
                let o = total.saturating_sub(idx);
                p.parser.screen_mut().set_scrollback(o);
                let sc = p.parser.screen();
                let skip = idx.saturating_sub(total - o.min(total));
                for (i, t) in sc.rows(0, cols).enumerate().skip(skip) { all.push((t, sc.row_wrapped(i as u16))); }
                idx += rows;
            }
            p.parser.screen_mut().set_scrollback(0);
        }
        let mut out: Vec<String> = Vec::new();
        let mut cur: Option<String> = None;
        for (t, wrapped) in all {
            let line = match cur.take() { Some(mut c) if unwrapped => { c.push_str(&t); c } _ => { if let Some(c) = cur.take() { out.push(c); } t } };
            if wrapped && unwrapped { cur = Some(line); } else { out.push(line); }
        }
        if let Some(c) = cur { out.push(c); }
        while out.last().map_or(false, |l| l.trim().is_empty()) { out.pop(); }
        let from = out.len().saturating_sub(lines.max(1));
        out[from..].iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    }

    pub fn control(&mut self, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let empty = json!({});
        let params = req.get("params").unwrap_or(&empty).clone();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
        match self.dispatch(&method, &params) {
            Ok(r) => json!({"id": id, "result": r}),
            Err((code, msg)) => json!({"id": id, "error": {"code": code, "message": msg}}),
        }
    }

    fn dispatch(&mut self, method: &str, p: &Value) -> Res {
        match method {
            "ping" => Ok(json!({"pong": true, "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id()})),
            "server.stop" => { self.end_clients("stopped"); self.quit = true; Ok(json!({"stopped": true})) }
            "server.reload_config" => Ok(json!({"status": "applied", "diagnostics": []})),
            "notification.show" => {
                // a desktop notification through the terminal (OSC 777), a bell for the sound, and a line on the bar
                let title = s(p, "title").or_else(|| p.get("text").and_then(|t| t.as_str())).unwrap_or("").replace(|c: char| c.is_control() || c == ';', " ");
                let body = s(p, "body").unwrap_or("").replace(|c: char| c.is_control() || c == ';', " ");
                self.clip.push(format!("\x1b]777;notify;{title};{body}\x07{}", if s(p, "sound").map_or(true, |x| x != "none") { "\x07" } else { "" }));
                self.toast = Some((if body.is_empty() { title } else { format!("{title}: {body}") }, std::time::Instant::now() + std::time::Duration::from_secs(8)));
                self.dirty = true;
                Ok(json!({}))
            }
            "api.snapshot" => {
                let panes: Vec<Value> = self.panes.values().map(|q| self.pane_json(q)).collect();
                let agents: Vec<Value> = panes.iter().filter(|q| q.get("agent").is_some()).cloned().collect();
                let ws: Vec<Value> = (0..self.world.ws.len()).map(|i| self.ws_json(i)).collect();
                let mut tabs = Vec::new(); for (wi, w) in self.world.ws.iter().enumerate() { for ti in 0..w.tabs.len() { tabs.push(self.tab_json(wi, ti)); } }
                Ok(json!({"snapshot": {"panes": panes, "agents": agents, "workspaces": ws, "tabs": tabs}, "type": "api_snapshot"}))
            }
            // ── panes
            "pane.list" => {
                let ws = s(p, "workspace_id"); let tab = s(p, "tab_id");
                let mut list: Vec<&Pane> = self.panes.values().filter(|q| ws.map_or(true, |w| world::ws_name(q.ws) == w) && tab.map_or(true, |t| world::tab_name(q.ws, q.tab) == t)).collect();
                list.sort_by_key(|q| (self.world.ws_index(q.ws), q.tab, q.n));
                Ok(json!({"panes": list.iter().map(|q| self.pane_json(q)).collect::<Vec<_>>(), "type": "pane_list"}))
            }
            "pane.get" => { let id = self.pane_of(p)?; Ok(json!({"pane": self.pane_json(&self.panes[&id]), "type": "pane_info"})) }
            "pane.layout" => {
                let (wi, ti) = if s(p, "pane_id").is_some() { let id = self.pane_of(p)?; self.world.tab_of_pane(id).ok_or_else(|| nf("tab"))? } else { self.tab_of(p)? };
                Ok(self.layout_json(wi, ti))
            }
            "pane.process_info" => {
                let id = self.pane_of(p)?; let q = &self.panes[&id];
                let (g, _) = q.foreground();
                let cmdline = std::fs::read(format!("/proc/{g}/cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ").trim().to_string()).ok()
                    .or_else(|| std::process::Command::new("ps").args(["-o", "command=", "-p", &g.to_string()]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())).unwrap_or_default();
                Ok(json!({"process_info": {"pid": q.pty.pid(), "foreground_process_group_id": g, "foreground_processes": [{"pid": g, "cmdline": cmdline}]}, "type": "pane_process_info"}))
            }
            "pane.split" => {
                let target = self.pane_of(p)?;
                let dir = match s(p, "direction") { Some("right") | None => Dir::Right, Some("down") => Dir::Down, Some(d) => return Err(bad(format!("direction {d}"))) };
                let ratio = p.get("ratio").and_then(|r| r.as_f64()).unwrap_or(0.5) as f32;
                let new = self.split_pane(target, dir, ratio, &spawn_of(p), flag(p, "focus").unwrap_or(true)).map_err(|e| ("split_failed", e.to_string()))?;
                Ok(json!({"pane": self.pane_json(&self.panes[&new]), "type": "pane_info"}))
            }
            "pane.swap" => {
                let (a, b) = if let (Some(_), Some(_)) = (s(p, "source_pane"), s(p, "target_pane")) {
                    let a = self.pane_by_name(s(p, "source_pane").unwrap()).ok_or_else(|| nf("pane"))?;
                    let b = self.pane_by_name(s(p, "target_pane").unwrap()).ok_or_else(|| nf("pane"))?;
                    (a, b)
                } else {
                    let a = self.pane_of(p)?;
                    let side = s(p, "direction").and_then(Side::parse).ok_or_else(|| bad("direction"))?;
                    let (wi, ti) = self.world.tab_of_pane(a).ok_or_else(|| nf("tab"))?;
                    let (rects, _) = World::rects(&self.world.ws[wi].tabs[ti], self.area());
                    (a, layout::neighbor(&rects, a, side).ok_or_else(|| ("no_neighbor", "no pane on that side".to_string()))?)
                };
                let (wi, ti) = self.world.tab_of_pane(a).ok_or_else(|| nf("tab"))?;
                if self.world.tab_of_pane(b) != Some((wi, ti)) { return Err(bad("panes of different tabs")); }
                let t = &mut self.world.ws[wi].tabs[ti];
                t.root.swap(a, b);
                self.after_change();
                Ok(json!({}))
            }
            "pane.resize" => {
                let a = self.pane_of(p)?;
                let side = s(p, "direction").and_then(Side::parse).ok_or_else(|| bad("direction"))?;
                let amount = p.get("amount").and_then(|x| x.as_f64()).unwrap_or(0.05) as f32;
                let (wi, ti) = self.world.tab_of_pane(a).ok_or_else(|| nf("tab"))?;
                if !self.world.ws[wi].tabs[ti].root.resize(a, side, amount) { return Err(("no_split", "nothing to resize that way".into())); }
                self.after_change();
                Ok(json!({}))
            }
            "pane.rename" => { let id = self.pane_of(p)?; self.panes.get_mut(&id).unwrap().label = s(p, "label").unwrap_or("").to_string(); self.dirty = true; Ok(json!({"pane": self.pane_json(&self.panes[&id]), "type": "pane_info"})) }
            "pane.run" => {
                let id = self.pane_of(p)?;
                let c = s(p, "command").ok_or_else(|| bad("command"))?;
                self.panes.get_mut(&id).unwrap().input(format!("{c}\r").as_bytes());
                Ok(json!({}))
            }
            "pane.send_text" => { let id = self.pane_of(p)?; self.panes.get_mut(&id).unwrap().input(s(p, "text").unwrap_or("").as_bytes()); Ok(json!({})) }
            "pane.send_keys" => {
                let id = self.pane_of(p)?;
                let keys: Vec<&str> = p.get("keys").and_then(|k| k.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
                let bytes: Vec<u8> = keys.iter().flat_map(|k| key_bytes(k)).collect();
                self.panes.get_mut(&id).unwrap().input(&bytes);
                Ok(json!({}))
            }
            "pane.read" => {
                let id = self.pane_of(p)?;
                let src = s(p, "source").unwrap_or("recent").to_string();
                let lines = p.get("lines").and_then(|l| l.as_u64()).unwrap_or(200) as usize;
                let text = self.read_text(id, &src, lines, src == "recent-unwrapped");
                Ok(json!({"text": text, "type": "pane_read"}))
            }
            "pane.close" => { let id = self.pane_of(p)?; self.close_pane(id); Ok(json!({})) }
            "pane.focus" => {
                let id = self.pane_of(p)?;
                let target = match s(p, "direction").and_then(Side::parse) {
                    Some(side) => { let (wi, ti) = self.world.tab_of_pane(id).ok_or_else(|| nf("tab"))?; let (r, _) = World::rects(&self.world.ws[wi].tabs[ti], self.area()); layout::neighbor(&r, id, side).ok_or_else(|| ("no_neighbor", "no pane on that side".to_string()))? }
                    None => id,
                };
                self.set_focus(target);
                Ok(json!({"pane": self.pane_json(&self.panes[&target]), "type": "pane_info"}))
            }
            "pane.zoom" => {
                let id = self.pane_of(p)?;
                let (wi, ti) = self.world.tab_of_pane(id).ok_or_else(|| nf("tab"))?;
                let t = &mut self.world.ws[wi].tabs[ti];
                let want = flag(p, "zoomed").unwrap_or(t.zoomed.is_none());
                t.zoomed = if want { Some(id) } else { None };
                self.after_change();
                Ok(json!({"zoomed": want}))
            }
            "pane.report_metadata" => {
                let id = self.pane_of(p)?;
                let q = self.panes.get_mut(&id).unwrap();
                if let Some(t) = p.get("tokens").and_then(|t| t.as_object()) { for (k, v) in t { q.tokens.insert(k.clone(), v.clone()); } }
                if let Some(c) = p.get("clear_tokens").and_then(|t| t.as_array()) { for k in c.iter().filter_map(|k| k.as_str()) { q.tokens.remove(k); } }
                self.dirty = true;
                Ok(json!({}))
            }
            // ── workspaces
            "workspace.list" => Ok(json!({"workspaces": (0..self.world.ws.len()).map(|i| self.ws_json(i)).collect::<Vec<_>>(), "type": "workspace_list"})),
            "workspace.get" => { let wi = self.ws_of(p)?; Ok(json!({"workspace": self.ws_json(wi), "type": "workspace_info"})) }
            "workspace.create" => {
                let (wi, pid) = self.create_workspace(s(p, "label").unwrap_or(""), &spawn_of(p), flag(p, "focus").unwrap_or(false)).map_err(|e| ("create_failed", e.to_string()))?;
                Ok(json!({"workspace": self.ws_json(wi), "tab": self.tab_json(wi, 0), "pane": self.pane_json(&self.panes[&pid]), "type": "workspace_info"}))
            }
            "workspace.focus" => { let wi = self.ws_of(p)?; self.world.active = wi; self.dirty = true; Ok(json!({"workspace": self.ws_json(wi), "type": "workspace_info"})) }
            "workspace.rename" => { let wi = self.ws_of(p)?; self.world.ws[wi].label = s(p, "label").unwrap_or("").to_string(); self.dirty = true; Ok(json!({"workspace": self.ws_json(wi), "type": "workspace_info"})) }
            "workspace.close" => {
                let wi = self.ws_of(p)?;
                let mut l = Vec::new(); for t in &self.world.ws[wi].tabs { t.root.leaves(&mut l); }
                for id in l { self.close_pane(id); }
                Ok(json!({}))
            }
            // ── tabs
            "tab.list" => {
                let mut v = Vec::new();
                for (wi, w) in self.world.ws.iter().enumerate() { if s(p, "workspace_id").map_or(true, |x| world::ws_name(w.id) == x) { for ti in 0..w.tabs.len() { v.push(self.tab_json(wi, ti)); } } }
                Ok(json!({"tabs": v, "type": "tab_list"}))
            }
            "tab.get" => { let (wi, ti) = self.tab_of(p)?; Ok(json!({"tab": self.tab_json(wi, ti), "type": "tab_info"})) }
            "tab.create" => {
                let wi = self.ws_of(p)?;
                let (tid, pid) = self.create_tab(wi, s(p, "label").unwrap_or(""), &spawn_of(p), flag(p, "focus").unwrap_or(true)).map_err(|e| ("create_failed", e.to_string()))?;
                let ti = self.world.ws[wi].tabs.iter().position(|t| t.id == tid).unwrap();
                Ok(json!({"tab": self.tab_json(wi, ti), "pane": self.pane_json(&self.panes[&pid]), "type": "tab_info"}))
            }
            "tab.focus" => { let (wi, ti) = self.tab_of(p)?; self.world.active = wi; self.world.ws[wi].active = ti; self.dirty = true; Ok(json!({"tab": self.tab_json(wi, ti), "type": "tab_info"})) }
            "tab.rename" => { let (wi, ti) = self.tab_of(p)?; self.world.ws[wi].tabs[ti].label = s(p, "label").unwrap_or("").to_string(); self.dirty = true; Ok(json!({"tab": self.tab_json(wi, ti), "type": "tab_info"})) }
            "tab.close" => {
                let (wi, ti) = self.tab_of(p)?;
                let mut l = Vec::new(); self.world.ws[wi].tabs[ti].root.leaves(&mut l);
                for id in l { self.close_pane(id); }
                Ok(json!({}))
            }
            // ── agents (the state comes from the hooks, see agent.rs)
            "pane.report_agent" | "pane.report_agent_session" => { let id = self.pane_of(p)?; self.report_agent(id, p); Ok(json!({})) }
            "pane.release_agent" => { let id = self.pane_of(p)?; let q = self.panes.get_mut(&id).unwrap(); q.reported = None; q.chk = None; Ok(json!({})) }
            "agent.list" => Ok(json!({"agents": self.panes.values().filter(|q| q.agent.is_some()).map(|q| self.pane_json(q)).collect::<Vec<_>>(), "type": "agent_list"})),
            "agent.get" => { let id = self.pane_of(p)?; if self.panes[&id].agent.is_none() { return Err(nf("agent")); } Ok(json!({"agent": self.pane_json(&self.panes[&id]), "type": "agent_info"})) }
            "agent.read" => {
                let id = self.pane_of(p)?;
                let src = s(p, "source").unwrap_or("recent-unwrapped").to_string();
                let lines = p.get("lines").and_then(|l| l.as_u64()).unwrap_or(200) as usize;
                Ok(json!({"text": self.read_text(id, &src, lines, src == "recent-unwrapped"), "type": "agent_read"}))
            }
            "agent.prompt" => {
                // the text as a paste (bracketed when the agent asked for that), then Enter a moment later: the agent takes it as typed and sent
                let id = self.pane_of(p)?;
                let text = s(p, "text").unwrap_or("").to_string();
                let q = self.panes.get_mut(&id).unwrap();
                if q.screen().bracketed_paste() { q.input(format!("\x1b[200~{text}\x1b[201~").as_bytes()); } else { q.input(text.as_bytes()); }
                self.later.push((std::time::Instant::now() + std::time::Duration::from_millis(250), id, b"\r".to_vec()));
                Ok(json!({}))
            }
            "agent.focus" => { let id = self.pane_of(p)?; self.set_focus(id); Ok(json!({"pane": self.pane_json(&self.panes[&id]), "type": "pane_info"})) }
            m => Err(("unknown_method", format!("no method {m}"))),
        }
    }
}
