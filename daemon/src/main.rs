//! bhoted prototype: the topic index of bhote (`topics.idx`), same format as `topics_index_build` in the bash script.
//!   bhoted index <topic dir> <out>      build once
//!   bhoted run   <topic dir> <out>      keep it current: rebuild when a topic file is added, removed or changed
mod agents;
mod monitor;
mod store;
mod ui;
mod term;
mod agentsui;
mod theme;
mod cards;
mod follow;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const US: char = '\u{1f}';
const KEYS: &str = "title status waiting_for waiting_since updated deleted description agent agent_machine agent_pane agent_session delegated transfer_to_name slack_label slack_msg agent_branch project kind deploy_wait dismissed auto_now auto review_src";

fn topic_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "topic")).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| ui::coll(&a.file_name().unwrap_or_default().to_string_lossy(), &b.file_name().unwrap_or_default().to_string_lossy())); // the shell glob sorts by name, in the locale's collation
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

/// One round of the follow-ups. Shadow: only logged (shared/shadow/rust.log: "round|<time>" and one line per action); on: done.
fn follow_round(shared: &Path, data: &Path, topics: &Path, host: &str, cfg: &HashMap<String, String>, shadow: bool) {
    let env = store::Env { topics: topics.to_path_buf(), shared: shared.to_path_buf(), data: data.to_path_buf() };
    let c = follow::context(&env, host, cfg.clone(), shadow);
    if shadow {
        let mut s = store::Shadow::new(&env);
        follow::round(&c, &mut s, None);
        if s.log.is_empty() { return; }
        let dir = shared.join("shadow"); let _ = fs::create_dir_all(&dir);
        let f = dir.join("rust.log");
        if fs::metadata(&f).map_or(false, |m| m.len() > 1_000_000) { let _ = fs::rename(&f, dir.join("rust.log.1")); }
        let ts = store::now(); let mut body = String::new();
        for l in &s.log { body.push_str(&format!("{}|@{}\n", l, ts)); }
        if let Ok(mut o) = fs::OpenOptions::new().append(true).create(true).open(&f) { let _ = o.write_all(body.as_bytes()); }
    } else {
        let mut s = store::Live { env: &env, notify_on: cfg.get("NOTIFY").map_or(true, |v| v == "on"), n: 0 };
        follow::round(&c, &mut s, None);
    }
}

/// The settings file: KEY=value lines, the last value of a key counts (cfg_var in the script).
fn config_read(file: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for line in fs::read_to_string(file).unwrap_or_default().lines() {
        if let Some(i) = line.find('=') { m.insert(line[..i].to_string(), line[i + 1..].to_string()); }
    }
    m
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
    /// One inotify descriptor over several folders; `wait` says which folders changed and which names appeared in them.
    pub struct Watch { fd: RawFd, wds: Vec<(i32, usize)> }
    pub fn open(dirs: &[(&std::path::Path, u32)]) -> Option<Watch> {
        let fd = unsafe { inotify_init1(0o2000000 | 0o4000) }; // IN_CLOEXEC | IN_NONBLOCK
        if fd < 0 { return None; }
        let mut wds = Vec::new();
        for (i, (dir, mask)) in dirs.iter().enumerate() {
            let c = std::ffi::CString::new(dir.to_string_lossy().as_bytes()).ok()?;
            let wd = unsafe { inotify_add_watch(fd, c.as_ptr(), *mask) };
            if wd < 0 { unsafe { close(fd); } return None; }
            wds.push((wd, i));
        }
        Some(Watch { fd, wds })
    }
    /// IN_MODIFY | IN_ATTRIB | IN_CLOSE_WRITE | IN_MOVED_FROM | IN_MOVED_TO | IN_CREATE | IN_DELETE
    pub const TOPICS: u32 = 0x2 | 0x4 | 0x8 | 0x40 | 0x80 | 0x100 | 0x200;
    /// IN_CREATE | IN_MOVED_TO (a poke is a file that appears)
    pub const APPEAR: u32 = 0x100 | 0x80;
    impl Watch {
        /// (index of the folder, name) of every event; empty after the timeout.
        pub fn wait(&self, ms: i32) -> Vec<(usize, String)> {
            let mut p = PollFd { fd: self.fd, events: 1, revents: 0 };
            let mut out = Vec::new();
            if unsafe { poll(&mut p, 1, ms) } <= 0 { return out; }
            use std::io::Read; use std::os::unix::io::FromRawFd;
            let mut f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(self.fd) });
            let mut buf = [0u8; 16384];
            while let Ok(n) = f.read(&mut buf) {
                if n == 0 { break; }
                let mut o = 0;
                while o + 16 <= n {
                    let wd = i32::from_ne_bytes(buf[o..o + 4].try_into().unwrap());
                    let len = u32::from_ne_bytes(buf[o + 12..o + 16].try_into().unwrap()) as usize;
                    let name = buf[o + 16..(o + 16 + len).min(n)].split(|b| *b == 0).next().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
                    if let Some((_, i)) = self.wds.iter().find(|(w, _)| *w == wd) { out.push((*i, name)); }
                    o += 16 + len;
                }
            }
            out
        }
    }
    impl Drop for Watch { fn drop(&mut self) { unsafe { close(self.fd); } } }
}

