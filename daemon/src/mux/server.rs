//! `bhoted mux`: the pane server. One event loop owns all state; threads only read (ptys, sockets) and send events.
use super::pane::Pane;
use super::proto::{self, read_frame, write_frame};
use super::pty::{self, Pty};
use super::render::{self, Grid};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

pub enum Ev {
    PaneOut(u32, Vec<u8>),
    PaneExit(u32),
    NewClient { id: u32, stream: UnixStream, cols: u16, rows: u16 },
    Frame(u32, u8, Vec<u8>),
    ClientGone(u32),
    Control(Value, Sender<Value>),
}

/// What the terminal of a client has to be told about the focused pane (keys and mouse come back in the form the program asked for).
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Modes { cursor_keys: bool, keypad: bool, paste: bool, mouse: u8, enc: u8 }

impl Modes {
    fn of(s: &vt100::Screen) -> Modes {
        use vt100::{MouseProtocolEncoding as E, MouseProtocolMode as M};
        Modes {
            cursor_keys: s.application_cursor(), keypad: s.application_keypad(), paste: s.bracketed_paste(),
            mouse: match s.mouse_protocol_mode() { M::None => 0, M::Press => 1, M::PressRelease => 2, M::ButtonMotion => 3, M::AnyMotion => 4 },
            enc: match s.mouse_protocol_encoding() { E::Default => 0, E::Utf8 => 1, E::Sgr => 2 },
        }
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

pub struct Client { pub id: u32, pub stream: UnixStream, pub cols: u16, pub rows: u16, prev: Option<Grid>, modes: Modes, prefix: bool }

pub struct Server {
    tx: Sender<Ev>,
    pub panes: BTreeMap<u32, Pane>,
    pub focus: u32,
    next_pane: u32,
    pub clients: Vec<Client>,
    dirty: bool,
    quit: bool,
    pub cwd: String,
}

fn pane_name(id: u32) -> String { format!("p{id}") }

impl Server {
    fn new(tx: Sender<Ev>) -> Server {
        Server { tx, panes: BTreeMap::new(), focus: 0, next_pane: 1, clients: Vec::new(), dirty: true, quit: false, cwd: std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default() }
    }

    /// A new pane (a shell when `cmd` is empty) of the given size; it gets the focus.
    pub fn spawn_pane(&mut self, cmd: Vec<String>, cwd: Option<String>, cols: u16, rows: u16) -> std::io::Result<u32> {
        let id = self.next_pane; self.next_pane += 1;
        let cwd = cwd.unwrap_or_else(|| self.cwd.clone());
        let env = vec![("BHOTE_MUX".to_string(), "native".to_string()), ("BHOTE_PANE".to_string(), pane_name(id))];
        let pty = Pty::spawn(&cmd, Some(std::path::Path::new(&cwd)), cols, rows, &env)?;
        let reader = pty.reader()?;
        let tx = self.tx.clone();
        std::thread::spawn(move || { pty::read_loop(reader, |b| tx.send(Ev::PaneOut(id, b)).is_ok()); let _ = tx.send(Ev::PaneExit(id)); });
        self.panes.insert(id, Pane::new(id, pty, cols, rows, cmd, cwd));
        self.focus = id; self.dirty = true;
        Ok(id)
    }

    /// The size the panes have: what the smallest attached client shows (80x24 without a client).
    fn view_size(&self) -> (u16, u16) {
        match self.clients.iter().map(|c| (c.cols, c.rows)).reduce(|a, b| (a.0.min(b.0), a.1.min(b.1))) { Some(s) => s, None => (80, 24) }
    }

    fn fit_panes(&mut self) {
        let (cols, rows) = self.view_size();
        for p in self.panes.values_mut() { p.resize(cols, rows); }
    }

    pub fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::PaneOut(id, b) => if let Some(p) = self.panes.get_mut(&id) { p.output(&b); self.dirty = true; },
            Ev::PaneExit(id) => {
                self.panes.remove(&id);
                if self.focus == id { self.focus = self.panes.keys().next().copied().unwrap_or(0); }
                if self.panes.is_empty() { self.end_clients("exited"); self.quit = true; }
                self.dirty = true;
            }
            Ev::NewClient { id, stream, cols, rows } => {
                self.clients.push(Client { id, stream, cols, rows, prev: None, modes: Modes::default(), prefix: false });
                self.fit_panes(); self.dirty = true;
            }
            Ev::Frame(id, t, p) => match t {
                b'I' => self.client_input(id, &p),
                b'R' => if let Some((c, r)) = proto::parse_resize(&p) {
                    if let Some(cl) = self.clients.iter_mut().find(|c| c.id == id) { cl.cols = c; cl.rows = r; cl.prev = None; }
                    self.fit_panes(); self.dirty = true;
                },
                b'D' => self.detach(id, "detached"),
                _ => {}
            },
            Ev::ClientGone(id) => { self.clients.retain(|c| c.id != id); self.fit_panes(); }
            Ev::Control(req, reply) => { let _ = reply.send(self.control(&req)); }
        }
    }

    fn end_clients(&mut self, reason: &str) {
        for c in &mut self.clients { let _ = write_frame(&mut c.stream, b'Q', reason.as_bytes()); }
        self.clients.clear();
    }

    fn detach(&mut self, id: u32, reason: &str) {
        if let Some(i) = self.clients.iter().position(|c| c.id == id) {
            let mut c = self.clients.remove(i);
            let _ = write_frame(&mut c.stream, b'Q', reason.as_bytes());
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
        }
        self.fit_panes();
    }

