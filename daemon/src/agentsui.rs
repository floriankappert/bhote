//! The head, the agents and the keys line of the main view (step 6): view_head, agents_block, agent_divider, agent_footer, view_bottom,
//! and the machine codes (machine_codes_load, machine_code_cands) of the script, byte for byte.
use crate::cards::{Cards, View};
use crate::ui::{clip, coll, dash_caption_section, dw, Ctx};
use std::fs;

const US: char = '\u{1f}';

fn pre(s: &str, n: usize) -> String { s.chars().take(n).collect() }
fn sp(n: i64) -> String { " ".repeat(n.unsigned_abs() as usize) }

// ── machine codes ───────────────────────────────────────────────────────────────────────────────────────────
fn cands(name: &str) -> Vec<String> {
    let w: String = { let mut s = String::new(); let mut sepa = false; for c in name.chars() { if c.is_ascii_alphabetic() { s.push(c); sepa = false; } else if !sepa { s.push(' '); sepa = true; } } s };
    let wc: Vec<char> = w.chars().collect();
    let mut o = String::new();
    for (i, c) in wc.iter().enumerate() { if c.is_ascii_uppercase() && i > 0 && wc[i - 1].is_ascii_lowercase() { o.push(' '); } o.push(*c); }
    let words: Vec<&str> = o.split_whitespace().collect(); let n = words.len();
    let mut l: String = o.to_uppercase().chars().filter(|c| *c != ' ').collect();
    if l.is_empty() { l = "M".into(); }
    while l.chars().count() < 3 { l.push('X'); }
    let lc: Vec<char> = l.chars().collect(); let f = lc[0];
    let vow = |c: char| "AEIOU".contains(c);
    let mut out: Vec<String> = Vec::new();
    let mut emit = |x: String| { if x.chars().count() == 3 && !out.contains(&x) { out.push(x); } };
    if lc.len() == 3 { emit(l.clone()); }
    if n >= 3 { emit(words.iter().take(3).map(|w| w.chars().next().unwrap_or(' ').to_ascii_uppercase()).collect()); }
    if n == 2 {
        let u: Vec<char> = words[1].to_uppercase().chars().collect(); let mut c = String::new();
        for i in 1..u.len() { if !vow(u[i]) { c = u[i].to_string(); break; } }
        if c.is_empty() { c = u.get(1).map(|x| x.to_string()).unwrap_or_default(); }
        emit(format!("{}{}{}", f, u.first().copied().unwrap_or(' '), c));
    }
    let mut c = String::new(); let mut i = 1; while i < lc.len() && c.chars().count() < 2 { if !vow(lc[i]) { c.push(lc[i]); } i += 1; }
    emit(format!("{}{}", f, c));
    emit(lc.iter().take(3).collect());
    for i in 1..lc.len() { for j in i + 1..lc.len() { emit(format!("{}{}{}", f, lc[i], lc[j])); } }
    let a: Vec<char> = ('A'..='Z').collect();
    for &x in &a { for &y in &a { emit(format!("{}{}{}", f, x, y)); } }
    out
}
fn code_ok(c: &str) -> bool { c.chars().count() == 3 && c.chars().all(|x| x.is_ascii_uppercase()) }

