//! `bhote attach`: a thin client. Raw mode, bytes in (to the server), bytes out (to the terminal), size changes.
use super::proto::{self, read_frame, write_frame};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Connects; when no server listens, starts one (`bhoted mux`, detached) and waits for its socket.
pub fn connect_or_start() -> std::io::Result<UnixStream> {
    let path = proto::sock_path();
    if let Ok(s) = UnixStream::connect(&path) { return Ok(s); }
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.arg("mux").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }); }
    cmd.spawn()?;
    let t = Instant::now();
    loop {
        match UnixStream::connect(&path) {
            Ok(s) => return Ok(s),
            Err(e) if t.elapsed() > Duration::from_secs(5) => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(40)),
        }
    }
}

fn winsize() -> (u16, u16) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(1, libc::TIOCGWINSZ as _, &mut ws) } == 0 && ws.ws_col > 0 && ws.ws_row > 0 { (ws.ws_col, ws.ws_row) } else { (80, 24) }
}

pub struct Raw { saved: libc::termios }
impl Raw {
    pub fn enter() -> Option<Raw> {
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut t) } != 0 { return None; }
        let saved = t;
        unsafe { libc::cfmakeraw(&mut t); libc::tcsetattr(0, libc::TCSANOW, &t); }
        let mut o = std::io::stdout();
        let _ = o.write_all(b"\x1b[?1049h\x1b[2J"); let _ = o.flush();
        Some(Raw { saved })
    }
}
impl Drop for Raw {
    fn drop(&mut self) {
        // every mode the panes may have switched on goes back to the terminal's defaults
        let mut o = std::io::stdout();
        let _ = o.write_all(b"\x1b[0m\x1b[?25h\x1b[?2004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1l\x1b>\x1b[?1049l");
        let _ = o.flush();
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &self.saved); }
    }
}

pub fn attach() -> i32 {
    if unsafe { libc::isatty(0) } != 1 || unsafe { libc::isatty(1) } != 1 { eprintln!("bhote attach needs a terminal"); return 1; }
    let stream = match connect_or_start() { Ok(s) => s, Err(e) => { eprintln!("bhote: no mux server: {e}"); return 1; } };
    let (cols, rows) = winsize();
    let mut r = BufReader::new(stream.try_clone().expect("clone"));
    let mut w = stream;
    let _ = writeln!(w, "{}", json!({"id": "a", "method": "attach", "params": {"cols": cols, "rows": rows}}));
    let mut line = String::new();
    if r.read_line(&mut line).unwrap_or(0) == 0 || !line.contains("\"attached\":true") { eprintln!("bhote: attach failed: {}", line.trim()); return 1; }
    let raw = Raw::enter();
    // keys
    let mut wi = w.try_clone().expect("clone");
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut stdin = std::io::stdin().lock();
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => { let _ = write_frame(&mut wi, b'D', &[]); return; }
                Ok(n) => if write_frame(&mut wi, b'I', &buf[..n]).is_err() { return; },
            }
        }
    });
    // size changes (a poll: no signal handler to get wrong)
    let mut wr = w.try_clone().expect("clone");
    std::thread::spawn(move || {
        let mut last = (cols, rows);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let now = winsize();
            if now != last { last = now; if write_frame(&mut wr, b'R', &proto::resize_payload(now.0, now.1)).is_err() { return; } }
        }
    });
    let mut out = std::io::stdout();
    let reason = loop {
        match read_frame(&mut r) {
            Ok((b'O', p)) => { let _ = out.write_all(&p); let _ = out.flush(); }
            Ok((b'Q', p)) => break String::from_utf8_lossy(&p).into_owned(),
            Ok(_) => {}
            Err(_) => break "lost connection".into(),
        }
    };
    drop(raw);
    println!("[{reason}]");
    if reason == "lost connection" { 1 } else { 0 }
}

/// One control request: `bhoted mux-call <method> [params json]`. Starts no server.
pub fn call(method: &str, params: Value) -> Result<Value, String> {
    let s = UnixStream::connect(proto::sock_path()).map_err(|e| format!("no mux server: {e}"))?;
    let mut w = s.try_clone().map_err(|e| e.to_string())?;
    writeln!(w, "{}", json!({"id": "1", "method": method, "params": params})).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).map_err(|e| e.to_string())?;
    serde_json::from_str(line.trim()).map_err(|e| format!("bad response: {e}"))
}

pub fn call_main(args: &[String]) -> i32 {
    let params = args.get(1).map(|p| serde_json::from_str(p).unwrap_or(Value::Null)).unwrap_or_else(|| json!({}));
    match call(&args[0], params) {
        Ok(v) => { println!("{v}"); if v.get("error").is_some() { 1 } else { 0 } }
        Err(e) => { eprintln!("bhote: {e}"); 1 }
    }
}

/// `bhote mux stop|status|restart`: the server of this user (the session stays: the next start restores it).
pub fn control_cmd(what: &str) -> i32 {
    let running = call("ping", json!({})).ok().and_then(|v| v.pointer("/result/pid").and_then(|p| p.as_u64()));
    match (what, running) {
        ("status", Some(pid)) => { println!("mux server running (pid {pid}, {})", proto::sock_path().display()); 0 }
        ("status", None) => { println!("no mux server running"); 1 }
        ("stop", None) => { println!("no mux server running"); 0 }
        ("stop", Some(_)) | ("restart", Some(_)) => {
            let _ = call("server.stop", json!({}));
            for _ in 0..50 { if call("ping", json!({})).is_err() { break; } std::thread::sleep(Duration::from_millis(100)); }
            println!("mux server stopped (the session is kept)");
            if what == "restart" { return match connect_or_start() { Ok(_) => { println!("mux server started"); 0 } Err(e) => { eprintln!("bhote: {e}"); 1 } }; }
            0
        }
        ("restart", None) => match connect_or_start() { Ok(_) => { println!("mux server started"); 0 } Err(e) => { eprintln!("bhote: {e}"); 1 } },
        _ => 2,
    }
}
