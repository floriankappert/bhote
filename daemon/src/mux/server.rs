//! `bhoted mux`: the pane server. One event loop owns all state; threads only read (ptys, sockets) and send events.
use super::input;
use super::keymap::Keymap;
use super::layout::{Dir, Rect};
use super::pane::Pane;
use super::proto::{self, read_frame, write_frame};
use super::pty::{self, Pty};
use super::render::{self, Cell, Grid};
use super::world::{self, Tab, World};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vt100::Color;

fn bar_mode_load() -> String {
    let v = crate::config_read(&super::panels::config_path()).get("MUX_BAR").cloned().unwrap_or_default();
    match v.as_str() { "on" | "full" | "off" => v, _ => "auto".into() }
}

pub enum Ev {
    PaneOut(u32, Vec<u8>),
    PaneExit(u32),
    NewClient { id: u32, stream: UnixStream, cols: u16, rows: u16, view: Option<String> },
    Frame(u32, u8, Vec<u8>),
    ClientGone(u32),
    Control(Value, Sender<Value>),
    Subscribe(UnixStream, Vec<(String, Option<String>)>),
}

/// What the terminal of a client has to be told about the focused pane (keys and mouse come back in the form the program asked for).
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Modes { cursor_keys: bool, keypad: bool, paste: bool, mouse: u8, enc: u8 }

impl Modes {
    fn of(s: &vt100::Screen) -> Modes {
        // the mouse is always reported to the mux (button motion, SGR); it hands the events on to the pane that asked for them
        Modes { cursor_keys: s.application_cursor(), keypad: s.application_keypad(), paste: s.bracketed_paste(), mouse: 3, enc: 2 }
    }
    /// The bytes that turn what the terminal has (`prev`) into `self`.
    fn diff(&self, prev: &Modes) -> String {
        let mut o = String::new();
        let onoff = |b: bool| if b { 'h' } else { 'l' };
        if self.cursor_keys != prev.cursor_keys { o.push_str(&format!("\x1b[?1{}", onoff(self.cursor_keys))); }
        if self.keypad != prev.keypad { o.push_str(if self.keypad { "\x1b=" } else { "\x1b>" }); }
        if self.paste != prev.paste { o.push_str(&format!("\x1b[?2004{}", onoff(self.paste))); }
        if self.mouse != prev.mouse {
            o.push_str("\x1b[?1000l\x1b[?1002l\x1b[?1003l");
            match self.mouse { 1 | 2 => o.push_str("\x1b[?1000h"), 3 => o.push_str("\x1b[?1002h"), 4 => o.push_str("\x1b[?1003h"), _ => {} }
        }
        if self.enc != prev.enc {
            o.push_str("\x1b[?1005l\x1b[?1006l");
            match self.enc { 1 => o.push_str("\x1b[?1005h"), 2 => o.push_str("\x1b[?1006h"), _ => {} }
        }
        o
    }
}

pub struct Client {
    pub id: u32, pub stream: UnixStream, pub cols: u16, pub rows: u16,
    /// a viewer of one pane (`bhote view`): shows that pane only, sends every key to it, and does not change any pane's size
    pub view: Option<String>,
    pub(super) prev: Option<Grid>, pub(super) modes: Modes,
    pub parser: input::Parser, pub mode: super::actions::Mode, pub sel: Option<super::actions::Selection>, pub drag: Option<super::actions::Drag>,
    /// when a lone escape byte is the escape key (no more bytes came)
    pub esc_at: Option<Instant>,
}

/// What a new pane is started with.
#[derive(Default, Clone)]
pub struct Spawn { pub cmd: Vec<String>, pub cwd: Option<String>, pub env: Vec<(String, String)> }

