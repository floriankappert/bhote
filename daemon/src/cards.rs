//! The topic cards and lines of the frame (step 5): topics_block (cards | lists), with topics_plan, ti_card, topic_meta,
//! topic_line, more_row, bored_agents and the deployment card, from the script, byte for byte. The state of the screen that is not in a
//! file (selection, hover, which groups are open, the dancing star) arrives in a small file written by the script for each frame.
use crate::store::{self, Topic};
use crate::ui::{abbr as _abbr, clip, dw, Ctx};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const US: char = '\u{1f}';
const REF: char = '\u{1d}';    // \035
const SPIN: char = '\u{1c}';   // \034
const SELMARK: char = '\u{1}'; // \001

pub struct View {
    pub sel_id: String, pub hover_id: String, pub expanded: String, pub scope_project: bool, pub cur_proj: String, pub spin_ch: String,
    pub bored_small: bool, pub archive_days: i64, pub github: bool, pub host: String, pub mach_fg: String, pub line_max: i64, pub done_max: i64, pub mcodes: Vec<(String, String)>,
    pub rows: i64, pub head_rows: i64, pub hover_btn: bool, pub flash: String, pub remote_on: bool, pub has_machines: bool, pub agent_rows: Option<i64>, pub pane_focus: Option<bool>, pub head_note: String, pub newfeat: bool, pub version_label: String, pub head_grad: String,
}
impl View {
    pub fn read(path: &str) -> View {
        let mut m: HashMap<String, String> = HashMap::new();
        for l in crate::proc::read_cap(path).unwrap_or_default().split('\n') { if let Some(i) = l.find('=') { m.insert(l[..i].to_string(), l[i + 1..].to_string()); } }
        let g = |k: &str| m.get(k).cloned().unwrap_or_default();
        let num = |k: &str, d: i64| { let v = g(k); if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) { v.parse().unwrap_or(d) } else { d } };
        View { sel_id: g("SEL_ID"), hover_id: g("HOVER_ID"), expanded: g("EXPANDED"), scope_project: g("U_SCOPE") == "project", cur_proj: g("CUR_PROJ"), spin_ch: if g("SPIN_CH").is_empty() { "✶".into() } else { g("SPIN_CH") },
            bored_small: g("BORED_SMALL") == "1", archive_days: num("U_ARCHIVE_DAYS", 7), github: g("U_GITHUB") != "off", host: g("HOST"), mach_fg: g("MACH_FG"), line_max: num("LINE_MAX", 10), done_max: num("U_DONE_MAX", 4),
            rows: num("ROWS", 40), head_rows: num("HEAD_ROWS", 0), hover_btn: g("HOVER_BTN") == "1", flash: g("FLASH"), remote_on: g("U_REMOTE") == "on", has_machines: g("HAS_MACHINES") == "1",
            agent_rows: { let v = g("U_AGENT_ROWS"); if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) { v.parse().ok() } else { None } },
            pane_focus: match g("PANE_FOCUS").as_str() { "true" => Some(true), "false" => Some(false), _ => None }, head_note: g("HEAD_NOTE"), newfeat: g("NEWFEAT") == "1", version_label: g("VERSION_LABEL"), head_grad: g("HEAD_GRAD"),
            mcodes: g("MCODES").split('\u{1e}').filter(|p| !p.is_empty()).map(|p| { let mut f = p.splitn(2, US); (f.next().unwrap_or("").to_string(), f.next().unwrap_or("").to_string()) }).collect() }
    }
}

#[derive(Clone)]
pub struct Row { pub m: String, pub id: String, pub st: String, pub name: String, pub task: String }
fn rows(p: &std::path::Path) -> Vec<Row> {
    crate::proc::read_cap(p).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(|l| { let mut f = l.split(US).map(|x| x.to_string()); Row { m: f.next().unwrap_or_default(), id: f.next().unwrap_or_default(), st: f.next().unwrap_or_default(), name: f.next().unwrap_or_default(), task: f.next().unwrap_or_default() } }).collect()
}

fn sp(n: i64) -> String { " ".repeat(n.unsigned_abs() as usize) }
fn pre(s: &str, n: usize) -> String { s.chars().take(n).collect() }
fn clean(s: &str) -> String { s.chars().filter(|c| !c.is_control()).collect() }
fn digits(s: &str) -> bool { !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) }

/// age_set + ago_set: "just now", "5m ago", "3h ago", "2d ago" (relative to now)
fn ago(now: u64, v: &str) -> String {
    if !digits(v) { return "? ago".into(); }
    let d = (now as i64 - v.parse::<i64>().unwrap_or(0)).max(0);
    if d < 60 { "just now".into() } else if d < 3600 { format!("{}m ago", d / 60) } else if d < 172800 { format!("{}h ago", d / 3600) } else { format!("{}d ago", d / 86400) }
}

pub struct Cards<'a> { pub c: &'a Ctx, pub v: &'a View, ts: Vec<Topic>, epj: Vec<String>, ag: Vec<Row>, now_l: Vec<Row>, now_r: Vec<Row>, pub projects: Vec<crate::ui::Project>, shared: PathBuf, pub host: String, pub rows_total: i64 }

pub struct Plan { pub vis: Vec<bool>, pub before: Vec<String>, pub after: Vec<String>, pub n_next: i64, pub n_waiting: i64, pub n_later: i64, pub sel_list: Vec<String> }

