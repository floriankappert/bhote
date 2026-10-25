//! The side panels of bhote, built into the mux: ONE left panel, ONE right panel and the chat strip at the bottom, for every workspace and tab alike
//! (they are not part of any tab: switching a workspace changes what the main area shows and nothing in the panels, which keep running, their scroll and their selection).
//! What a panel shows is a list of sections (settings PANEL_LEFT, PANEL_RIGHT); the widths are fixed (PANEL_LEFT_WIDTH, the right one PANEL_MIN_WIDTH..PANEL_MAX_WIDTH)
//! and give way, left panel last, when the window gets narrow; a panel that does not fit is not drawn (its program keeps running).
use super::server::{Server, Spawn};
use std::path::PathBuf;

pub const LEFT: &str = "bhote-left";
pub const RIGHT: &str = "bhote-right";
pub const CHAT: &str = "bhote-chat";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge { Left, Right, Bottom }

/// A side panel: where it is, its program's pane, the size it wants (columns, rows for the bottom one).
#[derive(Clone, Debug)]
pub struct Sidebar { pub edge: Edge, pub pane: u32, pub want: u16, /// a panel of your own, stacked under the bhote panel of its column (want = its rows)
    pub extra: bool, pub name: String }

/// A panel of your own: `PANEL_<NAME>_CMD` is its program, `PANEL_<NAME>` its column (left, right; default right), `PANEL_<NAME>_HEIGHT` its rows.
pub struct Extra { pub name: String, pub edge: Edge, pub cmd: String, pub rows: u16 }

pub struct PanelCfg {
    pub enabled: bool, pub left: u16, pub right: u16, pub keep: u16, pub chat: bool, pub chat_h: u16,
    /// the sections of the panels (None: no such panel)
    pub left_sections: Option<String>, pub right_sections: Option<String>,
    pub extras: Vec<Extra>,
}

fn num(cfg: &std::collections::HashMap<String, String>, k: &str, d: u16, lo: u16, hi: u16) -> u16 { cfg.get(k).and_then(|v| v.parse::<u16>().ok()).filter(|v| *v >= lo && *v <= hi).unwrap_or(d) }

pub fn config_path() -> String {
    std::env::var("BHOTE_CONFIG").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        let base = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config"))).unwrap_or_default();
        format!("{base}/bhote/config")
    })
}

/// The widths of the left and the right panel (0: none) for a row of `total` columns: the main area keeps `keep`, the left panel
/// has priority (it shrinks to 24 at the narrowest), the right one takes what is left (not below 30, else it stays out).
pub fn plan_widths(total: u16, left: u16, right: u16, keep: u16) -> (u16, u16) {
    let need = |l: u16, r: u16| l + r + keep + (l > 0) as u16 + (r > 0) as u16;
    let (mut l, mut r) = (left, right);
    if need(l, r) > total {
        r = total.saturating_sub(keep + l + 2);
        if r < 30 { r = 0; }
    }
    if need(l, r) > total {
        l = total.saturating_sub(keep + 1);
        if l < 24 { l = 0; }
    }
    (l, r)
}

impl PanelCfg {
    pub fn load() -> PanelCfg {
        let cfg = crate::config_read(&config_path());
        let (lo, hi) = (num(&cfg, "PANEL_MIN_WIDTH", 46, 30, 200), num(&cfg, "PANEL_MAX_WIDTH", 64, 30, 300));
        let want = std::env::var("BHOTE_PANEL_WIDTH").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(56);
        let single = cfg.get("PANEL_LAYOUT").map_or(false, |v| v == "single");
        let list = |k: &str, d: &str| -> Option<String> { match cfg.get(k).map(|v| v.trim()) { Some("off") | Some("none") => None, Some(v) if !v.is_empty() => Some(v.to_string()), _ => Some(d.to_string()) } };
        PanelCfg {
            enabled: cfg.get("AUTOSTART").map_or(true, |v| v != "off") && bhote_bin().is_some(),
            left: num(&cfg, "PANEL_LEFT_WIDTH", 40, 24, 120),
            right: want.clamp(lo, hi.max(lo)),
            keep: std::env::var("BHOTE_AGENT_MIN_WIDTH").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            chat: cfg.get("CHAT_PANEL").map_or(false, |v| matches!(v.as_str(), "bottom" | "right" | "on")),
            chat_h: num(&cfg, "CHAT_HEIGHT", 14, 6, 80),
            // PANEL_LAYOUT=single: one panel on the right with everything (as the panel was before there were two)
            left_sections: if single { None } else { list("PANEL_LEFT", "workspaces,agents,overview") },
            extras: {
                let mut v: Vec<Extra> = cfg.iter().filter_map(|(k, cmd)| {
                    let name = k.strip_prefix("PANEL_")?.strip_suffix("_CMD")?; if name.is_empty() || cmd.trim().is_empty() { return None; }
                    let edge = if cfg.get(&format!("PANEL_{name}")).map_or(false, |e| e == "left") { Edge::Left } else { Edge::Right };
                    if cfg.get(&format!("PANEL_{name}")).map_or(false, |e| e == "off") { return None; }
                    Some(Extra { name: name.to_ascii_lowercase(), edge, cmd: cmd.clone(), rows: num(&cfg, &format!("PANEL_{name}_HEIGHT"), 12, 4, 80) })
                }).collect();
                v.sort_by(|a, b| a.name.cmp(&b.name)); v
            },
            right_sections: if single { Some("head,agents,overview,topics,done,deployments,tests".into()) } else { list("PANEL_RIGHT", "topics,done,deployments,tests") },
        }
    }
}