pub struct Server {
    tx: Sender<Ev>,
    pub panes: BTreeMap<u32, Pane>,
    pub world: World,
    next_pane: u32,
    pub clients: Vec<Client>,
    pub dirty: bool,
    pub quit: bool,
    pub cwd: String,
    /// added to the environment of every pane: PATH with the `herdr` shim in front, the socket
    pub env: Vec<(String, String)>,
    pub km: Keymap,
    pub last_focus: Option<u32>,
    /// OSC 52 sequences to hand to every terminal
    pub clip: Vec<String>,
    /// panels are being made (their splits must not start that again)
    pub ensuring: bool,
    /// bytes for a pane later (the Enter after a prompt): when, pane, bytes
    pub later: Vec<(Instant, u32, Vec<u8>)>,
    /// a notification line on the bar and until when
    pub toast: Option<(String, Instant)>,
    pub last_detect: Option<Instant>,
    /// the session file (session.rs) and what was written to it last
    pub state_file: Option<std::path::PathBuf>,
    pub state_saved: String,
    pub state_at: Option<Instant>,
    /// event subscribers (events.subscribe): the connection and what it wants (event type, optional pane)
    pub subs: Vec<(UnixStream, Vec<(String, Option<String>)>)>,
    /// the kharka sidecar (sidecar.rs)
    pub sidecar: Option<std::process::Child>,
    pub sidecar_at: Option<Instant>,
    pub sidecar_first: bool,
    /// MUX_BAR: auto (the bar of the tabs only when a workspace has more than one tab), on (always), full (also the workspaces), off
    pub bar_mode: String,
    pub no_client_at: Option<Instant>,
    pub warm_at: Option<Instant>,
}

impl Server {
    pub fn new(tx: Sender<Ev>, env: Vec<(String, String)>) -> Server {
        Server { tx, panes: BTreeMap::new(), world: World::default(), next_pane: 1, clients: Vec::new(), dirty: true, quit: false, cwd: std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(), env, km: Keymap::load(&super::actions::keys_path()), last_focus: None, clip: Vec::new(), ensuring: false, later: Vec::new(), toast: None, last_detect: None, state_file: None, state_saved: String::new(), state_at: None, subs: Vec::new(), sidecar: None, sidecar_at: None, sidecar_first: true, bar_mode: bar_mode_load(), no_client_at: None, warm_at: None }
    }

    /// The size panes are laid out for: what the smallest attached client shows (80x24 without a client), the last row is the bar.
    pub fn area(&self) -> Rect {
        let (c, r) = self.clients.iter().filter(|c| c.view.is_none()).map(|c| (c.cols, c.rows)).reduce(|a, b| (a.0.min(b.0), a.1.min(b.1))).unwrap_or((80, 24));
        Rect { x: 0, y: 0, w: c, h: r.saturating_sub(self.bar_rows()).max(1) }
    }

    /// 1 when the bar is there: always with `on` and `full`, with `auto` when some workspace has more than one tab.
    pub fn bar_rows(&self) -> u16 {
        match self.bar_mode.as_str() { "off" => 0, "on" | "full" => 1, _ => self.world.ws.iter().any(|w| w.tabs.len() > 1) as u16 }
    }

    /// A pane process in the workspace `ws` for tab `tab` (not yet in the tree).
    pub fn make_pane(&mut self, wi: usize, tab: u32, sp: &Spawn) -> std::io::Result<u32> { self.make_pane_n(wi, tab, None, sp) }

    /// `n`: the number inside the workspace (a restored pane keeps its name), else the next free one.
    pub fn make_pane_n(&mut self, wi: usize, tab: u32, n: Option<u32>, sp: &Spawn) -> std::io::Result<u32> {
        let id = self.next_pane; self.next_pane += 1;
        let w = &mut self.world.ws[wi];
        let n = match n { Some(n) => { w.next_pane = w.next_pane.max(n + 1); n } None => { let n = w.next_pane; w.next_pane += 1; n } };
        let cwd = sp.cwd.clone().unwrap_or_else(|| if w.cwd.is_empty() { self.cwd.clone() } else { w.cwd.clone() });
        let name = world::pane_name(w.id, n);
        let mut env = self.env.clone();
        env.extend([("HERDR_PANE_ID".to_string(), name), ("HERDR_TAB_ID".to_string(), world::tab_name(w.id, tab)), ("HERDR_WORKSPACE_ID".to_string(), world::ws_name(w.id)), ("BHOTE_MUX".to_string(), "native".to_string()), ("HERDR_ENV".to_string(), "1".to_string())]);
        env.extend(sp.env.iter().cloned());
        let a = self.area();
        let pty = Pty::spawn(&sp.cmd, Some(std::path::Path::new(&cwd)), a.w, a.h, &env)?;
        let reader = pty.reader()?;
        let tx = self.tx.clone();
        std::thread::spawn(move || { pty::read_loop(reader, |b| tx.send(Ev::PaneOut(id, b)).is_ok()); let _ = tx.send(Ev::PaneExit(id)); });
        self.panes.insert(id, Pane::new(id, (self.world.ws[wi].id, n, tab), pty, a.w, a.h, sp.cmd.clone(), cwd));
        let (name, pj) = (self.panes[&id].name(), self.pane_json(&self.panes[&id]));
        self.emit("pane.created", None, json!({"pane": pj, "pane_id": name}));
        Ok(id)
    }