/// machine_codes_load: (name, code) for this machine, the machines of the agents and the machine records
pub fn machine_codes(topics: &std::path::Path, host: &str, agent_machines: &[String]) -> Vec<(String, String)> {
    let mut recs: Vec<(String, String)> = Vec::new();
    let mut files: Vec<_> = fs::read_dir(topics).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); n.starts_with("mach-") && n.ends_with(".topic") })).collect()).unwrap_or_default();
    files.sort();
    for f in files {
        let (mut t, mut cd, mut kd, mut del) = (String::new(), String::new(), String::new(), "0".to_string());
        for l in crate::proc::read_cap(&f).unwrap_or_default().lines() { if let Some(v) = l.strip_prefix("title=") { if t.is_empty() { t = v.to_string(); } } else if let Some(v) = l.strip_prefix("code=") { cd = v.to_string(); } else if let Some(v) = l.strip_prefix("kind=") { kd = v.to_string(); } else if let Some(v) = l.strip_prefix("deleted=") { del = v.to_string(); } }
        let t: String = t.chars().filter(|c| !c.is_control()).collect();
        if kd == "machine" && del != "1" && !t.is_empty() { recs.push((t, cd)); }
    }
    let mut names: Vec<String> = vec![host.to_string()];
    names.extend(recs.iter().map(|r| r.0.clone()));
    for m in agent_machines { let m: String = m.chars().filter(|c| !c.is_control()).collect(); if !m.is_empty() { names.push(m); } }
    names.sort_by(|a, b| coll(a, b)); names.dedup();
    let mut codes: Vec<(String, String, bool)> = names.iter().map(|n| (n.clone(), String::new(), false)).collect();
    let mut used: Vec<String> = Vec::new();
    for e in codes.iter_mut() { for (rn, rc) in &recs { if *rn == e.0 && code_ok(rc) && !used.contains(rc) { e.1 = rc.clone(); e.2 = true; used.push(rc.clone()); } } }
    for e in codes.iter_mut() { if !e.1.is_empty() { continue; } for c in cands(&e.0) { if !used.contains(&c) { e.1 = c.clone(); used.push(c); break; } } }
    codes.into_iter().map(|(n, c, _)| (n, c)).collect()
}

// ── the head ────────────────────────────────────────────────────────────────────────────────────────────────
pub struct HeadView { pub label: String, pub pane_focus: Option<bool>, pub head_grad: String, pub note: String, pub newfeat: bool }

pub fn head(c: &Ctx, h: &HeadView) -> String {
    let g = |k: &str| c.c.g(k).to_string();
    let name = format!("Bhote v{}", h.label);
    let nl = name.chars().count() as i64;
    let t = clip(" - Herd Management", c.cols - 5 - nl - 2);
    let grad = if h.head_grad.is_empty() { format!("{}{}{}", g("L"), name, g("X")) } else { h.head_grad.clone() };
    let mut out = match h.pane_focus {
        Some(true) => { let lt = dw(&t); format!("\n  {}{}{}{}{}{}●{}\n", grad, g("D"), t, g("X"), sp(c.cols - 5 - nl - lt), g("GREEN"), g("X")) }
        Some(false) => format!("\n  {}{}{}{}\n", g("D"), name, t, g("X")),
        None => format!("\n  {}{}{}{}\n", grad, g("D"), t, g("X")),
    };
    if !h.note.is_empty() { out.push_str(&format!("  {}{}{}\n", g("GREEN"), clip(&h.note, c.cols - 4), g("X"))); }
    if h.newfeat { out.push_str(&format!("  {}{}{}\n", g("ORANGE"), clip("✦ New Features Available · w Start Wizard", c.cols - 4), g("X"))); }
    out.push_str(&format!("  {}{}{}\n", g("B"), "─".repeat((c.cols - 5).max(0) as usize), g("X")));
    out
}

// ── the agents ──────────────────────────────────────────────────────────────────────────────────────────────
pub struct AgentView { pub head_rows: i64, pub hover_btn: bool, pub flash: String, pub remote_on: bool, pub has_machines: bool, pub agent_rows: Option<i64>, pub name_colors: String, pub project_colors: String, pub offline: Vec<String> }
pub struct Bottom { pub text: String, pub agrows: String, pub agmap: String }

fn color_by(rules: &str, name: &str, default: &str, c: &Ctx) -> String {
    let ln = name.to_lowercase();
    for r in rules.split(',') {
        let (p, col) = match r.find('=') { Some(i) => (&r[..i], &r[i + 1..]), None => (r, r) };
        if p.is_empty() { continue; }
        if ln.contains(&p.to_lowercase()) {
            let k = match col { "red" => "NN", "teal" => "S", "peach" => "C", "mauve" => "R", "blue" => "BLUE", "green" => "GREEN", "yellow" => "Y", "pink" => "PINK", "orange" => "ORANGE", "grey" | "gray" => "D", _ => return default.to_string() };
            return c.c.g(k).to_string();
        }
    }
    default.to_string()
}

