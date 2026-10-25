//! Agents in panes: which agent runs in a pane (the process in front), what it does (a state it reported, else the screen),
//! "done" for work finished while nobody looked, and the event for bhote when the state changes.
use super::server::Server;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const KINDS: &[(&str, &[&str])] = &[("claude", &["claude"]), ("codex", &["codex"]), ("gemini", &["gemini"]), ("opencode", &["opencode"]), ("aider", &["aider"]), ("cursor-agent", &["cursor-agent"]), ("amp", &["amp"]), ("droid", &["droid"])];

/// The agent a command line is: the program or the script it runs is named like the agent (also `node .../claude-code/cli.js`).
pub fn agent_of_cmdline(cmdline: &str) -> Option<&'static str> {
    let words: Vec<&str> = cmdline.split_whitespace().take(3).collect();
    for w in &words {
        let base = w.rsplit('/').next().unwrap_or(w).to_ascii_lowercase();
        let path = w.to_ascii_lowercase();
        for (name, keys) in KINDS {
            for k in *keys {
                if base == *k || base.starts_with(&format!("{k}-")) && *name == "claude" { return Some(name); }
                if *name == "claude" && path.contains("claude-code") { return Some(name); }
                if *name == "codex" && path.contains("/codex/") { return Some(name); }
            }
        }
    }
    None
}

/// What the agent shows on the screen: blocked (it asks), working (it can be interrupted), else idle.
pub fn detect_status(text: &str) -> &'static str {
    let lines: Vec<String> = text.lines().rev().filter(|l| !l.trim().is_empty()).take(30).map(|l| l.trim().to_lowercase()).collect();
    const ASK: &[&str] = &["do you want to proceed", "do you want to make this edit", "do you want to create", "do you want to allow", "do you want to run", "allow once", "allow always", "yes, and don't ask again", "(y/n)", "[y/n]", "esc to cancel · tab to", "enter to confirm · esc to cancel", "would you like to run the following command", "yes, proceed"];
    for l in &lines {
        if ASK.iter().any(|a| l.contains(a)) || l.starts_with("❯ 1.") || l.starts_with("› 1.") || l.starts_with("> 1. yes") { return "blocked"; }
    }
    for l in &lines {
        if l.contains("esc to interrupt") || l.contains("ctrl+c to interrupt") || l.contains("esc to cancel,") || l.contains("esc interrupt") { return "working"; }
    }
    "idle"
}

