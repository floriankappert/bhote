//! The panel in Rust (step 6): the terminal, the keys and the mouse, the main view, and the actions on it. The views that are not ported yet
//! (the topic's menu, settings, the wizard, the choosers) run as the script's own view in a child on the same terminal, then this takes over again.
use crate::agentsui;
use crate::cards::{Cards, Layout, View};
use crate::store::{self, Env, Live, Sink};
use crate::term::{Ev, Term};
use crate::theme;
use crate::ui::{self, Colors, Ctx};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const US: char = '\u{1f}';
const SPIN_FRAMES: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
const MIN_COLS: u32 = 30; const MIN_ROWS: u32 = 20;
const FEATURE_LEVEL: u64 = 4;

fn envs(k: &str) -> String { std::env::var(k).unwrap_or_default() }

#[derive(Clone, PartialEq)]
enum Screen { Main, Menu(i64), Pick(String, i64) }

struct Pending { child: Child, out: Option<PathBuf>, kind: &'static str }

pub struct Panel {
    host: String, script: PathBuf, shared: PathBuf, topics: PathBuf, data: PathBuf, cfg_path: PathBuf, version: String, vlabel: String, has_machines: bool,
    pane_id: String, tab_id: String,
    cfg: HashMap<String, String>, cfg_sig: String, theme: theme::Theme, color: bool,
    cols: u32, rows: u32, real_cols: u32, real_rows: u32,
    sel_id: String, hover_id: String, hover_btn: bool, expanded: String,
    flash: Option<(String, Instant)>, pane_focus: Option<bool>, tab_visible: bool, dirty: bool,
    tmap: Vec<(i64, String)>, divrow: i64, agmap: Vec<String>, spin_any: bool, prev_toff: Option<i64>,
    click_last: Option<(String, Instant)>, jump_at: Option<(String, Instant)>, spin_tick: usize, last_tick: u64, files_sig: String, last_chk: u64, select_seen: Option<String>,
    pending: Vec<Pending>, collector: Option<Child>, redraw: bool, clamp_at: Option<Instant>, clamp_adopt: bool, shown_at: Option<Instant>,
    screen: Screen, pmap: Vec<(i64, String)>, menu_flash: Option<(String, Instant)>, collector_at: Instant, collector_tries: u32,
}

fn mtime(p: &std::path::Path) -> u128 { fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos()) }

