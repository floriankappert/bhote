//! The terminal of the Rust panel: raw mode, size, the resize signal, and the input as events (keys, arrows, SGR mouse reports).
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    Key(char), Enter, Esc, Backspace, Tab, Up, Down, Left, Right,
    /// button (0 left, 1 middle, 2 right, 35 motion without a button, 64/65 wheel), column, row (1-based), pressed (M) or released (m)
    Mouse { b: u32, x: u32, y: u32, press: bool },
    Resize, Timeout, Eof,
}

extern "C" {
    fn select(nfds: i32, r: *mut [u8; 128], w: *mut [u8; 128], e: *mut [u8; 128], tv: *mut TimeVal) -> i32;
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    fn ioctl(fd: i32, req: u64, ...) -> i32;
    fn signal(sig: i32, handler: usize) -> usize;
    fn pipe(fds: *mut i32) -> i32;
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    fn isatty(fd: i32) -> i32;
}
#[repr(C)] struct TimeVal { sec: i64, usec: i64 }
/// Waits until one of the descriptors can be read (select: poll is not reliable on terminals on macOS); the bitmap is the same bytes on both systems
fn wait_read(fds: &[i32], ms: i32) -> Vec<bool> {
    let mut set = [0u8; 128]; let mut max = 0;
    for &fd in fds { if fd >= 0 && fd < 1024 { set[(fd / 8) as usize] |= 1 << (fd % 8); max = max.max(fd + 1); } }
    let mut tv = TimeVal { sec: (ms / 1000) as i64, usec: ((ms % 1000) * 1000) as i64 };
    let n = unsafe { select(max, &mut set, std::ptr::null_mut(), std::ptr::null_mut(), if ms < 0 { std::ptr::null_mut() } else { &mut tv }) };
    fds.iter().map(|&fd| n > 0 && fd >= 0 && fd < 1024 && set[(fd / 8) as usize] & (1 << (fd % 8)) != 0).collect()
}
#[repr(C)] #[derive(Default)] struct Winsize { rows: u16, cols: u16, x: u16, y: u16 }

#[cfg(target_os = "linux")] const TIOCGWINSZ: u64 = 0x5413;
#[cfg(not(target_os = "linux"))] const TIOCGWINSZ: u64 = 0x40087468;

static mut WAKE_W: i32 = -1;
static QUIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SAVED: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
extern "C" fn on_quit(_: i32) { QUIT.store(true, std::sync::atomic::Ordering::SeqCst); unsafe { if WAKE_W >= 0 { let b = [2u8]; write(WAKE_W, b.as_ptr(), 1); } } }
extern "C" fn on_winch(_: i32) { unsafe { if WAKE_W >= 0 { let b = [1u8]; write(WAKE_W, b.as_ptr(), 1); } } }

pub fn quit_requested() -> bool { QUIT.load(std::sync::atomic::Ordering::SeqCst) }

pub struct Term { saved: String, wake_r: i32, buf: Vec<u8> }

