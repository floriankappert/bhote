//! What a terminal sends, cut into events: keys (with a name like `ctrl+alt+left`), SGR mouse reports, pastes, focus changes.

#[derive(Debug, Clone, PartialEq)]
pub struct Key { pub name: String, pub raw: Vec<u8> }

#[derive(Debug, Clone, PartialEq)]
pub struct Mouse {
    /// the button code of the report: 0 left, 1 middle, 2 right, 3 none (old encoding release), +4 shift, +8 alt, +16 ctrl, +32 motion, 64/65 wheel
    pub b: u32,
    /// 0-based column and row
    pub x: u16, pub y: u16,
    pub press: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input { Key(Key), Mouse(Mouse), Paste(Vec<u8>), Focus(bool) }

#[derive(Default)]
pub struct Parser { buf: Vec<u8>, paste: Option<Vec<u8>> }

const PASTE_END: &[u8] = b"\x1b[201~";

fn mods_prefix(m: u32) -> String {
    let m = m.saturating_sub(1);
    let mut s = String::new();
    if m & 4 != 0 { s.push_str("ctrl+"); }
    if m & 2 != 0 || m & 8 != 0 { s.push_str("alt+"); }
    if m & 1 != 0 { s.push_str("shift+"); }
    s
}

/// The name of a typed character: letters in capitals are `shift+letter`.
fn char_name(c: char) -> String {
    match c {
        ' ' => "space".into(),
        c if c.is_uppercase() && c.to_lowercase().count() == 1 => format!("shift+{}", c.to_lowercase()),
        c => c.to_string(),
    }
}

fn utf8_len(b: u8) -> usize { match b { 0xc0..=0xdf => 2, 0xe0..=0xef => 3, 0xf0..=0xf7 => 4, _ => 1 } }

fn key(name: String, raw: &[u8]) -> Input { Input::Key(Key { name, raw: raw.to_vec() }) }

impl Parser {
    pub fn pending(&self) -> bool { !self.buf.is_empty() }

    /// A lone escape that waited long enough is the escape key.
    pub fn flush(&mut self) -> Vec<Input> {
        let mut out = Vec::new();
        if self.buf == [0x1b] { self.buf.clear(); out.push(key("esc".into(), b"\x1b")); }
        out
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Input> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if let Some(p) = self.paste.as_mut() {
                match self.buf.windows(PASTE_END.len()).position(|w| w == PASTE_END) {
                    Some(i) => { p.extend_from_slice(&self.buf[..i]); self.buf.drain(..i + PASTE_END.len()); out.push(Input::Paste(self.paste.take().unwrap())); continue; }
                    None => { // everything but a possible start of the end marker belongs to the paste
                        let keep = self.buf.len().min(PASTE_END.len() - 1);
                        let cut = self.buf.len() - keep;
                        p.extend_from_slice(&self.buf[..cut]); self.buf.drain(..cut); break;
                    }
                }
            }
            if self.buf.is_empty() { break; }
            match self.one() { Some((n, ev)) => { let raw: Vec<u8> = self.buf.drain(..n).collect(); if let Some(mut ev) = ev { if let Input::Key(k) = &mut ev { k.raw = raw; } out.push(ev); } } None => break }
        }
        out
    }

    /// The first event of the buffer: (bytes used, event); None when the bytes so far are the start of one.
    fn one(&mut self) -> Option<(usize, Option<Input>)> {
        let b = &self.buf;
        let b0 = b[0];
        if b0 == 0x1b {
            if b.len() == 1 { return None; }
            return match b[1] {
                b'[' => self.csi(),
                b'O' => { if b.len() < 3 { return None; } let n = match b[2] { b'P' => "f1", b'Q' => "f2", b'R' => "f3", b'S' => "f4", b'A' => "up", b'B' => "down", b'C' => "right", b'D' => "left", b'H' => "home", b'F' => "end", _ => "" }; Some((3, if n.is_empty() { None } else { Some(key(n.into(), &b[..3])) })) }
                0x1b => Some((2, Some(key("alt+esc".into(), &b[..2])))),
                c => {
                    let n = utf8_len(c);
                    if b.len() < 1 + n { return None; }
                    let inner = Self::plain(&b[1..1 + n]).unwrap_or_default();
                    // modifiers in the order ctrl, alt, shift
                    let name = match inner.strip_prefix("ctrl+") { Some(r) => format!("ctrl+alt+{r}"), None => format!("alt+{inner}") };
                    Some((1 + n, Some(key(name, &b[..1 + n]))))
                }
            };
        }
        let n = utf8_len(b0);
        if b.len() < n { return None; }
        Some((n, Self::plain(&b[..n]).map(|nm| key(nm, &b[..n]))))
    }