impl<'a> Cards<'a> {
    pub fn new(c: &'a Ctx, v: &'a View) -> Cards<'a> {
        let mut ts = store::load_fast(&c.topics, &c.shared);
        let shared = c.shared.clone();
        // a topic that waits for a deployment is a card in DEPLOYING only while there is a deployment it follows; without one it is a line in the Waiting list
        if ts.iter().any(|t| t.status == "waiting" && t.dep == "1") {
            let names: HashMap<String, String> = crate::ui::projects(c).into_iter().map(|p| (p.id, p.name)).collect();
            for t in ts.iter_mut() { if t.status == "waiting" && t.dep == "1" && deploy_lookup_pub(&shared, &c.topics, &names, t).is_none() { t.dep = String::new(); } }
        }
        let mut ag = rows(&shared.join("agents.local")); ag.extend(rows(&shared.join("agents.remote")));
        // topics_projects: a topic's project, else its agent's (the pane meta lists: machine US pane US tab US a US repo US project US cwd)
        let mut metas = String::new();
        for f in ["agents.meta.local", "agents.meta.remote"] { metas.push_str(&crate::proc::read_cap(shared.join(f)).unwrap_or_default()); }
        let epj = ts.iter().map(|t| {
            if !t.pj.is_empty() { return t.pj.clone(); }
            if t.ap.is_empty() { return String::new(); }
            for l in metas.lines() { let f: Vec<&str> = l.split(US).collect(); if f.len() >= 6 && f[0] == t.am && f[1] == t.ap { return f[5].to_string(); } }
            String::new()
        }).collect();
        Cards { c, v, ts, epj, ag, now_l: rows(&shared.join("agents.now.local")), now_r: rows(&shared.join("agents.now.remote")), projects: crate::ui::projects(c), shared, host: v.host.clone(), rows_total: v.rows }
    }
    fn g(&self, k: &str) -> &str { self.c.c.g(k) }
    pub fn agent_rows(&self) -> &[Row] { &self.ag }
    pub fn ts(&self) -> &[Topic] { &self.ts }
    /// the pane meta lists of both machines: (machine, pane, project)
    pub fn metas(&self) -> Vec<(String, String, String)> {
        let mut v = Vec::new();
        for f in ["agents.meta.local", "agents.meta.remote"] { for l in crate::proc::read_cap(self.shared.join(f)).unwrap_or_default().lines() { let p: Vec<&str> = l.split(US).collect(); if p.len() >= 6 { v.push((p[0].to_string(), p[1].to_string(), p[5].to_string())); } } }
        v
    }
    pub fn in_scope(&self, i: usize) -> bool { if !(self.v.scope_project && !self.v.cur_proj.is_empty()) { return true; } self.epj[i].is_empty() || self.epj[i] == self.v.cur_proj }
    fn archived(&self, i: usize) -> bool {
        let d = self.v.archive_days; let t = &self.ts[i];
        d > 0 && t.upd > 0 && t.id != self.v.sel_id && (self.c.now as i64 - t.upd as i64) >= d * 86400
    }
    fn shows(st: &str) -> bool { matches!(st, "now" | "testing" | "finished" | "next" | "review" | "waiting") }
    pub fn sel_index(&self) -> i64 {
        if self.v.sel_id.starts_with("more:") { return -1; }
        for (i, t) in self.ts.iter().enumerate() { if t.id == self.v.sel_id { return i as i64; } }
        for (i, t) in self.ts.iter().enumerate() { if Self::shows(&t.status) && self.in_scope(i) { return i as i64; } }
        0
    }
    pub fn done_total(&self) -> i64 { (0..self.ts.len()).filter(|&i| self.ts[i].status == "done" && self.in_scope(i) && !self.archived(i)).count() as i64 }
    pub fn expanded(&self, st: &str) -> bool { self.v.expanded.contains(&format!(" {} ", st)) }

    pub fn plan(&self) -> Plan {
        let n = self.ts.len(); let lim = self.v.line_max;
        let mut p = Plan { vis: vec![false; n], before: vec![String::new(); n], after: vec![String::new(); n], n_next: 0, n_waiting: 0, n_later: 0, sel_list: Vec::new() };
        let (mut g, mut gl) = (String::new(), -1i64);
        let (mut cn_next, mut cn_waiting, mut cn_later) = (0i64, 0i64, 0i64);
        for i in 0..n {
            let t = &self.ts[i]; let st = t.status.as_str();
            if st == "later" && t.dm == "1" { continue; }
            if !self.in_scope(i) { continue; }
            match st { "review" | "now" | "testing" | "finished" => { p.vis[i] = true; p.sel_list.push(t.id.clone()); continue; } "waiting" => if t.dep == "1" { p.vis[i] = true; p.sel_list.push(t.id.clone()); continue; }, _ => {} }
            if self.archived(i) { continue; }
            if st != g {
                let c = match g.as_str() { "next" => cn_next, "waiting" => cn_waiting, "later" => cn_later, _ => 0 };
                if c > lim && self.expanded(&g) && gl >= 0 { p.after[gl as usize] = g.clone(); p.sel_list.push(format!("more:{}", g)); }
                g = st.to_string();
            }
            let c;
            match st { "next" => { cn_next += 1; c = cn_next; } "waiting" => { cn_waiting += 1; c = cn_waiting; } "later" => { cn_later += 1; c = cn_later; } "done" => { continue; } _ => { c = 0; } }
            let exp = self.expanded(st);
            if c <= lim || exp || t.id == self.v.sel_id { p.vis[i] = true; gl = i as i64; p.sel_list.push(t.id.clone()); }
            else if c == lim + 1 { p.before[i] = st.to_string(); p.sel_list.push(format!("more:{}", st)); }
        }
        let c = match g.as_str() { "next" => cn_next, "waiting" => cn_waiting, "later" => cn_later, _ => 0 };
        if c > lim && self.expanded(&g) && gl >= 0 { p.after[gl as usize] = g.clone(); p.sel_list.push(format!("more:{}", g)); }
        let dm = self.v.done_max; if dm > 0 && self.done_total() > dm { p.sel_list.push("more:done".into()); }
        p.n_next = cn_next; p.n_waiting = cn_waiting; p.n_later = cn_later;
        p
    }

    fn agent_state(&self, m: &str, id: &str) -> String { for r in &self.ag { if r.m == m && r.id == id { return if r.st.is_empty() { "idle".into() } else { r.st.clone() }; } } "gone".into() }
    fn agent_task(&self, m: &str, id: &str) -> String { for r in &self.ag { if r.m == m && r.id == id { return r.task.clone(); } } String::new() }
    fn agent_now(&self, m: &str, id: &str) -> String {
        let mut n = String::new();
        for r in &self.now_l { if r.m == m && r.id == id { n = r.st.clone(); break; } }
        if n.is_empty() { for r in &self.now_r { if r.m == m && r.id == id { n = r.st.clone(); break; } } }
        let first = n.split(' ').next().unwrap_or("").to_string();
        if !first.is_empty() && !first.starts_with(|c: char| c.is_alphanumeric()) { n = n.splitn(2, ' ').nth(1).unwrap_or("").to_string(); }
        n
    }

    /// topic_meta: (who, state, state colour, time, branch)
    fn meta(&self, i: usize) -> (String, String, String, String, String) {
        let t = &self.ts[i];
        let (mut who, mut state, mut sc, mut tm) = (String::new(), String::new(), self.g("D").to_string(), String::new());
        let upd = t.upd.to_string();
        match t.status.as_str() {
            "now" => {
                tm = ago(self.c.now, &upd);
                if !t.tn.is_empty() { who = format!("{} → {}", if t.ag.is_empty() { "?" } else { &t.ag }, t.tn); state = "handing over".into(); sc = self.g("C").into(); tm.clear(); }
                else if !t.ag.is_empty() {
                    who = t.ag.clone(); tm.clear();
                    let ast = self.agent_state(&t.am, &t.ap);
                    match ast.as_str() {
                        "working" => { state = "working".into(); sc = self.g("ORANGE").into(); }
                        "testing" => { state = "running tests".into(); sc = self.g("ORANGE").into(); }
                        "blocked" => { state = "needs you".into(); sc = self.g("NN").into(); }
                        "idle" | "done" => { state = "finished".into(); sc = self.g("GREEN").into(); }
                        s if s.starts_with("off-") => { state = format!("offline ({} reconnecting)", t.am); sc = self.g("D").into(); }
                        _ => { state = "gone".into(); sc = self.g("D").into(); }
                    }
                }
            }
            "finished" => { who = t.ag.clone(); state = "finished".into(); sc = self.g("GREEN").into(); tm = ago(self.c.now, &upd); }
            "testing" => { who = t.ag.clone(); state = "tests running".into(); sc = self.g("C").into(); tm = ago(self.c.now, &upd); }
            "waiting" => {
                who = format!("for {}", if t.wfor.is_empty() { "?" } else { &t.wfor });
                tm = ago(self.c.now, if t.wsince.is_empty() { "NOW" } else { &t.wsince }.replace("NOW", &self.c.now.to_string()).as_str());
                if !t.ag.is_empty() { state = format!("{} waits", t.ag); sc = self.g("Y").into(); }
                if !t.sl.is_empty() { state = format!("Slack monitor ({})", t.sl); sc = self.g("S").into(); }
            }
            "review" => {
                who = t.ag.clone(); state = "review".into(); sc = self.g("BLUE").into(); tm = ago(self.c.now, &upd);
                if !t.sm.is_empty() {
                    who = "slack".into();
                    let mut s: String = t.sm.replace('¶', "").replace('`', "").replace('*', "");
                    while s.contains("  ") { s = s.replacen("  ", " ", 1); }
                    state = s; sc = self.g("S").into();
                }
            }
            "next" | "later" | "done" => { tm = ago(self.c.now, &upd); }
            _ => {}
        }
        let br = if !t.ap.is_empty() && !t.br.is_empty() && self.v.github { t.br.clone() } else { String::new() };
        (who, state, sc, tm, br)
    }

    // ── a card ─────────────────────────────────────────────────────────────────────────────────────────────
    /// A card of the panel, the same for all of them, three rows (see ti_card in the script): with an agent "<icon> <machine> · <agent>   <state · age>",
    /// "∟ <what it works on>", "⎇ <branch>" (plus what it does right now, the bars of a running deployment or test, and the description of a chosen card);
    /// without one the title, "∟ <who> · <state> · <age>" and the branch. No frame: the status colour is a bar, the gap between cards a blank row.
    #[allow(clippy::too_many_arguments)]
    fn card(&self, sel: bool, state_col: &str, time: &str, title_col: &str, title: &str, desc: &str, who: &str, state: &str, branch: &str, o: &Item) -> String {
        let g = |k: &str| self.g(k).to_string();
        let w = self.c.cols - 2; let cw = self.c.cols - 4;
        let (x, d_) = (g("X"), g("D"));
        let (mut tc, mut dc, mut mc, mut gc, wc) = (title_col.to_string(), g("DESC"), if state_col.is_empty() { d_.clone() } else { state_col.to_string() }, d_.clone(), g("BLUE"));
        let rm = if o.id.is_empty() { String::new() } else { format!("{}{}{}", REF, o.id, REF) };
        let mut bg = String::new(); let mut mk = String::new();   // (no ground: only the chosen card and the one under the pointer have one)
        let mut bar = if o.bar.is_empty() { if state_col.is_empty() { d_.clone() } else { state_col.to_string() } } else { o.bar.clone() };
        let mut hov = false;
        if !o.id.is_empty() && o.id == self.v.hover_id { bg = g("CARDHOV"); hov = true; }
        if hov && !sel { dc = g("DESCSEL"); gc = g("DSEL"); if mc == d_ { mc = g("DSEL"); } }
        if sel {
            mk = SELMARK.to_string(); dc = g("DESCSEL"); gc = g("DSEL"); if mc == d_ { mc = g("DSEL"); }
            tc = if tc == g("TITLE") || tc == g("L") { g("TITLESEL") } else { format!("{}{}", g("BOLD"), g("DSEL")) };
            bg = g("CARDSEL");
        }
        if hov { bar = "\u{1b}[38;2;255;111;97m".into(); }
        let bl = format!("{}▌{}{} ", bar, x, bg);
        let xb = format!("{}{}", x, bg);
        let mut out = String::new();
        if !o.agn.is_empty() {
            let mcol = if self.v.mach_fg.is_empty() || self.v.mach_fg == "none" { g("PINK") } else { self.v.mach_fg.clone() };
            let am = clip(&o.agm, 12); let amw = dw(&am);
            let lead = if am.is_empty() { 2 } else { 2 + amw + 3 };
            let (mut tm, st) = (time.to_string(), state.to_string());
            let build = |tm: &str| -> (String, String, i64) {
                let (mut p, mut c) = (String::new(), String::new());
                if !tm.is_empty() { p = tm.to_string(); c = format!("{}{}", gc, tm); }   // the age on the left, the state on the right of it, and a space behind it
                if !st.is_empty() { if !p.is_empty() { p.push_str(" · "); c.push_str(&format!("{} · ", gc)); } p.push_str(&st); c.push_str(&format!("{}{}", mc, st)); }
                if !p.is_empty() { p.push(' '); c.push(' '); }
                let rw = dw(&p); (p, c, rw)
            };
            let (mut _rtp, mut rtc, mut rw) = build(&tm);
            let mut navail = cw - lead - if rw > 0 { rw + 2 } else { 0 };
            if navail < 8 && !tm.is_empty() { tm.clear(); let r = build(&tm); _rtp = r.0; rtc = r.1; rw = r.2; navail = cw - lead - if rw > 0 { rw + 2 } else { 0 }; }
            if navail < 8 { rtc.clear(); rw = 0; navail = cw - lead; }
            let an = clip(&o.agn, navail); let anw = dw(&an);
            let amc = if am.is_empty() { String::new() } else { format!("{}{}{}{}{}{} · ", mcol, g("BOLD"), am, x, bg, gc) };
            let icon = if o.agic.is_empty() { format!("{}○", gc) } else { o.agic.clone() };
            let spk = if bg == g("CARDSEL") || bg == g("SELBG") { 's' } else if bg == g("CARDHOV") { 'h' } else { 'n' };
            let spin = if o.spin { format!("{}{}{}", SPIN, spk, SPIN) } else { String::new() };
            out.push_str(&format!("{}{}{}{}{}{} {}{}{}{}{}{}{}\n", spin, rm, mk, bg, bl, icon, amc, format!("{}{}", g("BOLD"), tc), an, xb, sp(cw - lead - anw - rw), rtc, x));
            let t2 = clip(if o.agtask.is_empty() { title } else { &o.agtask }, w - 5); let tw2 = dw(&t2);
            if !t2.is_empty() { out.push_str(&format!("{}{}{}{}∟ {}{}{}{}{}\n", rm, bg, bl, gc, tc, t2, xb, sp(w - 4 - tw2), x)); }
            if !o.now.is_empty() {
                let t3 = clip(&o.now, w - 5); let tw3 = dw(&t3);
                out.push_str(&format!("{}{}{}  {}{}{}{}{}\n", rm, bg, bl, g("ORANGE"), t3, xb, sp(w - 4 - tw3), x));
            }
            let mut brp = String::new();
            if !branch.is_empty() { brp = format!("⎇ {}", branch); }
            if !who.is_empty() { if !brp.is_empty() { brp.push_str(" · "); } brp.push_str(who); }
            if !brp.is_empty() {
                let brp = clip(&brp, w - 4); let bw = dw(&brp);
                out.push_str(&format!("{}{}{}  {}{}{}{}{}\n", rm, bg, bl, gc, brp, xb, sp(w - 4 - bw), x));
            }
            if sel && !desc.is_empty() { let d = clip(desc, w - 4); let dw2 = dw(&d); out.push_str(&format!("{}{}{}  {}{}{}{}{}\n", rm, bg, bl, dc, d, xb, sp(w - 4 - dw2), x)); }
        } else {
            let t = clip(title, w - 3); let tw = dw(&t);
            out.push_str(&format!("{}{}{}{}{}{}{}{}{}\n", rm, mk, bg, bl, tc, t, xb, sp(w - 2 - tw), x));
            if !(who.is_empty() && state.is_empty() && time.is_empty()) {
                let room = self.c.cols - 2 - 4;
                let mut who = who.to_string(); let mut st = state.to_string(); let mut tm = time.to_string();
                if !who.is_empty() { who = clip(&who, room); }
                let ww = dw(&who); let mut sw = dw(&st);
                let calc = |ww: i64, sw: i64, tm: &str| ww + sw + tm.chars().count() as i64 + (if ww > 0 && sw > 0 { 3 } else { 0 }) + (if !tm.is_empty() && ww + sw > 0 { 3 } else { 0 });
                let mut n = calc(ww, sw, &tm);
                if n > room { tm.clear(); n = ww + sw + (if ww > 0 && sw > 0 { 3 } else { 0 }); }
                if n > room {
                    st = if !who.is_empty() { clip(&st, room - ww - 3) } else { clip(&st, room) };
                    sw = dw(&st); n = ww + sw + (if ww > 0 && sw > 0 { 3 } else { 0 });
                }
                let mut line = format!("{}{}{}{}∟ ", rm, bg, bl, gc);
                if !who.is_empty() { line.push_str(&format!("{}{}", wc, who)); }
                if !who.is_empty() && !st.is_empty() { line.push_str(&format!("{} · ", gc)); }
                if !st.is_empty() { line.push_str(&format!("{}{}", mc, st)); }
                if !tm.is_empty() { if !who.is_empty() || !st.is_empty() { line.push_str(&format!("{} · ", gc)); } line.push_str(&format!("{}{}", gc, tm)); }
                line.push_str(&format!("{}{}{}{}\n", x, bg, sp(room - n), x));
                out.push_str(&line);
            }
            if !branch.is_empty() {
                let bt = clip(branch, w - 6); let bw = dw(&bt);
                out.push_str(&format!("{}{}{}{}⎇ {}{}{}{}\n", rm, bg, bl, gc, bt, xb, sp(w - 4 - bw), x));
            }
            if sel && !desc.is_empty() { let d = clip(desc, w - 3); let dw2 = dw(&d); out.push_str(&format!("{}{}{}{}{}{}{}{}\n", rm, bg, bl, dc, d, xb, sp(w - 2 - dw2), x)); }
        }
        if !o.prog.is_empty() {
            let mut it = o.prog.split(' '); let pd: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0); let pt: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let pw = w - 10;
            if pt > 0 && pw > 3 {
                let mut pf = (pd * pw + pt / 2) / pt; if pf > pw { pf = pw; }
                let (bf, be): (String, String) = ("■".repeat(pf as usize), "■".repeat((pw - pf) as usize));
                out.push_str(&format!("{}{}{}  {}{}{}{}{}{} {}{:>3}%{} {}\n", rm, bg, bl, g("ORANGE"), bf, g("MUTED"), be, x, bg, gc, pd * 100 / pt, format!("{}{}", x, bg), x));
            }
        }
        if !o.tbars.is_empty() {
            let lines: Vec<&str> = o.tbars.split('\n').collect();
            let mut tw0 = 0usize;
            for tl in &lines { let tn = tl.splitn(3, ' ').nth(2).unwrap_or(""); tw0 = tw0.max(tn.chars().count()); }
            let tw0 = tw0.min(10) as i64; let pw = w - 10 - tw0 - 1;
            for tl in &lines {
                let mut p = tl.splitn(3, ' '); let td: i64 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0); let tt: i64 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0); let tsn = p.next().unwrap_or("");
                if !(tt > 0 && pw > 3) { continue; }
                let tn = clip(tsn, tw0); let tn = format!("{:<width$}", tn, width = tw0 as usize);
                let mut pf = (td * pw + tt / 2) / tt; if pf > pw { pf = pw; }
                let (bf, be) = ("■".repeat(pf as usize), "■".repeat((pw - pf) as usize));
                out.push_str(&format!("{}{}{}  {}{} {}{}{}{} {}{:>3}%{} {}\n", rm, bg, bl, gc, tn, g("ORANGE"), bf, g("MUTED"), be, gc, td * 100 / tt, format!("{}{}", x, bg), x));
            }
        }
        out
    }

    fn line(&self, i: usize, sel_idx: i64) -> String {
        let t = &self.ts[i]; let g = |k: &str| self.g(k).to_string(); let x = g("X");
        let w = self.c.cols - 2;
        let (_, _, _, mut tm, _) = self.meta(i);
        let (who, _, _, _, _) = self.meta(i);
        let dc = match t.status.as_str() { "next" => g("S"), "waiting" => { tm = format!("{} {}", clip(who.strip_prefix("for ").unwrap_or(&who), 10), tm); g("Y") } _ => g("R") };
        let title = if t.title.is_empty() { "(untitled)".to_string() } else { t.title.clone() };
        let tt = clip(&title, w - 7 - tm.chars().count() as i64); let tw = dw(&tt); let tml = tm.chars().count() as i64;
        let (bold, lsel, dsel, selbg, cardhov, d_) = (g("BOLD"), g("LSEL"), g("DSEL"), g("SELBG"), g("CARDHOV"), g("D"));
        let xs = format!("{}{}", x, selbg); let xh = format!("{}{}", x, cardhov); let bl_ = format!("{}{}", bold, lsel);
        if i as i64 == sel_idx {
            format!("{}{}{}{}{}  {}●{} {}{}{}{}{}{}{} {}\n", REF, t.id, REF, SELMARK, selbg, dc, xs, bl_, tt, xs, sp(w - 5 - tw - tml), dsel, tm, xs, x)
        } else if t.id == self.v.hover_id {
            format!("{}{}{}{}  {}●{}{} {}{}{}{}{}{}{} {}\n", REF, t.id, REF, cardhov, dc, x, cardhov, bl_, tt, xh, sp(w - 5 - tw - tml), dsel, tm, xh, x)
        } else {
            format!("{}{}{}  {}●{} {}{}{}{}{}{}{}\n", REF, t.id, REF, dc, x, g("TITLE"), tt, x, sp(w - 5 - tw - tml), d_, tm, x)
        }
    }

    fn more_row(&self, st: &str, hidden: i64) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X"); let w = self.c.cols - 2;
        let lbl = if st == "later" { "parked" } else { st };
        let text = if self.expanded(st) { "− show less".to_string() } else { format!("+{} more {}", hidden, lbl) };
        let rf = format!("{}more:{}{}", REF, st, REF);
        if self.v.sel_id == format!("more:{}", st) { format!("{}{}{}  {}{}{}{}\n", rf, SELMARK, g("SELBG"), g("DSEL"), text, sp(w - 2 - text.chars().count() as i64), x) }
        else { format!("{}  {}{}{}\n", rf, g("D"), text, x) }
    }

    fn bored(&self) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X"); let inw = self.c.cols - 4;
        if inw < 20 || self.v.bored_small { return format!("\n  {}{}{}\n", g("D"), clip("agents are bored · n", inw), x); }
        let art = [".-----.  .-----.  zZ", "| -_- |  | u_u | z", "'--+--'  '--+--'", "  /|\\      /|\\"];
        let mut o = String::from("\n\n");
        for l in art { o.push_str(&format!("{}{}{}{}\n", sp(2 + (inw - 16) / 2), g("D"), l, x)); }
        o.push_str(&format!("\n{}{}the agents are bored{}\n", sp(2 + (inw - 20) / 2), g("L"), x));
        o.push_str(&format!("{}{}n{} {}gives them work{}\n\n", sp(2 + (inw - 17) / 2), g("S"), x, g("D"), x));
        o
    }

    /// deploy_lookup: the deployment of a waiting topic: (state, done, total, note, url, project, label)
    fn deploy_lookup(&self, t: &Topic) -> Option<(String, String, String, String, String, String, String)> {
        let list = crate::proc::read_cap(self.shared.join("deploys.list")).ok().filter(|s| !s.is_empty())?;
        let get = |k: &str| store::topic_get(&t.file, k);
        let (url, matchl) = (get("deploy_url"), get("deploy_match"));
        let since = store::num_or(&get("deploy_since"), 0);
        let pj = get("project"); let br = get("agent_branch");
        let pn = if pj.is_empty() { String::new() } else { self.projects_by_id(&pj) };
        for l in list.lines() {
            let f: Vec<&str> = l.split(US).collect(); let at = |i: usize| f.get(i).copied().unwrap_or("");
            let (tm, p, lb, st, d, tot, note, u) = (at(0), at(1), at(2), at(3), at(4), at(5), at(6), at(7));
            if !deploy_branch_ok(note, &br) { continue; }
            if !url.is_empty() { if u != url { continue; } }
            else {
                if !pn.is_empty() && p != pn { continue; }
                if !matchl.is_empty() && lb != matchl { continue; }
                if !matches!(st, "running" | "queued" | "approval") { let tt = if tm.is_empty() { 0 } else { tm.parse::<u64>().unwrap_or(u64::MAX) }; if tt < since { continue; } }
            }
            return Some((st.into(), d.into(), tot.into(), note.into(), u.into(), p.into(), lb.into()));
        }
        None
    }
    fn projects_by_id(&self, id: &str) -> String {
        // project_name <id>: by record id (proj-<slug>): the name of the record
        let f = self.c.topics.join(format!("{}.topic", id));
        let _ = &self.projects;
        let txt = crate::proc::read_cap(f).unwrap_or_default();
        let (mut name, mut del, mut kd) = (String::new(), "0".to_string(), String::new());
        for l in txt.lines() { if let Some(i) = l.find('=') { let (k, v) = (&l[..i], &l[i + 1..]); match k { "title" if name.is_empty() => name = v.into(), "deleted" => del = v.into(), "kind" => kd = v.into(), _ => {} } } }
        if kd == "project" && del != "1" { clean(&name) } else { String::new() }
    }


    // ── the done area ─────────────────────────────────────────────────────────────────────────────────────
    fn archived_count(&self) -> i64 {
        let mut n = 0;
        for i in 0..self.ts.len() {
            let t = &self.ts[i];
            if t.status == "later" && t.dm == "1" { continue; }
            if !self.in_scope(i) { continue; }
            match t.status.as_str() { "review" | "now" | "testing" | "finished" => continue, "waiting" => if t.dep == "1" { continue; }, _ => {} }
            if self.archived(i) { n += 1; }
        }
        n
    }
    fn code_of(&self, machine: &str) -> Option<&str> { self.v.mcodes.iter().find(|(n, _)| n == machine).map(|(_, c)| c.as_str()) }
    /// done_mc: the agent that did it (w3:a1, MAC/w3:a1 on another machine), only with more than one machine
    fn done_mc(&self, t: &Topic) -> String {
        if t.am.is_empty() || self.v.mcodes.len() <= 1 { return String::new(); }
        let p = &t.ap;
        let ok = p.starts_with('w') && p.len() > 1 && p.chars().nth(1).map_or(false, |c| c.is_ascii_alphanumeric()) && p.contains(":p") && p.rsplit(":p").next().map_or(false, |n| n.starts_with(|c: char| c.is_ascii_digit()));
        if ok {
            let base = format!("{}:a{}", p.split(":p").next().unwrap_or(""), p.rsplit(":p").next().unwrap_or(""));
            if !t.am.is_empty() && t.am != self.v.host { let c = self.code_of(&t.am).unwrap_or(&t.am).to_string(); return format!("{}/{}", c, base); }
            return base;
        }
        self.code_of(&t.am).unwrap_or("").to_string()
    }
    fn done_summary(&self, d: i64, a: i64) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X");
        let mut out = String::new();
        if d > 0 { out = format!("{}✓{} {}{} done{}", g("GREEN"), x, g("D"), d, x); }
        if a > 0 { if !out.is_empty() { out = format!("{} {}·{} ", out, g("D"), x); } out = format!("{}{}▤ {} archived{}", out, g("D"), a, x); }
        if out.is_empty() { String::new() } else { format!("  {}\n", out) }
    }
    /// done_block <max>
    pub fn done_block(&self, max: i64) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X"); let w = self.c.cols - 2;
        let dn: Vec<usize> = (0..self.ts.len()).filter(|&i| self.ts[i].status == "done" && self.in_scope(i) && !self.archived(i)).collect();
        let all = dn.len() as i64; let arch = self.archived_count();
        let mut out = String::new();
        let compact = self.c.cfg.get("DONE_COMPACT").map_or(false, |v| v == "on");
        if compact || max <= 0 {
            if all == 0 && arch == 0 { return out; }
            out.push_str(&crate::ui::dash_caption_section(self.c, "Done", "done")); out.push_str(&self.done_summary(all, arch)); return out;
        }
        let (mut idx, mut used, mut sel_in): (Vec<usize>, Vec<usize>, bool) = (Vec::new(), Vec::new(), false);
        while (idx.len() as i64) < max && (idx.len() as i64) < all {
            let (mut best, mut bu) = (usize::MAX, -1i64);
            for &j in &dn { if used.contains(&j) { continue; } let u = self.ts[j].upd as i64; if u >= bu { bu = u; best = j; } }
            idx.push(best); used.push(best); if self.ts[best].id == self.v.sel_id { sel_in = true; }
        }
        if !sel_in && !self.v.sel_id.is_empty() { for &j in &dn { if self.ts[j].id == self.v.sel_id { idx.push(j); break; } } }
        if idx.is_empty() { if arch == 0 { return out; } out.push_str(&crate::ui::dash_caption_section(self.c, "Done", "done")); out.push_str(&self.done_summary(0, arch)); return out; }
        out.push_str(&crate::ui::dash_caption_section(self.c, "Done", "done"));
        for &i in &idx {
            let t = &self.ts[i];
            let mut tm = ago(self.c.now, &t.upd.to_string());
            let mc = self.done_mc(t); if !mc.is_empty() { tm = format!("{} {}", mc, tm); }
            let title = if t.title.is_empty() { "(untitled)".to_string() } else { t.title.clone() };
            let tt = clip(&title, w - 7 - tm.chars().count() as i64); let tw = dw(&tt); let tml = tm.chars().count() as i64;
            let mut tc = format!("{}{}", g("BOLD"), g("D"));
            let (xs, xh) = (format!("{}{}", x, g("SELBG")), format!("{}{}", x, g("CARDHOV")));
            if t.id == self.v.sel_id {
                tc = format!("{}{}", g("BOLD"), g("DSEL"));
                out.push_str(&format!("{}{}{}{}{}  {}✓{} {}{}{}{}{}{}{} {}\n", REF, t.id, REF, SELMARK, g("SELBG"), g("GREEN"), xs, tc, tt, xs, sp(w - 5 - tw - tml), g("DSEL"), tm, xs, x));
            } else if t.id == self.v.hover_id {
                tc = format!("{}{}", g("BOLD"), g("DSEL"));
                out.push_str(&format!("{}{}{}{}  {}✓{}{} {}{}{}{}{}{}{} {}\n", REF, t.id, REF, g("CARDHOV"), g("GREEN"), x, g("CARDHOV"), tc, tt, xh, sp(w - 5 - tw - tml), g("DSEL"), tm, xh, x));
            } else {
                out.push_str(&format!("{}{}{}  {}✓{} {}{}{}{}{}{}{}\n", REF, t.id, REF, g("GREEN"), x, tc, tt, x, sp(w - 5 - tw - tml), g("D"), tm, x));
            }
        }
        let n = idx.len() as i64;
        if all > n { out.push_str(&self.more_row("done", all - n)); }
        else if all > self.v.done_max { out.push_str(&self.more_row("done", 0)); }
        out.push_str(&self.done_summary(all, arch));
        out
    }

    /// topics_block [cards|lists]
    pub fn block(&self, mode: &str) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X");
        let sel_idx = self.sel_index(); let plan = self.plan();
        let mut out = String::new();
        let open = (0..self.ts.len()).filter(|&i| Self::shows(&self.ts[i].status) && self.in_scope(i)).count();
        if mode != "lists" && self.v.scope_project && !self.v.cur_proj.is_empty() { out.push_str(&format!("\n  {}{}{} {}· its topics{}\n", g("L"), self.projects_by_id(&self.v.cur_proj), x, g("D"), x)); }
        if open == 0 && mode != "lists" { out.push_str(&self.bored()); }
        let mut prev = String::new();
        for i in 0..self.ts.len() {
            let t = &self.ts[i]; let st = t.status.as_str();
            let gk = if st == "waiting" && t.dep == "1" { "deploying" } else { st };
            if st == "done" { continue; }
            match gk { "next" | "waiting" | "later" => if mode == "cards" { continue; }, _ => if mode == "lists" { continue; } }
            if !self.in_scope(i) { continue; }
            if !plan.before[i].is_empty() { let n = match plan.before[i].as_str() { "next" => plan.n_next, "waiting" => plan.n_waiting, _ => plan.n_later }; out.push_str(&self.more_row(&plan.before[i], n - self.v.line_max)); }
            if !plan.vis[i] { continue; }
            if matches!(gk, "next" | "waiting" | "later") {
                if gk != prev {
                    let label = match st { "next" => "Next", "waiting" => "Waiting", _ => "Later" };
                    out.push('\n'); out.push_str(&crate::ui::dash_caption_pub(self.c, label)); prev = gk.to_string();
                }
                out.push_str(&self.line(i, sel_idx));
                if !plan.after[i].is_empty() { out.push_str(&self.more_row(&plan.after[i], 0)); }
                continue;
            }
            if gk != prev {
                let (label, lc) = match gk { "deploying" => ("DEPLOYING", g("Y")), "finished" => ("RECENTLY FINISHED", g("GREEN")), "now" => ("NOW", g("ORANGE")), "testing" => ("TESTING", g("C")), "review" => ("REVIEW", g("BLUE")), _ => ("", g("D")) };
                out.push('\n'); out.push_str(&format!("  {}{}{}\n", lc, label, x)); prev = gk.to_string();
            } else { out.push('\n'); }   // (the gap between two cards of a group: a blank row)
            let (mut who, mut state, mut sc, mut tm, br) = self.meta(i);
            let mut prog = String::new();
            if gk == "deploying" {
                prog.clear();
                if let Some((dst, d, tot, _n, _u, pj, lb)) = self.deploy_lookup(t) {
                    who = format!("{} · {}", pj, lb);
                    match dst.as_str() {
                        "running" => { state = "deploying".into(); sc = g("ORANGE"); if digits(&d) && digits(&tot) && tot.parse::<i64>().unwrap_or(0) > 0 { prog = format!("{} {}", d, tot); } }
                        "queued" => { state = "queued".into(); sc = g("D"); } "approval" => { state = "needs approval".into(); sc = g("Y"); }
                        "ok" => { state = "deployed".into(); sc = g("GREEN"); } "failed" => { state = "deployment failed".into(); sc = g("NN"); } _ => { state = "deployment canceled".into(); sc = g("NN"); }
                    }
                } else { state = "no deployment seen yet".into(); sc = g("D"); }
            }
            let lc = match gk { "deploying" => g("Y"), "finished" => g("GREEN"), "now" => g("ORANGE"), "testing" => g("C"), "review" => g("BLUE"), _ => g("D") };
            let _ = &mut tm;
            if st == "now" && state == "finished" && i as i64 == sel_idx { state = "finished · x = done".into(); }
            let sl = i as i64 == sel_idx;
            let bar = if st == "now" && state.starts_with("finished") { g("GREEN") } else { lc.clone() };   // (its agent has stopped: the bar is green like the text, not orange like a working one)
            let mut o = Item { id: t.id.clone(), bar, prog, ..Default::default() };
            if st == "testing" && !t.ap.is_empty() {
                let m = if t.am.is_empty() { self.host.clone() } else { t.am.clone() };
                let key: String = format!("{}_{}", m, t.ap).chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
                let tp = self.shared.join("testprog").join(key);
                let lines: Vec<String> = crate::proc::read_cap(tp).unwrap_or_default().lines().map(clean).collect();
                o.tbars = lines.join("\n");
            }
            if matches!(gk, "now" | "testing" | "finished" | "review" | "deploying") && !t.ap.is_empty() && !t.ag.is_empty() && (gk != "review" || t.sm.is_empty()) {
                o.agm = if t.am.is_empty() { self.host.clone() } else { t.am.clone() }; o.agn = t.ag.clone();
                let mut ast = self.agent_state(&o.agm, &t.ap); o.agtask = self.agent_task(&o.agm, &t.ap);
                if st == "finished" { ast = "finished".into(); }
                o.agic = match ast.as_str() {
                    "finished" => format!("{}✓", g("GREEN")), "working" | "testing" => { o.spin = true; o.now = self.agent_now(&o.agm, &t.ap); format!("{}{}", g("ORANGE"), self.v.spin_ch) }
                    "blocked" => format!("{}◉", g("NN")), "done" => format!("{}✓", g("GREEN")), s if s == "gone" || s.starts_with("off-") => format!("{}◌", g("D")), _ => format!("{}○", g("D")),
                };
                if gk != "deploying" && state != "handing over" { who.clear(); }   // (the agent's name is in the first row; who stays for where it hands over to, and the deployment's project)
            }
            let title = if t.title.is_empty() { "(untitled)".to_string() } else { t.title.clone() };
            let desc = t.desc.clone();
            out.push_str(&self.card(sl, &sc, &tm, &g("TITLE"), &title, &desc, &who, &state, &br, &o));
        }
        if mode != "lists" { out.push_str(&self.idle_cards()); }
        out
    }

    /// idle_cards: the agents that are idle and have no card (nothing in review, now, testing, deploying or recently finished): a group IDLE of their own,
    /// one card each; a click on it shows the agent (ref agent:<machine>/<pane>)
    fn idle_cards(&self) -> String {
        let g = |k: &str| self.g(k).to_string(); let x = g("X");
        let mut used: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
        for t in &self.ts {
            let card = match t.status.as_str() { "review" | "now" | "testing" | "finished" => true, "waiting" => t.dep == "1", _ => false };
            if card && !t.ap.is_empty() { used.insert((if t.am.is_empty() { self.host.clone() } else { t.am.clone() }, t.ap.clone())); }
        }
        let mut out = String::new(); let mut first = true;
        for r in &self.ag {
            if !(r.st.is_empty() || r.st == "idle") || used.contains(&(r.m.clone(), r.id.clone())) { continue; }
            if first { out.push_str(&format!("\n  {}IDLE{}\n", g("D"), x)); first = false; } else { out.push('\n'); }
            let o = Item { id: format!("agent:{}/{}", r.m, r.id), bar: g("D"), agm: r.m.clone(), agn: r.name.strip_prefix(&format!("{} / ", r.m)).unwrap_or(&r.name).to_string(), agic: format!("{}○", g("D")), agtask: r.task.clone(), ..Default::default() };
            out.push_str(&self.card(false, &g("D"), "", &g("TITLE"), "", "", "", "idle", "", &o));
        }
        out
    }
}

