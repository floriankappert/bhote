//! One pane: a pty with a terminal state. The state answers what programs ask the terminal (cursor position, device attributes).
use super::pty::Pty;
use std::io::Write;

#[derive(Default)]
pub struct Hooks { pub reply: Vec<u8>, pub title: Option<String>, pub clipboard: Vec<String>, pub bell: bool }

impl vt100::Callbacks for Hooks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, t: &[u8]) { self.title = Some(String::from_utf8_lossy(t).into_owned()); }
    fn audible_bell(&mut self, _: &mut vt100::Screen) { self.bell = true; }
    fn copy_to_clipboard(&mut self, _: &mut vt100::Screen, ty: &[u8], data: &[u8]) {
        self.clipboard.push(format!("\x1b]52;{};{}\x07", String::from_utf8_lossy(ty), String::from_utf8_lossy(data)));
    }
    fn unhandled_csi(&mut self, s: &mut vt100::Screen, i1: Option<u8>, i2: Option<u8>, params: &[&[u16]], c: char) {
        let p0 = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, i2, c) {
            (None, None, 'n') if p0 == 5 => self.reply.extend_from_slice(b"\x1b[0n"),
            (None, None, 'n') if p0 == 6 => { let (r, c) = s.cursor_position(); self.reply.extend_from_slice(format!("\x1b[{};{}R", r + 1, c + 1).as_bytes()) }
            (Some(b'?'), None, 'n') if p0 == 6 => { let (r, c) = s.cursor_position(); self.reply.extend_from_slice(format!("\x1b[?{};{}R", r + 1, c + 1).as_bytes()) }
            (None, None, 'c') if p0 == 0 => self.reply.extend_from_slice(b"\x1b[?62;4;22c"),
            (Some(b'>'), None, 'c') => self.reply.extend_from_slice(b"\x1b[>0;0;0c"),
            _ => {}
        }
    }
}

pub struct Pane {
    pub id: u32,
    pub parser: vt100::Parser<Hooks>,
    pub pty: Pty,
    pub label: String,
    pub cwd: String,
    pub cmd: Vec<String>,
}

fn nz(n: u16) -> std::num::NonZeroU16 { std::num::NonZeroU16::new(n.max(1)).unwrap() }

impl Pane {
    pub fn new(id: u32, pty: Pty, cols: u16, rows: u16, cmd: Vec<String>, cwd: String) -> Pane {
        Pane { id, parser: vt100::Parser::new_with_callbacks(nz(rows), nz(cols), 5000, Hooks::default()), pty, label: String::new(), cwd, cmd }
    }
    pub fn screen(&self) -> &vt100::Screen { self.parser.screen() }
    pub fn title(&self) -> &str { self.parser.callbacks().title.as_deref().unwrap_or("") }
    /// Terminal output of the child: into the state; what the state has to answer goes back into the pty.
    pub fn output(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        let reply = std::mem::take(&mut self.parser.callbacks_mut().reply);
        if !reply.is_empty() { self.input(&reply); }
    }
    pub fn input(&mut self, bytes: &[u8]) { let _ = (&self.pty.master).write_all(bytes); }
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let (r, c) = self.parser.screen().size();
        if (r.get(), c.get()) == (rows.max(1), cols.max(1)) { return; }
        self.pty.resize(cols.max(1), rows.max(1));
        self.parser.set_size(nz(rows), nz(cols));
    }
}
