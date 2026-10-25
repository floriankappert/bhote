//! The keys of the mux: `~/.config/bhote/keys`, analogous to herdr's [keys] (same action names, prefix key, `prefix+x` chords).
//!
//!     prefix = ctrl+b
//!     split_vertical = prefix+v alt+enter     # action = key key ...   (`prefix+` first: after the prefix key)
//!     switch_tab = prefix+1..9 alt+1..9        # a digit range: the key's digit is the argument
//!     run prefix+t notify-send hello           # run <key> <shell command>
//!     popup ctrl+alt+t bhote find              # popup <key> <shell command>: a floating pane over the tab
//!     close_pane = none                        # removes the keys of an action
use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
pub struct Bind { pub action: String, pub arg: Option<String> }

pub struct Keymap { pub prefix: String, prefixed: HashMap<String, Bind>, global: HashMap<String, Bind> }

pub const DEFAULTS: &str = "\
prefix = ctrl+b
detach = prefix+d
copy_mode = prefix+[
help = prefix+?
split_vertical = prefix+v prefix+%
split_horizontal = prefix+h prefix+\"
close_pane = prefix+x alt+esc
zoom = prefix+z
last_pane = prefix+;
focus_pane_left = prefix+left ctrl+alt+left
focus_pane_down = prefix+down ctrl+alt+down
focus_pane_up = prefix+up ctrl+alt+up
focus_pane_right = prefix+right ctrl+alt+right
resize_mode = prefix+ctrl+left prefix+ctrl+down prefix+ctrl+up prefix+ctrl+right prefix+r
resize_pane_left = ctrl+alt+shift+left
resize_pane_down = ctrl+alt+shift+down
resize_pane_up = ctrl+alt+shift+up
resize_pane_right = ctrl+alt+shift+right
rename_pane = prefix+shift+o
new_tab = prefix+c
rename_tab = prefix+,
close_tab = prefix+k
switch_tab = prefix+1..9 alt+1..9
previous_tab = prefix+p alt+left
next_tab = prefix+n alt+right
move_tab_previous = alt+shift+left
move_tab_next = alt+shift+right
new_workspace = prefix+shift+c
rename_workspace = prefix+shift+r
close_workspace = prefix+shift+k
previous_workspace = prefix+shift+p alt+up
next_workspace = prefix+shift+n alt+down
reload_config = prefix+q
focus_panel = prefix+t
toggle_sidebar = prefix+b
open_panel = prefix+shift+b
";

/// A key as written in a file, as the input parser names it: modifiers in the order ctrl, alt, shift; capitals are shift+letter.
pub fn canon(spec: &str) -> String {
    let s = spec.trim();
    let (mods, key) = match s.rfind('+') { Some(i) if i + 1 < s.len() => (&s[..i], &s[i + 1..]), _ => ("", s) };
    let alias = |k: &str| match k.to_ascii_lowercase().as_str() { "return" => "enter".to_string(), "escape" => "esc".into(), "pgup" => "pageup".into(), "pgdn" | "pgdown" => "pagedown".into(), "del" => "delete".into(), "bs" => "backspace".into(), "plus" => "+".into(), l if l.chars().count() > 1 => l.to_string(), _ => k.to_string() };
    let mut key = alias(key);
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for m in mods.split('+').filter(|m| !m.is_empty()) { match m.to_ascii_lowercase().as_str() { "ctrl" | "control" | "c" => ctrl = true, "alt" | "meta" | "m" => alt = true, "shift" | "s" => shift = true, _ => {} } }
    if key.chars().count() == 1 { let c = key.chars().next().unwrap(); if c.is_uppercase() { shift = true; key = c.to_lowercase().to_string(); } }
    let mut out = String::new();
    if ctrl { out.push_str("ctrl+"); } if alt { out.push_str("alt+"); } if shift { out.push_str("shift+"); }
    out.push_str(&key); out
}

impl Keymap {
    pub fn parse(text: &str) -> Keymap {
        let mut km = Keymap { prefix: "ctrl+b".into(), prefixed: HashMap::new(), global: HashMap::new() };
        km.apply(text);
        km
    }

    /// The defaults, then the user's file on top.
    pub fn load(path: &std::path::Path) -> Keymap {
        let mut km = Keymap::parse(DEFAULTS);
        if let Ok(t) = std::fs::read_to_string(path) { km.apply(&t); }
        km
    }

    fn bind(&mut self, spec: &str, b: Bind) {
        let (scope, key) = match spec.strip_prefix("prefix+") { Some(k) => (true, k), None => (false, spec) };
        let table = if scope { &mut self.prefixed } else { &mut self.global };
        // a digit range 1..9
        if let Some((lo, hi)) = key.split_once("..") {
            if let (Ok(l), Ok(h)) = (lo.chars().last().map(|c| c.to_string()).unwrap_or_default().parse::<u32>(), hi.parse::<u32>()) {
                let head = &lo[..lo.len() - 1];
                for d in l..=h { table.insert(canon(&format!("{head}{d}")), Bind { action: b.action.clone(), arg: Some(d.to_string()) }); }
                return;
            }
        }
        table.insert(canon(key), b);
    }

    pub fn apply(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.split(" #").next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            for verb in ["run", "popup"] {
                if let Some(rest) = line.strip_prefix(verb).and_then(|r| r.strip_prefix(' ')) {
                    if let Some((k, cmd)) = rest.trim().split_once(char::is_whitespace) { self.bind(k, Bind { action: verb.into(), arg: Some(cmd.trim().to_string()) }); }
                    continue;
                }
            }
            if line.starts_with("run ") || line.starts_with("popup ") { continue; }
            let Some((name, keys)) = line.split_once('=') else { continue };
            let (name, keys) = (name.trim(), keys.trim());
            if name == "prefix" { self.prefix = canon(keys); continue; }
            // the old keys of the action go (a line replaces them)
            self.prefixed.retain(|_, b| b.action != name);
            self.global.retain(|_, b| b.action != name);
            if keys == "none" { continue; }
            for k in keys.split_whitespace() { self.bind(k, Bind { action: name.to_string(), arg: None }); }
        }
    }

    pub fn prefixed(&self, key: &str) -> Option<&Bind> { self.prefixed.get(key) }
    pub fn global(&self, key: &str) -> Option<&Bind> { self.global.get(key) }

    /// The first key of an action, as shown to the user ("prefix+d").
    pub fn show(&self, action: &str) -> Option<String> {
        let mut p: Vec<&String> = self.prefixed.iter().filter(|(_, b)| b.action == action).map(|(k, _)| k).collect(); p.sort();
        if let Some(k) = p.first() { return Some(format!("prefix+{k}")); }
        let mut g: Vec<&String> = self.global.iter().filter(|(_, b)| b.action == action).map(|(k, _)| k).collect(); g.sort();
        g.first().map(|k| k.to_string())
    }

    /// All bindings for the help screen: (keys, action).
    pub fn list(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.prefixed.iter().map(|(k, b)| (format!("prefix+{k}"), b.action.clone())).chain(self.global.iter().map(|(k, b)| (k.clone(), b.action.clone()))).collect();
        v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0))); v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_names() {
        assert_eq!(canon("Ctrl+Alt+Shift+Left"), "ctrl+alt+shift+left");
        assert_eq!(canon("shift+alt+enter"), "alt+shift+enter");
        assert_eq!(canon("O"), "shift+o");
        assert_eq!(canon("%"), "%");
        assert_eq!(canon("ctrl+space"), "ctrl+space");
        assert_eq!(canon("escape"), "esc");
    }
    #[test]
    fn defaults_and_overrides() {
        let mut k = Keymap::parse(DEFAULTS);
        assert_eq!(k.prefix, "ctrl+b");
        assert_eq!(k.prefixed("d").unwrap().action, "detach");
        assert_eq!(k.prefixed("3").unwrap(), &Bind { action: "switch_tab".into(), arg: Some("3".into()) });
        assert_eq!(k.global("alt+2").unwrap().arg.as_deref(), Some("2"));
        assert_eq!(k.prefixed("shift+o").unwrap().action, "rename_pane");
        k.apply("prefix = ctrl+space\ndetach = prefix+q\nzoom = none\nrun prefix+t echo hi there  # note\n");
        assert_eq!(k.prefix, "ctrl+space");
        assert!(k.prefixed("d").is_none());
        assert_eq!(k.prefixed("q").unwrap().action, "detach"); // (reload_config had q: the later line wins for the key)
        assert!(k.prefixed("z").is_none());
        assert_eq!(k.prefixed("t").unwrap(), &Bind { action: "run".into(), arg: Some("echo hi there".into()) });
        assert_eq!(k.show("detach").as_deref(), Some("prefix+q"));
    }
}
