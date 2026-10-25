//! The panels of bhote as built-in panes: every tab gets a right panel (and with PANEL_LAYOUT=split a left one) next to its main pane,
//! as the herdr plugin did (open_panel.sh), from the same settings (AUTOSTART, PANEL_LAYOUT, PANEL_LEFT_WIDTH, PANEL_MIN_WIDTH, PANEL_MAX_WIDTH).
use super::layout::Dir;
use super::server::{Server, Spawn};
use std::path::PathBuf;

pub const LEFT: &str = "bhote-left";
pub const RIGHT: &str = "bhote-right";
pub const CHAT: &str = "bhote-chat";

/// `chat`: off, bottom (under the main pane) or right (under the right panel); `chat_h` its height in rows
pub struct PanelCfg { pub enabled: bool, pub split: bool, pub left: u16, pub right: u16, pub keep: u16, pub chat: String, pub chat_h: u16 }

fn num(cfg: &std::collections::HashMap<String, String>, k: &str, d: u16, lo: u16, hi: u16) -> u16 { cfg.get(k).and_then(|v| v.parse::<u16>().ok()).filter(|v| *v >= lo && *v <= hi).unwrap_or(d) }

pub fn config_path() -> String {
    std::env::var("BHOTE_CONFIG").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        let base = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config"))).unwrap_or_default();
        format!("{base}/bhote/config")
    })
}