impl<'a> Cards<'a> {
    /// agents_block + the divider and the footer: the agents by project, two columns
    pub fn agents_block(&self, av: &AgentView, mcodes: &[(String, String)]) -> (String, String, String) {
        let g = |k: &str| self.c.c.g(k).to_string(); let x = g("X");
        let cols = self.c.cols; let rows = self.rows_total;
        let mut maxr = rows - 21 - av.head_rows; if maxr < 1 { maxr = 1; }
        let want = av.agent_rows;
        if let Some(n) = want { let mx = (rows - 3 - 4 - 4).max(1); maxr = n.max(1).min(mx).max(1); }
        let cw = (cols - 4) / 2;
        let mut out = crate::ui::dash_caption_agents(self.c, av.hover_btn);
        let footer = if !av.remote_on && av.has_machines { format!("\n  {}{}{}\n", g("D"), clip("Remote agents off · , settings", cols - 4), x) } else { String::new() };
        let compact = self.c.cfg.get("AGENTS_COMPACT").map_or(false, |v| v == "on");
        let ag = self.agent_rows();
        if compact && !ag.is_empty() {
            let (mut act, mut needs, mut inact) = (0, 0, 0);
            for r in ag { match r.st.trim_start_matches("off-") { "working" | "testing" => act += 1, "blocked" => needs += 1, "idle" | "done" | "" => {} _ => inact += 1 } }   // (an idle agent is on a card: its own, or in the group IDLE)
            let (mut ac, mut ab) = (g("ORANGE"), format!("{}{}", g("BOLD"), g("ORANGE"))); if act == 0 { ac = g("D"); ab = g("D"); }
            out.push_str(&format!("  {}✶{} {}{} active{}", ac, x, ab, act, x));
            if needs > 0 { out.push_str(&format!(" {}·{} {}◉ {} need you{}", g("D"), x, g("NN"), needs, x)); }
            if inact > 0 { out.push_str(&format!(" {}·{} {}○ {} inactive{}", g("D"), x, g("D"), inact, x)); }
            out.push('\n');
            out.push_str(&footer); return (out, "1\n".into(), "0\n".into());
        }
        if ag.is_empty() { out.push_str(&footer); return (out, "1\n".into(), "0\n".into()); }
        // each agent's project, the machines, the projects in their order
        let metas = self.metas();
        let apj: Vec<String> = ag.iter().map(|r| metas.iter().find(|m| m.0 == r.m && m.1 == r.id).map(|m| m.2.clone()).unwrap_or_default()).collect();
        let host = &self.host;
        let mut machines: Vec<String> = vec![host.clone()]; let mut hosted = false;
        for r in ag { if !machines.contains(&r.m) { machines.push(r.m.clone()); } if r.m == *host { hosted = true; } }
        if !hosted { machines.remove(0); }
        let nm = machines.len();
        let projs = &self.projects;
        let pname_of = |id: &str| projs.iter().find(|p| p.id == id).map(|p| p.name.clone());
        let mut order: Vec<String> = Vec::new();
        if !self.v.cur_proj.is_empty() { order.push(self.v.cur_proj.clone()); }
        for p in projs { if p.id != self.v.cur_proj { order.push(p.id.clone()); } }
        order.push("-".into());
        let code_of = |m: &str| mcodes.iter().find(|(n, _)| n == m).map(|(_, c)| c.clone());
        let (mut lines, mut lc, mut lid, mut rid): (Vec<String>, Vec<i64>, Vec<String>, Vec<String>) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let grouped = self.c.cfg("CARD_GROUPING", "ungrouped") == "status";
        let idle = |i: usize| -> bool { match ag[i].st.as_str() { "" | "idle" | "done" => true, "working" | "testing" | "blocked" => !grouped, _ => false } };   // (an idle agent is on a card: its own, or in the group IDLE; here only the others)
        let in_pm = |pm: &str, i: usize| -> bool { if idle(i) { return false; } if pm == "-" { pname_of(&apj[i]).is_none() } else { apj[i] == pm } };
        let rule_len = (cols - 5).max(0);
        for pm in &order {
            let cnt = (0..ag.len()).filter(|&i| in_pm(pm, i)).count();
            if cnt == 0 { continue; }
            let mut pname = if pm == "-" { "no project".to_string() } else { pname_of(pm).unwrap_or_default() };
            let mut pmach: Vec<&String> = Vec::new();
            for mm in &machines { for i in 0..ag.len() { if ag[i].m != *mm { continue; } if !in_pm(pm, i) { continue; } pmach.push(mm); break; } }
            for mm in &pmach { if av.offline.contains(mm) { let cc = code_of(mm).unwrap_or_default(); pname = format!("{} · {} ◐ reconnecting", pname, if cc.is_empty() { mm.to_string() } else { cc }); } }
            let cap = clip(&pname, rule_len - 6); let cw2 = dw(&cap); let rest2 = (rule_len - 4 - cw2).max(1);
            if !lines.is_empty() { lines.push(String::new()); lc.push(0); lid.push(String::new()); rid.push(String::new()); }
            let pcol = if pm == "-" { g("D") } else { color_by(&av.project_colors, &pname, &g("D"), self.c) };
            let dash: String = "┈".repeat(rule_len as usize);
            lines.push(format!("  {}┈┈{} {}{}{}{} {}{}{}", g("MUTED"), x, g("BOLD"), pcol, cap, x, g("MUTED"), pre(&dash, rest2 as usize), x));
            lc.push(0); lid.push(String::new()); rid.push(String::new());
            let (mut cells, mut cid, mut cwid): (Vec<String>, Vec<String>, Vec<i64>) = (Vec::new(), Vec::new(), Vec::new());
            for rk in 0..3 {
                for mm in &machines {
                    let pre0 = if nm > 1 { code_of(mm).unwrap_or_default() } else { String::new() };
                    for i in 0..ag.len() {
                        let r = &ag[i]; if r.m != *mm || !in_pm(pm, i) { continue; }
                        let (mut pr, mut plen) = (String::new(), 0i64);
                        if !pre0.is_empty() { pr = self.pane_code(&r.m, &r.id, mcodes); if !pr.is_empty() { pr.push('|'); plen = dw(&pr); } }
                        let raw = r.st.clone(); let mut off = false; let mut st = r.st.as_str();
                        if let Some(s) = st.strip_prefix("off-") { off = true; st = s; }
                        let rkst = match st { "working" | "testing" => 0, "blocked" => 1, _ => 2 };
                        if rkst != rk { continue; }
                        let mut ic = match st { "working" | "testing" => format!("{}{}", g("ORANGE"), self.v.spin_ch), "blocked" => format!("{}◉", g("NN")), _ => format!("{}○", g("D")) };
                        let mut name = r.name.clone();
                        if !pr.is_empty() { if let Some(n) = name.strip_prefix(&format!("{} / ", mm)) { name = n.to_string(); } }
                        let mut nc = match st { "working" | "testing" => g("ORANGE"), "blocked" => g("NN"), _ => g("D") };
                        let label = clip(&name, cw - 3 - plen);
                        if off { ic = format!("{}{}", g("MUTED"), match st { "working" | "testing" => "✶", "blocked" => "◉", "done" => "✓", _ => "○" }); nc = g("MUTED"); }
                        let plain = dw(&label);
                        let mut cell = format!("{}{} {}{}{}{}", ic, x, if pr.is_empty() { String::new() } else { format!("{}{}{}", g("D"), pr, x) }, nc, label, x);
                        let mut cwv = plain + plen + 2;
                        let tt = self.ts().iter().find(|t| t.ap == r.id && t.am == r.m && matches!(t.status.as_str(), "now" | "testing" | "review" | "waiting" | "next")).map(|t| t.title.clone()).unwrap_or_default();
                        if !tt.is_empty() && cw - 1 - cwv >= 5 {
                            let tw = (cw - 1 - cwv).min(7); let tt = clip(&tt, tw); let tw = dw(&tt);
                            cell = format!("{} {}{}{}", cell, g("D"), tt, x); cwv += 1 + tw;
                        }
                        cells.push(cell); cwid.push(cwv); cid.push(format!("{}{}{}{}{}", r.m, US, r.id, US, raw));
                    }
                }
            }
            let nc2 = cells.len(); let mut r = 0;
            while r < nc2 {
                let second = if r + 1 < nc2 { cells[r + 1].clone() } else { String::new() };
                lines.push(format!("  {}{} {}", cells[r], sp(cw - cwid[r]), second));
                lc.push(1 + (r + 1 < nc2) as i64); lid.push(cid[r].clone()); rid.push(cid.get(r + 1).cloned().unwrap_or_default());
                r += 2;
            }
        }
        let n = lines.len() as i64; let shown = n.min(maxr);
        let agrows = format!("{}\n", if want.is_some() { maxr } else { shown });
        let mut agmap = format!("{}\n", cw + 3);
        for r in 0..shown as usize { agmap.push_str(&format!("{}\u{1e}{}\u{1e}{}\n", r, lid[r], rid[r])); }
        let mut hidden = 0;
        for r in 0..n as usize { if (r as i64) < shown { out.push_str(&lines[r]); out.push('\n'); } else { hidden += lc[r]; } }
        if hidden > 0 { out.push_str(&format!("  {}+{} more agents{}\n", g("D"), hidden, x)); }
        else if want.is_some() { for _ in shown..maxr { out.push('\n'); } }
        out.push_str(&footer);
        (out, agrows, agmap)
    }

