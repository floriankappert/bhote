//! A child process on a pseudo-terminal: openpty, setsid + controlling terminal, resize, a reader thread.
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct Pty { pub master: File, child: Option<Child> }

pub fn set_size(fd: i32, cols: u16, rows: u16) {
    let ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(fd, libc::TIOCSWINSZ as _, &ws); }
}

impl Pty {
    /// `argv` empty: the user's shell (login shell of $SHELL, else /bin/sh).
    pub fn spawn(argv: &[String], cwd: Option<&Path>, cols: u16, rows: u16, env: &[(String, String)]) -> io::Result<Pty> {
        let (mut m, mut s) = (-1i32, -1i32);
        let mut ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
        if unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null_mut(), &mut ws as *mut _ as _) } != 0 { return Err(io::Error::last_os_error()); }
        let (master, slave) = unsafe { (OwnedFd::from_raw_fd(m), OwnedFd::from_raw_fd(s)) };
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC); libc::fcntl(slave.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC); }
        let mut cmd = if argv.is_empty() {
            let sh = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
            Command::new(sh)
        } else { let mut c = Command::new(&argv[0]); c.args(&argv[1..]); c };
        if let Some(d) = cwd { if d.is_dir() { cmd.current_dir(d); } }
        cmd.env("TERM", "xterm-256color").env("COLORTERM", "truecolor").env_remove("TERM_PROGRAM");
        // what the server inherited from the session it was started in is not the panes' (a program started there would think it is a child of that Claude session and save no transcript)
        for k in ["CLAUDECODE", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_SESSION_ATTENDED", "CLAUDE_CODE_MESSAGING_SOCKET", "CLAUDE_CODE_MESSAGING_TOKEN", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_EXECPATH", "CLAUDE_PID", "CLAUDE_EFFORT", "HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID", "HERDR_ENV"] { cmd.env_remove(k); }
        for (k, v) in env { cmd.env(k, v); }
        cmd.stdin(Stdio::from(slave.try_clone()?)).stdout(Stdio::from(slave.try_clone()?)).stderr(Stdio::from(slave));
        unsafe { cmd.pre_exec(|| { libc::setsid(); libc::ioctl(0, libc::TIOCSCTTY as _, 0); Ok(()) }); }
        let child = cmd.spawn()?;
        Ok(Pty { master: File::from(master), child: Some(child) })
    }

    pub fn resize(&self, cols: u16, rows: u16) { set_size(self.master.as_raw_fd(), cols, rows); }
    pub fn pid(&self) -> u32 { self.child.as_ref().map_or(0, |c| c.id()) }
    /// A handle for a thread that reads the child's output (the descriptor is duplicated).
    pub fn reader(&self) -> io::Result<File> { self.master.try_clone() }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            unsafe { libc::kill(c.id() as i32, libc::SIGHUP); }
            std::thread::spawn(move || { let _ = c.wait(); });
        }
    }
}

/// Reads until the child closes the terminal; the chunks go to `f`, which returns false to stop.
pub fn read_loop(mut f: File, mut sink: impl FnMut(Vec<u8>) -> bool) {
    let mut buf = vec![0u8; 65536];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => { if !sink(buf[..n].to_vec()) { break; } }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}
