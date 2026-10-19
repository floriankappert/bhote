//! The drawing of the panel (step 5), block by block, with the same bytes as the bash functions: the colours arrive as the script's own
//! variables (a file written when the theme is applied), the rest is read from the same files the script reads. Blocks so far: monitors.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const US: char = '\u{1f}';
const RS_REF: char = '\u{1d}';   // \035 around the ref of a clickable line

pub struct Colors(HashMap<String, String>);
impl Colors {
    pub fn from_map(m: HashMap<String, String>) -> Colors { Colors(m) }
    pub fn read(path: &str) -> Colors {
        let mut m = HashMap::new();
        for l in crate::proc::read_cap(path).unwrap_or_default().split('\n') { if let Some(i) = l.find('=') { m.insert(l[..i].to_string(), l[i + 1..].to_string()); } }
        Colors(m)
    }
    pub fn g(&self, k: &str) -> &str { self.0.get(k).map(|s| s.as_str()).unwrap_or("") }
}

pub struct Ctx { pub cols: i64, pub now: u64, pub shared: PathBuf, pub topics: PathBuf, pub cfg: HashMap<String, String>, pub c: Colors }
impl Ctx {
    fn cfg(&self, k: &str, d: &str) -> String { self.cfg.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string()) }
    fn rule_len(&self) -> i64 { (self.cols - 5).max(0) }
    fn dash(&self, n: i64) -> String { "┈".repeat(n.max(0) as usize) }
}

// ── widths ──────────────────────────────────────────────────────────────────────────────────────────────────
/// dw_set: the columns a text takes (as the script counts them: characters, emoji and East Asian wide ones 2, combining marks 0)
pub fn dw(s: &str) -> i64 {
    let mut n = s.chars().count() as i64;
    for c in s.chars() {
        let cp = c as u32;
        if cp >= 0x10000 { n += 1; }
        else if cp >= 0x800 {
            if (0x1100..=0x115F).contains(&cp) || ((0x2E80..=0xA4CF).contains(&cp) && cp != 0x303F) || (0xAC00..=0xD7A3).contains(&cp) || (0xF900..=0xFAFF).contains(&cp) || (0xFE30..=0xFE4F).contains(&cp) || (0xFF00..=0xFF60).contains(&cp) || (0xFFE0..=0xFFE6).contains(&cp) { n += 1; }
            else if (0x200B..=0x200F).contains(&cp) || (0xFE00..=0xFE0F).contains(&cp) || (0x20D0..=0x20FF).contains(&cp) { n -= 1; }
        } else if cp >= 0x80 && (0x300..=0x36F).contains(&cp) { n -= 1; }
    }
    n
}
/// clip_set: cut with a … when wider than `max` columns
pub fn clip(t: &str, max: i64) -> String {
    if max < 1 { return String::new(); }
    if t.is_ascii() { return if t.chars().count() as i64 > max { format!("{}…", t.chars().take((max - 1) as usize).collect::<String>()) } else { t.to_string() }; }
    if dw(t) <= max { return t.to_string(); }
    let (mut o, mut u) = (String::new(), 0);
    for c in t.chars() { let w = dw(&c.to_string()); if u + w > max - 1 { break; } o.push(c); u += w; }
    o.push('…'); o
}
/// printf '%*s' n "": n spaces (a negative width pads on the other side, but still |n| spaces)
fn sp(n: i64) -> String { " ".repeat(n.unsigned_abs() as usize) }
fn chars_to(s: &str, n: usize) -> String { s.chars().take(n).collect() }

