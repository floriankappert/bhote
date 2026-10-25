//! `bhoted chat`: a pane with an input field and the history of an agent. Enter sends the text to the chosen agent (`herdr agent prompt`),
//! the last turns come from the agent's transcript (Claude Code's JSONL), else from what `herdr agent read` shows. `@` picks the agent.
//! It talks to herdr or the native mux through the `herdr` command (the mux puts a link of that name in front of the PATH of its panes).
use crate::mux::input::{Input, Key, Parser};
use crate::ui::dw;
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Debug, PartialEq)]
pub enum Role { User, Assistant, Tool }
#[derive(Clone, Debug, PartialEq)]
pub struct Msg { pub role: Role, pub text: String }

#[derive(Clone, Debug, Default)]
pub struct AgentRow { pub pane: String, pub agent: String, pub status: String, pub ws: String, pub tab: String, pub session: String }

// ───────────────────────────── transcript

fn tool_hint(input: &Value) -> String {
    for k in ["command", "file_path", "pattern", "path", "url", "query", "description"] {
        if let Some(s) = input.get(k).and_then(|v| v.as_str()) { return s.lines().next().unwrap_or("").chars().take(80).collect(); }
    }
    String::new()
}

/// The turns of a Claude Code transcript (JSONL): what you typed and what the agent said, tool calls as one line each.
pub fn parse_transcript(text: &str) -> Vec<Msg> {
    let mut out: Vec<Msg> = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v.get("isSidechain").and_then(|s| s.as_bool()) == Some(true) || v.get("isMeta").and_then(|s| s.as_bool()) == Some(true) { continue; }
        let role = match v.get("type").and_then(|t| t.as_str()) { Some("user") => Role::User, Some("assistant") => Role::Assistant, _ => continue };
        let Some(content) = v.pointer("/message/content") else { continue };
        let mut push = |r: Role, t: &str| { let t = t.trim(); if !t.is_empty() { out.push(Msg { role: r, text: t.to_string() }); } };
        match content {
            Value::String(s) => { if !s.starts_with('<') { push(role.clone(), s); } }
            Value::Array(items) => for it in items {
                match it.get("type").and_then(|t| t.as_str()) {
                    Some("text") => { let t = it.get("text").and_then(|t| t.as_str()).unwrap_or(""); if !t.starts_with('<') { push(role.clone(), t); } }
                    Some("tool_use") => { let n = it.get("name").and_then(|n| n.as_str()).unwrap_or("tool"); let h = tool_hint(it.get("input").unwrap_or(&Value::Null)); push(Role::Tool, &if h.is_empty() { n.to_string() } else { format!("{n} {h}") }); }
                    _ => {}
                }
            },
            _ => {}
        }
    }
    out
}

/// The transcript file of a session: `<claude dir>/projects/<project>/<session id>.jsonl`, in every Claude configuration folder of this machine.
pub fn find_transcript(session: &str) -> Option<PathBuf> {
    if session.is_empty() || !session.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { return None; }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR") { dirs.push(PathBuf::from(d)); }
    if let Ok(rd) = std::fs::read_dir(&home) { for e in rd.flatten() { if e.file_name().to_string_lossy().starts_with(".claude") { dirs.push(e.path()); } } }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d.join("projects")) else { continue };
        for p in rd.flatten() { let f = p.path().join(format!("{session}.jsonl")); if f.is_file() { return Some(f); } }
    }
    None
}

// ───────────────────────────── text layout

/// A text cut into lines of at most `w` columns, at spaces where there are some; line breaks stay.
pub fn wrap(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new(); let mut width = 0usize;
        for word in para.split(' ') {
            let ww = dw(word) as usize;
            if width > 0 && width + 1 + ww <= w { line.push(' '); line.push_str(word); width += 1 + ww; continue; }
            if width > 0 { out.push(std::mem::take(&mut line)); width = 0; }
            // a word longer than a line is cut
            let mut cur = String::new(); let mut cw = 0usize;
            for ch in word.chars() { let c = dw(&ch.to_string()) as usize; if cw + c > w { out.push(std::mem::take(&mut cur)); cw = 0; } cur.push(ch); cw += c; }
            line = cur; width = cw;
        }
        out.push(line);
    }
    out
}

// ───────────────────────────── the chat

#[derive(Default)]
struct Picker { filter: String, idx: usize }

