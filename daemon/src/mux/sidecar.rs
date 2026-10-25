//! The kharka sidecar: with STORE=kharka or AGENTS_VIA=kharka the mux runs `herdr-kharka` against its own socket, so that the other machines
//! see this one's workspaces and agents (`herdr/<node>` in kharka) and can focus and prompt them (`herdr.commands`). It is started with the
//! server, started again when it ends, and ended with the server. Settings: KHARKA_SIDECAR (auto, off), KHARKA_LABEL (the machine's name).
use super::server::Server;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn find_bin(cfg: &std::collections::HashMap<String, String>) -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Some(b) = std::env::var_os("HERDR_KHARKA").filter(|b| !b.is_empty()) { c.push(PathBuf::from(b)); }
    if let Some(p) = std::env::var_os("PATH") { for d in std::env::split_paths(&p) { c.push(d.join("herdr-kharka")); } }
    if let Some(k) = cfg.get("KHARKA").filter(|k| !k.is_empty()) { if let Some(d) = std::path::Path::new(k).parent() { c.push(d.join("herdr-kharka")); } }
    if let Some(h) = std::env::var_os("HOME") { c.push(PathBuf::from(h).join(".cargo/bin/herdr-kharka")); }
    c.into_iter().find(|p| p.is_file())
}

pub fn wanted(cfg: &std::collections::HashMap<String, String>) -> bool {
    if std::env::var_os("BHOTE_NO_SIDECAR").is_some() { return false; }
    match cfg.get("KHARKA_SIDECAR").map(|s| s.as_str()) {
        Some("off") => false,
        _ => cfg.get("STORE").map_or(false, |v| v == "kharka") || cfg.get("AGENTS_VIA").map_or(false, |v| v == "kharka"),
    }
}

fn pid_file() -> PathBuf { super::proto::sock_path().with_file_name("kharka.pid") }

/// A sidecar left by a server that was killed hard is ended before a new one starts.
fn kill_stale() {
    let Ok(t) = std::fs::read_to_string(pid_file()) else { return };
    let Ok(pid) = t.trim().parse::<i32>() else { return };
    let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).ok()
        .or_else(|| Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned())).unwrap_or_default();
    if cmd.contains("herdr-kharka") { unsafe { libc::kill(pid, libc::SIGTERM); } }
    let _ = std::fs::remove_file(pid_file());
}

impl Server {
    pub fn sidecar_tick(&mut self) {
        let now = Instant::now();
        if let Some(c) = self.sidecar.as_mut() { if matches!(c.try_wait(), Ok(Some(_))) { self.sidecar = None; } }
        if self.sidecar.is_some() || self.sidecar_at.map_or(false, |t| now.duration_since(t) < Duration::from_secs(5)) { return; }
        self.sidecar_at = Some(now);
        let cfg = crate::config_read(&super::panels::config_path());
        if !wanted(&cfg) { return; }
        let Some(bin) = find_bin(&cfg) else { return };
        if self.sidecar_first { self.sidecar_first = false; kill_stale(); }
        let sock = super::proto::sock_path();
        let mut c = Command::new(bin);
        c.args(["--commands", "--allow-method", "agent.prompt", "--herdr-socket"]).arg(&sock);
        if let Some(l) = cfg.get("KHARKA_LABEL").filter(|l| !l.is_empty()) { c.args(["--label", l]); }
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(k) = cfg.get("KHARKA").filter(|k| !k.is_empty()) { c.env("PATH", format!("{}:{}", std::path::Path::new(k).parent().map(|p| p.display().to_string()).unwrap_or_default(), std::env::var("PATH").unwrap_or_default())); }
        if let Ok(ch) = c.spawn() { let _ = std::fs::write(pid_file(), ch.id().to_string()); self.sidecar = Some(ch); }
    }

    pub fn sidecar_stop(&mut self) {
        if let Some(mut c) = self.sidecar.take() { unsafe { libc::kill(c.id() as i32, libc::SIGTERM); } let _ = c.wait(); let _ = std::fs::remove_file(pid_file()); }
    }
}

#[allow(dead_code)]
type _C = Child;