// ── the projects ────────────────────────────────────────────────────────────────────────────────────────────
extern "C" { fn setlocale(cat: i32, loc: *const std::os::raw::c_char) -> *mut std::os::raw::c_char; fn strcoll(a: *const std::os::raw::c_char, b: *const std::os::raw::c_char) -> i32; }
pub fn init_locale() { unsafe { setlocale(6, b"\0".as_ptr() as *const _); } }   // LC_ALL: the collation of the user's locale, as bash compares
pub fn coll(a: &str, b: &str) -> std::cmp::Ordering {
    match (std::ffi::CString::new(a), std::ffi::CString::new(b)) { (Ok(x), Ok(y)) => unsafe { strcoll(x.as_ptr(), y.as_ptr()) }.cmp(&0), _ => a.cmp(b) }
}
pub struct Project { pub id: String, pub name: String }
/// projects_load (the names, by name) with whether the record has a CI (mon_projects_ci)
pub fn projects(c: &Ctx) -> Vec<Project> {
    let mut v = Vec::new();
    let Ok(rd) = fs::read_dir(&c.topics) else { return v };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); n.starts_with("proj-") && n.ends_with(".topic") })).collect();
    files.sort();
    for p in files {
        let txt = crate::proc::read_cap(&p).unwrap_or_default();
        let (mut name, mut del, mut kd) = (String::new(), "0".to_string(), String::new());
        for l in txt.lines() { if let Some(i) = l.find('=') { let (k, val) = (&l[..i], &l[i + 1..]); match k { "title" if name.is_empty() => name = val.to_string(), "deleted" => del = val.to_string(), "kind" => kd = val.to_string(), _ => {} } } }
        if kd == "project" && del != "1" && !name.is_empty() { v.push(Project { id: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), name: name.chars().filter(|c| !c.is_control()).collect() }); }
    }
    v.sort_by(|a, b| coll(&a.name, &b.name));
    v
}

// ── small helpers of the monitors ───────────────────────────────────────────────────────────────────────────
fn uc(s: &str) -> String { s.chars().map(|c| c.to_ascii_uppercase()).collect() }
fn list_in(list: &str, name: &str) -> bool { let n = name.to_lowercase(); list.split(',').any(|e| e.to_lowercase() == n) }
fn hidden(off: &str, pj: &str, ps: &[Project]) -> bool {
    let lp = pj.to_lowercase(); let mut best = "";
    for p in ps { let ln = p.name.to_lowercase(); if (lp == ln || lp.starts_with(&format!("{}-", ln)) || lp.starts_with(&format!("{}_", ln))) && p.name.chars().count() > best.chars().count() { best = &p.name; } }
    !best.is_empty() && list_in(off, best)
}
/// mon_abbr_set: at most 3 characters, capitals
pub fn abbr(name: &str, taken: &str) -> String {
    let n = uc(name);
    let flat: String = n.chars().filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit()).collect();
    let words: Vec<String> = n.split(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit())).filter(|w| !w.is_empty()).map(|w| w.to_string()).collect();
    let ini: String = words.iter().filter_map(|w| w.chars().next()).collect();
    let flat = if flat.is_empty() { "PRJ".to_string() } else { flat };
    let last = |s: &str, k: usize| -> String { let c: Vec<char> = s.chars().collect(); c[c.len().saturating_sub(k)..].iter().collect() };
    let c0 = if words.len() > 1 { chars_to(&ini, 3) } else { chars_to(&flat, 3) };
    let cands = [c0, chars_to(&flat, 3), format!("{}{}", chars_to(&flat, 2), last(&ini, 1)), format!("{}{}", chars_to(&flat, 1), last(&flat, 2))];
    let t = format!(" {} ", taken);
    for c in cands.iter() { if !c.is_empty() && !t.contains(&format!(" {} ", c)) { return c.clone(); } }
    for i in 2..10 { let c = format!("{}{}", chars_to(&flat, 2), i); if !t.contains(&format!(" {} ", c)) { return c; } }
    chars_to(&flat, 3)
}
fn bar_cells(d: i64, tot: i64, cells: i64) -> (String, String, String) {
    let mut f = (d * cells + tot / 2) / tot; if f > cells { f = cells; }
    let (mut all, mut on, mut off) = (String::new(), String::new(), String::new());
    for i in 0..cells { all.push('■'); if i < f { on.push('■'); } else { off.push('■'); } }
    (all, on, off)
}
fn age(now: u64, t: &str) -> String {
    let a = if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) && t.len() <= 18 { t.trim_start_matches('0').parse::<u64>().unwrap_or(0) } else { now };
    let a = now.saturating_sub(a);
    if a < 60 { format!("{}s", a) } else if a < 3600 { format!("{}m", a / 60) } else if a < 86400 { format!("{}h", a / 3600) } else { format!("{}d", a / 86400) }
}

