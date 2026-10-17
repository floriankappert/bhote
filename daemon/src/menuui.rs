//! The topic's page (menu) and the chooser of free agents in Rust (step 6b): bhote_menu, menu_build, bhote_pick, pick_build, with the
//! same bytes as the script (the markers for the clicks and the selected line included). Topics with a Slack pin or answer stay with the script's view.
use crate::cards::Row;
use crate::store::{self, topic_get};
use crate::ui::{clip, dw, Ctx, Project};
use std::fs;
use std::path::Path;

const REF: char = '\u{1d}';
const SELMARK: char = '\u{1}';
const US: char = '\u{1f}';

fn sp(n: i64) -> String { " ".repeat(n.unsigned_abs() as usize) }

/// wrap_print <indent> <colour> <text>: the lines of the text, in words, each with the colour
pub fn wrap_print(c: &Ctx, ind: i64, col: &str, text: &str) -> Vec<String> {
    let width = c.cols - 2 - ind; let x = c.c.g("X");
    let pad = sp(ind); let mut out = Vec::new(); let (mut line, mut lw) = (String::new(), 0i64);
    for w0 in text.split_whitespace() {
        let mut w = w0.to_string(); let mut ww = dw(&w);
        if ww > width { w = clip(&w, width); ww = dw(&w); }
        if line.is_empty() { line = w; lw = ww; }
        else if lw + 1 + ww <= width { line = format!("{} {}", line, w); lw += 1 + ww; }
        else { out.push(format!("{}{}{}{}", pad, col, line, x)); line = w; lw = ww; }
    }
    if !line.is_empty() { out.push(format!("{}{}{}{}", pad, col, line, x)); }
    out
}

/// What the menu can do with a topic (menu_build): (key, label)
pub fn menu_items(file: &Path, projects: &[Project], project_name: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let st = { let s = topic_get(file, "status"); if s.is_empty() { "now".to_string() } else { s } }; let ag = topic_get(file, "agent");
    let mut v: Vec<(String, String)> = Vec::new();
    let mut add = |k: &str, l: String| v.push((k.to_string(), l));
    let proj_label = |file: &Path| format!("Project: {} (⏎ next)", project_name(&topic_get(file, "project")).unwrap_or_else(|| "none".into()));
    if st == "done" {
        add("reopen", "Reopen: work on it again".into()); add("next", "Up next".into()); add("later", "Park for later".into());
        if !projects.is_empty() { add("project", proj_label(file)); }
        add("rename", "Rename".into()); add("describe", "Describe ...".into()); add("delete", "Delete".into()); add("back", "Back".into());
        return v;
    }
    let pane = topic_get(file, "agent_pane");
    if !topic_get(file, "transfer_to").is_empty() { add("cancelxfer", format!("Cancel the hand-over to {}", topic_get(file, "transfer_to_name"))); }
    if !ag.is_empty() && !pane.is_empty() { add("jump", "Show topic".into()); }
    match st.as_str() {
        "review" => { add("now", "Back to now: work goes on".into()); add("dismiss", "Dismiss: close the card".into()); add("done", "Mark done".into()); }
        "finished" => { add("dismiss", "Dismiss: close the card".into()); add("done", "Mark done".into()); add("now", "Back to now: work goes on".into()); }
        "now" | "testing" => { add("review", "Ready for review".into()); add("done", "Mark done".into()); }
        "waiting" => { if !ag.is_empty() && !pane.is_empty() { add("resume", format!("The wait is over: {} goes on", ag)); } else { add("now", "The wait is over: back to now".into()); } add("done", "Mark done".into()); }
        "next" | "later" => { add("now", "Work on it now".into()); add("done", "Mark done".into()); }
        _ => {}
    }
    if pane.is_empty() { add("start", "Start with agent ...".into()); }
    if !ag.is_empty() && !pane.is_empty() { add("transfer", "Transfer to another agent ...".into()); }
    if st == "waiting" { add("wait", "Change who or what is waited for ...".into()); } else { add("wait", "Waiting for ...".into()); }
    if !(st == "waiting" && topic_get(file, "deploy_wait") == "1") { add("deploy", "Waiting for the deployment".into()); }
    if st != "next" { add("next", "Up next".into()); }
    if st != "later" { add("later", "Park for later".into()); }
    if !projects.is_empty() { add("project", proj_label(file)); }
    if !topic_get(file, "slack_channel").is_empty() { add("slack", format!("Slack: unpin {}", topic_get(file, "slack_label"))); } else { add("slack", "Slack: pin a channel ...".into()); }
    add("rename", "Rename".into()); add("describe", "Describe ...".into()); add("delete", "Delete".into()); add("back", "Back".into());
    v
}

fn ago(now: u64, v: &str) -> String {
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) { return "? ago".into(); }
    let d = (now as i64 - v.parse::<i64>().unwrap_or(0)).max(0);
    if d < 60 { "just now".into() } else if d < 3600 { format!("{}m ago", d / 60) } else if d < 172800 { format!("{}h ago", d / 3600) } else { format!("{}d ago", d / 86400) }
}