fn alive(pid: i32) -> bool { unsafe { kill(pid, 0) == 0 } }

/// bhoted run <topic dir> <out> [--parent <pid>] [--pid-file <file>]: ends when the parent (the lead collector) is gone.
fn main() {
    ui::init_locale();
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 { eprintln!("usage: bhoted index|run <topic dir> <out> [--parent <pid>] [--pid-file <file>]"); std::process::exit(2); }
    let opt = |name: &str| a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned();
    let parent: Option<i32> = opt("--parent").and_then(|v| v.parse().ok());
    let pid_file = opt("--pid-file");
    let (dir, out) = (Path::new(&a[2]), Path::new(&a[3]));
    match a[1].as_str() {
        "index" => build(dir, out, &token()).unwrap_or_else(|e| { eprintln!("bhoted: {e}"); std::process::exit(1) }),
        "fetch" => { // bhoted fetch <shared dir> <data dir> --config <file> --kind deploys|tests (for the parity test)
            let ctx = monitor::Ctx { shared: PathBuf::from(&a[2]), data: PathBuf::from(&a[3]), topics: PathBuf::from(&a[3]).join("topics"), cfg: opt("--config").map(|c| config_read(&c)).unwrap_or_default() };
            monitor::fetch(&ctx, &opt("--kind").unwrap_or_else(|| "deploys".into()));
        }
        "follow" => { // bhoted follow <topic dir> --shared S --data D --host H --config C [--shadow] [--only <function>]: one round of the follow-ups
            let env = store::Env { topics: PathBuf::from(&a[2]), shared: PathBuf::from(opt("--shared").unwrap_or_default()), data: PathBuf::from(opt("--data").unwrap_or_default()) };
            let cfg = opt("--config").map(|c| config_read(&c)).unwrap_or_default();
            let shadow = a.iter().any(|x| x == "--shadow");
            let c = follow::context(&env, &opt("--host").unwrap_or_default(), cfg.clone(), shadow);
            let only = opt("--only");
            if shadow { let mut s = store::Shadow::new(&env); follow::round(&c, &mut s, only.as_deref()); for l in s.log { println!("{}", l); } }
            else { let mut s = store::Live { env: &env, notify_on: cfg.get("NOTIFY").map_or(true, |v| v == "on"), n: 0 }; follow::round(&c, &mut s, only.as_deref()); }
        }
        "block" => { // bhoted block <name> --cols N --now T --config F --shared S --topics T --colors F: one block of the frame, as the script draws it
            ui::init_locale();
            let ctx = ui::Ctx { cols: opt("--cols").and_then(|v| v.parse().ok()).unwrap_or(44), now: opt("--now").and_then(|v| v.parse().ok()).unwrap_or_else(store::now),
                shared: PathBuf::from(opt("--shared").unwrap_or_default()), topics: PathBuf::from(opt("--topics").unwrap_or_default()),
                cfg: opt("--config").map(|c| config_read(&c)).unwrap_or_default(), c: ui::Colors::read(&opt("--colors").unwrap_or_default()) };
            let view = cards::View::read(&opt("--view").unwrap_or_default());
            if a[2] == "head" { let out = agentsui::head(&ctx, &view.head_view()); use std::io::Write; let _ = std::io::stdout().write_all(out.as_bytes()); return; }
            if a[2] == "bottom" {   // bhoted block bottom --run DIR: the agents, the dash line and the keys; agrows and agmap into the run folder
                let mut cards = cards::Cards::new(&ctx, &view); cards.rows_total = view.rows;
                let mc = agentsui::machine_codes(&ctx.topics, &view.host, &cards.agent_rows().iter().map(|r| r.m.clone()).collect::<Vec<_>>());
                let b = agentsui::bottom(&cards, &agentsui::agent_view(&view, &ctx.cfg, &ctx.shared), if cards.agent_rows().iter().map(|r| r.m.clone()).collect::<std::collections::HashSet<_>>().len() > 1 || true { &mc } else { &mc });
                let run = PathBuf::from(opt("--run").unwrap_or_default());
                let _ = fs::write(run.join("agrows"), &b.agrows); let _ = fs::write(run.join("agmap"), &b.agmap);
                use std::io::Write; let _ = std::io::stdout().write_all(b.text.as_bytes()); return;
            }
            if a[2] == "frame" {   // the whole main view: bhoted block frame --head F --bottom F --rows N --run DIR
                let cards = cards::Cards::new(&ctx, &view);
                let lay = cards::Layout { cards: &cards, rows: opt("--rows").and_then(|v| v.parse().ok()).unwrap_or(40), head: fs::read_to_string(opt("--head").unwrap_or_default()).unwrap_or_default(), bottom: fs::read_to_string(opt("--bottom").unwrap_or_default()).unwrap_or_default(), run_dir: PathBuf::from(opt("--run").unwrap_or_default()) };
                let out = lay.view(); use std::io::Write; let _ = std::io::stdout().write_all(out.as_bytes()); return;
            }
            let out = match a[2].as_str() { "monitors" => ui::monitors(&ctx), "cards" => cards::Cards::new(&ctx, &view).block("cards"), "lists" => cards::Cards::new(&ctx, &view).block("lists"), "done" => cards::Cards::new(&ctx, &view).done_block(opt("--max").and_then(|v| v.parse().ok()).unwrap_or(4)), _ => { eprintln!("bhoted: no block {}", a[2]); std::process::exit(2) } };
            use std::io::Write; let _ = std::io::stdout().write_all(out.as_bytes());
        }
        "paint" => { // bhoted paint --rows N --run DIR <frame on stdin>: what bhote_paint makes of a frame (the markers out, the line ends, the map of the clickable rows); status 3: an edit field (bash does it)
            use std::io::{Read, Write};
            let mut text = String::new(); let _ = std::io::stdin().read_to_string(&mut text);
            let rows: usize = opt("--rows").and_then(|v| v.parse().ok()).unwrap_or(40);
            let text = text.trim_end_matches('\n');
            let (mut fl, mut fr): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new()); let mut sel: i64 = -1;
            for (n, l0) in text.split('\n').enumerate() {
                let mut line = l0.to_string(); let mut rf = String::new();
                if let Some(r) = line.strip_prefix('\u{1d}') { let i = r.find('\u{1d}').unwrap_or(r.len()); rf = r[..i].to_string(); line = r[(i + 1).min(r.len())..].to_string(); }
                if let Some(r) = line.strip_prefix('\u{1}') { sel = n as i64; line = r.to_string(); }
                if line.starts_with('\u{2}') { std::process::exit(3); }
                fl.push(line); fr.push(rf);
            }
            let n = fl.len();
            let mut off = 0usize;
            if n > rows && sel >= 0 { let mut o = sel - (rows as i64) / 2; if o > n as i64 - rows as i64 { o = n as i64 - rows as i64; } if o < 0 { o = 0; } off = o as usize; }
            let (mut out, mut map) = (String::new(), String::new());
            for i in off..n.min(off + rows) {
                if i > off { out.push('\n'); }
                out.push_str(&fl[i]); out.push_str("\u{1b}[K");
                if !fr[i].is_empty() { map.push_str(&format!("{}\u{1e}{}\n", i - off + 1, fr[i])); }
            }
            let _ = fs::write(PathBuf::from(opt("--run").unwrap_or_default()).join("pmap"), map);
            let _ = std::io::stdout().write_all(format!("\u{1b}[H{}\u{1b}[J", out).as_bytes());
        }
        "theme" => { // bhoted theme --config F --head TEXT [--nocolor]: the colour variables (NAME=value) and HEAD_GRAD, as colors_dump and ui_load make them
            let cfg = opt("--config").map(|c| config_read(&c)).unwrap_or_default();
            let t = theme::apply(&cfg, !a.iter().any(|x| x == "--nocolor"), &opt("--head").unwrap_or_default());
            for k in ["X", "R", "NN", "S", "C", "L", "D", "BOLD", "B", "Y", "GREEN", "BLUE", "ORANGE", "MUTED", "PINK", "TEAL", "SELBG", "CARDBG", "CARDSEL", "CARDHOV", "CARDSUB", "CARDBTN", "LSEL", "DSEL", "DESC", "DESCSEL", "TITLE", "TITLESEL"] { println!("{}={}", k, t.vars.get(k).cloned().unwrap_or_default()); }
            println!("HEAD_GRAD={}", t.head_grad);
        }
        "collect-remote" => { // bhoted collect-remote <shared dir> <host> --config <file>: one look at the other machines through kharka (the parity test)
            let cfg = opt("--config").map(|c| config_read(&c)).unwrap_or_default();
            let bin = std::env::var("KHARKA").ok().filter(|v| !v.is_empty()).or_else(|| cfg.get("KHARKA").cloned().filter(|v| !v.is_empty())).unwrap_or_else(|| "kharka".into());
            let ok = agents::collect_remote_kharka(Path::new(&a[2]), &a[3], &bin, cfg.get("MACHINES_OFF").map_or("", |s| s.as_str()), cfg.get("TESTS_BUSY").map_or(true, |v| v != "off"));
            std::process::exit(if ok { 0 } else { 1 })
        }
        "collect" => { // bhoted collect <shared dir> <host>: one collection of this machine (for the parity test)
            let tests_busy = opt("--config").map(|c| config_read(&c)).and_then(|c| c.get("TESTS_BUSY").cloned()).map_or(true, |v| v != "off");
            let ok = agents::collect_local(Path::new(&a[2]), &a[3], tests_busy);
            std::process::exit(if ok { 0 } else { 1 })
        }
        "run" => {
            let shared = out.parent().unwrap_or(Path::new(".")).to_path_buf();
            let host = opt("--host").unwrap_or_default();
            let config = opt("--config");
            // the agents duty needs inotify (the poke file wakes it): Linux only for now, bash keeps it on macOS
            let duty = a.iter().any(|x| x == "--agents") && cfg!(target_os = "linux") && !host.is_empty();
            let monitors = a.iter().any(|x| x == "--monitors");
            let data = opt("--data").map(PathBuf::from).unwrap_or_else(|| dir.parent().unwrap_or(Path::new(".")).to_path_buf());
            let running = [Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false))];
            let mut next_fetch = [Instant::now(), Instant::now()];
            let remote_duty = a.iter().any(|x| x == "--remote") && cfg!(target_os = "linux") && !host.is_empty();
            let remote_running = Arc::new(AtomicBool::new(false));
            let mut next_remote = Instant::now();
            let mut last = Vec::new();
            if let Some(f) = &pid_file { let _ = fs::write(f, format!("{} {}\n", std::process::id(), dir.display())); if duty { let _ = fs::write(format!("{}.agents", f), "agents\n"); } if monitors { let _ = fs::write(format!("{}.monitors", f), "monitors\n"); } if remote_duty { let _ = fs::write(format!("{}.remote", f), "remote\n"); } }
            #[cfg(target_os = "linux")]
            let w = watch::open(&[(dir, watch::TOPICS), (&shared, watch::APPEAR)]);
            let mut next_collect = Instant::now();
            loop {
                if let Some(p) = parent { if !alive(p) { if let Some(f) = &pid_file { let _ = fs::remove_file(f); let _ = fs::remove_file(format!("{}.agents", f)); let _ = fs::remove_file(format!("{}.monitors", f)); let _ = fs::remove_file(format!("{}.remote", f)); } return; } }
                let sig = signature(dir);
                if sig != last || !out.exists() { if build(dir, out, &token()).is_ok() { last = sig; } }
                if duty && Instant::now() >= next_collect {
                    let cfg = config.as_deref().map(config_read).unwrap_or_default();
                    let every: u64 = cfg.get("LOCAL_EVERY").and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(3);
                    let tests_busy = cfg.get("TESTS_BUSY").map_or(true, |v| v != "off");
                    agents::collect_local(&shared, &host, tests_busy);
                    next_collect = Instant::now() + Duration::from_secs(every);
                    if a.iter().any(|x| x == "--follow") {                       // the follow-ups of the collector, behind the fresh agent list
                        let mode = cfg.get("BHOTED_FOLLOW").map(|s| s.as_str()).unwrap_or("off");
                        if mode == "shadow" || mode == "on" { follow_round(&shared, &data, dir, &host, &cfg, mode == "shadow"); }
                    }
                }
                if remote_duty && Instant::now() >= next_remote && !remote_running.load(Ordering::SeqCst) {
                    let cfg = config.as_deref().map(config_read).unwrap_or_default();
                    if cfg.get("REMOTE_AGENTS").map_or(false, |v| v == "on") && cfg.get("AGENTS_VIA").map_or(false, |v| v == "kharka") {
                        let every: u64 = cfg.get("REMOTE_EVERY").and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(5);
                        next_remote = Instant::now() + Duration::from_secs(every);
                        let bin = std::env::var("KHARKA").ok().filter(|v| !v.is_empty()).or_else(|| cfg.get("KHARKA").cloned().filter(|v| !v.is_empty())).unwrap_or_else(|| "kharka".into());
                        let (sh, hs, flag) = (shared.clone(), host.clone(), remote_running.clone());
                        let (off, tb) = (cfg.get("MACHINES_OFF").cloned().unwrap_or_default(), cfg.get("TESTS_BUSY").map_or(true, |v| v != "off"));
                        flag.store(true, Ordering::SeqCst);
                        std::thread::spawn(move || { agents::collect_remote_kharka(&sh, &hs, &bin, &off, tb); flag.store(false, Ordering::SeqCst); });
                    } else { next_remote = Instant::now() + Duration::from_secs(2); }
                }
                if monitors {
                    let cfg = config.as_deref().map(config_read).unwrap_or_default();
                    for (i, (kind, on, every)) in [("deploys", "DEPLOY_MONITOR", "DEPLOY_EVERY"), ("tests", "TEST_MONITOR", "TEST_EVERY")].into_iter().enumerate() {
                        if cfg.get(on).map_or(true, |v| v != "on") || running[i].load(Ordering::SeqCst) || Instant::now() < next_fetch[i] { continue; }
                        let secs: u64 = cfg.get(every).and_then(|v| v.parse().ok()).unwrap_or(60);
                        next_fetch[i] = Instant::now() + Duration::from_secs(secs);
                        running[i].store(true, Ordering::SeqCst);
                        let (flag, ctx) = (running[i].clone(), monitor::Ctx { shared: shared.clone(), data: data.clone(), topics: dir.to_path_buf(), cfg: cfg.clone() });
                        std::thread::spawn(move || { monitor::fetch(&ctx, kind); flag.store(false, Ordering::SeqCst); });
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if let Some(w) = &w {
                        let ms = if duty { next_collect.saturating_duration_since(Instant::now()).as_millis().min(1000) as i32 } else { 1000 };
                        let events = w.wait(ms);
                        if events.iter().any(|(i, n)| *i == 1 && n == "poke") { next_collect = Instant::now(); }   // a poke: look at once
                        if events.iter().any(|(i, n)| *i == 1 && n == "remote.due") { next_remote = Instant::now(); }   // kharka's watch: another machine's agents moved
                        if events.iter().any(|(i, _)| *i == 0) { std::thread::sleep(Duration::from_millis(15)); let _ = w.wait(0); } // a burst of writes: one rebuild
                        continue;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        _ => { eprintln!("usage: bhoted index|run <topic dir> <out>"); std::process::exit(2); }
    }
}