    /// agent_pane_code_set: w3:a1 (pane w3:p1), MAC/w3:a1 on another machine; "" when the pane is not of that kind
    fn pane_code(&self, m: &str, p: &str, mcodes: &[(String, String)]) -> String {
        let ok = p.starts_with('w') && p.chars().nth(1).map_or(false, |c| c.is_ascii_alphanumeric()) && p.contains(":p") && p.rsplit(":p").next().map_or(false, |n| n.starts_with(|c: char| c.is_ascii_digit()));
        if !ok { return String::new(); }
        let b = format!("{}:a{}", p.split(":p").next().unwrap_or(""), p.rsplit(":p").next().unwrap_or(""));
        if !m.is_empty() && m != self.host { let c = mcodes.iter().find(|(n, _)| n == m).map(|(_, c)| c.clone()).unwrap_or_else(|| m.to_string()); return format!("{}/{}", c, b); }
        b
    }
}

/// view_bottom: the agents, a dash line and the keys (or a short note instead)
pub fn bottom(cards: &Cards, av: &AgentView, mcodes: &[(String, String)]) -> Bottom {
    let g = |k: &str| cards.c.c.g(k).to_string(); let x = g("X");
    let (mut text, agrows, agmap) = cards.agents_block(av, mcodes);
    text.push_str(&format!("  {}{}{}\n", g("MUTED"), "┈".repeat((cards.c.cols - 5).max(0) as usize), x));
    let room = cards.c.cols - 4;
    if !av.flash.is_empty() { text.push_str(&format!("  {}{}{}", g("S"), clip(&av.flash, room), x)); }
    else { text.push_str(&format!("  {}{}{}", g("D"), clip(", settings", room), x)); }
    Bottom { text, agrows, agmap }
}
#[allow(dead_code)]
fn unused(_: &View) { let _ = dash_caption_section; }

impl View {
    pub fn head_view(&self) -> HeadView { HeadView { label: self.version_label.clone(), pane_focus: self.pane_focus, head_grad: self.head_grad.clone(), note: self.head_note.clone(), newfeat: self.newfeat } }
}
pub fn agent_view(v: &View, cfg: &std::collections::HashMap<String, String>, shared: &std::path::Path) -> AgentView {
    let offline: Vec<String> = crate::proc::read_cap(shared.join("machines.offline")).unwrap_or_default().lines().map(|l| l.to_string()).collect();
    AgentView { head_rows: v.head_rows, hover_btn: v.hover_btn, flash: v.flash.clone(), remote_on: v.remote_on, has_machines: v.has_machines, agent_rows: v.agent_rows,
        name_colors: cfg.get("NAME_COLORS").cloned().unwrap_or_default(), project_colors: cfg.get("PROJECT_COLORS").cloned().unwrap_or_default(), offline }
}