fn field_block(c: &Ctx, which: &str, col: &str, text: &str, idx: i64) -> String {
    let g = |k: &str| c.c.g(k).to_string(); let x = g("X"); let w = c.cols - 2;
    let sel = (which == "title" && idx == -2) || (which == "description" && idx == -1);
    let mut mk = if sel { SELMARK.to_string() } else { String::new() };
    let rm = if which == "title" { format!("{}-2{}", REF, REF) } else { format!("{}-1{}", REF, REF) };
    let mut out = String::new();
    let lines = { let l = wrap_print(c, 2, "", text); if l.is_empty() { vec![String::new()] } else { l } };
    for l in lines {
        let l = l.replace(&x, "");
        if sel { let lw = dw(&l); out.push_str(&format!("{}{}{}{}{}{}{}\n", rm, mk, g("SELBG"), g("LSEL"), l, sp(w - lw), x)); mk.clear(); }
        else { out.push_str(&format!("{}{}{}{}\n", rm, col, l, x)); }
    }
    out
}

fn num_line(c: &Ctx, sel: bool, n: usize, text: &str) -> String {
    let g = |k: &str| c.c.g(k).to_string(); let x = g("X"); let w = c.cols - 2;
    let t = clip(text, w - 6);
    if sel { format!("{}  {}{:<2}{}{} {}{}{}{}\n", g("SELBG"), g("C"), n, x, g("SELBG"), g("LSEL"), t, sp(w - 5 - t.chars().count() as i64), x) }
    else { format!("  {}{:<2}{} {}{}{}\n", g("C"), n, x, g("L"), t, x) }
}

fn rule(c: &Ctx) -> String { format!("  {}{}{}\n", c.c.g("B"), "─".repeat((c.cols - 5).max(0) as usize), c.c.g("X")) }

/// Does the script have to draw this topic's page (a Slack pin or answer)?
pub fn needs_script(file: &Path) -> bool { !topic_get(file, "slack_channel").is_empty() || !topic_get(file, "slack_msg").is_empty() }

pub struct MenuIn<'a> { pub c: &'a Ctx, pub file: &'a Path, pub idx: i64, pub items: &'a [(String, String)], pub project_name: &'a dyn Fn(&str) -> Option<String>, pub github: bool, pub flash: &'a str, pub now: u64 }

/// bhote_menu: the page, with its markers
pub fn render_menu(m: &MenuIn) -> String {
    let c = m.c; let g = |k: &str| c.c.g(k).to_string(); let x = g("X");
    let f = m.file; let st = { let s = topic_get(f, "status"); if s.is_empty() { "now".to_string() } else { s } };
    let (wf, ws, ag, am) = (topic_get(f, "waiting_for"), topic_get(f, "waiting_since"), topic_get(f, "agent"), topic_get(f, "agent_machine"));
    let sc = match st.as_str() { "now" => g("ORANGE"), "testing" => g("C"), "finished" => g("GREEN"), "next" => g("S"), "waiting" => g("Y"), "review" => g("BLUE"), "later" => g("R"), "done" => g("GREEN"), _ => g("D") };
    let mut out = format!("\n  {}TITLE{}\n", g("D"), x);
    let title = { let t = topic_get(f, "title"); if t.is_empty() { "(untitled)".into() } else { t } };
    out.push_str(&field_block(c, "title", &g("TITLE"), &title, m.idx));
    out.push_str(&format!("\n  {}DESCRIPTION{}\n", g("D"), x));
    let desc = topic_get(f, "description");
    if !desc.is_empty() { out.push_str(&field_block(c, "description", &g("DESC"), &desc, m.idx)); } else { out.push_str(&field_block(c, "description", &g("D"), "– (⏎ adds one)", m.idx)); }
    out.push_str(&format!("\n  {}STATUS{}\n", g("D"), x));
    out.push_str(&format!("  {}{}{}", sc, st, x));
    if st == "waiting" && !wf.is_empty() { let age = ago(m.now, if ws.is_empty() { "x" } else { &ws }); let age = if ws.is_empty() { ago(m.now, &m.now.to_string()) } else { age }; out.push_str(&format!(" {}·{} {}{}{} {}· {}{}", g("D"), x, g("BLUE"), wf, x, g("D"), age, x)); }
    out.push('\n');
    if !ag.is_empty() { out.push_str(&format!("  {}agent{} {}{}{}{}\n", g("D"), x, g("BLUE"), clip(&ag, c.cols - 12), x, if am.is_empty() { String::new() } else { format!(" {}· {}{}", g("D"), am, x) })); }
    let br = topic_get(f, "agent_branch");
    if !br.is_empty() && !topic_get(f, "agent_pane").is_empty() && m.github { out.push_str(&format!("  {}branch{} {}⎇ {}{}\n", g("D"), x, g("S"), clip(&br, c.cols - 13), x)); }
    let pj = topic_get(f, "project");
    if !pj.is_empty() { out.push_str(&format!("  {}project{} {}{}{}\n", g("D"), x, g("L"), (m.project_name)(&pj).unwrap_or(pj.clone()), x)); }
    out.push('\n'); out.push_str(&rule(c));
    out.push_str(&format!("\n  {}ACTIONS{}\n", g("D"), x));
    for (i, (_, label)) in m.items.iter().enumerate() { out.push_str(&format!("{}{}{}", REF, i, REF)); out.push_str(&num_line(c, i as i64 == m.idx, i + 1, label)); }
    out.push('\n'); out.push_str(&rule(c));
    if !m.flash.is_empty() {
        let col = if m.flash.starts_with('✓') { g("GREEN") } else if m.flash.starts_with('✗') { g("NN") } else { g("S") };
        for l in wrap_print(c, 2, &col, m.flash) { out.push_str(&l); out.push('\n'); }
    } else { out.push_str(&format!("  {}{}{}\n", g("D"), clip("↑↓ 1-9 choose · ⏎ do it / edit · esc back", c.cols - 4), x)); }
    out
}

