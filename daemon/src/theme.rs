//! theme_apply of the script: the colour variables from THEME and the COLOR_* overrides, and the gradient of the head line.
use std::collections::HashMap;

fn rgb(s: &str) -> String { format!("\u{1b}[38;2;{}m", s) }
fn hex_rgb(h: &str) -> Option<String> {
    let h = h.trim_start_matches('#');
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    Some(format!("{};{};{}", p(0), p(2), p(4)))
}

pub struct Theme { pub vars: HashMap<String, String>, pub head_grad: String }

/// The colours of this run. `color` false (NO_COLOR): everything empty.
pub fn apply(cfg: &HashMap<String, String>, color: bool, head_text: &str) -> Theme {
    let mut v: HashMap<String, String> = HashMap::new();
    let set = |v: &mut HashMap<String, String>, k: &str, x: String| { v.insert(k.to_string(), x); };
    for k in ["X", "BOLD", "R", "NN", "S", "C", "L", "D", "B", "Y", "GREEN", "BLUE", "ORANGE", "MUTED", "PINK", "TEAL", "SELBG", "CARDBG", "CARDSEL", "CARDHOV", "CARDSUB", "CARDBTN", "LSEL", "DSEL", "DESC", "DESCSEL", "TITLE", "TITLESEL"] { set(&mut v, k, String::new()); }
    if !color { return Theme { vars: v, head_grad: head_text.to_string() }; }
    let esc = "\u{1b}";
    set(&mut v, "X", format!("{}[0m", esc)); set(&mut v, "BOLD", format!("{}[1m", esc));
    let th = cfg.get("THEME").filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| "terminal".into());
    let mut stops: Vec<(i64, i64, i64)> = vec![(148, 226, 213), (137, 180, 250), (203, 166, 247), (245, 194, 231), (250, 179, 135)];
    let mut grad_ansi = String::new();
    let g = |v: &HashMap<String, String>, k: &str| v.get(k).cloned().unwrap_or_default();
    match th.as_str() {
        "catppuccin" => {
            set(&mut v, "R", rgb("203;166;247")); set(&mut v, "NN", rgb("243;139;168")); set(&mut v, "S", rgb("148;226;213")); set(&mut v, "C", rgb("250;179;135")); set(&mut v, "PINK", rgb("245;194;231")); set(&mut v, "ORANGE", rgb("217;119;87"));
            set(&mut v, "L", format!("{}[1m{}", esc, rgb("205;214;244"))); set(&mut v, "D", rgb("108;112;134")); let d = g(&v, "D"); set(&mut v, "B", d);
            set(&mut v, "MUTED", rgb("69;71;90")); set(&mut v, "Y", rgb("249;226;175")); set(&mut v, "GREEN", rgb("166;227;161")); set(&mut v, "BLUE", rgb("137;180;250"));
            for (k, s) in [("SELBG", "49;50;68"), ("CARDBG", "33;34;50"), ("CARDSEL", "49;50;68"), ("CARDHOV", "40;41;58"), ("CARDSUB", "25;25;38")] { set(&mut v, k, format!("{}[48;2;{}m", esc, s)); }
            set(&mut v, "CARDBTN", format!("{}[48;2;108;112;134m{}[1;38;2;30;30;46m", esc, esc));
            set(&mut v, "LSEL", format!("{}[1m{}", esc, rgb("245;247;255"))); set(&mut v, "DSEL", rgb("186;194;222"));
            set(&mut v, "TITLE", format!("{}[1m{}", esc, rgb("166;173;200"))); set(&mut v, "TITLESEL", format!("{}[1m{}", esc, rgb("205;214;244")));
            set(&mut v, "DESC", format!("{}[3m{}", esc, rgb("205;214;244"))); set(&mut v, "DESCSEL", format!("{}[3m{}", esc, rgb("245;247;255")));
        }
        "dracula" => {
            set(&mut v, "R", rgb("255;121;198")); set(&mut v, "NN", rgb("255;85;85")); set(&mut v, "S", rgb("139;233;253")); set(&mut v, "C", rgb("255;184;108")); set(&mut v, "PINK", rgb("255;146;223")); set(&mut v, "ORANGE", rgb("217;119;87"));
            set(&mut v, "L", format!("{}[1m{}", esc, rgb("248;248;242"))); set(&mut v, "D", rgb("98;114;164")); let d = g(&v, "D"); set(&mut v, "B", d);
            set(&mut v, "MUTED", rgb("68;71;90")); set(&mut v, "Y", rgb("241;250;140")); set(&mut v, "GREEN", rgb("80;250;123")); set(&mut v, "BLUE", rgb("189;147;249"));
            for (k, s) in [("SELBG", "68;71;90"), ("CARDBG", "46;48;62"), ("CARDSEL", "68;71;90"), ("CARDHOV", "56;58;74"), ("CARDSUB", "33;34;44")] { set(&mut v, k, format!("{}[48;2;{}m", esc, s)); }
            set(&mut v, "CARDBTN", format!("{}[48;2;98;114;164m{}[1;38;2;40;42;54m", esc, esc));
            set(&mut v, "LSEL", format!("{}[1m{}", esc, rgb("255;255;255"))); set(&mut v, "DSEL", rgb("189;195;223"));
            set(&mut v, "TITLE", format!("{}[1m{}", esc, rgb("214;216;228"))); set(&mut v, "TITLESEL", format!("{}[1m{}", esc, rgb("248;248;242")));
            set(&mut v, "DESC", format!("{}[3m{}", esc, rgb("248;248;242"))); set(&mut v, "DESCSEL", format!("{}[3m{}", esc, rgb("255;255;255")));
        }
        _ => {
            for (k, s) in [("R", "[35m"), ("NN", "[31m"), ("S", "[36m"), ("C", "[33m"), ("L", "[1;39m"), ("D", "[90m"), ("PINK", "[95m"), ("ORANGE", "[38;5;173m"), ("Y", "[93m"), ("GREEN", "[32m"), ("BLUE", "[34m"), ("MUTED", "[2;90m"),
                ("SELBG", "[48;5;236m"), ("CARDBG", "[48;5;234m"), ("CARDSEL", "[48;2;46;46;60m"), ("CARDHOV", "[48;2;36;36;46m"), ("CARDSUB", "[48;2;22;22;30m"), ("LSEL", "[1;97m"), ("DSEL", "[38;5;251m"),
                ("TITLE", "[1;37m"), ("TITLESEL", "[1;97m"), ("DESC", "[3;39m"), ("DESCSEL", "[3;97m")] { set(&mut v, k, format!("{}{}", esc, s)); }
            let d = g(&v, "D"); set(&mut v, "B", d);
            set(&mut v, "CARDBTN", format!("{}[48;5;244m{}[1;38;5;232m", esc, esc));
            grad_ansi = "36 34 35 95 33".into();
        }
    }
    if th == "catppuccin" { stops = vec![(148, 226, 213), (137, 180, 250), (203, 166, 247), (245, 194, 231), (250, 179, 135)]; }
    if th == "dracula" { stops = vec![(139, 233, 253), (189, 147, 249), (255, 121, 198), (255, 184, 108), (241, 250, 140)]; }
    for k in ["TEXT", "DIM", "FRAME", "SELECTION", "ACCENT", "NOW", "WAITING", "REVIEW", "LATER", "DONE", "ERROR"] {
        let Some(x) = cfg.get(&format!("COLOR_{}", k)).filter(|s| !s.is_empty()) else { continue };
        let Some(c) = hex_rgb(x) else { continue };
        match k {
            "TEXT" => { let l = format!("{}[1m{}", esc, rgb(&c)); for n in ["L", "LSEL", "TITLE", "TITLESEL"] { set(&mut v, n, l.clone()); } let d = format!("{}[3m{}", esc, rgb(&c)); set(&mut v, "DESC", d.clone()); set(&mut v, "DESCSEL", d); }
            "DIM" => { set(&mut v, "D", rgb(&c)); set(&mut v, "DSEL", rgb(&c)); }
            "FRAME" => set(&mut v, "B", rgb(&c)),
            "SELECTION" => { let s = format!("{}[48;2;{}m", esc, c); set(&mut v, "SELBG", s.clone()); set(&mut v, "CARDSEL", s); }
            "ACCENT" => set(&mut v, "S", rgb(&c)),
            "NOW" => { set(&mut v, "C", rgb(&c)); set(&mut v, "ORANGE", rgb(&c)); }
            "WAITING" => set(&mut v, "Y", rgb(&c)), "REVIEW" => set(&mut v, "BLUE", rgb(&c)), "LATER" => set(&mut v, "R", rgb(&c)), "DONE" => set(&mut v, "GREEN", rgb(&c)), "ERROR" => set(&mut v, "NN", rgb(&c)),
            _ => {}
        }
    }
    // MUTED=${MUTED:-$B}; TEAL=$S; ORANGE=${ORANGE:-$C}; LSEL=${LSEL:-$L}; DSEL=${DSEL:-$D}; DESC=${DESC:-$D}; DESCSEL=${DESCSEL:-$DSEL}; TITLE=${TITLE:-$L}; TITLESEL=${TITLESEL:-$LSEL}
    let dflt = |v: &mut HashMap<String, String>, k: &str, from: &str| { if v.get(k).map_or(true, |s| s.is_empty()) { let f = v.get(from).cloned().unwrap_or_default(); v.insert(k.to_string(), f); } };
    dflt(&mut v, "MUTED", "B"); let s = g(&v, "S"); set(&mut v, "TEAL", s); dflt(&mut v, "ORANGE", "C"); dflt(&mut v, "LSEL", "L"); dflt(&mut v, "DSEL", "D"); dflt(&mut v, "DESC", "D"); dflt(&mut v, "DESCSEL", "DSEL"); dflt(&mut v, "TITLE", "L"); dflt(&mut v, "TITLESEL", "LSEL");
    // the gradient of the head line (HEAD_GRAD: bold in front)
    let x = g(&v, "X");
    let chars: Vec<char> = head_text.chars().collect(); let w = chars.len() as i64;
    let mut out = String::new();
    if !grad_ansi.is_empty() {
        let cols: Vec<&str> = grad_ansi.split(' ').collect(); let nc = cols.len() as i64;
        for (i, ch) in chars.iter().enumerate() { out.push_str(&format!("{}[{}m{}", esc, cols[((i as i64) * nc / w.max(1)) as usize], ch)); }
    } else {
        let n = stops.len() as i64 - 1;
        for (i, ch) in chars.iter().enumerate() {
            if *ch == ' ' { out.push(' '); continue; }
            let p = (i as i64) * n * 100 / (w - 1).max(1); let mut seg = p / 100; if seg >= n { seg = n - 1; } let t = p - seg * 100;
            let (a, b) = (stops[seg as usize], stops[seg as usize + 1]);
            out.push_str(&rgb(&format!("{};{};{}", a.0 + (b.0 - a.0) * t / 100, a.1 + (b.1 - a.1) * t / 100, a.2 + (b.2 - a.2) * t / 100))); out.push(*ch);
        }
    }
    out.push_str(&x);
    Theme { vars: v, head_grad: format!("{}[1m{}", esc, out) }
}