#[derive(Default)]
struct Item { id: String, bar: String, prog: String, tbars: String, agm: String, agn: String, agic: String, agtask: String, now: String, spin: bool }

/// deploy_lookup for the sync: the deployment a waiting topic is bound to or waits for
pub fn deploy_lookup_pub(shared: &std::path::Path, topics: &std::path::Path, names: &HashMap<String, String>, t: &Topic) -> Option<(String, String, String, String, String, String, String)> {
    let list = crate::proc::read_cap(shared.join("deploys.list")).ok().filter(|s| !s.is_empty())?;
    let get = |k: &str| store::topic_get(&t.file, k);
    let (url, matchl) = (get("deploy_url"), get("deploy_match"));
    let since = store::num_or(&get("deploy_since"), 0);
    let pj = get("project"); let br = get("agent_branch");
    let pn = if pj.is_empty() { String::new() } else { names.get(&pj).cloned().unwrap_or_default() };
    let _ = topics;
    for l in list.lines() {
        let f: Vec<&str> = l.split(US).collect(); let at = |i: usize| f.get(i).copied().unwrap_or("");
        let (tm, p, lb, st, d, tot, note, u) = (at(0), at(1), at(2), at(3), at(4), at(5), at(6), at(7));
        if !deploy_branch_ok(note, &br) { continue; }
        if !url.is_empty() { if u != url { continue; } }
        else {
            if !pn.is_empty() && p != pn { continue; }
            if !matchl.is_empty() && lb != matchl { continue; }
            if !matches!(st, "running" | "queued" | "approval") { let tt = if tm.is_empty() { 0 } else { tm.parse::<u64>().unwrap_or(u64::MAX) }; if tt < since { continue; } }
        }
        return Some((st.into(), d.into(), tot.into(), note.into(), u.into(), p.into(), lb.into()));
    }
    None
}
fn deploy_branch_ok(n: &str, b: &str) -> bool {
    if b.is_empty() || b == "main" || b == "master" || b == "develop" || b.starts_with("wip/") { return true; }
    if n.is_empty() || n == "main" || n == "master" || n == "develop" || n.starts_with('@') { return true; }
    if let Some(p) = n.strip_suffix('…') { return b.starts_with(p); }
    n == b
}
#[allow(dead_code)]
fn unused() { let _ = _abbr; }