pub fn dash_caption_pub(c: &Ctx, text: &str) -> String { dash_caption(c, text, "", false) }
pub fn dash_caption_agents(c: &Ctx, hover: bool) -> String { dash_caption(c, "Agents", "agents", hover) }
pub fn dash_caption_section(c: &Ctx, text: &str, section: &str) -> String { dash_caption(c, text, section, false) }
fn dash_caption(c: &Ctx, text: &str, section: &str, hover_btn: bool) -> String {
    let (mut rest, mut rf, mut btn) = (c.rule_len() - 4 - text.chars().count() as i64, String::new(), String::new());
    if !section.is_empty() {
        let mut bc = c.c.g("D").to_string();
        if section == "agents" { if hover_btn { bc = format!("{}{}", c.c.g("BOLD"), c.c.g("ORANGE")); } } else { rf = format!("{}cap:{}{}", RS_REF, section, RS_REF); }
        rest -= 2; btn = format!(" {}↕{}", bc, c.c.g("X"));
    }
    if rest < 1 { rest = 1; }
    format!("{}  {}┈┈{} {}{}{} {}{}{}{}\n", rf, c.c.g("MUTED"), c.c.g("X"), c.c.g("D"), text, c.c.g("X"), c.c.g("MUTED"), chars_to(&c.dash(c.rule_len()), rest as usize), c.c.g("X"), btn)
}

fn compact(c: &Ctx, kind: &str) -> bool { c.cfg(&format!("{}_COMPACT", uc(kind)), "off") == "on" }