    /// Keys of a client: the prefix key (ctrl+b) and what follows is ours, everything else goes to the focused pane.
    fn client_input(&mut self, id: u32, bytes: &[u8]) {
        let mut fwd = Vec::new(); let mut detach = false;
        let Some(c) = self.clients.iter_mut().find(|c| c.id == id) else { return };
        for &b in bytes {
            if c.prefix {
                c.prefix = false;
                match b { b'd' => detach = true, 0x02 => fwd.push(0x02), _ => {} }
            } else if b == 0x02 { c.prefix = true } else { fwd.push(b) }
        }
        if !fwd.is_empty() { if let Some(p) = self.panes.get_mut(&self.focus) { p.input(&fwd); } }
        if detach { self.detach(id, "detached"); }
    }

    /// The picture of the focused pane for one client of this size.
    fn compose(&self, cols: u16, rows: u16) -> (Grid, Modes) {
        let mut g = Grid::new(cols, rows);
        let Some(p) = self.panes.get(&self.focus) else { return (g, Modes::default()) };
        let s = p.screen();
        let (pr, pc) = s.size();
        g.blit(s, 0, 0, pc.get().min(cols), pr.get().min(rows));
        if !s.hide_cursor() { let (r, c) = s.cursor_position(); if r < rows && c < cols { g.cursor = Some((c, r)); } }
        (g, Modes::of(s))
    }

    fn render(&mut self) {
        self.dirty = false;
        let mut extra = String::new();
        if let Some(p) = self.panes.get_mut(&self.focus) {
            let h = p.parser.callbacks_mut();
            for c in h.clipboard.drain(..) { extra.push_str(&c); }
            if std::mem::take(&mut h.bell) { extra.push('\x07'); }
        }
        let mut gone = Vec::new();
        for i in 0..self.clients.len() {
            let (g, modes) = self.compose(self.clients[i].cols, self.clients[i].rows);
            let c = &mut self.clients[i];
            let same = c.prev.as_ref().map_or(false, |p| p.cells == g.cells && p.cursor == g.cursor) && c.modes == modes;
            if same && extra.is_empty() { continue; }
            let mut out = modes.diff(&c.modes);
            if !same { out.push_str(&render::diff(c.prev.as_ref(), &g)); }
            out.push_str(&extra);
            c.modes = modes; c.prev = Some(g);
            if write_frame(&mut c.stream, b'O', out.as_bytes()).is_err() { gone.push(c.id); }
        }
        for id in gone { self.clients.retain(|c| c.id != id); self.fit_panes(); }
    }

    fn resolve(&self, params: &Value) -> Option<u32> {
        match params.get("pane_id").or_else(|| params.get("pane")) {
            None | Some(Value::Null) => Some(self.focus),
            Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
            Some(Value::String(s)) => s.trim_start_matches('p').parse().ok(),
            _ => None,
        }.filter(|id| self.panes.contains_key(id))
    }

    pub fn pane_json(&self, p: &Pane) -> Value {
        let (r, c) = p.screen().size();
        json!({"pane_id": pane_name(p.id), "focused": p.id == self.focus, "cols": c.get(), "rows": r.get(), "title": p.title(), "label": p.label, "cwd": p.cwd, "pid": p.pty.pid(), "command": p.cmd})
    }

    fn control(&mut self, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let empty = json!({});
        let params = req.get("params").unwrap_or(&empty);
        let err = |code: &str, msg: String| json!({"id": id, "error": {"code": code, "message": msg}});
        let ok = |r: Value| json!({"id": id, "result": r});
        match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
            "ping" => ok(json!({"pong": true, "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id()})),
            "server.stop" => { self.end_clients("stopped"); self.quit = true; ok(json!({"stopped": true})) }
            "pane.list" => ok(json!({"panes": self.panes.values().map(|p| self.pane_json(p)).collect::<Vec<_>>()})),
            "pane.send_text" => match self.resolve(params) {
                Some(pid) => { let t = params.get("text").and_then(|t| t.as_str()).unwrap_or(""); self.panes.get_mut(&pid).unwrap().input(t.as_bytes()); ok(json!({})) }
                None => err("not_found", "no such pane".into()),
            },
            "pane.read" => match self.resolve(params) {
                Some(pid) => {
                    let s = self.panes[&pid].screen();
                    let (_, cols) = s.size();
                    let mut rows: Vec<String> = s.rows(0, cols.get()).collect();
                    while rows.last().map_or(false, |r| r.is_empty()) { rows.pop(); }
                    ok(json!({"text": rows.join("\n")}))
                }
                None => err("not_found", "no such pane".into()),
            },
            m => err("unknown_method", format!("no method {m}")),
        }
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
            if tx.send(Ev::NewClient { id, stream: w.try_clone()?, cols, rows }).is_err() { return Ok(()); }
            while let Ok((t, p)) = read_frame(&mut r) { if tx.send(Ev::Frame(id, t, p)).is_err() { return Ok(()); } }
            let _ = tx.send(Ev::ClientGone(id));
            return Ok(());
        }
        let (rtx, rrx) = mpsc::channel();
        if tx.send(Ev::Control(req, rtx)).is_err() { return Ok(()); }
        match rrx.recv() { Ok(resp) => writeln!(w, "{resp}")?, Err(_) => return Ok(()) }
    }
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
    let mut s = Server::new(tx);
    if let Err(e) = s.spawn_pane(Vec::new(), None, 80, 24) { eprintln!("bhoted mux: no pane: {e}"); let _ = std::fs::remove_file(&path); return 1; }
    while !s.quit {
        let wait = if s.dirty { Duration::from_millis(4) } else { Duration::from_secs(3600) };
        match rx.recv_timeout(wait) {
            Ok(ev) => s.handle(ev),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        for _ in 0..512 { match rx.try_recv() { Ok(ev) => s.handle(ev), Err(_) => break } }
        if s.dirty { s.render(); }
    }
    let _ = std::fs::remove_file(&path);
    0
}
