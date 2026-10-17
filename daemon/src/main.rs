//! bhoted prototype: the topic index of bhote (`topics.idx`), same format as `topics_index_build` in the bash script.
//!   bhoted index <topic dir> <out>      build once
//!   bhoted run   <topic dir> <out>      keep it current: rebuild when a topic file is added, removed or changed
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const US: char = '\u{1f}';
const KEYS: &str = "title status waiting_for waiting_since updated deleted description agent agent_machine agent_pane agent_session delegated transfer_to_name slack_label slack_msg agent_branch project kind deploy_wait dismissed auto_now auto review_src";

fn topic_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "topic")).collect())
        .unwrap_or_default();
    v.sort(); // the shell glob sorts by name
    v
}

/// One line per file: path, then the first value of each key (like the awk: the first one counts, also when it is empty).
fn record(path: &Path, keys: &[&str]) -> String {
    let text = fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let (k, v) = match line.find('=') { Some(i) => (&line[..i], &line[i + 1..]), None => (line, "") };
        seen.entry(k).or_insert(v);
    }
    let mut out = path.to_string_lossy().into_owned();
    for k in keys { out.push(US); out.push_str(seen.get(k).copied().unwrap_or("")); }
    out
}

fn build(dir: &Path, out: &Path, token: &str) -> std::io::Result<()> {
    let keys: Vec<&str> = KEYS.split(' ').collect();
    let files = topic_files(dir);
    let mut body = format!("#{} {} {}\n", files.len(), dir.display(), token);
    for f in &files { body.push_str(&record(f, &keys)); body.push('\n'); }
    let tmp = out.with_extension(format!("tmp.{}", std::process::id()));
    fs::File::create(&tmp)?.write_all(body.as_bytes())?;
    fs::rename(&tmp, out)
}

fn signature(dir: &Path) -> Vec<(PathBuf, u128, u64)> {
    topic_files(dir).into_iter().map(|p| {
        let m = fs::metadata(&p).ok();
        let t = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        (p, t, m.map_or(0, |m| m.len()))
    }).collect()
}

fn token() -> String {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{}{}", std::process::id(), n)
}

extern "C" { fn kill(pid: i32, sig: i32) -> i32; }

/// Waits until something in `dir` changed (inotify, no process, no polling), at most `ms`; true when it may have changed.
/// Without inotify (macOS here) it sleeps a short step and says "maybe" (the caller compares the file list).
#[cfg(target_os = "linux")]
mod watch {
    use std::os::unix::io::RawFd;
    extern "C" {
        fn inotify_init1(flags: i32) -> i32;
        fn inotify_add_watch(fd: i32, path: *const std::os::raw::c_char, mask: u32) -> i32;
        fn poll(fds: *mut PollFd, n: u64, timeout: i32) -> i32;
        fn close(fd: i32) -> i32;
    }
    #[repr(C)] struct PollFd { fd: i32, events: i16, revents: i16 }
    pub struct Watch(RawFd);
    pub fn open(dir: &std::path::Path) -> Option<Watch> {
        let fd = unsafe { inotify_init1(0o2000000 | 0o4000) }; // IN_CLOEXEC | IN_NONBLOCK
        if fd < 0 { return None; }
        let c = std::ffi::CString::new(dir.to_string_lossy().as_bytes()).ok()?;
        // IN_MODIFY | IN_ATTRIB | IN_CLOSE_WRITE | IN_MOVED_FROM | IN_MOVED_TO | IN_CREATE | IN_DELETE
        let wd = unsafe { inotify_add_watch(fd, c.as_ptr(), 0x2 | 0x4 | 0x8 | 0x40 | 0x80 | 0x100 | 0x200) };
        if wd < 0 { unsafe { close(fd); } return None; }
        Some(Watch(fd))
    }
    impl Watch {
        pub fn wait(&self, ms: i32) -> bool {
            let mut p = PollFd { fd: self.0, events: 1, revents: 0 };
            let n = unsafe { poll(&mut p, 1, ms) };
            if n <= 0 { return false; }
            let mut buf = [0u8; 8192];
            {
                use std::io::Read; use std::os::unix::io::FromRawFd;
                let mut f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(self.0) });
                while matches!(f.read(&mut buf), Ok(n) if n > 0) {}
            }   // drain: many events are one change
            true
        }
    }
    impl Drop for Watch { fn drop(&mut self) { unsafe { close(self.0); } } }
}
fn alive(pid: i32) -> bool { unsafe { kill(pid, 0) == 0 } }

/// bhoted run <topic dir> <out> [--parent <pid>] [--pid-file <file>]: ends when the parent (the lead collector) is gone.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 { eprintln!("usage: bhoted index|run <topic dir> <out> [--parent <pid>] [--pid-file <file>]"); std::process::exit(2); }
    let opt = |name: &str| a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned();
    let parent: Option<i32> = opt("--parent").and_then(|v| v.parse().ok());
    let pid_file = opt("--pid-file");
    let (dir, out) = (Path::new(&a[2]), Path::new(&a[3]));
    match a[1].as_str() {
        "index" => build(dir, out, &token()).unwrap_or_else(|e| { eprintln!("bhoted: {e}"); std::process::exit(1) }),
        "run" => {
            let mut last = Vec::new();
            if let Some(f) = &pid_file { let _ = fs::write(f, format!("{} {}\n", std::process::id(), dir.display())); }
            #[cfg(target_os = "linux")]
            let w = watch::open(dir);
            loop {
                if let Some(p) = parent { if !alive(p) { if let Some(f) = &pid_file { let _ = fs::remove_file(f); } return; } }
                let sig = signature(dir);
                if sig != last || !out.exists() { if build(dir, out, &token()).is_ok() { last = sig; } }
                #[cfg(target_os = "linux")]
                {
                    if let Some(w) = &w {
                        if w.wait(1000) { std::thread::sleep(Duration::from_millis(15)); } // a burst of writes: one rebuild
                        let _ = w.wait(0);
                        // the signature check below still decides (a change that left no event costs one look per second)
                        continue;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        _ => { eprintln!("usage: bhoted index|run <topic dir> <out>"); std::process::exit(2); }
    }
}
