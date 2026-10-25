//! What the user does in a client: keys (prefix, resize mode, copy mode, prompts), the mouse, and what is drawn over the panes for it.
use super::input::{Input, Key, Mouse};
use super::keymap::Keymap;
use super::layout::{self, Dir, Rect, Side};
use super::render::{Cell, Grid};
use super::server::{Server, Spawn};
use super::world::{self, World};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use vt100::{Color, MouseProtocolEncoding as Enc, MouseProtocolMode as MMode};

pub fn keys_path() -> PathBuf {
    if let Some(p) = std::env::var_os("BHOTE_KEYS") { return PathBuf::from(p); }
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))).unwrap_or_default();
    base.join("bhote").join("keys")
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Click { None, Workspace(usize), Tab(usize) }

#[derive(Clone, PartialEq, Debug)]
pub enum PromptKind { Pane, Tab, Workspace }
#[derive(Clone, PartialEq, Debug)]
pub struct Prompt { pub kind: PromptKind, pub text: String, /// the text is the old name: the first character typed replaces it
    pub fresh: bool }
impl Prompt { pub fn label(&self) -> &'static str { match self.kind { PromptKind::Pane => "rename pane", PromptKind::Tab => "rename tab", PromptKind::Workspace => "rename workspace" } } }

/// The cursor of copy mode, in the rows of the pane as it is shown now.
#[derive(Clone, PartialEq, Debug)]
pub struct CopyCursor { pub pane: u32, pub r: u16, pub c: u16 }

#[derive(Clone, PartialEq, Debug)]
pub enum Mode { Normal, Prefix, Resize, Copy(CopyCursor), Prompt(Prompt), Help }

impl Mode {
    pub fn tag(&self, km: &Keymap) -> String {
        match self {
            Mode::Normal | Mode::Prompt(_) => String::new(),
            Mode::Prefix => format!("{} …", km.prefix),
            Mode::Resize => "RESIZE  arrows · esc".into(),
            Mode::Copy(_) => "COPY  v select · y copy · q quit".into(),
            Mode::Help => "HELP  any key closes".into(),
        }
    }
}

/// A selection: absolute lines (0 = the oldest line of the scrollback) and columns.
#[derive(Clone, PartialEq, Debug)]
pub struct Selection { pub pane: u32, pub a: (usize, u16), pub b: (usize, u16), pub linewise: bool, pub active: bool }

#[derive(Clone, PartialEq, Debug)]
pub enum Drag { Divider { path: Vec<bool>, rect: Rect, dir: Dir }, Select }

fn ci(s: &Server, id: u32) -> Option<usize> { s.clients.iter().position(|c| c.id == id) }

pub fn on_input(s: &mut Server, id: u32, bytes: &[u8]) {
    let Some(i) = ci(s, id) else { return };
    let c = &mut s.clients[i];
    let events = c.parser.feed(bytes);
    c.esc_at = if c.parser.pending() { Some(Instant::now() + Duration::from_millis(25)) } else { None };
    for ev in events { if ci(s, id).is_none() { return; } handle(s, id, ev); }
}

/// A lone escape byte whose time is up is the escape key.
pub fn flush_esc(s: &mut Server) {
    let due: Vec<u32> = s.clients.iter().filter(|c| c.esc_at.map_or(false, |t| t <= Instant::now())).map(|c| c.id).collect();
    for id in due {
        let Some(i) = ci(s, id) else { continue };
        s.clients[i].esc_at = None;
        let evs = s.clients[i].parser.flush();
        for ev in evs { if ci(s, id).is_none() { break; } handle(s, id, ev); }
    }
}

fn handle(s: &mut Server, id: u32, ev: Input) {
    match ev {
        Input::Key(k) => key(s, id, k),
        Input::Mouse(m) => mouse(s, id, m),
        Input::Paste(data) => paste(s, id, data),
        Input::Focus(_) => {}
    }
}

fn paste(s: &mut Server, id: u32, data: Vec<u8>) {
    let Some(i) = ci(s, id) else { return };
    match &mut s.clients[i].mode {
        Mode::Prompt(p) => { if p.fresh { p.text.clear(); p.fresh = false; } p.text.push_str(&String::from_utf8_lossy(&data).replace(['\n', '\r'], " ")); s.dirty = true; }
        Mode::Normal | Mode::Prefix => {
            s.clients[i].mode = Mode::Normal; s.clients[i].sel = None;
            let Some(p) = s.world.focused_pane().and_then(|f| s.panes.get_mut(&f)) else { return };
            p.parser.screen_mut().set_scrollback(0);
            if p.screen().bracketed_paste() { let mut v = b"\x1b[200~".to_vec(); v.extend_from_slice(&data); v.extend_from_slice(b"\x1b[201~"); p.input(&v); } else { p.input(&data); }
        }
        _ => {}
    }
}

fn key(s: &mut Server, id: u32, k: Key) {
    let Some(i) = ci(s, id) else { return };
    s.dirty = true;
    match s.clients[i].mode.clone() {
        Mode::Normal => {
            s.clients[i].sel = None;
            if k.name == s.km.prefix { s.clients[i].mode = Mode::Prefix; return; }
            if let Some(b) = s.km.global(&k.name).cloned() { run(s, id, &b.action, b.arg.as_deref()); return; }
            forward(s, &k.raw);
        }
        Mode::Prefix => {
            s.clients[i].mode = Mode::Normal;
            if k.name == s.km.prefix { forward(s, &k.raw); return; }
            if let Some(b) = s.km.prefixed(&k.name).cloned() { run(s, id, &b.action, b.arg.as_deref()); }
        }
        Mode::Resize => {
            let side = match k.name.as_str() { "left" | "h" => Some(Side::Left), "right" | "l" => Some(Side::Right), "up" | "k" => Some(Side::Up), "down" | "j" => Some(Side::Down), _ => None };
            if let Some(side) = side { resize_focused(s, side, 0.03); }
            else if matches!(k.name.as_str(), "esc" | "enter" | "q") { s.clients[i].mode = Mode::Normal; }
        }
        Mode::Prompt(mut p) => {
            match k.name.as_str() {
                "enter" => { s.clients[i].mode = Mode::Normal; apply_rename(s, &p); return; }
                "esc" | "ctrl+c" => { s.clients[i].mode = Mode::Normal; return; }
                "backspace" => { p.text.pop(); p.fresh = false; }
                "ctrl+u" => { p.text.clear(); p.fresh = false; }
                _ => if let Some(ch) = std::str::from_utf8(&k.raw).ok().and_then(|t| { let mut c = t.chars(); let f = c.next()?; if c.next().is_none() && !f.is_control() { Some(f) } else { None } }) { if p.fresh { p.text.clear(); p.fresh = false; } p.text.push(ch); },
            }
            s.clients[i].mode = Mode::Prompt(p);
        }
        Mode::Help => { s.clients[i].mode = Mode::Normal; }
        Mode::Copy(cur) => copy_key(s, id, cur, &k),
    }
}

/// Typed text goes to the focused pane; its scrollback view snaps back to the live screen.
fn forward(s: &mut Server, raw: &[u8]) {
    if let Some(p) = s.world.focused_pane().and_then(|f| s.panes.get_mut(&f)) {
        if p.screen().scrollback() > 0 { p.parser.screen_mut().set_scrollback(0); }
        p.input(raw);
    }
}

fn resize_focused(s: &mut Server, side: Side, amount: f32) {
    let Some(f) = s.world.focused_pane() else { return };
    let Some((wi, ti)) = s.world.tab_of_pane(f) else { return };
    if s.world.ws[wi].tabs[ti].root.resize(f, side, amount) { s.after_change(); }
}

fn apply_rename(s: &mut Server, p: &Prompt) {
    match p.kind {
        PromptKind::Pane => if let Some(pane) = s.world.focused_pane().and_then(|f| s.panes.get_mut(&f)) { pane.label = p.text.clone(); },
        PromptKind::Tab => if let Some(t) = s.world.current_mut() { t.label = p.text.clone(); },
        PromptKind::Workspace => if let Some(w) = s.world.ws.get_mut(s.world.active) { w.label = p.text.clone(); },
    }
    s.dirty = true;
}

impl Server {
    pub fn set_focus(&mut self, pane: u32) {
        let prev = self.world.focused_pane();
        if prev != Some(pane) { self.last_focus = prev; }
        self.world.focus_pane(pane);
        self.dirty = true;
        if prev != Some(pane) { if let Some(p) = self.panes.get(&pane) { let (n, w, t) = (p.name(), world::ws_name(p.ws), world::tab_name(p.ws, p.tab)); self.emit("pane.focused", Some(&n.clone()), json!({"pane_id": n, "workspace_id": w, "tab_id": t})); } }
    }
    fn follow_cwd(&self) -> Spawn { Spawn { cwd: self.world.focused_pane().and_then(|f| self.panes.get(&f)).map(|p| p.foreground().1), ..Default::default() } }
    fn tab_rects(&self) -> Vec<(u32, Rect)> { self.world.current().map(|(_, t)| World::rects(t, self.area()).0).unwrap_or_default() }