// ── the whole main view: bhote_view ──────────────────────────────────────────────────────────────────────────

/// count_lines: the lines of a block (command substitution has cut its trailing newlines; an empty text counts as one line, as the script's read loop does)
fn count_lines(t: &str) -> i64 { t.split('\n').count() as i64 }

struct Parsed { line: String, ref_: String, spin: String }
/// view_lines: the lines without their markers (the dancing star, the topic id of the line, the selected line) and the index of the selected one
fn parse_lines(text: &str) -> (Vec<Parsed>, i64) {
    let mut out = Vec::new(); let mut sel = -1i64;
    for (n, l0) in text.split('\n').enumerate() {
        let mut l = l0.to_string(); let mut spin = String::new(); let mut rf = String::new();
        let cs: Vec<char> = l.chars().collect();
        if cs.len() >= 3 && cs[0] == SPIN && cs[2] == SPIN { spin = cs[1].to_string(); l = cs[3..].iter().collect(); }
        if l.starts_with(REF) { let r: String = l.chars().skip(1).collect(); let i = r.find(REF).unwrap_or(r.len()); rf = r[..i].to_string(); l = r[(i + REF.len_utf8()).min(r.len())..].to_string(); }
        if l.starts_with(SELMARK) { sel = n as i64; l = l.chars().skip(1).collect(); }
        out.push(Parsed { line: l, ref_: rf, spin });
    }
    (out, sel)
}