/// The program a panel pane runs: BHOTE_BIN, the `bhote` script of the checkout this program was built in, the PATH, the usual places.
pub fn bhote_bin() -> Option<PathBuf> {
    if let Some(b) = std::env::var_os("BHOTE_BIN").filter(|b| !b.is_empty()) { return Some(PathBuf::from(b)); }
    let exe = std::env::current_exe().ok()?;
    let mut cands: Vec<PathBuf> = Vec::new();
    for up in exe.ancestors().skip(1).take(5) { cands.push(up.join("bhote")); }
    if let Some(p) = std::env::var_os("PATH") { for d in std::env::split_paths(&p) { cands.push(d.join("bhote")); } }
    if let Some(h) = std::env::var_os("HOME") { cands.push(PathBuf::from(h).join(".local/bin/bhote")); }
    cands.extend(["/opt/homebrew/bin/bhote", "/usr/local/bin/bhote", "/usr/bin/bhote"].map(PathBuf::from));
    cands.into_iter().find(|c| c.is_file() && std::fs::metadata(c).map_or(false, |m| { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o111 != 0 }))
}

impl Server {
    pub fn is_panel(&self, pane: u32) -> bool { self.sidebars.iter().any(|s| s.pane == pane) }
    /// The bhote panel of an edge (not the panels of your own under it).
    pub fn sidebar_at(&self, edge: Edge) -> Option<&Sidebar> { self.sidebars.iter().find(|s| s.edge == edge && !s.extra) }
    pub fn is_extra(&self, pane: u32) -> bool { self.sidebars.iter().any(|s| s.pane == pane && s.extra) }

    /// The pane that gets the keys: the focused side panel, else the focused pane of the shown tab.
    pub fn input_pane(&self) -> Option<u32> { self.sb_focus.filter(|p| self.panes.contains_key(p)).or_else(|| self.world.focused_pane()) }

    /// The sizes the side panels have in a window of this size: (left, right, bottom) columns / rows, 0 when one does not fit. Hidden: all 0.
    pub fn sidebar_sizes(&self, cols: u16, rows: u16) -> (u16, u16, u16) {
        if self.sb_hidden { return (0, 0, 0); }
        let want = |e: Edge| self.sidebar_at(e).map_or(0, |s| s.want);
        let (l, r) = plan_widths(cols, want(Edge::Left), want(Edge::Right), 30);
        let keep = 10;
        let b = if want(Edge::Bottom) > 0 && rows >= want(Edge::Bottom) + 1 + keep { want(Edge::Bottom) } else { 0 };
        (l, r, b)
    }

    fn panel_spawn(&self, sections: &str, label: &str) -> Option<Spawn> {
        let bin = bhote_bin()?;
        let side = super::super::cards::Sections::parse(sections).map_or(0, |s| s.side());
        Some(Spawn { cmd: vec![bin.to_string_lossy().into_owned()], cwd: None, env: vec![
            ("BHOTE_PANEL".into(), "1".into()), ("BHOTE_RUST_PANEL".into(), "1".into()), ("BHOTE_GLOBAL".into(), "1".into()),
            ("BHOTE_SECTIONS".into(), sections.to_string()), ("BHOTE_SIDE".into(), ["", "left", "right"][side as usize].into()), ("BHOTE_LABEL".into(), label.to_string())] })
    }

    fn chat_spawn(&self) -> Option<Spawn> {
        let exe = std::env::current_exe().ok()?;
        Some(Spawn { cmd: vec![exe.to_string_lossy().into_owned(), "chat".into()], cwd: None, env: vec![("BHOTE_GLOBAL".into(), "1".into())] })
    }