    fn undo_pane(&mut self, wi: usize, id: u32) { self.panes.remove(&id); let _ = wi; }

    pub fn create_workspace(&mut self, label: &str, sp: &Spawn, focus: bool) -> std::io::Result<(usize, u32)> {
        let cwd = sp.cwd.clone().unwrap_or_else(|| self.cwd.clone());
        let label = if label.is_empty() { std::path::Path::new(&cwd).file_name().map(|f| f.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "/".into()) } else { label.to_string() };
        let wi = self.world.new_workspace(&label, &cwd);
        let pid = match self.make_pane(wi, 1, sp) { Ok(p) => p, Err(e) => { self.world.ws.remove(wi); self.world.next_ws -= 1; return Err(e); } };
        let w = &mut self.world.ws[wi];
        w.next_tab = 2;
        w.tabs.push(Tab::new(1, "1".into(), pid));
        if focus || self.world.ws.len() == 1 { self.world.active = wi; }
        self.after_change();
        let wid = world::ws_name(self.world.ws[wi].id);
        self.emit("workspace.created", None, json!({"workspace_id": wid}));
        Ok((wi, pid))
    }

    pub fn create_tab(&mut self, wi: usize, label: &str, sp: &Spawn, focus: bool) -> std::io::Result<(u32, u32)> {
        let tid = self.world.ws[wi].next_tab;
        let pid = self.make_pane(wi, tid, sp)?;
        let w = &mut self.world.ws[wi];
        w.next_tab += 1;
        let label = if label.is_empty() { (w.tabs.len() + 1).to_string() } else { label.to_string() };
        w.tabs.push(Tab::new(tid, label, pid));
        if focus { w.active = w.tabs.len() - 1; self.world.active = wi; }
        self.after_change();
        Ok((tid, pid))
    }