pub struct Chat {
    pub agents: Vec<AgentRow>,
    pub sel: Option<String>,
    pub msgs: Vec<Msg>,
    pub input: Vec<char>, pub cur: usize,
    pub hist: Vec<String>, hist_i: Option<usize>, hist_draft: String,
    picker: Option<Picker>,
    pub scroll: usize,
    pub cols: usize, pub rows: usize,
    my_tab: String,
    flash: Option<(String, Instant)>,
    transcript: Option<(PathBuf, SystemTime)>,
    hist_file: Option<PathBuf>,
    /// what was sent and is not in the transcript yet (the agent writes it a moment later)
    pending: Vec<(String, Instant)>,
}

fn herdr(args: &[&str]) -> Option<String> {
    let o = crate::proc::run("herdr", args, 8, None).filter(|o| o.0 == 0)?;
    Some(String::from_utf8_lossy(&o.1).into_owned())
}

impl Chat {
    pub fn new(cols: usize, rows: usize) -> Chat {
        let hist_file = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/bhote/chat-history"));
        let hist = hist_file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| t.lines().map(|l| l.replace("\\n", "\n").replace("\\\\", "\\")).collect()).unwrap_or_default();
        Chat { agents: Vec::new(), sel: None, msgs: Vec::new(), input: Vec::new(), cur: 0, hist, hist_i: None, hist_draft: String::new(), picker: None, scroll: 0, cols, rows,
               my_tab: std::env::var("HERDR_TAB_ID").unwrap_or_default(), flash: None, transcript: None, hist_file, pending: Vec::new() }
    }

    fn flash(&mut self, t: &str) { self.flash = Some((t.to_string(), Instant::now() + Duration::from_secs(4))); }

    /// The agents of this machine (`pane list`), with the names of their workspaces.
    pub fn load_agents(&mut self) {
        let Some(panes) = herdr(&["pane", "list"]).and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return };
        let ws: std::collections::HashMap<String, String> = herdr(&["workspace", "list"]).and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.pointer("/result/workspaces").cloned()).and_then(|a| a.as_array().cloned()).unwrap_or_default().iter()
            .filter_map(|w| Some((w.get("workspace_id")?.as_str()?.to_string(), w.get("label")?.as_str()?.to_string()))).collect();
        let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        self.agents = panes.pointer("/result/panes").and_then(|a| a.as_array()).cloned().unwrap_or_default().iter().filter(|p| p.get("agent").is_some()).map(|p| AgentRow {
            pane: s(p, "pane_id"), agent: s(p, "agent"), status: s(p, "agent_status"), ws: ws.get(&s(p, "workspace_id")).cloned().unwrap_or_else(|| s(p, "workspace_id")), tab: s(p, "tab_id"),
            session: p.pointer("/agent_session/value").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        }).collect();
        if self.sel.as_ref().map_or(true, |s| !self.agents.iter().any(|a| &a.pane == s)) {
            // the agent of this tab, else the first one
            self.sel = self.agents.iter().find(|a| !self.my_tab.is_empty() && a.tab == self.my_tab).or_else(|| self.agents.first()).map(|a| a.pane.clone());
            self.transcript = None; self.msgs.clear();
        }
    }

    pub fn current(&self) -> Option<&AgentRow> { self.sel.as_ref().and_then(|s| self.agents.iter().find(|a| &a.pane == s)) }

    /// The history of the chosen agent: its transcript when it has one, else the text of its screen.
    pub fn load_history(&mut self) {
        let Some(a) = self.current().cloned() else { self.msgs.clear(); return };
        if self.transcript.is_none() { self.transcript = find_transcript(&a.session).map(|p| (p, SystemTime::UNIX_EPOCH)); }
        if let Some((p, seen)) = &mut self.transcript {
            if let Ok(m) = std::fs::metadata(&*p).and_then(|m| m.modified()) {
                if m != *seen {
                    *seen = m;
                    if let Ok(t) = std::fs::read_to_string(&*p) { let mut v = parse_transcript(&t); let n = v.len().saturating_sub(80); self.msgs = v.split_off(n); }
                }
                // sent text the transcript does not have yet stays at the end
                let tail: Vec<String> = self.msgs.iter().rev().take(12).filter(|m| m.role == Role::User).map(|m| m.text.clone()).collect();
                self.pending.retain(|(t, at)| at.elapsed() < Duration::from_secs(30) && !tail.contains(t));
                for (t, _) in &self.pending { self.msgs.push(Msg { role: Role::User, text: t.clone() }); }
                return;
            }
        }
        if let Some(t) = herdr(&["agent", "read", &a.pane, "--lines", "60"]) { self.msgs = vec![Msg { role: Role::Assistant, text: t.trim_end().to_string() }]; }
    }

    pub fn text(&self) -> String { self.input.iter().collect() }

    fn save_history(&self) {
        if let Some(f) = &self.hist_file {
            if let Some(d) = f.parent() { let _ = std::fs::create_dir_all(d); }
            let keep = self.hist.len().saturating_sub(200);
            let body: String = self.hist[keep..].iter().map(|h| format!("{}\n", h.replace('\\', "\\\\").replace('\n', "\\n"))).collect();
            let _ = std::fs::write(f, body);
        }
    }

    pub fn send(&mut self) {
        let text = self.text().trim().to_string();
        if text.is_empty() { return; }
        let Some(a) = self.current().cloned() else { self.flash("no agent chosen: type @"); return };
        if herdr(&["agent", "prompt", &a.pane, &text]).is_none() { self.flash("the agent did not take it"); return; }
        if self.hist.last() != Some(&text) { self.hist.push(text.clone()); self.save_history(); }
        self.input.clear(); self.cur = 0; self.hist_i = None; self.scroll = 0;
        self.pending.push((text.clone(), Instant::now()));
        self.msgs.push(Msg { role: Role::User, text });
        if let Some((_, seen)) = &mut self.transcript { *seen = SystemTime::UNIX_EPOCH; }
    }

    fn picker_rows(&self) -> Vec<&AgentRow> {
        let f = self.picker.as_ref().map(|p| p.filter.to_lowercase()).unwrap_or_default();
        self.agents.iter().filter(|a| f.is_empty() || format!("{} {} {}", a.ws, a.agent, a.pane).to_lowercase().contains(&f)).collect()
    }

    fn insert(&mut self, s: &str) { for ch in s.chars() { self.input.insert(self.cur, ch); self.cur += 1; } }

    pub fn key(&mut self, k: &Key) -> bool {
        // the agent picker takes the keys while it is open
        if let Some(p) = &mut self.picker {
            match k.name.as_str() {
                "esc" => { self.picker = None; }
                "up" => p.idx = p.idx.saturating_sub(1),
                "down" => p.idx += 1,
                "enter" | "tab" => {
                    let rows = self.picker_rows(); let i = self.picker.as_ref().map_or(0, |p| p.idx).min(rows.len().saturating_sub(1));
                    if let Some(a) = rows.get(i) { self.sel = Some(a.pane.clone()); self.transcript = None; self.msgs.clear(); self.scroll = 0; }
                    // the "@..." that opened the picker goes
                    let f = self.picker.as_ref().map_or(0, |p| p.filter.chars().count()) + 1;
                    for _ in 0..f { if self.cur > 0 { self.cur -= 1; self.input.remove(self.cur); } }
                    self.picker = None;
                }
                "backspace" => { if p.filter.pop().is_none() { self.picker = None; if self.cur > 0 { self.cur -= 1; self.input.remove(self.cur); } } else if self.cur > 0 { self.cur -= 1; self.input.remove(self.cur); } }
                _ => if let Some(ch) = std::str::from_utf8(&k.raw).ok().and_then(|t| t.chars().next()).filter(|c| !c.is_control() && k.raw.len() <= 4) { p.filter.push(ch); p.idx = 0; self.insert(&ch.to_string()); },
            }
            return true;
        }
        match k.name.as_str() {
            "enter" => self.send(),
            "alt+enter" | "ctrl+j" | "shift+enter" => self.insert("\n"),
            "backspace" => if self.cur > 0 { self.cur -= 1; self.input.remove(self.cur); },
            "delete" => if self.cur < self.input.len() { self.input.remove(self.cur); },
            "left" => self.cur = self.cur.saturating_sub(1),
            "right" => self.cur = (self.cur + 1).min(self.input.len()),
            "home" | "ctrl+a" => { while self.cur > 0 && self.input[self.cur - 1] != '\n' { self.cur -= 1; } }
            "end" | "ctrl+e" => { while self.cur < self.input.len() && self.input[self.cur] != '\n' { self.cur += 1; } }
            "ctrl+u" => { self.input.clear(); self.cur = 0; }
            "ctrl+w" => { while self.cur > 0 && self.input[self.cur - 1] == ' ' { self.cur -= 1; self.input.remove(self.cur); } while self.cur > 0 && self.input[self.cur - 1] != ' ' && self.input[self.cur - 1] != '\n' { self.cur -= 1; self.input.remove(self.cur); } }
            "up" => {
                // in the history of what was sent (only from the first line)
                if self.input[..self.cur].contains(&'\n') { while self.cur > 0 && self.input[self.cur - 1] != '\n' { self.cur -= 1; } if self.cur > 0 { self.cur -= 1; } }
                else if !self.hist.is_empty() {
                    let i = match self.hist_i { None => { self.hist_draft = self.text(); self.hist.len() - 1 } Some(i) => i.saturating_sub(1) };
                    self.hist_i = Some(i); self.input = self.hist[i].chars().collect(); self.cur = self.input.len();
                }
            }
            "down" => if let Some(i) = self.hist_i {
                if i + 1 < self.hist.len() { self.hist_i = Some(i + 1); self.input = self.hist[i + 1].chars().collect(); } else { self.hist_i = None; self.input = self.hist_draft.chars().collect(); }
                self.cur = self.input.len();
            },
            "pageup" => self.scroll += self.rows.saturating_sub(6).max(1),
            "pagedown" => self.scroll = self.scroll.saturating_sub(self.rows.saturating_sub(6).max(1)),
            "ctrl+c" | "ctrl+d" if self.input.is_empty() => return false,
            _ => if let Some(ch) = std::str::from_utf8(&k.raw).ok().and_then(|t| { let mut c = t.chars(); let f = c.next()?; if c.next().is_none() && !f.is_control() { Some(f) } else { None } }) {
                if ch == '@' && (self.cur == 0 || self.input[self.cur - 1] == ' ' || self.input[self.cur - 1] == '\n') { self.picker = Some(Picker::default()); }
                self.insert(&ch.to_string());
            },
        }
        true
    }

    pub fn handle(&mut self, ev: Input) -> bool {
        match ev {
            Input::Key(k) => return self.key(&k),
            Input::Paste(p) => { let t = String::from_utf8_lossy(&p).replace('\r', "\n"); self.insert(&t); }
            Input::Mouse(m) => { if m.b & 64 != 0 && m.press { if m.b & 1 == 0 { self.scroll += 3 } else { self.scroll = self.scroll.saturating_sub(3) } } }
            Input::Focus(_) => {}
        }
        true
    }

    /// The lines of the input box (wrapped) and the cursor in them.
    fn input_lines(&self, w: usize) -> (Vec<String>, (usize, usize)) {
        let before: String = self.input[..self.cur].iter().collect();
        let all = wrap(&self.text(), w);
        let pre = wrap(&before, w);
        let cy = pre.len().saturating_sub(1);
        let cx = pre.last().map_or(0, |l| dw(l) as usize);
        (if all.is_empty() { vec![String::new()] } else { all }, (cx, cy))
    }

    /// The whole picture as the bytes for the terminal.
    pub fn render(&mut self) -> String {
        let (w, h) = (self.cols.max(10), self.rows.max(6));
        let (blue, mauve, teal, dim, text, warn) = ("\x1b[38;2;137;180;250m", "\x1b[38;2;203;166;247m", "\x1b[38;2;148;226;213m", "\x1b[38;2;108;112;134m", "\x1b[38;2;205;214;244m", "\x1b[38;2;250;179;135m");
        let mut rows: Vec<String> = Vec::new();
        // header: the agent
        let head = match self.current() {
            Some(a) => { let c = match a.status.as_str() { "working" => "\x1b[38;2;249;226;175m", "blocked" => "\x1b[38;2;243;139;168m", "done" => teal, _ => dim }; format!("{blue}◆{text} {} {dim}·{text} {} {c}{}", a.ws, a.agent, a.status) }
            None => format!("{blue}◆{dim} no agent  (type @ to pick one)"),
        };
        rows.push(head);
        rows.push(format!("{dim}{}", "┈".repeat(w)));
        // the input box
        let (ilines, (cx, cy)) = self.input_lines(w.saturating_sub(2));
        let box_h = ilines.len().clamp(1, 5);
        let first = (cy + 1).saturating_sub(box_h).min(ilines.len().saturating_sub(box_h));
        let hist_h = h.saturating_sub(2 + box_h + 2);
        // the history, newest at the bottom
        let mut lines: Vec<String> = Vec::new();
        for m in &self.msgs {
            let (mark, col, indent) = match m.role { Role::User => ("› ", mauve, "  "), Role::Assistant => ("● ", text, "  "), Role::Tool => ("⚙ ", dim, "  ") };
            let body = if m.role == Role::Tool { wrap(&m.text, w.saturating_sub(2)).into_iter().take(1).collect() } else { wrap(&m.text, w.saturating_sub(2)) };
            for (i, l) in body.iter().enumerate() { lines.push(format!("{col}{}{l}", if i == 0 { mark } else { indent })); }
            if m.role != Role::Tool { lines.push(String::new()); }
        }
        let max_scroll = lines.len().saturating_sub(hist_h);
        self.scroll = self.scroll.min(max_scroll);
        let end = lines.len() - self.scroll; let start = end.saturating_sub(hist_h);
        let mut shown: Vec<String> = lines[start..end].to_vec();
        while shown.len() < hist_h { shown.insert(0, String::new()); }
        rows.extend(shown);
        // the picker over the bottom of the history
        if self.picker.is_some() {
            let list: Vec<String> = self.picker_rows().iter().map(|a| format!("{} · {} · {} · {}", a.ws, a.agent, a.status, a.pane)).collect();
            let idx = self.picker.as_ref().map_or(0, |p| p.idx).min(list.len().saturating_sub(1));
            let n = list.len().min(hist_h.max(1));
            let top = rows.len() - n.max(1).min(rows.len() - 2);
            for (i, l) in list.iter().take(n).enumerate() {
                let r = top + i; if r < rows.len() && r >= 2 { rows[r] = format!("{}{}{}", if i == idx { "\x1b[7m" } else { "" }, text, crate::ui::clip(l, w as i64)); }
            }
            if list.is_empty() { let r = rows.len() - 1; rows[r] = format!("{warn}no agent matches"); }
        }
        rows.push(format!("{dim}{}", "─".repeat(w)));
        for i in 0..box_h { let l = ilines.get(first + i).cloned().unwrap_or_default(); rows.push(format!("{teal}{}{text}{}", if first + i == 0 { "› " } else { "  " }, l)); }
        let foot = match &self.flash { Some((t, until)) if *until > Instant::now() => format!("{warn}{t}"), _ => format!("{dim}enter send · alt+enter new line · @ agent · pgup/pgdn scroll") };
        rows.push(foot);
        let mut out = String::from("\x1b[?25l\x1b[H");
        for (i, r) in rows.iter().take(h).enumerate() { out.push_str(&format!("\x1b[{};1H\x1b[2K{}\x1b[0m", i + 1, crate::ui::clip(&strip_len(r, w), (w + 200) as i64))); }
        let cy_screen = 2 + hist_h + 1 + (cy.saturating_sub(first));
        out.push_str(&format!("\x1b[{};{}H\x1b[?25h", cy_screen + 1, 2 + cx + 1));
        out
    }
}