    /// The side panels that are not there yet are made (once a client shows a size). A panel whose program ended stays away until `open_panel`.
    pub fn ensure_sidebars(&mut self, force: bool) {
        if self.clients.iter().all(|c| c.view.is_some()) || self.ensuring { return; }
        if force { self.sb_dismissed.clear(); }
        let cfg = PanelCfg::load();
        if !cfg.enabled { return; }
        self.ensuring = true;
        let mut todo: Vec<(Edge, Spawn, u16, String, bool)> = Vec::new();
        let sz = |e: &str, d: u16| self.sb_sizes.get(e).copied().unwrap_or(d);
        if let Some(s) = &cfg.left_sections { if let Some(sp) = self.panel_spawn(s, LEFT) { todo.push((Edge::Left, sp, sz("left", cfg.left), LEFT.into(), false)); } }
        if let Some(s) = &cfg.right_sections { if let Some(sp) = self.panel_spawn(s, RIGHT) { todo.push((Edge::Right, sp, sz("right", cfg.right), RIGHT.into(), false)); } }
        if cfg.chat { if let Some(sp) = self.chat_spawn() { todo.push((Edge::Bottom, sp, sz("bottom", cfg.chat_h), CHAT.into(), false)); } }
        for x in &cfg.extras {
            let sp = Spawn { cmd: vec!["sh".into(), "-c".into(), x.cmd.clone()], cwd: None, env: vec![("BHOTE_GLOBAL".into(), "1".into()), ("BHOTE_PANEL".into(), "1".into())] };
            todo.push((x.edge, sp, x.rows, format!("bhote-x-{}", x.name), true));
        }
        for (edge, sp, want, label, extra) in todo {
            if self.sidebars.iter().any(|s| s.name == label) || self.sb_dismissed.contains(&label) { continue; }
            if let Ok(pane) = self.make_global_pane(&sp, &label) { self.sidebars.push(Sidebar { edge, pane, want, extra, name: label }); }
        }
        self.ensuring = false;
    }

    /// A side panel's pane ended or was closed: it is gone, and not made again by itself.
    pub fn sidebar_closed(&mut self, pane: u32) {
        if let Some(i) = self.sidebars.iter().position(|s| s.pane == pane) {
            let s = self.sidebars.remove(i);
            self.sb_dismissed.push(s.name);
            if self.sb_focus == Some(pane) { self.sb_focus = None; }
        }
    }

    /// The jump key: into a side panel (the right one, else the left) and back to the pane that had the focus.
    pub fn focus_panel(&mut self) {
        if self.sb_focus.is_some() { self.sb_focus = None; self.dirty = true; return; }
        if self.sb_hidden { self.sb_hidden = false; self.after_change(); }
        let cols = self.clients.iter().filter(|c| c.view.is_none()).map(|c| c.cols).min().unwrap_or(80);
        let rows = self.clients.iter().filter(|c| c.view.is_none()).map(|c| c.rows).min().unwrap_or(24);
        let (l, r, _) = self.sidebar_sizes(cols, rows);
        let pick = if r > 0 { self.sidebar_at(Edge::Right) } else if l > 0 { self.sidebar_at(Edge::Left) } else { None };
        if let Some(s) = pick { self.sb_focus = Some(s.pane); self.dirty = true; }
    }

    /// The panels go away (the main area gets the room) and come back; their programs keep running.
    pub fn toggle_sidebars(&mut self) { self.sb_hidden = !self.sb_hidden; if self.sb_hidden { self.sb_focus = None; } self.after_change(); }

    /// A floating pane over the shown tab (`bhote find`, a search): it gets the focus and ends with its process.
    pub fn open_popup(&mut self, cmd: Vec<String>) {
        let Some((wi, ti)) = self.world.ws.get(self.world.active).map(|w| (self.world.active, w.active)) else { return };
        if let Some(old) = self.world.ws[wi].tabs[ti].popup { self.close_pane(old); }
        let tid = self.world.ws[wi].tabs[ti].id;
        let sp = Spawn { cmd, cwd: self.world.focused_pane().and_then(|f| self.panes.get(&f)).map(|p| p.foreground().1), env: Vec::new() };
        let Ok(pid) = self.make_pane(wi, tid, &sp) else { return };
        let t = &mut self.world.ws[wi].tabs[ti];
        t.popup_prev = t.focus; t.popup = Some(pid); t.focus = pid;
        self.sb_focus = None;
        self.after_change();
    }
}

#[cfg(test)]
mod tests {
    use super::plan_widths;
    #[test]
    fn widths_for_narrow_windows() {
        assert_eq!(plan_widths(200, 40, 56, 30), (40, 56));
        assert_eq!(plan_widths(120, 40, 56, 30), (40, 48));   // the right panel gives way
        assert_eq!(plan_widths(100, 40, 56, 30), (40, 0));    // too narrow for it: gone
        assert_eq!(plan_widths(80, 40, 56, 30), (40, 0));
        assert_eq!(plan_widths(60, 40, 56, 30), (29, 0));     // the left one shrinks, the main area keeps 30
        assert_eq!(plan_widths(50, 40, 56, 30), (0, 0));
    }
}