    /// `target` is cut in two; the new pane is on the right or below. `ratio` is what the old pane keeps.
    pub fn split_pane(&mut self, target: u32, dir: Dir, ratio: f32, sp: &Spawn, focus: bool) -> std::io::Result<u32> {
        let (wi, ti) = self.world.tab_of_pane(target).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such pane"))?;
        let tab = self.world.ws[wi].tabs[ti].id;
        let mut sp = sp.clone();
        if sp.cwd.is_none() { sp.cwd = self.panes.get(&target).map(|p| p.foreground().1); }
        let pid = self.make_pane(wi, tab, &sp)?;
        let t = &mut self.world.ws[wi].tabs[ti];
        t.zoomed = None;
        if !t.root.split(target, pid, dir, ratio) { self.undo_pane(wi, pid); return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no such pane")); }
        if focus { t.focus = pid; self.world.active = wi; self.world.ws[wi].active = ti; }
        self.after_change();
        Ok(pid)
    }

    pub fn close_pane(&mut self, id: u32) {
        if self.is_panel(id) { if let Some((wi, ti)) = self.world.tab_of_pane(id) { self.world.ws[wi].tabs[ti].no_panels = true; } }
        let closed = self.panes.get(&id).map(|p| (p.name(), world::ws_name(p.ws)));
        self.panes.remove(&id);
        self.world.remove_pane(id);
        if let Some((n, w)) = closed { self.emit("pane.closed", Some(&n.clone()), json!({"pane_id": n, "workspace_id": w})); }
        if self.world.ws.is_empty() { self.end_clients("exited"); self.quit = true; if let Some(f) = &self.state_file { let _ = std::fs::remove_file(f); } self.state_file = None; }
        self.after_change();
    }

    /// The panes get the size their rectangle has now (also those of tabs that are not shown).
    pub fn after_change(&mut self) {
        self.ensure_panels();
        let area = self.area();
        let mut sizes = Vec::new();
        for w in &self.world.ws { for t in &w.tabs {
            let (r, _) = World::rects(t, area); sizes.extend(r);
            if let Some(p) = t.popup { let q = World::popup_rect(area); sizes.push((p, Rect { x: q.x + 1, y: q.y + 1, w: q.w.saturating_sub(2).max(1), h: q.h.saturating_sub(2).max(1) })); }
        } }
        for (id, r) in sizes { if let Some(p) = self.panes.get_mut(&id) { p.resize(r.w, r.h); } }
        self.dirty = true;
    }

    /// What happens without an event: delayed keys, the agents, the toast.
    fn tick(&mut self) {
        let now = Instant::now();
        let due: Vec<(Instant, u32, Vec<u8>)> = { let (d, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later).into_iter().partition(|(t, _, _)| *t <= now); self.later = rest; d };
        for (_, id, b) in due { if let Some(p) = self.panes.get_mut(&id) { p.input(&b); } }
        if self.last_detect.map_or(true, |t| now.duration_since(t) >= Duration::from_millis(400)) { self.last_detect = Some(now); self.detect_agents(); }
        if self.toast.as_ref().map_or(false, |(_, t)| *t <= now) { self.toast = None; self.dirty = true; }
        self.save_state(false);
        self.sidecar_tick();
        self.warm_tick();
        // views of workspaces of other machines hold an ssh connection (and a size over there): they end when nobody has looked for a while
        if self.clients.iter().all(|c| c.view.is_some()) { if self.no_client_at.is_none() { self.no_client_at = Some(now); } } else { self.no_client_at = None; }
        if self.no_client_at.map_or(false, |t| now.duration_since(t) > Duration::from_secs(std::env::var("BHOTE_VIEW_GRACE").ok().and_then(|v| v.parse().ok()).unwrap_or(30))) {
            let hidden: Vec<u32> = self.world.ws.iter().filter(|w| w.hidden).flat_map(|w| { let mut l = Vec::new(); for t in &w.tabs { t.root.leaves(&mut l); } l }).collect();
            for p in hidden { self.close_pane(p); }
        }
    }

    pub fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::PaneOut(id, b) => if let Some(p) = self.panes.get_mut(&id) { p.output(&b); self.dirty = true; },
            Ev::PaneExit(id) => if self.panes.contains_key(&id) { self.close_pane(id); },
            Ev::NewClient { id, stream, cols, rows, view } => {
                self.clients.push(Client { id, stream, cols, rows, view, prev: None, modes: Modes::default(), parser: input::Parser::default(), mode: super::actions::Mode::Normal, sel: None, drag: None, esc_at: None });
                self.after_change();
            }
            Ev::Frame(id, t, p) => match t {
                b'I' => self.client_input(id, &p),
                b'R' => if let Some((c, r)) = proto::parse_resize(&p) {
                    if let Some(cl) = self.clients.iter_mut().find(|c| c.id == id) { cl.cols = c; cl.rows = r; cl.prev = None; }
                    self.after_change();
                },
                b'D' => self.detach_client(id, "detached"),
                _ => {}
            },
            Ev::ClientGone(id) => { self.clients.retain(|c| c.id != id); self.after_change(); }
            Ev::Control(req, reply) => { let _ = reply.send(self.control(&req)); }
            Ev::Subscribe(stream, filters) => self.subs.push((stream, filters)),
        }
    }

    /// Tells the subscribers of events.subscribe: one line `{"event": type, "data": ...}`; a subscriber that cannot be written to is dropped.
    pub fn emit(&mut self, ty: &str, pane: Option<&str>, data: Value) {
        if self.subs.is_empty() { return; }
        let line = format!("{}\n", json!({"event": ty, "data": data}));
        self.subs.retain_mut(|(st, filters)| {
            if !filters.iter().any(|(t, p)| t == ty && (p.is_none() || p.as_deref() == pane)) { return true; }
            st.write_all(line.as_bytes()).is_ok()
        });
    }

    pub fn end_clients(&mut self, reason: &str) {
        for c in &mut self.clients { let _ = write_frame(&mut c.stream, b'Q', reason.as_bytes()); }
        self.clients.clear();
    }

    pub fn detach_client(&mut self, id: u32, reason: &str) {
        if let Some(i) = self.clients.iter().position(|c| c.id == id) {
            let mut c = self.clients.remove(i);
            let _ = write_frame(&mut c.stream, b'Q', reason.as_bytes());
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
        }
        self.after_change();
    }

    fn client_input(&mut self, id: u32, bytes: &[u8]) {
        // a viewer types into its pane, nothing else
        if let Some(name) = self.clients.iter().find(|c| c.id == id).and_then(|c| c.view.clone()) {
            if let Some(p) = self.pane_by_name(&name).and_then(|pid| self.panes.get_mut(&pid)) { p.parser.screen_mut().set_scrollback(0); p.input(bytes); }
            return;
        }
        super::actions::on_input(self, id, bytes);
    }

    /// The picture for a viewer: the pane as it is (cut where the viewer is smaller), its cursor, its own modes (mouse only when it asked).
    fn compose_view(&self, ci: usize) -> Option<(Grid, Modes)> {
        let c = &self.clients[ci];
        let p = self.pane_by_name(c.view.as_deref()?).and_then(|id| self.panes.get(&id))?;
        let mut g = Grid::new(c.cols, c.rows);
        let s = p.screen(); let (pr, pc) = s.size();
        g.blit(s, 0, 0, pc.get().min(c.cols), pr.get().min(c.rows));
        if !s.hide_cursor() { let (r, cc) = s.cursor_position(); if r < c.rows && cc < c.cols { g.cursor = Some((cc, r)); } }
        let mut m = Modes::of(s);
        use vt100::{MouseProtocolEncoding as E, MouseProtocolMode as M};
        m.mouse = match s.mouse_protocol_mode() { M::None => 0, M::Press => 1, M::PressRelease => 2, M::ButtonMotion => 3, M::AnyMotion => 4 };
        m.enc = match s.mouse_protocol_encoding() { E::Default => 0, E::Utf8 => 1, E::Sgr => 2 };
        Some((g, m))
    }

    /// The bar under the panes: workspaces, tabs, and on the right what the mode of the client is.
    fn bar(&self, g: &mut Grid, y: u16, c: &Client) {
        let base = Cell { fg: Color::Rgb(170, 176, 200), bg: Color::Rgb(30, 32, 48), ..Cell::default() };
        let hi = Cell { fg: Color::Rgb(20, 22, 34), bg: Color::Rgb(137, 220, 235), bold: true, ..Cell::default() };
        let warn = Cell { fg: Color::Rgb(20, 22, 34), bg: Color::Rgb(250, 179, 135), bold: true, ..Cell::default() };
        let cols = g.cols;
        g.fill(0, y, cols, 1, &base);
        if let super::actions::Mode::Prompt(p) = &c.mode {
            g.put(1, y, &format!("{}: {}", p.label(), p.text), &warn);
            return;
        }
        for (x, text, _, active) in self.bar_items() { g.put(x, y, &text, if active { &hi } else { &base }); }
        if let Some((t, _)) = &self.toast { let w = t.chars().count() as u16 + 2; g.put(cols.saturating_sub(w + 1), y, &format!(" {t} "), &warn); return; }
        let tag = c.mode.tag(&self.km);
        if !tag.is_empty() { let w = tag.chars().count() as u16 + 2; g.put(cols.saturating_sub(w), y, &format!(" {tag} "), &warn); }
    }

    /// Without a bar the mode, a prompt or a notification is shown on the last row, over the pane, as long as there is something to say.
    fn status_overlay(&self, g: &mut Grid, y: u16, c: &Client) {
        let warn = Cell { fg: Color::Rgb(20, 22, 34), bg: Color::Rgb(250, 179, 135), bold: true, ..Cell::default() };
        let cols = g.cols;
        if let super::actions::Mode::Prompt(p) = &c.mode { g.fill(0, y, cols, 1, &warn); g.put(1, y, &format!("{}: {}", p.label(), p.text), &warn); return; }
        let text = if let Some((t, _)) = &self.toast { t.clone() } else { c.mode.tag(&self.km) };
        if text.is_empty() { return; }
        let w = text.chars().count() as u16 + 2;
        g.put(cols.saturating_sub(w), y, &format!(" {text} "), &warn);
    }

    /// What is on the bar: x, text, what a click does, whether it is the shown one.
    pub fn bar_items(&self) -> Vec<(u16, String, super::actions::Click, bool)> {
        let mut v = Vec::new();
        let mut x = 1u16;
        let width = |t: &str| t.chars().map(|c| if render::wide(c) { 2 } else { 1 }).sum::<u16>();
        if self.bar_mode == "full" { for (i, w) in self.world.ws.iter().enumerate().filter(|(_, w)| !w.hidden) {
            let t = format!(" {} {} ", i + 1, w.label);
            let n = width(&t); v.push((x, t, super::actions::Click::Workspace(i), i == self.world.active)); x += n;
        } }
        if let Some(w) = self.world.ws.get(self.world.active) {
            if self.bar_mode == "full" { x += 3; }
            for (i, t) in w.tabs.iter().enumerate() {
                let tx = format!(" {} ", t.label);
                let n = width(&tx); v.push((x, tx, super::actions::Click::Tab(i), i == w.active)); x += n;
            }
            // the separator between workspaces and tabs
            if self.bar_mode == "full" { v.push((v.iter().filter(|i| matches!(i.2, super::actions::Click::Workspace(_))).map(|i| i.0 + width(&i.1)).max().unwrap_or(1), " │ ".into(), super::actions::Click::None, false)); }
        }
        v
    }

    /// The picture for a client: the panes of the shown tab, the dividers, the bar.
    fn compose(&self, ci: usize) -> (Grid, Modes) {
        if self.clients[ci].view.is_some() { if let Some(v) = self.compose_view(ci) { return v; } return (Grid::new(self.clients[ci].cols, self.clients[ci].rows), Modes::default()); }
        let (cols, rows) = (self.clients[ci].cols, self.clients[ci].rows);
        let mut g = Grid::new(cols, rows);
        let Some((_, tab)) = self.world.current() else { return (g, Modes { mouse: 3, enc: 2, ..Modes::default() }) };
        let area = self.area();
        let (rects, divs) = World::rects(tab, area);
        let dim = Cell { fg: Color::Idx(8), ..Cell::default() };
        for d in &divs {
            match d.dir {
                Dir::Right => for y in d.y..d.y + d.len { g.put(d.x, y, "│", &dim); },
                Dir::Down => for x in d.x..d.x + d.len { g.put(x, d.y, "─", &dim); },
            }
        }
        let mut modes = Modes { mouse: 3, enc: 2, ..Modes::default() };
        for (id, r) in &rects {
            let Some(p) = self.panes.get(id) else { continue };
            let s = p.screen();
            let (pr, pc) = s.size();
            g.blit(s, r.x, r.y, pc.get().min(r.w), pr.get().min(r.h));
            if *id == tab.focus {
                modes = Modes::of(s);
                if !s.hide_cursor() { let (cr, cc) = s.cursor_position(); if cr < r.h && cc < r.w { g.cursor = Some((r.x + cc, r.y + cr)); } }
            }
        }
        if let Some(pid) = tab.popup {
            if let Some(p) = self.panes.get(&pid) {
                let q = World::popup_rect(area);
                let edge = Cell { fg: Color::Rgb(137, 220, 235), ..Cell::default() };
                g.fill(q.x, q.y, q.w, q.h, &Cell::default());
                for x in q.x + 1..q.x + q.w.saturating_sub(1) { g.put(x, q.y, "─", &edge); g.put(x, q.y + q.h - 1, "─", &edge); }
                for y in q.y + 1..q.y + q.h.saturating_sub(1) { g.put(q.x, y, "│", &edge); g.put(q.x + q.w - 1, y, "│", &edge); }
                g.put(q.x, q.y, "╭", &edge); g.put(q.x + q.w - 1, q.y, "╮", &edge); g.put(q.x, q.y + q.h - 1, "╰", &edge); g.put(q.x + q.w - 1, q.y + q.h - 1, "╯", &edge);
                let s = p.screen(); let (pr, pc) = s.size();
                g.blit(s, q.x + 1, q.y + 1, pc.get().min(q.w.saturating_sub(2)), pr.get().min(q.h.saturating_sub(2)));
                modes = Modes::of(s);
                g.cursor = if s.hide_cursor() { None } else { let (cr, cc) = s.cursor_position(); Some((q.x + 1 + cc, q.y + 1 + cr)) };
            }
        }
        super::actions::overlay(self, ci, &rects, &mut g);
        if area.h < rows { self.bar(&mut g, area.h, &self.clients[ci]); } else if rows > 0 { self.status_overlay(&mut g, rows - 1, &self.clients[ci]); }
        (g, modes)
    }

    fn render(&mut self) {
        self.dirty = false;
        let mut extra = String::new();
        if let Some(p) = self.world.focused_pane().and_then(|f| self.panes.get_mut(&f)) {
            let h = p.parser.callbacks_mut();
            for c in h.clipboard.drain(..) { extra.push_str(&c); }
            for c in self.clip.drain(..) { extra.push_str(&c); }
            if std::mem::take(&mut h.bell) { extra.push('\x07'); }
        }
        let ended: Vec<u32> = self.clients.iter().filter(|c| c.view.as_ref().map_or(false, |n| self.pane_by_name(n).is_none())).map(|c| c.id).collect();
        for id in ended { self.detach_client(id, "pane closed"); }
        let mut gone = Vec::new();
        for i in 0..self.clients.len() {
            let (g, modes) = self.compose(i);
            let c = &mut self.clients[i];
            let same = c.prev.as_ref().map_or(false, |p| p.cells == g.cells && p.cursor == g.cursor) && c.modes == modes;
            if same && extra.is_empty() { continue; }
            let mut out = modes.diff(&c.modes);
            if !same { out.push_str(&render::diff(c.prev.as_ref(), &g)); }
            out.push_str(&extra);
            c.modes = modes; c.prev = Some(g);
            if write_frame(&mut c.stream, b'O', out.as_bytes()).is_err() { gone.push(c.id); }
        }
        if !gone.is_empty() { self.clients.retain(|c| !gone.contains(&c.id)); self.after_change(); }
    }
}