impl Term {
    pub fn is_tty() -> bool { unsafe { isatty(0) == 1 } }
    /// stty on the terminal: byte by byte, no echo (a waiting escape sequence then counts as input waiting)
    fn stty(args: &[&str]) -> String {
        let tty = std::fs::File::open("/dev/tty").ok();
        let mut c = Command::new("stty"); c.args(args).stderr(Stdio::null());
        if let Some(t) = tty { c.stdin(t); }
        c.output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
    }
    pub fn enter(mouse: bool) -> Term {
        // the settings of the shell's terminal are taken once: after a child (a view of the script) the terminal is raw already
        let saved = { let mut s = SAVED.lock().unwrap_or_else(|e| e.into_inner()); if s.is_empty() { *s = Term::stty(&["-g"]); } s.clone() };
        Term::stty(&["-echo", "-icanon", "min", "1", "time", "0"]);
        let mut fds = [0i32; 2];
        let wake_r = unsafe { if pipe(fds.as_mut_ptr()) == 0 { WAKE_W = fds[1]; signal(28, on_winch as usize); signal(2, on_quit as usize); signal(15, on_quit as usize); signal(1, on_quit as usize); fds[0] } else { -1 } };
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |i| { Term::restore_terminal(); prev(i); }));
        let mut o = std::io::stdout();
        let _ = o.write_all(format!("\x1b[?1049h\x1b[?25l{}", if mouse { "\x1b[?1002h\x1b[?1006h" } else { "" }).as_bytes()); let _ = o.flush();
        Term { saved, wake_r, buf: Vec::new() }
    }
    /// Back to the shell's terminal: mouse off, cursor on, the normal screen, the saved settings (also after a panic)
    pub fn restore_terminal() {
        let mut o = std::io::stdout();
        let _ = o.write_all(b"\x1b[?1003l\x1b[?1002l\x1b[?1006l\x1b[0m\x1b[?25h\x1b[?1049l\x1b]2;\x07"); let _ = o.flush();
        let s = SAVED.lock().map(|s| s.clone()).unwrap_or_default();
        if !s.is_empty() { Term::stty(&[&s]); }
    }
    pub fn leave(&self) {
        let mut o = std::io::stdout();
        let _ = o.write_all(b"\x1b[?1003l\x1b[?1002l\x1b[?1006l\x1b[0m\x1b[?25h\x1b[?1049l\x1b]2;\x07"); let _ = o.flush();
        if !self.saved.is_empty() { Term::stty(&[&self.saved]); }
    }
    pub fn size() -> (u32, u32) {
        let mut w = Winsize::default();
        let ok = unsafe { ioctl(1, TIOCGWINSZ, &mut w as *mut Winsize) } == 0 && w.cols > 0;
        if ok { (w.cols as u32, w.rows as u32) } else { (44, 40) }
    }
    /// The next event; waits at most `ms`. A lone Esc is told from a sequence by waiting 50 ms for the next byte.
    pub fn next(&mut self, ms: i32) -> Ev {
        if quit_requested() { return Ev::Eof; }
        if let Some(e) = parse(&mut self.buf, false) { return e; }
        loop {
            let ready = wait_read(&[0, self.wake_r], if self.buf.is_empty() { ms } else { 50 });
            let n = ready.iter().filter(|r| **r).count();
            if n == 0 { if self.buf.is_empty() { return Ev::Timeout; } if let Some(e) = parse(&mut self.buf, true) { return e; } continue; }
            if self.wake_r >= 0 && ready[1] { let mut b = [0u8; 16]; unsafe { read(self.wake_r, b.as_mut_ptr(), 16); } if QUIT.load(std::sync::atomic::Ordering::SeqCst) { return Ev::Eof; } return Ev::Resize; }
            if ready[0] {
                let mut b = [0u8; 4096];
                let r = unsafe { read(0, b.as_mut_ptr(), b.len()) };
                if r <= 0 { return Ev::Eof; }
                self.buf.extend_from_slice(&b[..r as usize]);
                if let Some(e) = parse(&mut self.buf, false) { return e; }
            }
        }
    }
    /// 0 when more input is waiting right now (key repeat, a drag): handle it first, draw once
    pub fn pending(&self) -> bool {
        if !self.buf.is_empty() { return true; }
        wait_read(&[0], 0)[0]
    }
}