/// pick_build: the free agents (rows of the agent lists) a topic can go to
pub fn pick_map<'a>(rows: &'a [Row], topics: &[store::Topic], mode: &str, source: Option<(&str, &str)>) -> Vec<&'a Row> {
    let mut v = Vec::new();
    for r in rows {
        if let Some((sm, sp)) = source { if !sp.is_empty() && r.m == sm && r.id == sp { continue; } }
        let _ = mode;
        if !matches!(r.st.as_str(), "idle" | "done") { continue; }
        let taken = topics.iter().any(|t| matches!(t.status.as_str(), "now" | "testing") && t.am == r.m && t.ap == r.id);
        if !taken { v.push(r); }
    }
    v
}

/// bhote_pick: who takes it (mode start) or where it goes (transfer)
pub fn render_pick(c: &Ctx, title: &str, mode: &str, agents: &[&Row], idx: i64) -> String {
    let g = |k: &str| c.c.g(k).to_string(); let x = g("X"); let w = c.cols - 2;
    let t = clip(if title.is_empty() { "(untitled)" } else { title }, c.cols - 8);
    let mut out = format!("\n {}▸ {}{}{}\n {}{}{}\n", g("ORANGE"), g("L"), t, x, g("D"), if mode == "transfer" { "Transfer to which agent?" } else { "Who takes it?" }, x);
    out.push_str(&rule(c)); out.push('\n');
    let first = if mode == "transfer" { 1 } else { 0 };
    for i in first..=agents.len() as i64 {
        let label = if i == 0 { "Just mark as started".to_string() } else { let r = agents[(i - 1) as usize]; let mut l = format!("{} · {}", r.m, r.name); if r.st == "done" { l.push_str(" ✓"); } l };
        let label = clip(&label, c.cols - 8);
        let t = clip(&label, w - 4); let tw = dw(&t);
        if i == idx { out.push_str(&format!("{}{}   {}{}{}{}\n", SELMARK, g("SELBG"), g("LSEL"), t, sp(w - 3 - tw), x)); } else { out.push_str(&format!("   {}{}{}\n", g("D"), t, x)); }
    }
    if agents.is_empty() { out.push_str(&format!("\n   {}no free agent right now{}\n", g("D"), x)); }
    out.push('\n'); out.push_str(&rule(c));
    out.push_str(&format!(" {}↑↓ choose · →/⏎ start · ←/esc back{}\n", g("D"), x));
    out
}

/// bhote_paint for the other views: the markers out, the line ends, the map of the clickable rows (row, ref); a tall frame follows the selected line
pub fn paint_marked(text: &str, rows: usize) -> (String, Vec<(i64, String)>) {
    let text = text.trim_end_matches('\n');
    let (mut fl, mut fr): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new()); let mut sel: i64 = -1;
    for (n, l0) in text.split('\n').enumerate() {
        let mut line = l0.to_string(); let mut rf = String::new();
        if let Some(r) = line.strip_prefix(REF) { let i = r.find(REF).unwrap_or(r.len()); rf = r[..i].to_string(); line = r[(i + 1).min(r.len())..].to_string(); }
        if let Some(r) = line.strip_prefix(SELMARK) { sel = n as i64; line = r.to_string(); }
        fl.push(line); fr.push(rf);
    }
    let n = fl.len(); let mut off = 0usize;
    if n > rows && sel >= 0 { let mut o = sel - (rows as i64) / 2; if o > n as i64 - rows as i64 { o = n as i64 - rows as i64; } if o < 0 { o = 0; } off = o as usize; }
    let (mut out, mut map) = (String::new(), Vec::new());
    for i in off..n.min(off + rows) { if i > off { out.push('\n'); } out.push_str(&fl[i]); out.push_str("\u{1b}[K"); if !fr[i].is_empty() { map.push(((i - off + 1) as i64, fr[i].clone())); } }
    (format!("\u{1b}[H{}\u{1b}[J", out), map)
}
#[allow(dead_code)]
fn unused() { let _ = (US, fs::metadata("/")); }