    /// All lines of a pane, scrollback first (and whether the terminal wrapped the line into the next one).
    pub fn all_lines(&mut self, id: u32) -> Vec<(String, bool)> {
        let p = self.panes.get_mut(&id).unwrap();
        let keep = p.screen().scrollback();
        let (rows, cols) = { let (r, c) = p.screen().size(); (r.get() as usize, c.get()) };
        let total = p.screen().scrollback_len();
        let mut all = Vec::new();
        let mut idx = 0;
        while idx < total + rows {
            let o = total.saturating_sub(idx);
            p.parser.screen_mut().set_scrollback(o);
            let sc = p.parser.screen();
            let skip = idx.saturating_sub(total - o.min(total));
            for (i, t) in sc.rows(0, cols).enumerate().skip(skip) { all.push((t, sc.row_wrapped(i as u16))); }
            idx += rows;
        }
        p.parser.screen_mut().set_scrollback(keep);
        all
    }
}

/// An action by name (also what `run` of the keys file and the herdr-like API call).
pub fn run(s: &mut Server, id: u32, action: &str, arg: Option<&str>) {
    let Some(i) = ci(s, id) else { return };
    s.dirty = true;
    let focused = s.world.focused_pane();
    let cur = s.world.ws.get(s.world.active).map(|w| (s.world.active, w.active));
    let side_of = |a: &str| match a.rsplit('_').next() { Some("left") => Some(Side::Left), Some("right") => Some(Side::Right), Some("up") => Some(Side::Up), Some("down") => Some(Side::Down), _ => None };
    match action {
        "detach" => s.detach_client(id, "detached"),
        "help" => s.clients[i].mode = Mode::Help,
        "copy_mode" => if let Some(f) = focused { let p = &s.panes[&f]; let (r, c) = p.screen().cursor_position(); s.clients[i].mode = Mode::Copy(CopyCursor { pane: f, r, c }); },
        "split_vertical" | "split_horizontal" => if let Some(f) = focused { let _ = s.split_pane(f, if action == "split_vertical" { Dir::Right } else { Dir::Down }, 0.5, &s.follow_cwd(), true); },
        "close_pane" => if let Some(f) = focused { s.close_pane(f); },
        "zoom" => if let Some(f) = focused { if let Some(t) = s.world.current_mut() { t.zoomed = if t.zoomed.is_some() { None } else { Some(f) }; } s.after_change(); },
        "last_pane" => if let Some(l) = s.last_focus.filter(|l| s.panes.contains_key(l)) { s.set_focus(l); },
        "focus_pane_left" | "focus_pane_right" | "focus_pane_up" | "focus_pane_down" => if let (Some(f), Some(side)) = (focused, side_of(action)) {
            if let Some(n) = layout::neighbor(&s.tab_rects(), f, side) { s.set_focus(n); }
        },
        "resize_mode" => s.clients[i].mode = Mode::Resize,
        "resize_pane_left" | "resize_pane_right" | "resize_pane_up" | "resize_pane_down" => if let Some(side) = side_of(action) { resize_focused(s, side, 0.05); },
        "rename_pane" => { let t = focused.and_then(|f| s.panes.get(&f)).map(|p| p.label.clone()).unwrap_or_default(); s.clients[i].mode = Mode::Prompt(Prompt { kind: PromptKind::Pane, text: t, fresh: true }); }
        "rename_tab" => { let t = s.world.current().map(|(_, t)| t.label.clone()).unwrap_or_default(); s.clients[i].mode = Mode::Prompt(Prompt { kind: PromptKind::Tab, text: t, fresh: true }); }
        "rename_workspace" => { let t = s.world.current().map(|(w, _)| w.label.clone()).unwrap_or_default(); s.clients[i].mode = Mode::Prompt(Prompt { kind: PromptKind::Workspace, text: t, fresh: true }); }
        "new_tab" => if let Some((wi, _)) = cur { let sp = s.follow_cwd(); let _ = s.create_tab(wi, "", &sp, true); },
        "close_tab" => if let Some((wi, ti)) = cur { let mut l = Vec::new(); s.world.ws[wi].tabs[ti].root.leaves(&mut l); for p in l { s.close_pane(p); } },
        "switch_tab" => if let (Some((wi, _)), Some(n)) = (cur, arg.and_then(|a| a.parse::<usize>().ok())) { if n >= 1 && n <= s.world.ws[wi].tabs.len() { s.world.ws[wi].active = n - 1; } },
        "previous_tab" | "next_tab" => if let Some((wi, ti)) = cur { let n = s.world.ws[wi].tabs.len(); s.world.ws[wi].active = if action == "next_tab" { (ti + 1) % n } else { (ti + n - 1) % n }; },
        "move_tab_previous" | "move_tab_next" => if let Some((wi, ti)) = cur {
            let n = s.world.ws[wi].tabs.len();
            let to = if action == "move_tab_next" { (ti + 1) % n } else { (ti + n - 1) % n };
            s.world.ws[wi].tabs.swap(ti, to); s.world.ws[wi].active = to;
        },
        "new_workspace" => { let sp = s.follow_cwd(); let _ = s.create_workspace("", &sp, true); }
        "close_workspace" => if let Some((wi, _)) = cur { let mut l = Vec::new(); for t in &s.world.ws[wi].tabs { t.root.leaves(&mut l); } for p in l { s.close_pane(p); } },
        "previous_workspace" | "next_workspace" => { let n = s.world.ws.len(); if n > 0 { s.world.active = if action == "next_workspace" { (s.world.active + 1) % n } else { (s.world.active + n - 1) % n }; } }
        "reload_config" => s.km = Keymap::load(&keys_path()),
        "focus_panel" => s.focus_panel(),
        "open_panel" => if let Some((wi, ti)) = cur { let (wid, tid) = (s.world.ws[wi].id, s.world.ws[wi].tabs[ti].id); let cfg = super::panels::PanelCfg::load(); s.make_panels(wid, tid, &cfg, true); },
        "popup" => if let Some(cmd) = arg { s.open_popup(vec!["sh".into(), "-c".into(), cmd.to_string()]); },
        "run" => if let Some(cmd) = arg {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", cmd]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            for (k, v) in &s.env { c.env(k, v); }
            if let Some(p) = focused.and_then(|f| s.panes.get(&f)) { c.env("HERDR_PANE_ID", p.name()).current_dir(&p.cwd); }
            if let Ok(mut ch) = c.spawn() { std::thread::spawn(move || { let _ = ch.wait(); }); }
        },
        _ => {}
    }
    if let Some(w) = s.world.ws.get(s.world.active) { let _ = w; }
    s.after_change();
}

// ───────────────────────────── copy mode

fn word_class(c: char) -> u8 { if c.is_whitespace() { 0 } else if c.is_alphanumeric() || c == '_' { 1 } else { 2 } }

fn col_to_idx(line: &str, col: u16) -> usize {
    let mut x = 0u16;
    for (i, ch) in line.chars().enumerate() { let w = if super::render::wide(ch) { 2 } else { 1 }; if x + w > col { return i; } x += w; }
    line.chars().count()
}
fn idx_to_col(line: &str, idx: usize) -> u16 { line.chars().take(idx).map(|ch| if super::render::wide(ch) { 2u16 } else { 1 }).sum() }

fn sel_text(lines: &[(String, bool)], sel: &Selection) -> String {
    let (mut a, mut b) = (sel.a, sel.b);
    if (b.0, b.1) < (a.0, a.1) { std::mem::swap(&mut a, &mut b); }
    let mut out = Vec::new();
    for l in a.0..=b.0.min(lines.len().saturating_sub(1)) {
        let line = &lines[l].0;
        let chars: Vec<char> = line.chars().collect();
        let from = if sel.linewise || l != a.0 { 0 } else { col_to_idx(line, a.1) };
        let to = if sel.linewise || l != b.0 { chars.len() } else { (col_to_idx(line, b.1) + 1).min(chars.len()) };
        let t: String = chars.get(from..to.max(from)).map(|c| c.iter().collect()).unwrap_or_default();
        out.push(t.trim_end().to_string());
    }
    out.join("\n")
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(T[(n >> 18) as usize & 63] as char); o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' }); o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
}

fn copy_out(s: &mut Server, text: &str) { if !text.is_empty() { s.clip.push(format!("\x1b]52;c;{}\x07", b64(text.as_bytes()))); } }

/// Absolute line of a row as the pane shows it now.
fn abs_line(s: &Server, pane: u32, row: u16) -> usize { let sc = s.panes[&pane].screen(); sc.scrollback_len().saturating_sub(sc.scrollback()) + row as usize }

fn copy_key(s: &mut Server, id: u32, mut cur: CopyCursor, k: &Key) {
    let Some(i) = ci(s, id) else { return };
    let pane = cur.pane;
    if !s.panes.contains_key(&pane) { s.clients[i].mode = Mode::Normal; return; }
    let (h, w) = { let (r, c) = s.panes[&pane].screen().size(); (r.get(), c.get()) };
    let set_off = |s: &mut Server, o: usize| { s.panes.get_mut(&pane).unwrap().parser.screen_mut().set_scrollback(o); };
    let up = |s: &mut Server, cur: &mut CopyCursor| { if cur.r > 0 { cur.r -= 1 } else { let sc = s.panes[&pane].screen(); let (o, m) = (sc.scrollback(), sc.scrollback_len()); if o < m { set_off(s, o + 1); } } };
    let down = |s: &mut Server, cur: &mut CopyCursor| { if cur.r + 1 < h { cur.r += 1 } else { let o = s.panes[&pane].screen().scrollback(); if o > 0 { set_off(s, o - 1); } } };
    let line_at = |s: &mut Server, row: u16| -> String { let a = abs_line(s, pane, row); s.all_lines(pane).get(a).map(|l| l.0.clone()).unwrap_or_default() };
    let name = k.name.as_str();
    match name {
        "q" | "esc" | "ctrl+c" => { set_off(s, 0); s.clients[i].mode = Mode::Normal; s.clients[i].sel = None; return; }
        "up" | "k" => up(s, &mut cur),
        "down" | "j" => down(s, &mut cur),
        "left" | "h" => cur.c = cur.c.saturating_sub(1),
        "right" | "l" => cur.c = (cur.c + 1).min(w - 1),
        "0" | "home" => cur.c = 0,
        "shift+4" | "$" | "end" => { let l = line_at(s, cur.r); cur.c = idx_to_col(&l, l.trim_end().chars().count().saturating_sub(1)); }
        "g" => { let m = s.panes[&pane].screen().scrollback_len(); set_off(s, m); cur.r = 0; }
        "shift+g" => { set_off(s, 0); cur.r = h - 1; }
        "ctrl+u" | "pageup" => for _ in 0..(if name == "pageup" { h } else { h / 2 }) { up(s, &mut cur) },
        "ctrl+d" | "pagedown" => for _ in 0..(if name == "pagedown" { h } else { h / 2 }) { down(s, &mut cur) },
        "ctrl+b" => for _ in 0..h { up(s, &mut cur) },
        "ctrl+f" => for _ in 0..h { down(s, &mut cur) },
        "w" | "b" => {
            let l = line_at(s, cur.r); let chars: Vec<char> = l.chars().collect();
            let mut x = col_to_idx(&l, cur.c).min(chars.len().saturating_sub(1));
            if name == "w" {
                let cls = chars.get(x).map(|c| word_class(*c)).unwrap_or(0);
                while x < chars.len() && word_class(chars[x]) == cls && cls != 0 { x += 1; }
                while x < chars.len() && word_class(chars[x]) == 0 { x += 1; }
                x = x.min(chars.len().saturating_sub(1));
            } else {
                while x > 0 && word_class(chars[x - 1]) == 0 { x -= 1; }
                if x > 0 { let cls = word_class(chars[x - 1]); while x > 0 && word_class(chars[x - 1]) == cls { x -= 1; } }
            }
            cur.c = idx_to_col(&l, x);
        }
        "v" | "space" | "shift+v" => {
            let a = (abs_line(s, pane, cur.r), cur.c);
            s.clients[i].sel = Some(Selection { pane, a, b: a, linewise: name == "shift+v", active: true });
        }
        "y" | "enter" => {
            let lines = s.all_lines(pane);
            let sel = s.clients[i].sel.clone().unwrap_or_else(|| { let a = abs_line(s, pane, cur.r); Selection { pane, a: (a, 0), b: (a, 0), linewise: true, active: true } });
            let t = sel_text(&lines, &sel);
            copy_out(s, &t);
            set_off(s, 0); s.clients[i].mode = Mode::Normal; s.clients[i].sel = None; return;
        }
        _ => {}
    }
    let b = (abs_line(s, pane, cur.r), cur.c);
    if let Some(sel) = s.clients[i].sel.as_mut() { sel.b = b; }
    s.clients[i].mode = Mode::Copy(cur);
}

// ───────────────────────────── mouse

fn mouse_bytes(enc: Enc, b: u32, x: u16, y: u16, press: bool) -> Vec<u8> {
    match enc {
        Enc::Sgr => format!("\x1b[<{};{};{}{}", b, x + 1, y + 1, if press { 'M' } else { 'm' }).into_bytes(),
        Enc::Utf8 => { let mut v = vec![0x1b, b'[', b'M', (if press { b } else { (b & !3) | 3 }) as u8 + 32]; for c in [x + 33, y + 33] { let mut buf = [0u8; 4]; v.extend_from_slice(char::from_u32(c as u32).unwrap_or(' ').encode_utf8(&mut buf).as_bytes()); } v }
        Enc::Default => vec![0x1b, b'[', b'M', (if press { b } else { (b & !3) | 3 }) as u8 + 32, (x + 33).min(255) as u8, (y + 33).min(255) as u8],
    }
}

fn mouse(s: &mut Server, id: u32, m: Mouse) {
    let Some(i) = ci(s, id) else { return };
    s.dirty = true;
    let area = s.area();
    let wheel = m.b & 64 != 0;
    let motion = m.b & 32 != 0;
    let button = m.b & 3;
    // a drag in progress
    match s.clients[i].drag.clone() {
        Some(Drag::Divider { path, rect, dir }) => {
            if !m.press && !motion { s.clients[i].drag = None; return; }
            if let Some(t) = s.world.current_mut() { let pos = match dir { Dir::Right => m.x.saturating_sub(rect.x), Dir::Down => m.y.saturating_sub(rect.y) }; t.root.set_divider(&path, rect, pos); }
            s.after_change(); return;
        }
        Some(Drag::Select) => {
            let Some(sel) = s.clients[i].sel.clone() else { s.clients[i].drag = None; return };
            let rects = s.tab_rects();
            let r = rects.iter().find(|(p, _)| *p == sel.pane).map(|x| x.1);
            if motion && m.press {
                if let Some(r) = r {
                    let (row, col) = (m.y.clamp(r.y, r.y + r.h.saturating_sub(1)) - r.y, m.x.clamp(r.x, r.x + r.w.saturating_sub(1)) - r.x);
                    let a = abs_line(s, sel.pane, row);
                    if let Some(x) = s.clients[i].sel.as_mut() { x.b = (a, col); x.active = true; }
                }
            } else if !m.press {
                s.clients[i].drag = None;
                let sel = s.clients[i].sel.clone().unwrap();
                if sel.active && sel.a != sel.b { let lines = s.all_lines(sel.pane); let t = sel_text(&lines, &sel); copy_out(s, &t); }
                else { s.clients[i].sel = None; }
            }
            return;
        }
        None => {}
    }
    // the bar
    if m.y == area.h {
        if m.press && button == 0 && !motion && !wheel {
            for (x, text, click, _) in s.bar_items() {
                let w = text.chars().count() as u16;
                if m.x >= x && m.x < x + w { match click { Click::Workspace(w) => s.world.active = w, Click::Tab(t) => { let a = s.world.active; if let Some(w) = s.world.ws.get_mut(a) { w.active = t; } } Click::None => {} } }
            }
            s.after_change();
        }
        return;
    }
    let Some((_, tab)) = s.world.current() else { return };
    let (rects, divs) = match tab.popup {
        Some(pp) => { let q = World::popup_rect(area); (vec![(pp, Rect { x: q.x + 1, y: q.y + 1, w: q.w.saturating_sub(2), h: q.h.saturating_sub(2) })], Vec::new()) }
        None => World::rects(tab, area),
    };
    // a divider
    if m.press && button == 0 && !motion && !wheel {
        for d in &divs {
            let hit = match d.dir { Dir::Right => m.x == d.x && m.y >= d.y && m.y < d.y + d.len, Dir::Down => m.y == d.y && m.x >= d.x && m.x < d.x + d.len };
            if hit { s.clients[i].drag = Some(Drag::Divider { path: d.path.clone(), rect: d.rect, dir: d.dir }); return; }
        }
    }
    let Some((pid, r)) = rects.iter().find(|(_, r)| m.x >= r.x && m.x < r.x + r.w && m.y >= r.y && m.y < r.y + r.h).cloned() else { return };
    if m.press && !motion && !wheel && s.world.focused_pane() != Some(pid) { s.set_focus(pid); }
    let (px, py) = (m.x - r.x, m.y - r.y);
    let p = &s.panes[&pid];
    let (mode, enc) = (p.screen().mouse_protocol_mode(), p.screen().mouse_protocol_encoding());
    let wants = match mode { MMode::None => false, MMode::Press => m.press && !motion, MMode::PressRelease => !motion, MMode::ButtonMotion => !motion || button != 3, MMode::AnyMotion => true };
    let shift = m.b & 4 != 0;
    if mode != MMode::None && !shift {
        if wants { let bytes = mouse_bytes(enc, m.b, px, py, m.press); s.panes.get_mut(&pid).unwrap().input(&bytes); }
        return;
    }
    if wheel {
        let alt = p.screen().alternate_screen();
        let app = p.screen().application_cursor();
        let up = button == 0;
        if alt { let k: &[u8] = match (up, app) { (true, true) => b"\x1bOA", (true, false) => b"\x1b[A", (false, true) => b"\x1bOB", (false, false) => b"\x1b[B" }; let v = k.repeat(3); s.panes.get_mut(&pid).unwrap().input(&v); }
        else {
            let sc = s.panes.get_mut(&pid).unwrap().parser.screen_mut();
            let o = sc.scrollback();
            sc.set_scrollback(if up { o + 3 } else { o.saturating_sub(3) });
        }
        return;
    }
    // text selection with the mouse
    if m.press && button == 0 && !motion {
        let a = abs_line(s, pid, py);
        s.clients[i].sel = Some(Selection { pane: pid, a: (a, px), b: (a, px), linewise: false, active: false });
        s.clients[i].drag = Some(Drag::Select);
    }
}

// ───────────────────────────── what is drawn over the panes

pub fn overlay(s: &Server, ci_: usize, rects: &[(u32, Rect)], g: &mut Grid) {
    let c = &s.clients[ci_];
    // selection
    if let Some(sel) = &c.sel {
        if let Some((_, r)) = rects.iter().find(|(p, _)| *p == sel.pane) {
            if let Some(p) = s.panes.get(&sel.pane) {
                let sc = p.screen();
                let top = sc.scrollback_len().saturating_sub(sc.scrollback());
                let (mut a, mut b) = (sel.a, sel.b);
                if (b.0, b.1) < (a.0, a.1) { std::mem::swap(&mut a, &mut b); }
                for row in 0..r.h {
                    let abs = top + row as usize;
                    if abs < a.0 || abs > b.0 { continue; }
                    let from = if sel.linewise || abs != a.0 { 0 } else { a.1 };
                    let to = if sel.linewise || abs != b.0 { r.w.saturating_sub(1) } else { b.1.min(r.w.saturating_sub(1)) };
                    for col in from..=to { if let Some(cell) = g.cell_mut(r.x + col, r.y + row) { cell.inverse = !cell.inverse; } }
                }
            }
        }
    }
    if let Mode::Copy(cur) = &c.mode {
        if let Some((_, r)) = rects.iter().find(|(p, _)| *p == cur.pane) { g.cursor = Some((r.x + cur.c, r.y + cur.r)); }
    }
    // a scrolled pane says so
    for (id, r) in rects {
        if let Some(p) = s.panes.get(id) {
            let o = p.screen().scrollback();
            if o > 0 { let t = format!(" ↑{o} "); let w = t.chars().count() as u16; g.put((r.x + r.w).saturating_sub(w), r.y, &t, &Cell { fg: Color::Rgb(20, 22, 34), bg: Color::Rgb(250, 179, 135), ..Cell::default() }); }
        }
    }
    if c.mode == Mode::Help { help(s, g); }
}

fn help(s: &Server, g: &mut Grid) {
    let items = s.km.list();
    let w = 56u16.min(g.cols); let h = (items.len() as u16 + 3).min(g.rows);
    let (x, y) = ((g.cols - w) / 2, (g.rows.saturating_sub(h)) / 2);
    let bg = Cell { fg: Color::Rgb(205, 214, 244), bg: Color::Rgb(24, 24, 37), ..Cell::default() };
    g.fill(x, y, w, h, &bg);
    g.put(x + 2, y, &format!(" keys (prefix {}) ", s.km.prefix), &Cell { bold: true, ..bg.clone() });
    for (n, (k, a)) in items.iter().enumerate().take(h.saturating_sub(2) as usize) { g.put(x + 2, y + 1 + n as u16, &format!("{:<28}{}", k, a), &bg); }
}