    /// The name of a key that is no escape sequence: control characters and text.
    fn plain(b: &[u8]) -> Option<String> {
        let b0 = b[0];
        Some(match b0 {
            0 => "ctrl+space".into(),
            9 => "tab".into(),
            13 | 10 => "enter".into(),
            27 => "esc".into(),
            127 => "backspace".into(),
            1..=26 => format!("ctrl+{}", (b'a' + b0 - 1) as char),
            28..=31 => format!("ctrl+{}", ["\\", "]", "^", "_"][(b0 - 28) as usize]),
            _ => char_name(std::str::from_utf8(b).ok()?.chars().next()?),
        })
    }

    fn csi(&mut self) -> Option<(usize, Option<Input>)> {
        let b = &self.buf;
        // ESC [ params intermediates final
        let mut i = 2;
        while i < b.len() && (0x30..=0x3f).contains(&b[i]) { i += 1; }
        while i < b.len() && (0x20..=0x2f).contains(&b[i]) { i += 1; }
        if i >= b.len() { return None; }
        let fin = b[i];
        if !(0x40..=0x7e).contains(&fin) { return Some((i + 1, None)); }
        let params = std::str::from_utf8(&b[2..i]).unwrap_or("");
        let total = i + 1;
        let nums: Vec<u32> = params.trim_start_matches(|c| c == '<' || c == '?' || c == '>').split(';').map(|p| p.split(':').next().unwrap_or("").parse().unwrap_or(0)).collect();
        let m = nums.get(1).copied().unwrap_or(1).max(1);
        let raw = &b[..total];
        let ev = match fin {
            b'M' | b'm' if params.starts_with('<') && nums.len() == 3 => Some(Input::Mouse(Mouse { b: nums[0], x: nums[1].saturating_sub(1) as u16, y: nums[2].saturating_sub(1) as u16, press: fin == b'M' })),
            b'M' if params.is_empty() => { // the old encoding: three bytes follow
                if b.len() < total + 3 { return None; }
                let (cb, cx, cy) = (b[total] as u32 - 32, b[total + 1] as u32 - 32, b[total + 2] as u32 - 32);
                let press = cb & 3 != 3 || cb >= 64;
                return Some((total + 3, Some(Input::Mouse(Mouse { b: cb, x: cx.saturating_sub(1) as u16, y: cy.saturating_sub(1) as u16, press }))));
            }
            b'I' if params.is_empty() => Some(Input::Focus(true)),
            b'O' if params.is_empty() => Some(Input::Focus(false)),
            b'Z' => Some(key("shift+tab".into(), raw)),
            b'A' | b'B' | b'C' | b'D' | b'H' | b'F' => {
                let n = match fin { b'A' => "up", b'B' => "down", b'C' => "right", b'D' => "left", b'H' => "home", _ => "end" };
                Some(key(format!("{}{}", mods_prefix(m), n), raw))
            }
            b'P' | b'Q' | b'R' | b'S' if nums.len() == 2 => Some(key(format!("{}f{}", mods_prefix(m), (fin - b'P') + 1), raw)),
            b'~' => match nums[0] {
                200 => { self.paste = Some(Vec::new()); return Some((total, None)); }
                201 => None,
                n => {
                    let name = match n { 1 | 7 => "home", 2 => "insert", 3 => "delete", 4 | 8 => "end", 5 => "pageup", 6 => "pagedown", 11 => "f1", 12 => "f2", 13 => "f3", 14 => "f4", 15 => "f5", 17 => "f6", 18 => "f7", 19 => "f8", 20 => "f9", 21 => "f10", 23 => "f11", 24 => "f12", _ => "" };
                    if name.is_empty() { None } else { Some(key(format!("{}{}", mods_prefix(m), name), raw)) }
                }
            },
            b'u' => { // the kitty keyboard protocol (when a terminal sends it unasked): code point and modifiers
                let base = match nums[0] { 13 => "enter".to_string(), 9 => "tab".into(), 27 => "esc".into(), 127 => "backspace".into(), 32 => "space".into(), c => char::from_u32(c).map(|c| c.to_string()).unwrap_or_default() };
                if base.is_empty() { None } else { Some(key(format!("{}{}", mods_prefix(m), base), raw)) }
            }
            _ => None,
        };
        Some((total, ev))
    }
}