struct Vp { row: i64, map: String, spin: String }
pub struct Out { pub frame: String, pub tmap: String, pub spin: String, pub divrow: i64, pub toff: Option<i64> }
pub struct Layout<'a> { pub cards: &'a Cards<'a>, pub rows: i64, pub head: String, pub bottom: String, pub run_dir: PathBuf, pub prev_toff: Option<i64> }
impl<'a> Layout<'a> {
    /// bhote_view: head on top, the topics below it (scrolling with the selection), the agents and the keys at the bottom edge. The click maps go to
    /// the run folder (tmap, spinrows, divrow, toff), as the script writes them.
    /// The same, written to the run folder as the script does (tmap, spinrows, divrow, toff)
    pub fn view(&self) -> String {
        let o = self.view_out();
        let toff = self.run_dir.join("toff");
        match o.toff { Some(n) => { let _ = fs::write(&toff, format!("{}\n", n)); } None => { let _ = fs::remove_file(&toff); } }
        let _ = fs::write(self.run_dir.join("tmap"), &o.tmap);
        let _ = fs::write(self.run_dir.join("spinrows"), &o.spin);
        let _ = fs::write(self.run_dir.join("divrow"), format!("{}\n", o.divrow));
        o.frame
    }
    pub fn view_out(&self) -> Out {
        let c = self.cards; let g = |k: &str| c.g(k).to_string();
        let rows = self.rows;
        let trim = |s: String| s.trim_end_matches('\n').to_string();
        let head = trim(self.head.clone()); let bottom = trim(self.bottom.clone());
        let mut topics = trim(c.block("cards")); let lists = trim(c.block("lists"));
        let (hh, mut th, bh) = (count_lines(&head), count_lines(&topics), count_lines(&bottom));
        let mut dn = c.v.done_max; if c.expanded("done") { dn = c.done_total(); }
        let mut dblk = trim(c.done_block(dn)); let mut dh = if dblk.is_empty() { 0 } else { count_lines(&dblk) };
        while dh > 0 && dn > 0 && rows - hh - bh - dh < th.min(6) { dn -= 1; dblk = trim(c.done_block(dn)); dh = if dblk.is_empty() { 0 } else { count_lines(&dblk) }; }
        let mut mblk = trim(crate::ui::monitors(c.c)); let mut mh = if mblk.is_empty() { 0 } else { count_lines(&mblk) };
        if mh > 0 && rows - hh - bh - dh - mh < th.min(6) { mblk.clear(); mh = 0; }
        let lh = if lists.is_empty() { 0 } else { count_lines(&lists) };
        let mut lcut = lh; let need = th.min(6);
        if rows - hh - bh - dh - mh - lcut < need { lcut = rows - hh - bh - dh - mh - need; if lcut < 0 { lcut = 0; } }
        let avail = rows - hh - bh - dh - mh - lcut;
        if topics.contains("the agents are bored") && th > avail { topics = trim(c.block_with("cards", true)); th = count_lines(&topics); }
        let mut out = format!("{}\n", head);
        let mut vp = Vp { row: hh, map: String::new(), spin: String::new() };
        fn put(vp: &mut Vp, p: &[Parsed], from: i64, to: i64, out: &mut String) {
            let mut i = from;
            while i < to { if let Some(l) = p.get(i as usize) {
                out.push_str(&l.line); out.push('\n'); vp.row += 1;
                if !l.ref_.is_empty() { vp.map.push_str(&format!("{}\u{1e}{}\n", vp.row, l.ref_)); }
                if !l.spin.is_empty() { vp.spin.push_str(&format!(" {}:3:{}", vp.row, l.spin)); }
            } else { out.push('\n'); vp.row += 1; } i += 1; }
        }
        let (tl, mut sel) = parse_lines(&topics); let n = tl.len() as i64;
        let mut pad = 0; let mut toff_out: Option<i64> = None;
        let prev_file: Option<i64> = crate::proc::read_cap(self.run_dir.join("toff")).ok().and_then(|t| t.lines().next().map(|x| x.to_string())).filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())).and_then(|v| v.parse().ok());
        let prev_state = if self.run_dir.as_os_str().is_empty() { self.prev_toff } else { prev_file };
        if th <= avail { put(&mut vp, &tl, 0, n, &mut out); pad = avail - th; }
        else if avail >= 4 {
            let content = avail - 1;
            if sel == -1 { sel = 0; }
            let mut sh = 1; while sh < 4 && sel + sh < n && !tl.get((sel + sh) as usize).map_or(true, |p| p.line.is_empty()) { sh += 1; }
            let prev: i64 = prev_state.unwrap_or(-1);
            let mut off;
            if prev < 0 { off = sel + sh - content; }
            else if sel < prev { off = sel; while off > 0 && tl.get((off - 1) as usize).map_or(false, |p| p.ref_.is_empty()) { off -= 1; } }
            else if sel + sh > prev + content { off = sel + sh - content; }
            else { off = prev; }
            if off > sel { off = sel; } if off < 0 { off = 0; } if off > th - content { off = th - content; }
            toff_out = Some(off);
            put(&mut vp, &tl, off, off + content, &mut out);
            out.push_str(&format!(" {}↑↓ {} more lines{}\n", g("D"), th - content, g("X"))); vp.row += 1;
        } else { put(&mut vp, &tl, 0, n.min(avail), &mut out); }
        for _ in 0..pad.max(0) { out.push('\n'); } vp.row += pad.max(0);
        if lcut > 0 { let (p, _) = parse_lines(&lists); put(&mut vp, &p, 0, lcut, &mut out); }
        if dh > 0 { let (p, _) = parse_lines(&dblk); put(&mut vp, &p, 0, p.len() as i64, &mut out); }
        if mh > 0 { let (p, _) = parse_lines(&mblk); put(&mut vp, &p, 0, p.len() as i64, &mut out); }
        out.push_str(&bottom);
        Out { frame: out, tmap: vp.map, spin: vp.spin, divrow: rows - bh + 1, toff: toff_out }
    }
}

