//! Child processes and guards that keep a long-running daemon healthy: a time limit for every call, no zombies, no flag that stays set
//! after a panic, files read with a cap, and the check that the shared folder is ours.
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::{Duration, Instant};

/// Runs a command with a time limit: (exit code, stdout, stderr); None when it could not start. A call that takes too long is killed (code -9).
pub fn run(cmd: &str, args: &[&str], secs: u64, stdin: Option<&[u8]>) -> Option<(i32, Vec<u8>, Vec<u8>)> {
    // `herdr ...` goes to the native mux when that is the multiplexer (no process, no herdr needed)
    if cmd == "herdr" && crate::mux::backend::native() {
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (code, so, se) = crate::mux::herdrcli::exec(&a);
        return Some((code, so.into_bytes(), se.into_bytes()));
    }
    let mut c = Command::new(cmd); c.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() { c.stdin(Stdio::piped()); } else { c.stdin(Stdio::null()); }
    let mut ch = c.spawn().ok()?;
    if let Some(d) = stdin { use std::io::Write; if let Some(mut s) = ch.stdin.take() { let d = d.to_vec(); std::thread::spawn(move || { let _ = s.write_all(&d); }); } }
    let (mut so, mut se) = (ch.stdout.take()?, ch.stderr.take()?);
    let h1 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = so.by_ref().take(64 << 20).read_to_end(&mut b); b });
    let h2 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = se.by_ref().take(1 << 20).read_to_end(&mut b); b });
    let t0 = Instant::now();
    let code = loop {
        match ch.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(1),
            Ok(None) if t0.elapsed() < Duration::from_secs(secs) => std::thread::sleep(Duration::from_millis(5)),
            _ => { let _ = ch.kill(); let _ = ch.wait(); break -9; }
        }
    };
    Some((code, h1.join().unwrap_or_default(), h2.join().unwrap_or_default()))
}

/// Starts a command and does not wait for it, but reaps it (a daemon that spawns and forgets leaves zombies)
pub fn detached(mut c: Command) {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Ok(mut ch) = c.spawn() { std::thread::spawn(move || { let _ = ch.wait(); }); }
}

/// A flag that is cleared when it goes out of scope, also when the thread panics
pub struct Flag(pub Arc<AtomicBool>);
impl Drop for Flag { fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); } }

/// Runs a part of the daemon's or the panel's work so that a panic in it is logged (shared/bhoted.log, kept small) and does not end the process
pub fn guarded<R>(what: &str, shared: &Path, f: impl FnOnce() -> R) -> Option<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => Some(r),
        Err(e) => {
            let msg = e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default();
            let log = shared.join("bhoted.log");
            if fs::metadata(&log).map_or(false, |m| m.len() > 200_000) { let _ = fs::rename(&log, shared.join("bhoted.log.1")); }
            use std::io::Write;
            if let Ok(mut o) = fs::OpenOptions::new().append(true).create(true).open(&log) { let _ = writeln!(o, "{} panic in {}: {}", crate::store::now(), what, msg); }
            None
        }
    }
}

/// A line in the daemon's log (shared/bhoted.log, kept small)
pub fn note(shared: &Path, msg: &str) {
    let log = shared.join("bhoted.log");
    if fs::metadata(&log).map_or(false, |m| m.len() > 200_000) { let _ = fs::rename(&log, shared.join("bhoted.log.1")); }
    use std::io::Write;
    if let Ok(mut o) = fs::OpenOptions::new().append(true).create(true).open(&log) { let _ = writeln!(o, "{} {}", crate::store::now(), msg); }
}

/// fs::read_to_string with a cap (a file of the data folder or the shared folder that is absurdly big is read as empty)
pub fn read_cap<P: AsRef<Path>>(p: P) -> std::io::Result<String> {
    let p = p.as_ref();
    if fs::metadata(p).map_or(false, |m| m.len() > 16 << 20) { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "too big")); }
    fs::read_to_string(p)
}

extern "C" { fn geteuid() -> u32; }
/// The shared folder must be ours and closed to others (it holds the agent lists, the lock files, the maps): else it is refused. Returns an error text.
pub fn check_private_dir(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if p.as_os_str().is_empty() { return Err("no shared folder".into()); }
    let m = fs::symlink_metadata(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    if m.file_type().is_symlink() || !m.is_dir() { return Err(format!("{} is not a plain folder", p.display())); }
    if m.uid() != unsafe { geteuid() } { return Err(format!("{} belongs to another user", p.display())); }
    if m.mode() & 0o077 != 0 { fs::set_permissions(p, fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", p.display(), e))?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_call_that_takes_too_long_is_killed() {
        let t0 = Instant::now(); let r = run("sleep", &["5"], 1, None).unwrap();
        assert_eq!(r.0, -9); assert!(t0.elapsed() < Duration::from_secs(3));
    }
    #[test] fn output_and_stdin() {
        let r = run("cat", &[], 5, Some(b"hello")).unwrap(); assert_eq!((r.0, r.1.as_slice()), (0, &b"hello"[..]));
        assert!(run("/nonexistent/x", &[], 1, None).is_none());
    }
    #[test] fn a_panic_is_logged_and_does_not_end_the_caller() {
        let d = std::env::temp_dir().join(format!("proc-test-{}", std::process::id())); let _ = fs::create_dir_all(&d);
        let r: Option<i32> = guarded("test", &d, || { if true { panic!("boom"); } 1 });
        assert!(r.is_none()); assert!(fs::read_to_string(d.join("bhoted.log")).unwrap().contains("boom"));
        let _ = fs::remove_dir_all(&d);
    }
    #[test] fn a_flag_is_cleared_after_a_panic_in_its_thread() {
        let f = Arc::new(AtomicBool::new(true)); let g = f.clone();
        let h = std::thread::spawn(move || { let _x = Flag(g); panic!("thread"); });
        let _ = h.join(); assert!(!f.load(Ordering::SeqCst));
    }
    #[test] fn the_shared_folder_is_checked() {
        let d = std::env::temp_dir().join(format!("proc-dir-{}", std::process::id())); let _ = fs::create_dir_all(&d);
        use std::os::unix::fs::PermissionsExt; fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_private_dir(&d).is_ok()); assert_eq!(fs::metadata(&d).unwrap().permissions().mode() & 0o777, 0o700);   // closed to others
        let l = std::env::temp_dir().join(format!("proc-link-{}", std::process::id())); let _ = std::os::unix::fs::symlink(&d, &l);
        assert!(check_private_dir(&l).is_err());   // a link is not taken
        let _ = fs::remove_file(&l); let _ = fs::remove_dir_all(&d);
        assert!(check_private_dir(Path::new("/")).is_err());   // root's folder: not ours
    }
    #[test] fn a_file_that_is_too_big_is_not_read() {
        let f = std::env::temp_dir().join(format!("proc-big-{}", std::process::id()));
        { let h = fs::File::create(&f).unwrap(); h.set_len(17 << 20).unwrap(); }
        assert!(read_cap(&f).is_err()); let _ = fs::remove_file(&f);
    }
}

/// `herdr <args>` without waiting and without output (a focus, a notification): the native mux or herdr, as `run` decides.
pub fn herdr_detached(args: &[&str]) {
    if crate::mux::backend::native() { let a: Vec<String> = args.iter().map(|s| s.to_string()).collect(); std::thread::spawn(move || { let _ = crate::mux::herdrcli::exec(&a); }); return; }
    let mut c = Command::new("herdr"); c.args(args); detached(c);
}