impl PanelCfg {
    pub fn load() -> PanelCfg {
        let cfg = crate::config_read(&config_path());
        let (lo, hi) = (num(&cfg, "PANEL_MIN_WIDTH", 46, 30, 200), num(&cfg, "PANEL_MAX_WIDTH", 64, 30, 300));
        let want = std::env::var("BHOTE_PANEL_WIDTH").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(56);
        PanelCfg {
            enabled: cfg.get("AUTOSTART").map_or(true, |v| v != "off") && bhote_bin().is_some(),
            split: cfg.get("PANEL_LAYOUT").map_or(true, |v| v != "single"),
            left: num(&cfg, "PANEL_LEFT_WIDTH", 40, 30, 64),
            right: want.clamp(lo, hi.max(lo)),
            keep: std::env::var("BHOTE_AGENT_MIN_WIDTH").ok().and_then(|v| v.parse().ok()).unwrap_or(64),
            chat: cfg.get("CHAT_PANEL").filter(|v| matches!(v.as_str(), "bottom" | "right")).cloned().unwrap_or_else(|| "off".into()),
            chat_h: num(&cfg, "CHAT_HEIGHT", 14, 6, 80),
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
    pub fn is_panel(&self, pane: u32) -> bool { self.panes.get(&pane).map_or(false, |p| matches!(p.label.as_str(), LEFT | RIGHT | CHAT | "bhote")) }

    fn panel_spawn(&self, side: &str) -> Option<Spawn> {
        let bin = bhote_bin()?;
        Some(Spawn { cmd: vec![bin.to_string_lossy().into_owned()], cwd: None, env: vec![("BHOTE_PANEL".into(), "1".into()), ("BHOTE_RUST_PANEL".into(), "1".into()), ("BHOTE_SIDE".into(), if side == LEFT { "left".into() } else { "right".into() })] })
    }

    fn chat_spawn(&self) -> Option<Spawn> {
        let exe = std::env::current_exe().ok()?;
        Some(Spawn { cmd: vec![exe.to_string_lossy().into_owned(), "chat".into()], cwd: None, env: Vec::new() })
    }

    /// Every tab without panels (that the user did not close them in) gets them, once a client shows a size to fit them into.
    pub fn ensure_panels(&mut self) {
        if self.ensuring || self.clients.is_empty() { return; }
        let cfg = PanelCfg::load();
        if !cfg.enabled { return; }
        self.ensuring = true;
        let mut todo = Vec::new();
        for (wi, w) in self.world.ws.iter().enumerate() { for (ti, t) in w.tabs.iter().enumerate() { if !t.no_panels { todo.push((w.id, t.id, wi, ti)); } } }
        for (wid, tid, _, _) in todo { self.make_panels(wid, tid, &cfg, false); }
        self.ensuring = false;
    }

    /// The panels of one tab (missing ones only). `force` also overrides "the user closed them".
    pub fn make_panels(&mut self, wid: u32, tid: u32, cfg: &PanelCfg, force: bool) {
        let Some((wi, ti)) = self.world.find_tab(wid, tid) else { return };
        if force { self.world.ws[wi].tabs[ti].no_panels = false; }
        let mut leaves = Vec::new(); self.world.ws[wi].tabs[ti].root.leaves(&mut leaves);
        let has = |s: &Server, names: &[&str]| leaves.iter().any(|p| s.panes.get(p).map_or(false, |q| names.contains(&q.label.as_str())));
        let has_right = has(self, &[RIGHT, "bhote"]);
        let has_left = has(self, &[LEFT]);
        let Some(main) = leaves.iter().copied().find(|p| !self.is_panel(*p)) else { return };
        let total = |s: &Server, p: u32| s.tab_rects_of(wi, ti).iter().find(|(id, _)| *id == p).map(|(_, r)| r.w).unwrap_or(0);
        if !has_right {
            let t = total(self, main);
            if t >= cfg.right + cfg.keep {
                if let Some(sp) = self.panel_spawn(RIGHT) {
                    let ratio = (t as f32 - 1.0 - cfg.right as f32) / (t as f32 - 1.0);
                    if let Ok(new) = self.split_pane(main, Dir::Right, ratio, &sp, false) { self.panes.get_mut(&new).unwrap().label = RIGHT.into(); }
                }
            }
        }
        if cfg.split && !has_left {
            let t = total(self, main);
            if t >= cfg.left + cfg.keep {
                if let Some(sp) = self.panel_spawn(LEFT) {
                    let ratio = cfg.left as f32 / (t as f32 - 1.0);
                    if let Ok(new) = self.split_pane(main, Dir::Right, ratio, &sp, false) {
                        self.panes.get_mut(&new).unwrap().label = LEFT.into();
                        if let Some((wi, ti)) = self.world.find_tab(wid, tid) { self.world.ws[wi].tabs[ti].root.swap(main, new); }
                    }
                }
            }
        }
        if cfg.chat != "off" { self.make_chat(wid, tid, cfg); }
    }

    /// (kept inside make_panels) the chat pane: under the main pane or under the right panel.
    fn make_chat(&mut self, wid: u32, tid: u32, cfg: &PanelCfg) {
        let Some((wi, ti)) = self.world.find_tab(wid, tid) else { return };
        let mut leaves = Vec::new(); self.world.ws[wi].tabs[ti].root.leaves(&mut leaves);
        if leaves.iter().any(|p| self.panes.get(p).map_or(false, |q| q.label == CHAT)) { return; }
        let target = if cfg.chat == "right" { leaves.iter().copied().find(|p| self.panes.get(p).map_or(false, |q| matches!(q.label.as_str(), RIGHT | "bhote"))) } else { leaves.iter().copied().find(|p| !self.is_panel(*p)) };
        let Some(target) = target else { return };
        let h = self.tab_rects_of(wi, ti).iter().find(|(id, _)| *id == target).map(|(_, r)| r.h).unwrap_or(0);
        if h < cfg.chat_h + 10 { return; }
        let Some(sp) = self.chat_spawn() else { return };
        let ratio = (h as f32 - 1.0 - cfg.chat_h as f32) / (h as f32 - 1.0);
        if let Ok(new) = self.split_pane(target, Dir::Down, ratio, &sp, false) { self.panes.get_mut(&new).unwrap().label = CHAT.into(); }
    }

    pub fn tab_rects_of(&self, wi: usize, ti: usize) -> Vec<(u32, super::layout::Rect)> { super::world::World::rects(&self.world.ws[wi].tabs[ti], self.area()).0 }

    /// The jump key: from a panel back to the pane that had the focus (else the first other pane); from another pane to the right panel.
    pub fn focus_panel(&mut self) {
        let Some(f) = self.world.focused_pane() else { return };
        if self.is_panel(f) {
            let Some((wi, ti)) = self.world.tab_of_pane(f) else { return };
            let mut l = Vec::new(); self.world.ws[wi].tabs[ti].root.leaves(&mut l);
            let back = self.last_focus.filter(|p| l.contains(p) && !self.is_panel(*p)).or_else(|| l.iter().copied().find(|p| !self.is_panel(*p)));
            if let Some(b) = back { self.set_focus(b); }
        } else {
            let Some((wi, ti)) = self.world.tab_of_pane(f) else { return };
            let (wid, tid) = (self.world.ws[wi].id, self.world.ws[wi].tabs[ti].id);
            let find = |s: &Server| { let mut l = Vec::new(); s.world.ws[wi].tabs[ti].root.leaves(&mut l); l.into_iter().find(|p| s.panes.get(p).map_or(false, |q| matches!(q.label.as_str(), RIGHT | "bhote"))) };
            if find(self).is_none() { let cfg = PanelCfg::load(); self.make_panels(wid, tid, &cfg, true); }
            if let Some(p) = find(self) { self.set_focus(p); }
        }
    }

    /// A floating pane over the shown tab (`bhote find`, a search): it gets the focus and ends with its process.
    pub fn open_popup(&mut self, cmd: Vec<String>) {
        let Some((wi, ti)) = self.world.ws.get(self.world.active).map(|w| (self.world.active, w.active)) else { return };
        if let Some(old) = self.world.ws[wi].tabs[ti].popup { self.close_pane(old); }
        let tid = self.world.ws[wi].tabs[ti].id;
        let sp = Spawn { cmd, cwd: self.world.focused_pane().and_then(|f| self.panes.get(&f)).map(|p| p.foreground().1), env: Vec::new() };
        let Ok(pid) = self.make_pane(wi, tid, &sp) else { return };
        let t = &mut self.world.ws[wi].tabs[ti];
        t.popup_prev = t.focus; t.popup = Some(pid); t.focus = pid;
        self.after_change();
    }
}