impl<'a> Cards<'a> {
    /// topics_block with the small "bored" line (BORED_SMALL=1): the frame has no room for the agents
    pub fn block_with(&self, mode: &str, bored_small: bool) -> String {
        if !bored_small { return self.block(mode); }
        let v2 = View { sel_id: self.v.sel_id.clone(), hover_id: self.v.hover_id.clone(), expanded: self.v.expanded.clone(), scope_project: self.v.scope_project, cur_proj: self.v.cur_proj.clone(), spin_ch: self.v.spin_ch.clone(),
            bored_small: true, archive_days: self.v.archive_days, github: self.v.github, host: self.v.host.clone(), mach_fg: self.v.mach_fg.clone(), line_max: self.v.line_max, done_max: self.v.done_max, mcodes: self.v.mcodes.clone(),
            rows: self.v.rows, head_rows: self.v.head_rows, hover_btn: self.v.hover_btn, flash: self.v.flash.clone(), remote_on: self.v.remote_on, has_machines: self.v.has_machines, agent_rows: self.v.agent_rows, pane_focus: self.v.pane_focus, head_note: self.v.head_note.clone(), newfeat: self.v.newfeat, version_label: self.v.version_label.clone(), head_grad: self.v.head_grad.clone() };
        let c2 = Cards { c: self.c, v: &v2, ts: self.ts.clone(), epj: self.epj.clone(), ag: rows(&self.shared.join("agents.local")).into_iter().chain(rows(&self.shared.join("agents.remote"))).collect(), now_l: rows(&self.shared.join("agents.now.local")), now_r: rows(&self.shared.join("agents.now.remote")), projects: Vec::new(), shared: self.shared.clone(), host: self.host.clone(), rows_total: self.rows_total };
        c2.block(mode)
    }
}