/// legacy: a key in the kitty form (ESC [ code ; modifiers u) as the old terminals send it, for a program that did not ask for the kitty protocol; anything else unchanged.
/// (Shift+Enter is a plain Enter there.)
pub fn legacy(raw: &[u8]) -> Vec<u8> {
    let Some(body) = raw.strip_prefix(b"\x1b[").and_then(|r| r.strip_suffix(b"u")) else { return raw.to_vec() };
    let Ok(t) = std::str::from_utf8(body) else { return raw.to_vec() };
    if t.starts_with(|c: char| !c.is_ascii_digit()) { return raw.to_vec(); }
    let mut it = t.split(';');
    let code: u32 = it.next().and_then(|c| c.split(':').next()).and_then(|c| c.parse().ok()).unwrap_or(0);
    let m = it.next().and_then(|c| c.split(':').next()).and_then(|c| c.parse::<u32>().ok()).unwrap_or(1).max(1) - 1;
    let (shift, alt, ctrl) = (m & 1 != 0, m & 2 != 0, m & 4 != 0);
    let Some(ch) = char::from_u32(code) else { return raw.to_vec() };
    let mut out: Vec<u8> = Vec::new();
    match code {
        9 if shift => { out.extend_from_slice(b"\x1b[Z"); return out; }
        13 | 9 | 27 | 127 => out.push(code as u8),
        _ if ctrl && ch.is_ascii_alphabetic() => out.push(ch.to_ascii_lowercase() as u8 & 0x1f),
        32 if ctrl => out.push(0),
        _ if ctrl && matches!(ch, '[' | '\\' | ']' | '^' | '_') => out.push(ch as u8 & 0x1f),
        _ => { let c = if shift && ch.is_ascii_lowercase() { ch.to_ascii_uppercase() } else { ch }; let mut b = [0u8; 4]; out.extend_from_slice(c.encode_utf8(&mut b).as_bytes()); }
    }
    if alt { out.insert(0, 0x1b); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names(bytes: &[u8]) -> Vec<String> { Parser::default().feed(bytes).into_iter().map(|e| match e { Input::Key(k) => k.name, other => format!("{:?}", other) }).collect() }
    #[test]
    fn keys() {
        assert_eq!(names(b"a\r\t\x7f "), ["a", "enter", "tab", "backspace", "space"]);
        assert_eq!(names(b"\x00\x02\x1c"), ["ctrl+space", "ctrl+b", "ctrl+\\"]);
        assert_eq!(names(b"O?%"), ["shift+o", "?", "%"]);
        assert_eq!(names(b"\x1b[A\x1b[1;5D\x1b[1;7C\x1b[1;8A\x1bOB"), ["up", "ctrl+left", "ctrl+alt+right", "ctrl+alt+shift+up", "down"]);
        assert_eq!(names(b"\x1bx\x1b\r\x1b\x1b\x1b\x14"), ["alt+x", "alt+enter", "alt+esc", "ctrl+alt+t"]);
        assert_eq!(names(b"\x1b[3~\x1b[5~\x1b[1;3H\x1b[Z"), ["delete", "pageup", "alt+home", "shift+tab"]);
        assert_eq!(names("ä日".as_bytes()), ["ä", "日"]);
        assert_eq!(names(b"\x1b[13;2u"), ["shift+enter"]);
        assert_eq!(legacy(b"\x1b[13;2u"), b"\r");
        assert_eq!(legacy(b"\x1b[99;5u"), b"\x03");
        assert_eq!(legacy(b"\x1b[27u"), b"\x1b");
        assert_eq!(legacy(b"\x1b[1;5A"), b"\x1b[1;5A");
    }
    #[test]
    fn split_across_reads() {
        let mut p = Parser::default();
        assert!(p.feed(b"\x1b[1;").is_empty());
        assert!(p.pending());
        let e = p.feed(b"5A");
        assert!(matches!(&e[0], Input::Key(k) if k.name == "ctrl+up" && k.raw == b"\x1b[1;5A"));
        assert!(p.feed(&"日".as_bytes()[..2]).is_empty());
        assert_eq!(p.feed(&"日".as_bytes()[2..]).len(), 1);
        assert!(p.feed(b"\x1b").is_empty());
        assert!(matches!(&p.flush()[0], Input::Key(k) if k.name == "esc"));
    }
    #[test]
    fn mouse_paste_focus() {
        let mut p = Parser::default();
        let e = p.feed(b"\x1b[<0;10;5M\x1b[<0;10;5m\x1b[<64;1;1M\x1b[I\x1b[O");
        assert_eq!(e[0], Input::Mouse(Mouse { b: 0, x: 9, y: 4, press: true }));
        assert_eq!(e[1], Input::Mouse(Mouse { b: 0, x: 9, y: 4, press: false }));
        assert_eq!(e[2], Input::Mouse(Mouse { b: 64, x: 0, y: 0, press: true }));
        assert_eq!((e[3].clone(), e[4].clone()), (Input::Focus(true), Input::Focus(false)));
        let e = p.feed(b"\x1b[M !!");
        assert_eq!(e[0], Input::Mouse(Mouse { b: 0, x: 0, y: 0, press: true }));
        let mut e = p.feed(b"\x1b[200~hello\nwo");
        e.extend(p.feed(b"rld\x1b[20"));
        e.extend(p.feed(b"1~x"));
        assert_eq!(e[0], Input::Paste(b"hello\nworld".to_vec()));
        assert!(matches!(&e[1], Input::Key(k) if k.name == "x"));
    }
}