fn serve_conn(stream: UnixStream, tx: Sender<Ev>, next_client: Arc<AtomicU32>) -> std::io::Result<()> {
    let mut r = BufReader::new(stream.try_clone()?);
    let mut w = stream;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 { return Ok(()); }
        let Ok(req) = serde_json::from_str::<Value>(line.trim()) else {
            writeln!(w, "{}", json!({"id": null, "error": {"code": "bad_request", "message": "not JSON"}}))?; continue;
        };
        if req.get("method").and_then(|m| m.as_str()) == Some("attach") {
            let p = req.get("params").cloned().unwrap_or(Value::Null);
            let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(80).clamp(1, 1000) as u16;
            let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(24).clamp(1, 1000) as u16;
            let id = next_client.fetch_add(1, Ordering::SeqCst);
            writeln!(w, "{}", json!({"id": req.get("id"), "result": {"attached": true, "client": id}}))?;
            let view = p.get("pane").and_then(|v| v.as_str()).map(String::from);
            if tx.send(Ev::NewClient { id, stream: w.try_clone()?, cols, rows, view }).is_err() { return Ok(()); }
            while let Ok((t, p)) = read_frame(&mut r) { if tx.send(Ev::Frame(id, t, p)).is_err() { return Ok(()); } }
            let _ = tx.send(Ev::ClientGone(id));
            return Ok(());
        }
        if req.get("method").and_then(|m| m.as_str()) == Some("events.subscribe") {
            let subs = req.pointer("/params/subscriptions").and_then(|s| s.as_array()).cloned().unwrap_or_default();
            let mut f = Vec::new();
            for sub in &subs {
                let Some(t) = sub.get("type").and_then(|t| t.as_str()) else { writeln!(w, "{}", json!({"id": req.get("id"), "error": {"code": "invalid_request", "message": "missing field `type`"}}))?; return Ok(()); };
                f.push((t.to_string(), sub.get("pane_id").and_then(|p| p.as_str()).map(String::from)));
            }
            writeln!(w, "{}", json!({"id": req.get("id"), "result": {"type": "subscription_started"}}))?;
            let _ = tx.send(Ev::Subscribe(w.try_clone()?, f));
            return Ok(());
        }
        let (rtx, rrx) = mpsc::channel();
        if tx.send(Ev::Control(req, rtx)).is_err() { return Ok(()); }
        match rrx.recv() { Ok(resp) => writeln!(w, "{resp}")?, Err(_) => return Ok(()) }
    }
}