impl Panel {
    fn new() -> Panel {
        let shared = PathBuf::from(envs("BHOTE_SHARED")); let data = PathBuf::from(envs("BHOTE_DATA"));
        let (cols, rows) = Term::size();
        Panel { host: envs("BHOTE_HOST"), script: PathBuf::from(envs("BHOTE_SCRIPT")), topics: data.join("topics"), shared, data, cfg_path: PathBuf::from(envs("BHOTE_CONFIG")), version: envs("BHOTE_VER"), vlabel: envs("BHOTE_VLABEL"),
            has_machines: envs("BHOTE_HAS_MACHINES") == "1", pane_id: envs("HERDR_PANE_ID"), tab_id: envs("HERDR_TAB_ID"),
            cfg: HashMap::new(), cfg_sig: String::new(), theme: theme::apply(&HashMap::new(), true, ""), color: std::env::var("NO_COLOR").map_or(true, |v| v.is_empty()),
            cols: cols.clamp(MIN_COLS, 64), rows, real_cols: cols, real_rows: rows, sel_id: String::new(), hover_id: String::new(), hover_btn: false, expanded: " ".into(),
            flash: None, pane_focus: None, tab_visible: true, dirty: false, tmap: Vec::new(), divrow: 0, agmap: Vec::new(), spin_any: false, prev_toff: None, click_last: None, jump_at: None, spin_tick: 0, last_tick: 0, files_sig: String::new(), last_chk: 0, select_seen: None,
            pending: Vec::new(), collector: None, redraw: true, clamp_at: Some(Instant::now() + Duration::from_secs(2)), clamp_adopt: true, shown_at: None, screen: Screen::Main, pmap: Vec::new(), menu_flash: None, collector_at: Instant::now(), collector_tries: 0 }
    }
    fn reload_cfg(&mut self) {
        self.cfg = crate::config_read(&self.cfg_path.to_string_lossy());
        let sig: String = ["THEME", "COLOR_TEXT", "COLOR_DIM", "COLOR_FRAME", "COLOR_SELECTION", "COLOR_ACCENT", "COLOR_NOW", "COLOR_WAITING", "COLOR_REVIEW", "COLOR_LATER", "COLOR_DONE", "COLOR_ERROR"].iter().map(|k| self.cfg.get(*k).cloned().unwrap_or_default()).collect::<Vec<_>>().join("|");
        if sig != self.cfg_sig || self.theme.vars.is_empty() { self.cfg_sig = sig; self.theme = theme::apply(&self.cfg, self.color, &format!("Bhote v{}", self.vlabel)); }
        let max: u32 = self.cfg.get("PANEL_MAX_WIDTH").and_then(|v| v.parse().ok()).unwrap_or(64);
        self.cols = self.real_cols.min(max).max(MIN_COLS);
    }
    fn c(&self, k: &str, d: &str) -> String { self.cfg.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string()) }
    fn num(&self, k: &str, d: i64) -> i64 { self.c(k, "").parse().unwrap_or(d) }
    fn env(&self) -> Env { Env { topics: self.topics.clone(), shared: self.shared.clone(), data: self.data.clone() } }

    // ── cfg_set: one writer at a time, a temp file moved into place ─────────────────────────────────────────────
    fn cfg_set(&mut self, key: &str, val: &str) {
        let lock = PathBuf::from(format!("{}.lock", self.cfg_path.display()));
        for _ in 0..40 { if std::os::unix::fs::symlink(std::process::id().to_string(), &lock).is_ok() { break; } std::thread::sleep(Duration::from_millis(50)); }
        let old = crate::proc::read_cap(&self.cfg_path).unwrap_or_default();
        let mut out: String = old.lines().filter(|l| l.split('=').next() != Some(key)).map(|l| format!("{}\n", l)).collect();
        if !val.is_empty() { out.push_str(&format!("{}={}\n", key, val)); }
        let tmp = PathBuf::from(format!("{}.tmp.{}", self.cfg_path.display(), std::process::id()));
        if fs::write(&tmp, out).is_ok() { use std::os::unix::fs::PermissionsExt; let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)); let _ = fs::rename(&tmp, &self.cfg_path); }   // (the settings hold a token: 0600)
        let _ = fs::remove_file(&lock);
        self.reload_cfg();
    }

    // ── drawing ───────────────────────────────────────────────────────────────────────────────────────────────
    fn herdr_machine_fg(&self) -> String {
        if !self.color { return "none".into(); }
        let p = envs("HERDR_CONFIG"); let path = if p.is_empty() { format!("{}/.config/herdr/config.toml", envs("HOME")) } else { p };
        let mut hx = "ff3ea5".to_string();
        for l in crate::proc::read_cap(path).unwrap_or_default().lines() {
            if l.contains("token = \"machine\"") { if let Some(i) = l.find("fg = \"#") { let h: String = l[i + 7..].chars().take(6).collect(); if h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()) { hx = h; } } break; }
        }
        let p = |i: usize| u8::from_str_radix(&hx[i..i + 2], 16).unwrap_or(0);
        format!("\u{1b}[38;2;{};{};{}m", p(0), p(2), p(4))
    }
    fn update_note(&self) -> String {
        if self.c("UPDATE_CHECK", "on") != "on" { return String::new(); }
        let txt = crate::proc::read_cap(self.data.join("update")).unwrap_or_default();
        let latest = txt.lines().filter_map(|l| l.strip_prefix("latest=")).last().unwrap_or("").to_string();
        if latest.is_empty() || !latest.bytes().all(|b| b.is_ascii_digit() || b == b'.') { return String::new(); }
        let parse = |s: &str| -> u64 { let mut it = s.split('.').map(|x| x.parse::<u64>().unwrap_or(0)); it.next().unwrap_or(0) * 1_000_000 + it.next().unwrap_or(0) * 1000 + it.next().unwrap_or(0) };
        if parse(&latest) > parse(&self.version) { format!("⬆ Update available v{} · , settings", latest) } else { String::new() }
    }
    fn current_project(&self) -> String {
        if self.pane_id.is_empty() { return String::new(); }
        let t = crate::proc::read_cap(self.shared.join("agents.meta.local")).unwrap_or_default();
        let rows: Vec<Vec<&str>> = t.lines().map(|l| l.split(US).collect()).collect();
        let tab = rows.iter().find(|r| r.get(1) == Some(&self.pane_id.as_str())).and_then(|r| r.get(2).copied()).unwrap_or("");
        if tab.is_empty() { return String::new(); }
        for r in &rows { if r.get(2) == Some(&tab) && r.get(3) == Some(&"a") && r.get(5).map_or(false, |j| !j.is_empty()) { return r[5].to_string(); } }
        String::new()
    }
    fn view(&self, mcodes: Vec<(String, String)>) -> View {
        let note = self.update_note();
        let newfeat = !self.c("SETUP_DONE", "").is_empty() && (self.c("WIZARD_LEVEL", "1").parse::<u64>().unwrap_or(1)) < FEATURE_LEVEL;
        let head_rows = (!note.is_empty()) as i64 + newfeat as i64;
        View { sel_id: self.sel_id.clone(), hover_id: self.hover_id.clone(), expanded: self.expanded.clone(), scope_project: self.c("TOPICS_SCOPE", "all") == "project", cur_proj: self.current_project(),
            spin_ch: SPIN_FRAMES[self.spin_tick % SPIN_FRAMES.len()].to_string(), bored_small: false, archive_days: self.c("ARCHIVE_DAYS", "7").parse().unwrap_or(7), github: self.c("GITHUB_BRANCHES", "on") != "off",
            host: self.host.clone(), mach_fg: self.herdr_machine_fg(), line_max: 10, done_max: self.num("DONE_MAX", 4), mcodes,
            rows: self.rows as i64, head_rows, hover_btn: self.hover_btn, flash: self.flash.as_ref().filter(|(_, u)| Instant::now() < *u).map(|(m, _)| m.clone()).unwrap_or_default(),
            remote_on: self.c("REMOTE_AGENTS", "off") == "on", has_machines: self.has_machines, agent_rows: self.c("AGENT_ROWS", "").parse().ok(),
            pane_focus: self.pane_focus, head_note: note, newfeat, version_label: self.vlabel.clone(), head_grad: self.theme.head_grad.clone() }
    }
    /// One frame: the screen text; the maps for the mouse are kept.
    fn frame(&mut self) -> String {
        if self.screen != Screen::Main { if let Some(s) = self.other_frame() { return s; } self.screen = Screen::Main; }
        if self.real_cols < MIN_COLS || self.real_rows < MIN_ROWS { return format!("\u{1b}[H\u{1b}[J{}", ui::clip(&format!("Too small (min {}x{})", MIN_COLS, MIN_ROWS), self.real_cols as i64 - 1)); }
        let ctx = Ctx { cols: self.cols as i64, now: store::now(), shared: self.shared.clone(), topics: self.topics.clone(), cfg: self.cfg.clone(), c: Colors::from_map(self.theme.vars.clone()) };
        let agents: Vec<String> = { let mut m = Vec::new(); for f in ["agents.local", "agents.remote"] { for l in crate::proc::read_cap(self.shared.join(f)).unwrap_or_default().lines() { if let Some(x) = l.split(US).next() { if !x.is_empty() { m.push(x.to_string()); } } } } m };
        let mc = agentsui::machine_codes(&self.topics, &self.host, &agents);
        let view = self.view(mc.clone());
        let mut cards = Cards::new(&ctx, &view); cards.rows_total = self.rows as i64;
        let av = agentsui::agent_view(&view, &ctx.cfg, &ctx.shared);
        let b = agentsui::bottom(&cards, &av, &mc);
        let head = agentsui::head(&ctx, &view.head_view());
        let lay = Layout { cards: &cards, rows: self.rows as i64, head, bottom: b.text, run_dir: PathBuf::new(), prev_toff: self.prev_toff };
        let o = lay.view_out();
        self.prev_toff = o.toff;
        self.tmap = o.tmap.lines().filter_map(|l| { let mut p = l.splitn(2, '\u{1e}'); Some((p.next()?.parse().ok()?, p.next()?.to_string())) }).collect();
        self.divrow = o.divrow; self.agmap = b.agmap.lines().map(|s| s.to_string()).collect();
        self.spin_any = !o.spin.trim().is_empty() || self.agmap.iter().skip(1).any(|l| { l.split('\u{1e}').skip(1).any(|a| { let st = a.rsplit(US).next().unwrap_or(""); matches!(st, "working" | "testing") }) });
        // the screen: the line ends cleared, one write
        let mut out = String::from("\u{1b}[H");
        for (i, l) in o.frame.split('\n').take(self.rows as usize).enumerate() { if i > 0 { out.push('\n'); } out.push_str(l); out.push_str("\u{1b}[K"); }
        out.push_str("\u{1b}[J");
        out
    }
    fn ctx(&self) -> Ctx { Ctx { cols: self.cols as i64, now: store::now(), shared: self.shared.clone(), topics: self.topics.clone(), cfg: self.cfg.clone(), c: Colors::from_map(self.theme.vars.clone()) } }
    fn project_name(&self, id: &str) -> Option<String> {
        let t = crate::proc::read_cap(self.topics.join(format!("{}.topic", id))).ok()?; let (mut name, mut del, mut kd) = (String::new(), "0".to_string(), String::new());
        for l in t.lines() { if let Some(v) = l.strip_prefix("title=") { if name.is_empty() { name = v.to_string(); } } else if let Some(v) = l.strip_prefix("deleted=") { del = v.to_string(); } else if let Some(v) = l.strip_prefix("kind=") { kd = v.to_string(); } }
        if kd == "project" && del != "1" { Some(name) } else { None }
    }
    fn agent_rows_all(&self) -> Vec<crate::cards::Row> {
        ["agents.local", "agents.remote"].iter().flat_map(|f| crate::proc::read_cap(self.shared.join(f)).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(|l| { let mut p = l.split(US).map(|x| x.to_string()); crate::cards::Row { m: p.next().unwrap_or_default(), id: p.next().unwrap_or_default(), st: p.next().unwrap_or_default(), name: p.next().unwrap_or_default(), task: p.next().unwrap_or_default() } }).collect::<Vec<_>>()).collect()
    }
    fn menu_list(&self, file: &std::path::Path) -> Vec<(String, String)> { let ctx = self.ctx(); let ps = ui::projects(&ctx); crate::menuui::menu_items(file, &ps, &|id| self.project_name(id)) }
    /// The topic's page or the chooser: the text with its markers made into a screen; the map of the clickable rows is kept
    fn other_frame(&mut self) -> Option<String> {
        if self.real_cols < MIN_COLS || self.real_rows < MIN_ROWS { return Some(format!("\u{1b}[H\u{1b}[J{}", ui::clip(&format!("Too small (min {}x{})", MIN_COLS, MIN_ROWS), self.real_cols as i64 - 1))); }
        let ctx = self.ctx();
        let text = match self.screen.clone() {
            Screen::Menu(idx) => {
                let (_, f, ..) = self.sel_topic()?;
                let items = self.menu_list(&f);
                let flash = self.menu_flash.as_ref().filter(|(_, u)| Instant::now() < *u).map(|(m, _)| m.clone()).unwrap_or_default();
                crate::menuui::render_menu(&crate::menuui::MenuIn { c: &ctx, file: &f, idx, items: &items, project_name: &|id| self.project_name(id), github: self.c("GITHUB_BRANCHES", "on") != "off", flash: &flash, now: ctx.now })
            }
            Screen::Pick(mode, idx) => {
                let (_, f, _, _, title) = self.sel_topic()?;
                let rows = self.agent_rows_all(); let ts = store::load_fast(&self.topics, &self.shared);
                let src = (store::topic_get(&f, "agent_machine"), store::topic_get(&f, "agent_pane"));
                let map = crate::menuui::pick_map(&rows, &ts, &mode, if mode == "transfer" { Some((src.0.as_str(), src.1.as_str())) } else { None });
                crate::menuui::render_pick(&ctx, &title, &mode, &map, idx)
            }
            Screen::Main => return None,
        };
        let (s, map) = crate::menuui::paint_marked(&text, self.rows as usize);
        self.pmap = map; self.spin_any = false;
        Some(s)
    }
    fn paint(&mut self) {
        let s = self.frame();
        let mut o = std::io::stdout().lock(); let _ = o.write_all(s.as_bytes()); let _ = o.flush();
        self.redraw = false; self.dirty = false;
        self.files_sig = self.sig();
    }
    fn sig(&self) -> String {
        ["topics.idx", "agents.rev", "deploys.at", "tests.at"].iter().map(|f| mtime(&self.shared.join(f)).to_string()).chain([mtime(&self.data.join("store.status")).to_string(), mtime(&self.cfg_path).to_string()]).collect::<Vec<_>>().join(",")
    }

    // ── what is selected ──────────────────────────────────────────────────────────────────────────────────────
    fn with_cards<R>(&self, f: impl FnOnce(&Cards) -> R) -> R {
        let ctx = Ctx { cols: self.cols as i64, now: store::now(), shared: self.shared.clone(), topics: self.topics.clone(), cfg: self.cfg.clone(), c: Colors::from_map(self.theme.vars.clone()) };
        let view = self.view(Vec::new()); let cards = Cards::new(&ctx, &view); f(&cards)
    }
    /// The selected topic: (id, file, status, agent pane, title), None for a "+N more" row or an empty list
    fn sel_topic(&self) -> Option<(String, PathBuf, String, String, String)> {
        if self.sel_id.starts_with("more:") { return None; }
        self.with_cards(|c| { let i = c.sel_index(); if i < 0 { return None; } let t = c.ts().get(i as usize)?; Some((t.id.clone(), t.file.clone(), t.status.clone(), t.ap.clone(), t.title.clone())) })
    }
    /// sel_leave: the selection goes to the next one of the section the selected topic leaves
    fn sel_leave(&mut self) {
        let (list, groups): (Vec<String>, HashMap<String, String>) = self.with_cards(|c| { let p = c.plan(); let g = c.ts().iter().map(|t| (t.id.clone(), t.status.clone())).collect(); (p.sel_list, g) });
        let id = if self.sel_id.is_empty() { match self.sel_topic() { Some(t) => t.0, None => return } } else { self.sel_id.clone() };
        let Some(cur) = list.iter().position(|x| *x == id) else { return };
        let grp = |x: &str| -> String { if let Some(s) = x.strip_prefix("more:") { s.to_string() } else { groups.get(x).cloned().unwrap_or_default() } };
        let g = grp(&id);
        let n = list.len();
        let mut nx = String::new(); let mut pv = String::new();
        if cur + 1 < n && grp(&list[cur + 1]) == g { nx = list[cur + 1].clone(); }
        if cur > 0 && grp(&list[cur - 1]) == g { pv = list[cur - 1].clone(); }
        if nx.is_empty() || nx.starts_with("more:") { if !pv.is_empty() && !pv.starts_with("more:") { nx = pv.clone(); } }
        if nx.is_empty() { nx = pv.clone(); }
        if nx.is_empty() { if cur + 1 < n { nx = list[cur + 1].clone(); } else if cur > 0 { nx = list[cur - 1].clone(); } }
        self.sel_id = nx;
    }
    fn toggle_expand(&mut self, st: &str) { let k = format!(" {} ", st); if self.expanded.contains(&k) { self.expanded = self.expanded.replacen(&format!(" {} ", st), " ", 1); } else { self.expanded.push_str(st); self.expanded.push(' '); } }

    // ── things the script does for us (herdr, kharka, the agents) ────────────────────────────────────────────
    fn act(&mut self, args: &[&str], kind: &'static str, capture: bool) {
        let out = if capture { Some(self.shared.join(format!("act.{}.{}", std::process::id(), self.pending.len()))) } else { None };
        let mut c = Command::new("bash"); c.arg(&self.script).arg("__act").args(args).stdin(Stdio::null()).stderr(Stdio::null());
        match &out { Some(p) => { if let Ok(f) = fs::File::create(p) { c.stdout(f); } else { c.stdout(Stdio::null()); } } None => { c.stdout(Stdio::null()); } }
        if let Ok(child) = c.spawn() { self.pending.push(Pending { child, out, kind }); }
    }
    fn poll_pending(&mut self) {
        let mut done = Vec::new();
        for (i, p) in self.pending.iter_mut().enumerate() { if let Ok(Some(_)) = p.child.try_wait() { done.push(i); } }
        for i in done.into_iter().rev() {
            let p = self.pending.remove(i);
            if let Some(f) = &p.out { let t = crate::proc::read_cap(f).unwrap_or_default(); let _ = fs::remove_file(f); if p.kind == "menuflash" && !t.trim().is_empty() { self.menu_flash = Some((store::nocntrl(t.trim()), Instant::now() + Duration::from_secs(8))); }
            if p.kind == "flash" && !t.trim().is_empty() { self.flash = Some((store::nocntrl(t.trim()), Instant::now() + Duration::from_secs(10))); } }
            self.redraw = true;
        }
    }
    fn agent_jump(&mut self, m: &str, p: &str) {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || ":_-".contains(c)) { return; }
        if m == self.host { let mut c = Command::new("herdr"); c.args(["agent", "focus", p]); crate::proc::detached(c); }
        else { self.act(&["flash", m, p], "flash", true); }
    }
    fn delegate(&mut self, term: &mut Term, view: &str, arg: &str) {
        let mut o = std::io::stdout(); let _ = o.write_all(b"\x1b[?1003l\x1b[?1002l\x1b[?1006l\x1b[0m\x1b[?25h"); let _ = o.flush();
        // the view runs in a child; a SIGTERM or a closed terminal for this panel ends the child as well
        if let Ok(mut ch) = Command::new("bash").arg(&self.script).env("BHOTE_DELEGATE", view).env("BHOTE_SEL_ID", &self.sel_id).env("BHOTE_ARG", arg).spawn() {
            loop {
                match ch.try_wait() { Ok(Some(_)) | Err(_) => break, Ok(None) => {} }
                if crate::term::quit_requested() { let _ = ch.kill(); let _ = ch.wait(); break; }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        *term = Term::enter(true);
        self.reload_cfg(); self.real_cols = Term::size().0; self.real_rows = Term::size().1; self.cols = self.real_cols.min(self.num("PANEL_MAX_WIDTH", 64).max(30) as u32).max(MIN_COLS); self.rows = self.real_rows;
        self.redraw = true; self.clamp_adopt = true; self.clamp_at = Some(Instant::now() + self.clamp_wait());   // (the size may have changed under the page: no lead)
    }

    /// How long a resize settles before the width is checked: a pane outside PANEL_MIN_WIDTH..PANEL_MAX_WIDTH (the panel of a workspace
    /// that was just shown) is corrected at once, so it does not stay narrow for a second; any other resize waits as before
    fn clamp_wait(&self) -> Duration {
        let (lo, hi) = (self.num("PANEL_MIN_WIDTH", 46), self.num("PANEL_MAX_WIDTH", 64).max(30));
        let w = self.real_cols as i64;
        if w < lo || w > hi.max(lo) { Duration::from_millis(50) } else { Duration::from_secs(1) }
    }

    // ── a line to type into (ask) ─────────────────────────────────────────────────────────────────────────────
    fn ask(&mut self, term: &mut Term, label: &str, start: &str, max: usize) -> Option<String> {
        let mut buf: Vec<char> = start.chars().take(max).collect(); let mut cur = buf.len();
        let draw = |buf: &Vec<char>, cur: usize, rows: u32, cols: u32| {
            let text: String = buf.iter().collect(); let lw = label.chars().count();
            let room = (cols as usize).saturating_sub(lw + 1);
            let skip = if cur > room { cur - room } else { 0 };
            let shown: String = buf.iter().skip(skip).take(room).collect();
            let mut o = std::io::stdout().lock();
            let _ = write!(o, "\x1b[?1003l\x1b[?1002l\x1b[?1006l\x1b[{};1H\x1b[0m\x1b[K{}{}\x1b[{};{}H\x1b[?25h", rows, label, shown, rows, lw + 1 + cur - skip);
            let _ = o.flush(); let _ = text;
        };
        draw(&buf, cur, self.real_rows, self.real_cols);
        loop {
            match term.next(1000) {
                Ev::Enter => break,
                Ev::Esc | Ev::Eof => { self.after_ask(); return None; }
                Ev::Backspace => { if cur > 0 { buf.remove(cur - 1); cur -= 1; } }
                Ev::Left => { cur = cur.saturating_sub(1); } Ev::Right => { if cur < buf.len() { cur += 1; } }
                Ev::Key('\u{1}') => cur = 0, Ev::Key('\u{5}') => cur = buf.len(),
                Ev::Key('\u{15}') => { buf.drain(..cur); cur = 0; } Ev::Key('\u{b}') => { buf.truncate(cur); }
                Ev::Key('\u{17}') => { let mut i = cur; while i > 0 && buf[i - 1] == ' ' { i -= 1; } while i > 0 && buf[i - 1] != ' ' { i -= 1; } buf.drain(i..cur); cur = i; }
                Ev::Key(c) if !c.is_control() => { if buf.len() < max { buf.insert(cur, c); cur += 1; } }
                Ev::Resize => { self.real_cols = Term::size().0; self.real_rows = Term::size().1; }
                _ => {}
            }
            draw(&buf, cur, self.real_rows, self.real_cols);
        }
        self.after_ask();
        let s: String = buf.iter().collect(); let s: String = s.chars().filter(|c| !c.is_control()).collect();
        Some(s.trim_matches(' ').to_string())
    }
    fn after_ask(&mut self) { let mut o = std::io::stdout(); let _ = o.write_all(b"\x1b[0m\x1b[?25l\x1b[?1002h\x1b[?1006h"); let _ = o.flush(); self.redraw = true; }

    // ── the mouse ─────────────────────────────────────────────────────────────────────────────────────────────
    fn ref_at(&self, y: i64) -> Option<&String> { self.tmap.iter().find(|(r, t)| *r == y && !t.is_empty()).map(|(_, t)| t) }
    fn mouse(&mut self, term: &mut Term, b: u32, x: u32, y: u32, press: bool) {
        let (x, y) = (x as i64, y as i64); let div = self.divrow;
        if matches!(b, 64..=67) { return; }
        if b == 35 {
            let hid = if div == 0 || y < div { self.ref_at(y).map(|t| t.split('!').next().unwrap_or("").to_string()).unwrap_or_default() } else { String::new() };
            let hb = div > 0 && y == div && x >= self.cols as i64 - 4 && x <= self.cols as i64 - 2;
            if hid != self.hover_id || hb != self.hover_btn { self.hover_id = hid; self.hover_btn = hb; self.redraw = true; }
            return;
        }
        if !press { return; }
        if b == 2 {   // the right button: the topic's menu
            if let Some(t) = self.ref_at(y).cloned() { let id = t.split('!').next().unwrap_or("").to_string(); if !(id.is_empty() || id.starts_with("more:") || id.starts_with("cap:") || id.starts_with("agent:")) { self.sel_id = id; self.open_menu(term); } }
            return;
        }
        if b != 0 { return; }
        if div > 0 && y >= div + 1 {   // below the divider: an agent?
            let split: i64 = self.agmap.first().and_then(|s| s.trim().parse().ok()).unwrap_or(0); let rel = y - div - 1;
            if split > 0 { if let Some(line) = self.agmap.iter().skip(1).find(|l| l.split('\u{1e}').next().and_then(|r| r.parse::<i64>().ok()) == Some(rel)) {
                let f: Vec<&str> = line.split('\u{1e}').collect(); let ag = if x <= split { f.get(1) } else { f.get(2) }.copied().unwrap_or("");
                if !ag.is_empty() { let mut p = ag.split(US); let (m, pane) = (p.next().unwrap_or("").to_string(), p.next().unwrap_or("").to_string()); self.agent_jump(&m, &pane); return; }
            } }
        }
        if div == 0 || y < div {   // above it: a topic?
            if let Some(t) = self.ref_at(y).cloned() {
                let id = t.clone();
                if let Some(rest) = id.strip_prefix("agent:") {   // an idle agent's card: show the agent
                    if let Some((m, pane)) = rest.rsplit_once('/') { let (m, pane) = (m.to_string(), pane.to_string()); self.agent_jump(&m, &pane); }
                    self.redraw = true; return;
                }
                let twice = self.click_twice(&id);
                if twice { // a double click: the topic's page
                    self.jump_at = None;   // (the click before it has not shown the agent yet: it would have taken the focus away from this panel)
                    if !(id.starts_with("cap:")) { self.sel_id = id.clone(); }
                    if !(id.starts_with("more:") || id.starts_with("cap:")) { self.open_menu(term); }
                    self.redraw = true; return;
                }
                if let Some(k) = id.strip_prefix("cap:") { let k = k.to_string(); self.section_toggle(&k); }
                else if let Some(s) = id.strip_prefix("more:") { self.sel_id = id.clone(); let s = s.to_string(); self.toggle_expand(&s); }
                else { self.sel_id = id.clone(); if self.is_card(&id) { self.jump_at = Some((id.clone(), Instant::now() + Duration::from_millis(500))); } }   // (the agent is shown when no second click follows: a double click must reach this panel)
                self.redraw = true; return;
            }
        }
        if y == div && x >= self.cols as i64 - 4 && x <= self.cols as i64 - 2 {   // the ↕ on the divider
            let on = self.c("AGENTS_COMPACT", "off") == "on"; self.cfg_set("AGENTS_COMPACT", if on { "off" } else { "on" }); self.redraw = true;
        }
    }
    fn click_twice(&mut self, id: &str) -> bool {
        let now = Instant::now();
        if let Some((last, at)) = &self.click_last { if last == id && now.duration_since(*at) <= Duration::from_millis(500) { self.click_last = None; return true; } }
        self.click_last = Some((id.to_string(), now)); false
    }
    fn is_card(&self, id: &str) -> bool { self.with_cards(|c| c.ts().iter().find(|t| t.id == id).map_or(false, |t| matches!(t.status.as_str(), "review" | "now" | "testing" | "finished") || (t.status == "waiting" && t.dep == "1"))) }
    fn section_toggle(&mut self, kind: &str) { let k = format!("{}_COMPACT", kind.to_uppercase()); let on = self.c(&k, "off") == "on"; self.cfg_set(&k, if on { "off" } else { "on" }); }
    // ── keys ──────────────────────────────────────────────────────────────────────────────────────────────────
    fn key(&mut self, term: &mut Term, c: char) -> bool {
        let mut live = Live { env: &self.env(), notify_on: false, n: 0 };
        let _ = &mut live;
        match c {
            'q' | 'Q' => return false,
            ',' => self.delegate(term, "settings", ""),
            'w' | 'W' => { let nf = (!self.c("SETUP_DONE", "").is_empty() && self.c("WIZARD_LEVEL", "1").parse::<u64>().unwrap_or(1) < FEATURE_LEVEL) as u8; self.delegate(term, "setup", &nf.to_string()); }
            'a' | 'A' => self.delegate(term, "suggest", ""),
            's' => { if self.sel_topic().is_some() { self.screen = Screen::Pick("start".into(), 0); self.redraw = true; } }
            't' | 'T' => { if self.sel_topic().map_or(false, |t| !t.3.is_empty()) { self.screen = Screen::Pick("transfer".into(), 1); self.redraw = true; } }
            'x' | 'X' => { if let Some((_, f, ..)) = self.sel_topic() { self.sel_leave(); let env = self.env(); Live { env: &env, notify_on: false, n: 0 }.set(&f, "status", "done"); } self.redraw = true; }
            'c' | 'C' => { if let Some((id, _, st, ..)) = self.sel_topic() { if st == "review" || st == "finished" { self.sel_leave(); self.act(&["dismiss", &id], "dismiss", false); } } self.redraw = true; }
            'e' | 'E' => { if let Some((_, f, _, _, title)) = self.sel_topic() { if let Some(a) = self.ask(term, "Title: ", &title, 500) { if !a.is_empty() { let env = self.env(); Live { env: &env, notify_on: false, n: 0 }.set(&f, "title", &a); } } } self.redraw = true; }
            'd' | 'D' => { if let Some((_, f, ..)) = self.sel_topic() { let cur = store::topic_get(&f, "description"); if let Some(a) = self.ask(term, "Description: ", &cur, 140) { let env = self.env(); Live { env: &env, notify_on: false, n: 0 }.set(&f, "description", &a); } } self.redraw = true; }
            'n' | 'N' => { if let Some(a) = self.ask(term, "Topic: ", "", 500) { self.new_topic_from(&a); } self.redraw = true; }
            'y' | 'Y' | '1' | '2' | '3' => { if let Some((id, ..)) = self.sel_topic() { let n = if c == 'y' || c == 'Y' { '1' } else { c }; self.act(&["review", &id, &n.to_string()], "review", false); } self.redraw = true; }
            _ => {}
        }
        true
    }
    /// new_topic_from: "Title", "Title; description", "Title @Name" or "Title > Name" (= waiting for Name)
    fn new_topic_from(&mut self, text: &str) {
        let mut title = text.to_string(); let mut who = String::new(); let mut desc = String::new();
        if let Some(i) = title.rfind(" @") { who = title[i + 2..].to_string(); title.truncate(i); } else if let Some(i) = title.rfind(" > ") { who = title[i + 3..].to_string(); title.truncate(i); }
        if let Some(i) = title.find(';') { desc = title[i + 1..].to_string(); title.truncate(i); }
        let title = title.trim_end().to_string(); let desc = desc.trim_start().to_string();
        let env = self.env(); let mut live = Live { env: &env, notify_on: false, n: 0 };
        let Some(f) = live.new_topic(&title, &who, &desc) else { return };
        self.sel_id = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let cp = self.current_project(); if !cp.is_empty() { live.set(&f, "project", &cp); }
    }

    // ── the loop ──────────────────────────────────────────────────────────────────────────────────────────────
    /// The collector (what is still the script's: lead election, the keys, Slack, the update check): a child that ends with this process.
    fn start_collector(&mut self) {
        self.collector_at = Instant::now();
        if let Ok(c) = Command::new("bash").arg(&self.script).env("BHOTE_COLLECTOR_ONLY", std::process::id().to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() { self.collector = Some(c); }
    }
    /// It is started again when it has ended (a crash, a kill), with a pause that grows when it keeps ending at once
    fn keep_collector(&mut self) {
        let ended = match self.collector.as_mut() { Some(c) => matches!(c.try_wait(), Ok(Some(_)) | Err(_)), None => true };
        if !ended { if self.collector_at.elapsed() > Duration::from_secs(60) { self.collector_tries = 0; } return; }
        let wait = Duration::from_secs((1u64 << self.collector_tries.min(5)).min(30));
        if self.collector_at.elapsed() < wait { return; }
        self.collector_tries += 1; self.collector = None; self.start_collector();
        crate::proc::note(&self.shared, &format!("the collector ended: started again (try {})", self.collector_tries));
    }
    /// This panel's pane and process in <shared>/panels/<pane>: how the width sync finds a panel that has no label (one started by hand)
    fn register(&self) {
        if self.pane_id.is_empty() || !self.pane_id.chars().all(|c| c.is_ascii_alphanumeric() || ":_-".contains(c)) { return; }
        let dir = self.shared.join("panels"); let _ = fs::create_dir_all(&dir);
        let f = dir.join(&self.pane_id); let me = format!("{}\n", std::process::id());
        if fs::read_to_string(&f).map_or(true, |t| t != me) { let _ = fs::write(&f, me); }
    }
    fn unregister(&self) { if !self.pane_id.is_empty() { let _ = fs::remove_file(self.shared.join("panels").join(&self.pane_id)); } }
    fn tick(&mut self) {
        self.keep_collector();
        let sec = store::now();
        if let Some((id, t)) = self.jump_at.clone() { if Instant::now() >= t { self.jump_at = None; self.act(&["topic_jump", &id], "jump", false); } }
        if let Some(t) = self.clamp_at { if Instant::now() >= t { self.clamp_at = None; if !self.pane_id.is_empty() { if self.clamp_adopt { self.act(&["clamp", "adopt"], "clamp", false); } else { self.act(&["clamp"], "clamp", false); } } self.clamp_adopt = false; } }   // (outside herdr there is no pane to widen: no process; not once a second: a pane that is too narrow is widened at once)
        if sec != self.last_tick {
            self.last_tick = sec;
            self.register();
            if !self.pane_id.is_empty() {
                if let Ok(t) = crate::proc::read_cap(self.shared.join("focus.pane")) {
                    let mut p = t.split_whitespace(); let (fp, ft) = (p.next().unwrap_or(""), p.next().unwrap_or(""));
                    let pf = Some(fp == self.pane_id); let vis = !(ft.len() > 0 && !self.tab_id.is_empty() && ft != self.tab_id);
                    if vis != self.tab_visible { self.tab_visible = vis; if vis { self.redraw = true; self.dirty = false; self.shown_at = Some(Instant::now()); } }
                    if pf != self.pane_focus { self.pane_focus = pf; self.redraw = true; }
                }
            }
            if let Some((_, u)) = &self.flash { if Instant::now() >= *u { self.flash = None; self.redraw = true; } }
            if sec >= self.last_chk + 2 { self.last_chk = sec; let s = self.sig(); if s != self.files_sig { self.files_sig = s; if self.tab_visible { self.redraw = true; self.reload_cfg(); } else { self.dirty = true; } } }
            // bhote find / a jump chose a topic: select it here
            if let Ok(t) = crate::proc::read_cap(self.shared.join("select")) {
                let t = t.trim().to_string();
                if self.select_seen.as_deref() != Some(&t) {
                    if self.select_seen.is_some() { let mut p = t.split_whitespace(); let id = p.next().unwrap_or("").to_string(); if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && self.topics.join(format!("{}.topic", id)).exists() { self.sel_id = id; self.redraw = true; } }
                    self.select_seen = Some(t);
                }
            } else if self.select_seen.is_none() { self.select_seen = Some(String::new()); }
        }
    }
}

pub fn run() -> i32 {
    if !Term::is_tty() { eprintln!("bhoted panel: needs a terminal"); return 1; }
    let mut p = Panel::new();
    p.reload_cfg();
    p.start_collector();
    let mut term = Term::enter(true);
    { let mut o = std::io::stdout(); let _ = o.write_all(b"\x1b]2;Bhote\x07"); let _ = o.flush(); }
    p.files_sig = p.sig();
    if p.c("SETUP_DONE", "").is_empty() { p.delegate(&mut term, "setup", "0"); }   // never set up on this machine: the wizard first
    let shared = p.shared.clone();
    if let Err(e) = crate::proc::check_private_dir(&shared) { term.leave(); eprintln!("bhoted panel: {}", e); return 1; }
    loop {
        if p.redraw && p.tab_visible && !term.pending() {
            if crate::proc::guarded("paint", &shared, || p.paint()).is_none() { p.redraw = false; p.flash = Some(("internal error: see bhoted.log in the shared folder".into(), Instant::now() + Duration::from_secs(10))); }
        }
        else if p.redraw && !p.tab_visible { p.redraw = false; p.dirty = true; }
        let mut timeout: i32 = if !p.pending.is_empty() { 150 } else if p.spin_any && p.tab_visible { 300 } else { 1000 };
        if let Some((_, t)) = &p.jump_at { timeout = timeout.min(t.saturating_duration_since(Instant::now()).as_millis() as i32 + 1); }
        if let Some(t) = p.clamp_at { timeout = timeout.min(t.saturating_duration_since(Instant::now()).as_millis() as i32 + 1); }   // (the width check is due: do not sleep past it)
        let ev = term.next(timeout);
        let go = crate::proc::guarded("event", &shared, || handle(&mut p, &mut term, ev)).unwrap_or_else(|| { p.redraw = true; true });
        if !go { break; }
        crate::proc::guarded("tick", &shared, || { p.poll_pending(); p.tick(); });
    }
    p.unregister();
    term.leave();
    if let Some(mut c) = p.collector.take() { let _ = c.kill(); }
    0
}

// ── the topic's page and the chooser ─────────────────────────────────────────────────────────────────────────
impl Panel {
    fn live(&self) -> Live<'static> { let env: &'static Env = Box::leak(Box::new(self.env())); Live { env, notify_on: false, n: 0 } }
    fn set(&self, f: &std::path::Path, k: &str, v: &str) { self.live().set(f, k, v); }
    fn poke(&self) { let _ = fs::write(self.shared.join("poke"), ""); }
    /// The page of the selected topic (the script draws it when a Slack pin or answer is on it)
    fn open_menu(&mut self, term: &mut Term) {
        let Some((_, f, ..)) = self.sel_topic() else { return };
        if crate::menuui::needs_script(&f) { self.delegate(term, "menu", ""); } else { self.screen = Screen::Menu(0); self.menu_flash = None; self.redraw = true; }
    }
    fn menu_do(&mut self, term: &mut Term, idx: usize) {
        let Some((id, f, ..)) = self.sel_topic() else { self.screen = Screen::Main; self.redraw = true; return };
        let items = self.menu_list(&f);
        let Some((key, _)) = items.get(idx).cloned() else { return };
        let mut stay = false;
        let now = store::now().to_string();
        match key.as_str() {
            "start" => { self.screen = Screen::Pick("start".into(), 0); self.redraw = true; return; }
            "transfer" => { self.screen = Screen::Pick("transfer".into(), 1); self.redraw = true; return; }
            "resume" => self.act(&["resume", &id], "resume", false),
            "jump" => { let (m, p) = (store::topic_get(&f, "agent_machine"), store::topic_get(&f, "agent_pane")); self.agent_jump(&m, &p); stay = true; }
            "now" => { self.set(&f, "status", "now"); self.set(&f, "waiting_for", ""); self.set(&f, "auto_back", ""); self.poke(); }
            "review" => { self.set(&f, "status", "review"); self.set(&f, "auto_back", ""); self.poke(); }
            "wait" => {
                let cur = store::topic_get(&f, "waiting_for");
                match self.ask(term, "Waiting for: ", &cur, 500) {
                    Some(a) if !a.is_empty() => { self.set(&f, "status", "waiting"); self.set(&f, "waiting_for", &a); self.set(&f, "waiting_since", &now); self.set(&f, "deploy_wait", ""); self.set(&f, "slack_msg", ""); }
                    _ => stay = true,
                }
            }
            "deploy" => self.act(&["deploy", &id], "deploy", false),
            "dismiss" => { self.sel_leave(); self.act(&["dismiss", &id], "dismiss", false); }
            "project" => {
                let cur = store::topic_get(&f, "project"); let ctx = self.ctx(); let ps = ui::projects(&ctx);
                let nx = if cur.is_empty() { ps.first().map(|p| p.id.clone()).unwrap_or_default() } else { ps.iter().position(|p| p.id == cur).and_then(|i| ps.get(i + 1)).map(|p| p.id.clone()).unwrap_or_default() };
                self.set(&f, "project", &nx); stay = true;
            }
            "slack" => {
                if !store::topic_get(&f, "slack_channel").is_empty() { self.act(&["slack", &id, "off"], "flash", true); stay = true; }
                else {
                    match self.ask(term, "Slack: a link, #channel, @person or a name: ", "", 500) {
                        Some(a) if !a.is_empty() => { self.menu_flash = Some((format!("… asking Claude for {} (a few seconds)", a), Instant::now() + Duration::from_secs(120))); self.act(&["slack", &id, &a], "menuflash", true); }
                        _ => {}
                    }
                    stay = true;
                }
            }
            "cancelxfer" => { self.set(&f, "transfer_to", ""); self.set(&f, "transfer_to_name", ""); self.set(&f, "transfer_from", ""); }
            "reopen" => { self.set(&f, "status", "now"); self.set(&f, "waiting_for", ""); }
            "next" => self.set(&f, "status", "next"), "later" => self.set(&f, "status", "later"), "done" => self.set(&f, "status", "done"),
            "rename" => { let cur = store::topic_get(&f, "title"); match self.ask(term, "Title: ", &cur, 500) { Some(a) if !a.is_empty() => self.set(&f, "title", &a), _ => stay = true } }
            "describe" => { let cur = store::topic_get(&f, "description"); match self.ask(term, "Description: ", &cur, 140) { Some(a) => self.set(&f, "description", &a), None => stay = true } }
            "delete" => {
                let t = store::topic_get(&f, "title");
                match self.ask(term, &format!("Delete \"{}\"? y/N: ", t), "", 5) { Some(a) if a == "y" || a == "Y" => { self.set(&f, "deleted", "1"); self.sel_id.clear(); } _ => stay = true }
            }
            _ => {}
        }
        if !stay { self.screen = Screen::Main; }
        self.redraw = true;
    }
    fn menu_edit_field(&mut self, term: &mut Term, idx: i64) {
        let Some((_, f, ..)) = self.sel_topic() else { return };
        if idx == -2 { let cur = store::topic_get(&f, "title"); if let Some(a) = self.ask(term, "", &cur, 500) { if !a.is_empty() { self.set(&f, "title", &a); } } }
        else { let cur = store::topic_get(&f, "description"); if let Some(a) = self.ask(term, "", &cur, 140) { self.set(&f, "description", &a); } }
        self.redraw = true;
    }
    fn menu_run(&mut self, term: &mut Term, idx: i64) { if idx == -2 || idx == -1 { self.menu_edit_field(term, idx); } else if idx >= 0 { self.menu_do(term, idx as usize); } }
    /// Keys on the page and in the chooser. Returns false to leave the panel.
    fn screen_key(&mut self, term: &mut Term, ev: &Ev) -> bool {
        match self.screen.clone() {
            Screen::Menu(idx) => {
                let n = self.sel_topic().map(|t| self.menu_list(&t.1).len() as i64).unwrap_or(0);
                match ev {
                    Ev::Up | Ev::Key('k') => { let mut i = idx - 1; if i < -2 { i = n - 1; } self.screen = Screen::Menu(i); self.redraw = true; }
                    Ev::Down | Ev::Key('j') => { let mut i = idx + 1; if i >= n { i = -2; } self.screen = Screen::Menu(i); self.redraw = true; }
                    Ev::Enter | Ev::Right => { self.menu_run(term, idx); }
                    Ev::Key(c) if c.is_ascii_digit() && *c != '0' => { let k = c.to_digit(10).unwrap_or(0) as i64; if k <= n { self.screen = Screen::Menu(k - 1); self.menu_do(term, (k - 1) as usize); } self.redraw = true; }
                    Ev::Esc | Ev::Backspace | Ev::Left | Ev::Key('q') | Ev::Key('Q') | Ev::Key('h') => { self.screen = Screen::Main; self.redraw = true; }
                    _ => {}
                }
            }
            Screen::Pick(mode, idx) => {
                let (n, first) = { let m = self.pick_agents(&mode); (m.len() as i64, if mode == "transfer" { 1 } else { 0 }) };
                match ev {
                    Ev::Up | Ev::Key('k') => { let mut i = idx - 1; if i < first { i = n; } self.screen = Screen::Pick(mode, i); self.redraw = true; }
                    Ev::Down | Ev::Key('j') => { let mut i = idx + 1; if i > n { i = first; } self.screen = Screen::Pick(mode, i); self.redraw = true; }
                    Ev::Enter | Ev::Right => { self.pick_do(term, &mode, idx); self.screen = Screen::Main; self.redraw = true; }
                    Ev::Esc | Ev::Backspace | Ev::Left | Ev::Key('q') | Ev::Key('Q') | Ev::Key('h') => { self.screen = Screen::Main; self.redraw = true; }
                    _ => {}
                }
            }
            Screen::Main => {}
        }
        true
    }
    fn pick_agents(&self, mode: &str) -> Vec<crate::cards::Row> {
        let Some((_, f, ..)) = self.sel_topic() else { return Vec::new() };
        let rows = self.agent_rows_all(); let ts = store::load_fast(&self.topics, &self.shared);
        let src = (store::topic_get(&f, "agent_machine"), store::topic_get(&f, "agent_pane"));
        crate::menuui::pick_map(&rows, &ts, mode, if mode == "transfer" { Some((src.0.as_str(), src.1.as_str())) } else { None }).into_iter().map(|r| crate::cards::Row { m: r.m.clone(), id: r.id.clone(), st: r.st.clone(), name: r.name.clone(), task: r.task.clone() }).collect()
    }
    fn pick_do(&mut self, term: &mut Term, mode: &str, idx: i64) {
        let Some((id, f, ..)) = self.sel_topic() else { return };
        let map = self.pick_agents(mode);
        if mode == "transfer" {
            if idx < 1 { return; }
            let Some(a) = map.get((idx - 1) as usize).cloned() else { return };
            let tpj = store::topic_get(&f, "project"); let mut warn = String::new();
            if !tpj.is_empty() {
                let metas = crate::proc::read_cap(self.shared.join("agents.meta.local")).unwrap_or_default() + &crate::proc::read_cap(self.shared.join("agents.meta.remote")).unwrap_or_default();
                let proj = metas.lines().find_map(|l| { let p: Vec<&str> = l.split(US).collect(); if p.len() >= 6 && p[0] == a.m && p[1] == a.id { Some(p[5].to_string()) } else { None } }).unwrap_or_default();
                if proj != tpj { warn = " (NOT an agent of this topic's project!)".into(); }
            }
            let from = { let g = store::topic_get(&f, "agent"); if g.is_empty() { "agent".to_string() } else { g } };
            let label = format!("Hand \"{}\" over from {} to {}{}? The source agent commits and stops. y/N: ", store::topic_get(&f, "title"), from, a.name, warn);
            match self.ask(term, &label, "", 5) { Some(x) if x == "y" || x == "Y" => {} _ => return }
            let mut args = vec!["transfer", id.as_str(), a.m.as_str(), a.id.as_str(), a.name.as_str()]; if !warn.is_empty() { args.push("force"); }
            self.act(&args, "transfer", false);
            return;
        }
        if idx > 0 {
            let Some(a) = map.get((idx - 1) as usize).cloned() else { return };
            self.act(&["pickstart", &id, &a.m, &a.id, &a.name], "pickstart", false);
        } else {
            self.set(&f, "status", "now"); self.set(&f, "waiting_for", "");
            for k in ["agent", "agent_machine", "agent_pane", "agent_session", "delegated"] { self.set(&f, k, ""); }
        }
    }
    fn screen_mouse(&mut self, term: &mut Term, b: u32, y: u32, press: bool) {
        if let Screen::Menu(_) = self.screen { if press && b == 0 { if let Some((_, r)) = self.pmap.iter().find(|(row, _)| *row == y as i64).cloned() { if let Ok(i) = r.parse::<i64>() { self.screen = Screen::Menu(i); self.menu_run(term, i); } } } }
    }
}

/// One event of the main loop; false: leave the panel
fn handle(p: &mut Panel, term: &mut Term, ev: Ev) -> bool {
    match ev {
        Ev::Timeout => { if p.spin_any && p.tab_visible { p.spin_tick += 1; p.redraw = true; } }
        Ev::Eof => return false,
        Ev::Resize => { let (c, r) = Term::size(); p.real_cols = c; p.real_rows = r; p.rows = r; p.reload_cfg(); p.redraw = true; if !p.tab_visible || p.shown_at.map_or(false, |t| t.elapsed() < Duration::from_secs(3)) { p.clamp_adopt = true; } p.clamp_at = Some(Instant::now() + p.clamp_wait()); }   // (a size that changed while the tab was hidden or just shown comes from the layout, not from a drag)
        Ev::Mouse { b, x, y, press } => { if p.screen == Screen::Main { p.mouse(term, b, x, y, press) } else { p.screen_mouse(term, b, y, press) } }
        e if p.screen != Screen::Main => { if !p.screen_key(term, &e) { return false; } }
        Ev::Key(c) => { if !p.key(term, c) { return false; } }
        Ev::Right => { if p.sel_id.starts_with("more:") { let s = p.sel_id[5..].to_string(); p.toggle_expand(&s); p.redraw = true; } else if p.sel_topic().is_some() { p.open_menu(term); } }
        _ => {}
    }
    true
}