fn monitor_block(c: &Ctx, kind: &str, max: usize, ps: &[Project]) -> String {
    let cap = if kind == "deploys" { "Deployments" } else { "Tests" };
    let list = c.shared.join(format!("{}.list", kind));
    if fs::metadata(&list).map_or(true, |m| m.len() == 0) { return String::new(); }
    let off = c.cfg(if kind == "deploys" { "DEPLOY_PROJECTS_OFF" } else { "TEST_PROJECTS_OFF" }, "");
    let all = crate::proc::read_cap(&list).unwrap_or_default();
    let lines: Vec<&str> = all.lines().filter(|l| { if off.is_empty() { return true; } let pj = l.split(US).nth(1).unwrap_or(""); !hidden(&off, pj, ps) }).collect();
    let mut out = dash_caption(c, cap, kind, false);
    if lines.is_empty() { return out; }
    let g = |k: &str| c.c.g(k).to_string();
    if compact(c, kind) {
        let (mut run, mut bad, mut fixed, mut seen) = (0, 0, 0, 0);
        let mut okk: Vec<String> = Vec::new();
        for l in &lines {
            let f: Vec<&str> = l.split(US).collect(); let (pj, lb, st) = (f.get(1).copied().unwrap_or(""), f.get(2).copied().unwrap_or(""), f.get(3).copied().unwrap_or(""));
            seen += 1; if st == "running" { run += 1; }
            let key = format!("{}|{}", pj, lb);
            if st == "ok" { okk.push(key.clone()); }
            if st == "failed" && seen <= 10 { if okk.contains(&key) { fixed += 1; } else { bad += 1; } }
        }
        let (mut rc, mut rb) = (g("ORANGE"), format!("{}{}", g("BOLD"), g("ORANGE")));
        if run == 0 { rc = g("D"); rb = g("D"); }
        out.push_str(&format!("  {}✶{} {}{} running{} {}·{} ", rc, g("X"), rb, run, g("X"), g("D"), g("X")));
        if bad > 0 { out.push_str(&format!("{}✗ {} failed{}", g("NN"), bad, g("X"))); } else { out.push_str(&format!("{}✗ 0 failed{}", g("D"), g("X"))); }
        if fixed > 0 { out.push_str(&format!(" {}·{} {}✓ {} resolved{}", g("D"), g("X"), g("GREEN"), fixed, g("X"))); }
        out.push('\n');
        return out;
    }
    let w = c.cols - 4;
    let (mut okk, mut n, mut more): (Vec<String>, usize, usize) = (Vec::new(), 0, 0);
    for l in &lines {
        let f: Vec<&str> = l.split(US).collect();
        let at = |i: usize| f.get(i).copied().unwrap_or("");
        let (t, pj, lb, st, d, tot, mut note) = (at(0), at(1), at(2), at(3), at(4), at(5), at(6).to_string());
        // (read -r with IFS=US: the last variable takes the rest of the line, url included: the note is the 7th field)
        let key = format!("{}|{}", pj, lb);
        let mut fx = false;
        if st == "ok" { okk.push(key.clone()); }
        if st == "failed" && okk.contains(&key) { fx = true; }
        if n >= max { more += 1; continue; }
        let mut bar = String::new(); let (ic, mut tx): (String, String);
        match st {
            "running" => {
                ic = format!("{}✶", g("ORANGE")); tx = "running".into();
                let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
                if digits(d) && digits(tot) { let (dn, tn): (i64, i64) = (d.parse().unwrap_or(0), tot.parse().unwrap_or(0)); if tn > 0 { let (b, _, _) = bar_cells(dn, tn, 5); bar = b.clone(); tx = format!("{} {}%", b, dn * 100 / tn); } }
            }
            "queued" => { ic = format!("{}◌", g("D")); tx = "queued".into(); }
            "approval" => { ic = format!("{}⏸", g("Y")); tx = "approval".into(); }
            "ok" => { ic = format!("{}✓", g("GREEN")); tx = String::new(); }
            "failed" => { if fx { ic = format!("{}✗", g("D")); tx = "failed · resolved".into(); } else { ic = format!("{}✗", g("NN")); tx = "failed".into(); } }
            _ => { ic = format!("{}⊘", g("D")); tx = "canceled".into(); }
        }
        let mut br = String::new();
        if let Some(i) = note.find("|~|") { br = note[..i].to_string(); note = note[i + 3..].to_string(); }
        if kind == "tests" && !note.is_empty() && st != "running" { tx = note.clone(); }
        if fx && kind == "tests" { tx = format!("{} · resolved", tx); }
        if bar.is_empty() { tx = clip(&tx, w / 2); }
        let ag = age(c.now, t);
        let right = if tx.is_empty() { ag.clone() } else { format!("{} · {}", tx, ag) };
        let rw = dw(&right);
        let mut rightc = right.clone();
        if !bar.is_empty() {
            let (_, on, offc) = bar_cells(d.parse().unwrap_or(0), tot.parse().unwrap_or(1), 5);
            let barc = format!("{}{}{}{}{}", g("ORANGE"), on, g("MUTED"), offc, g("D"));
            rightc = right.replacen(&bar, &barc, 1);
        }
        let mut lbl = format!("{} · {}", pj, lb);
        let shown = if kind == "deploys" { note.clone() } else { br.clone() };
        if !shown.is_empty() { lbl = format!("{} ({})", lbl, shown); }
        let left = clip(&lbl, w - rw - 3); let lw = dw(&left);
        let pad = (w - 2 - lw - rw).max(1);
        out.push_str(&format!("  {}{} {}{}{}{}{}{}\n", ic, g("X"), g("L"), left, sp(pad), g("D"), rightc, g("X")));
        n += 1;
    }
    if more > 0 { out.push_str(&format!("  {}+{} more {}{}\n", g("D"), more, if kind == "deploys" { "deployments" } else { "tests" }, g("X"))); }
    out
}

/// monitors_block: both monitors, when on
pub fn monitors(c: &Ctx) -> String {
    let r: usize = { let v = c.cfg("MONITOR_ROWS", "4"); if v.bytes().all(|b| b.is_ascii_digit()) && !v.is_empty() { v.parse().unwrap_or(4) } else { 4 } };
    let ps = projects(c);
    let mut out = String::new();
    if c.cfg("DEPLOY_MONITOR", "off") == "on" { out.push_str(&monitor_block(c, "deploys", r, &ps)); }
    if c.cfg("TEST_MONITOR", "off") == "on" { out.push_str(&monitor_block(c, "tests", r, &ps)); }
    out
}
#[allow(dead_code)]
fn unused(_: &Path) {}