/// A directory with a `herdr` that is this program: scripts and tools of bhote call `herdr ...`, inside a native pane they get the mux.
pub fn shim_dir(sock: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = sock.parent()?.join("bin");
    std::fs::create_dir_all(&dir).ok()?;
    let exe = std::env::current_exe().ok()?;
    let link = dir.join("herdr");
    if std::fs::read_link(&link).ok().as_deref() != Some(exe.as_path()) { let _ = std::fs::remove_file(&link); std::os::unix::fs::symlink(&exe, &link).ok()?; }
    Some(dir)
}

pub fn run() -> i32 {
    let path = proto::sock_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700));
    }
    if UnixStream::connect(&path).is_ok() { eprintln!("bhoted mux: already running ({})", path.display()); return 1; }
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) { Ok(l) => l, Err(e) => { eprintln!("bhoted mux: {}: {e}", path.display()); return 1; } };
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }
    let (tx, rx): (Sender<Ev>, Receiver<Ev>) = mpsc::channel();
    let next_client = Arc::new(AtomicU32::new(1));
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let (tx, nc) = (tx.clone(), next_client.clone());
                std::thread::spawn(move || { let _ = serve_conn(s, tx, nc); });
            }
        });
    }
    let mut env = vec![("BHOTE_MUX_SOCK".to_string(), path.to_string_lossy().into_owned()), ("HERDR_SOCKET_PATH".to_string(), path.to_string_lossy().into_owned())];
    if let Some(d) = shim_dir(&path) {
        env.push(("PATH".into(), format!("{}:{}", d.display(), std::env::var("PATH").unwrap_or_default())));
    }
    let mut s = Server::new(tx, env);
    s.state_file = Some(super::session::state_path());
    let restored = super::session::load(&mut s);
    if !restored { if let Err(e) = s.create_workspace("", &Spawn::default(), true) { eprintln!("bhoted mux: no pane: {e}"); let _ = std::fs::remove_file(&path); return 1; } }
    while !s.quit {
        let wait = if s.dirty { Duration::from_millis(4) } else if s.clients.iter().any(|c| c.esc_at.is_some()) { Duration::from_millis(10) } else { Duration::from_millis(400) };
        match rx.recv_timeout(wait) {
            Ok(ev) => s.handle(ev),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        for _ in 0..512 { match rx.try_recv() { Ok(ev) => s.handle(ev), Err(_) => break } }
        super::actions::flush_esc(&mut s);
        s.tick();
        if s.dirty { s.render(); }
    }
    s.save_state(true);
    s.sidecar_stop();
    let _ = std::fs::remove_file(&path);
    0
}