/// A row with colour codes, cut at `w` columns of text (the codes do not count).
fn strip_len(s: &str, w: usize) -> String {
    let mut out = String::new(); let mut width = 0usize; let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' { out.push(c); for n in chars.by_ref() { out.push(n); if n.is_ascii_alphabetic() { break; } } continue; }
        let cw = dw(&c.to_string()) as usize;
        if width + cw > w { break; }
        out.push(c); width += cw;
    }
    out
}

fn winsize() -> (usize, usize) { let (c, r) = crate::term::Term::size(); (c as usize, r as usize) }

fn wait_stdin(ms: i32) -> bool {
    let mut set = [0u8; 128]; set[0] = 1;
    let mut tv = libc::timeval { tv_sec: (ms / 1000) as _, tv_usec: ((ms % 1000) * 1000) as _ };
    unsafe { libc::select(1, set.as_mut_ptr() as *mut libc::fd_set, std::ptr::null_mut(), std::ptr::null_mut(), &mut tv) > 0 }
}

pub fn run() -> i32 {
    if unsafe { libc::isatty(0) } != 1 { eprintln!("bhoted chat needs a terminal"); return 1; }
    let raw = crate::mux::client::Raw::enter();
    let mut o = std::io::stdout();
    let _ = o.write_all(b"\x1b[?2004h\x1b[?1000h\x1b[?1006h"); let _ = o.flush();
    let (c, r) = winsize();
    let mut chat = Chat::new(c, r);
    let mut parser = Parser::default();
    let mut last = Instant::now() - Duration::from_secs(10);
    let mut dirty = true;
    let code = loop {
        if last.elapsed() >= Duration::from_millis(1200) {
            chat.load_agents(); chat.load_history(); last = Instant::now(); dirty = true;
        }
        let (c, r) = winsize();
        if (c, r) != (chat.cols, chat.rows) { chat.cols = c; chat.rows = r; dirty = true; }
        if dirty { let s = chat.render(); let _ = o.write_all(s.as_bytes()); let _ = o.flush(); dirty = false; }
        if wait_stdin(if parser.pending() { 25 } else { 300 }) {
            let mut buf = [0u8; 4096];
            let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut _, buf.len()) };
            if n <= 0 { break 0; }
            let mut quit = false;
            for ev in parser.feed(&buf[..n as usize]) { if !chat.handle(ev) { quit = true; } }
            if quit { break 0; }
            dirty = true;
        } else {
            for ev in parser.flush() { if !chat.handle(ev) { drop(raw); return 0; } dirty = true; }
        }
        if chat.flash.as_ref().map_or(false, |(_, t)| *t <= Instant::now()) { chat.flash = None; dirty = true; }
    };
    drop(raw);
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transcript_turns() {
        let t = r#"{"type":"user","message":{"role":"user","content":"fix the build"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Looking."},{"type":"tool_use","name":"Bash","input":{"command":"cargo build\nmore"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}
{"type":"user","isMeta":true,"message":{"role":"user","content":"meta"}}
{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"}}
{"type":"assistant","isSidechain":true,"message":{"role":"assistant","content":"side"}}
{"type":"summary","summary":"x"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done."}]}}"#;
        let m = parse_transcript(t);
        assert_eq!(m.iter().map(|m| (m.role.clone(), m.text.as_str())).collect::<Vec<_>>(), vec![(Role::User, "fix the build"), (Role::Assistant, "Looking."), (Role::Tool, "Bash cargo build"), (Role::Assistant, "Done.")]);
    }
    #[test]
    fn wrapping() {
        assert_eq!(wrap("hello world foo", 11), vec!["hello world", "foo"]);
        assert_eq!(wrap("a\n\nb", 5), vec!["a", "", "b"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("日本語日本語", 6), vec!["日本語", "日本語"]);
    }
    fn key(name: &str, raw: &str) -> Key { Key { name: name.into(), raw: raw.as_bytes().to_vec() } }
    #[test]
    fn editing_and_history() {
        let mut c = Chat::new(40, 20);
        c.hist = vec!["first".into(), "second".into()];
        for ch in "hi".chars() { c.key(&key(&ch.to_string(), &ch.to_string())); }
        assert_eq!(c.text(), "hi");
        c.key(&key("alt+enter", "\x1b\r")); c.key(&key("x", "x"));
        assert_eq!(c.text(), "hi\nx");
        c.key(&key("ctrl+u", "\x15"));
        assert_eq!(c.text(), "");
        c.key(&key("up", "")); assert_eq!(c.text(), "second");
        c.key(&key("up", "")); assert_eq!(c.text(), "first");
        c.key(&key("down", "")); c.key(&key("down", "")); assert_eq!(c.text(), "");
        c.input = "foo bar".chars().collect(); c.cur = 7;
        c.key(&key("ctrl+w", "\x17")); assert_eq!(c.text(), "foo ");
    }
    #[test]
    fn picker_selects_an_agent() {
        let mut c = Chat::new(40, 20);
        c.agents = vec![AgentRow { pane: "w1:p1".into(), agent: "claude".into(), ws: "api".into(), ..Default::default() }, AgentRow { pane: "w2:p1".into(), agent: "codex".into(), ws: "web".into(), ..Default::default() }];
        c.sel = Some("w1:p1".into());
        c.key(&key("@", "@"));
        assert!(c.picker.is_some());
        for ch in "web".chars() { c.key(&key(&ch.to_string(), &ch.to_string())); }
        c.key(&key("enter", "\r"));
        assert_eq!(c.sel.as_deref(), Some("w2:p1"));
        assert_eq!(c.text(), "", "the @web is taken out of the text");
        let out = c.render();
        assert!(out.contains("web") && out.contains("codex"));
    }
}