/// One event from the front of the buffer (None: not complete yet; `idle`: nothing more came, a lone Esc is Esc).
pub fn parse(buf: &mut Vec<u8>, idle: bool) -> Option<Ev> {
    if buf.is_empty() { return None; }
    let b0 = buf[0];
    if b0 == 0x1b {
        if buf.len() == 1 { if idle { buf.remove(0); return Some(Ev::Esc); } return None; }
        let b1 = buf[1];
        if b1 == b'[' || b1 == b'O' {
            // up to the final byte (@ .. ~): parameters, mouse reports and paste markers must not leak as keys
            let mut i = 2;
            while i < buf.len() && (buf[i].is_ascii_digit() || b";<=>? !\"#$%&'()*+,-./".contains(&buf[i])) { if b1 == b'O' { break; } i += 1; }
            if b1 == b'[' && i < buf.len() && buf[i] == b'M' && i == 2 { // X10 mouse: three raw bytes follow
                if buf.len() < 6 { return None; }
                let (b, x, y) = (buf[3] as i32 - 32, buf[4] as i32 - 32, buf[5] as i32 - 32); buf.drain(..6);
                return Some(Ev::Mouse { b: b.max(0) as u32, x: x.max(0) as u32, y: y.max(0) as u32, press: true });
            }
            if i >= buf.len() { return None; }
            let fin = buf[i]; let params: String = buf[2..i].iter().map(|c| *c as char).collect(); buf.drain(..=i);
            if b1 == b'[' && params.starts_with('<') && (fin == b'M' || fin == b'm') {
                let p: Vec<u32> = params[1..].split(';').filter_map(|v| v.parse().ok()).collect();
                if p.len() == 3 { return Some(Ev::Mouse { b: p[0], x: p[1], y: p[2], press: fin == b'M' }); }
                return parse(buf, idle);
            }
            if params.is_empty() { match fin { b'A' => return Some(Ev::Up), b'B' => return Some(Ev::Down), b'C' => return Some(Ev::Right), b'D' => return Some(Ev::Left), _ => {} } }
            return parse(buf, idle);     // another sequence: ignored
        }
        buf.remove(0); buf.remove(0); return parse(buf, idle);   // Alt+key: nothing
    }
    // a UTF-8 character
    let len = if b0 < 0x80 { 1 } else if b0 >= 0xf0 { 4 } else if b0 >= 0xe0 { 3 } else if b0 >= 0xc0 { 2 } else { 1 };
    if buf.len() < len { return None; }
    let bytes: Vec<u8> = buf.drain(..len).collect();
    let s = String::from_utf8_lossy(&bytes); let c = s.chars().next().unwrap_or(' ');
    Some(match c { '\n' | '\r' => Ev::Enter, '\u{7f}' | '\u{8}' => Ev::Backspace, '\t' => Ev::Tab, c => Ev::Key(c) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn all(s: &[u8]) -> Vec<Ev> { let mut b = s.to_vec(); let mut v = Vec::new(); while let Some(e) = parse(&mut b, true) { v.push(e); } v }
    #[test] fn keys() { assert_eq!(all(b"ax\r"), vec![Ev::Key('a'), Ev::Key('x'), Ev::Enter]); assert_eq!(all("ü".as_bytes()), vec![Ev::Key('ü')]); }
    #[test] fn arrows() { assert_eq!(all(b"\x1b[A\x1b[B\x1bOC\x1b[D"), vec![Ev::Up, Ev::Down, Ev::Right, Ev::Left]); }
    #[test] fn lone_esc() { let mut b = vec![0x1b]; assert_eq!(parse(&mut b, false), None); assert_eq!(parse(&mut b, true), Some(Ev::Esc)); }
    #[test] fn sgr_mouse() { assert_eq!(all(b"\x1b[<0;47;30M"), vec![Ev::Mouse { b: 0, x: 47, y: 30, press: true }]); assert_eq!(all(b"\x1b[<35;5;7m"), vec![Ev::Mouse { b: 35, x: 5, y: 7, press: false }]); }
    #[test] fn ignored() { assert_eq!(all(b"\x1b[1;5C\x1b[200~x"), vec![Ev::Key('x')]); assert_eq!(all(b"\x1bfq"), vec![Ev::Key('q')]); }
    #[test] fn incomplete() { let mut b = b"\x1b[<0;4".to_vec(); assert_eq!(parse(&mut b, false), None); b.extend_from_slice(b"7;30M"); assert!(matches!(parse(&mut b, false), Some(Ev::Mouse { .. }))); }
}