impl Server {
    /// One round over all panes: process in front, state, "done", events. Called a few times a second.
    pub fn detect_agents(&mut self) {
        let shown: Option<u32> = if self.clients.is_empty() { None } else { self.world.focused_pane() };
        let now = Instant::now();
        let ids: Vec<u32> = self.panes.keys().copied().collect();
        let mut events: Vec<u32> = Vec::new();
        for id in ids {
            let Some(p) = self.panes.get_mut(&id) else { continue };
            // the process in front (about once a second, and when the screen changed after a pause)
            if p.chk.map_or(true, |t| now.duration_since(t) >= Duration::from_millis(1000)) {
                p.chk = Some(now);
                let (g, _) = p.foreground();
                let cmd = std::fs::read(format!("/proc/{g}/cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).ok()
                    .or_else(|| std::process::Command::new("ps").args(["-o", "command=", "-p", &g.to_string()]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned())).unwrap_or_default();
                let mut name = agent_of_cmdline(&cmd).map(String::from);
                // an agent that reported itself stays while something other than a shell is in front (its own helper process, a wrapper)
                let first = cmd.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").trim_start_matches('-');
                let shell = matches!(first, "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "nu" | "") && agent_of_cmdline(&cmd).is_none();
                if name.is_none() && p.reported.is_some() && !shell { name = p.agent.clone(); }
                if name != p.agent {
                    p.agent = name;
                    if p.agent.is_none() { p.status.clear(); p.base.clear(); p.reported = None; p.pend = None; p.session = None; }
                }
            }
            if p.agent.is_none() { continue; }
            let raw = match &p.reported { Some(r) => r.clone(), None => { let t = p.screen().contents(); detect_status(&t).to_string() } };
            if p.base.is_empty() { p.base = raw.clone(); }
            // leaving "working" waits a moment (the status line goes away between two steps of the agent)
            if raw != p.base {
                if p.base == "working" && p.reported.is_none() {
                    match &p.pend { Some((s, t)) if *s == raw => { if now.duration_since(*t) >= Duration::from_millis(1500) { p.base = raw.clone(); p.pend = None; } } _ => p.pend = Some((raw.clone(), now)) }
                } else { p.base = raw.clone(); p.pend = None; }
            } else { p.pend = None; }
            let was_working = p.status == "working" || p.status == "blocked";
            let seen = shown == Some(id);
            // work that ends while nobody looks at the pane stays "done" until the pane is shown
            let new = if p.base == "idle" && (p.status == "done" || was_working) && !seen { "done".to_string() } else { p.base.clone() };
            if new != p.status { p.status = new; p.rev += 1; }
            if p.status != p.evsent { p.evsent = p.status.clone(); events.push(id); }
        }
        for id in events { self.agent_event(id); self.dirty = true; }
    }

    /// bhote hears of a changed state (what the herdr plugin did on `pane.agent_status_changed`): `bhote event` with the event as JSON.
    fn agent_event(&self, id: u32) {
        let Some(p) = self.panes.get(&id) else { return };
        let Some(bin) = super::panels::bhote_bin() else { return };
        let ev = json!({"event": "pane.agent_status_changed", "data": {"pane": self.pane_json(p), "agent_status": p.agent_status()}});
        let mut c = std::process::Command::new(bin);
        c.arg("event").env("HERDR_PLUGIN_EVENT_JSON", ev.to_string()).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        for (k, v) in &self.env { c.env(k, v); }
        c.env("HERDR_ENV", "1");
        if let Ok(mut ch) = c.spawn() { std::thread::spawn(move || { let _ = ch.wait(); }); }
    }

    /// `pane.report_agent`: a hook says what the agent does; that counts until `pane.release_agent`.
    pub fn report_agent(&mut self, id: u32, p: &Value) {
        let Some(pane) = self.panes.get_mut(&id) else { return };
        if let Some(a) = p.get("agent").and_then(|a| a.as_str()) { pane.agent = Some(a.to_string()); }
        match p.get("state").and_then(|s| s.as_str()) { Some("unknown") | None => {} Some(s) => pane.reported = Some(s.to_string()) }
        if let Some(sess) = p.get("agent_session_id").and_then(|s| s.as_str()) { pane.session = Some(json!({"agent": pane.agent, "kind": "id", "source": p.get("source"), "value": sess})); }
        if let Some(r) = p.get("resume").and_then(|r| r.as_array()) { pane.resume = r.iter().filter_map(|x| x.as_str().map(String::from)).collect(); }
        pane.chk = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds() {
        assert_eq!(agent_of_cmdline("claude --resume abc"), Some("claude"));
        assert_eq!(agent_of_cmdline("/home/u/.local/bin/claude"), Some("claude"));
        assert_eq!(agent_of_cmdline("node /usr/lib/node_modules/@anthropic-ai/claude-code/cli.js"), Some("claude"));
        assert_eq!(agent_of_cmdline("/bin/sh /tmp/x/claude"), Some("claude"));
        assert_eq!(agent_of_cmdline("codex exec"), Some("codex"));
        assert_eq!(agent_of_cmdline("-bash"), None);
        assert_eq!(agent_of_cmdline("vim claude.md"), None);
        assert_eq!(agent_of_cmdline("/usr/bin/sleep 600"), None);
    }
    #[test]
    fn screens() {
        assert_eq!(detect_status("✻ Thinking… (12s · ↑ 1.2k tokens · esc to interrupt)\n\n╭─────╮\n│ > │"), "working");
        assert_eq!(detect_status("Edit file\n Do you want to make this edit to a.rs?\n ❯ 1. Yes\n   2. No\n Esc to cancel · Tab to amend"), "blocked");
        assert_eq!(detect_status("done.\n\n╭─────╮\n│ >   │\n╰─────╯\n  ? for shortcuts"), "idle");
        assert_eq!(detect_status(""), "idle");
    }
}
