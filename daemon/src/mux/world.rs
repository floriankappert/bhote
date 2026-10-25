//! Workspaces, tabs and their split trees. Panes are only ids here; the server owns them.
use super::layout::{Divider, Node, Rect};

pub struct Tab {
    pub id: u32, pub label: String, pub root: Node, pub focus: u32, pub zoomed: Option<u32>,
    /// a floating pane over the tab (not in the tree) and the pane that had the focus before it
    pub popup: Option<u32>, pub popup_prev: u32,
    /// the user closed the panels of this tab: they are not made again by themselves
    pub no_panels: bool,
}

impl Tab {
    pub fn new(id: u32, label: String, pane: u32) -> Tab { Tab { id, label, root: Node::Leaf(pane), focus: pane, zoomed: None, popup: None, popup_prev: pane, no_panels: false } }
}

pub struct Workspace { pub id: u32, pub label: String, pub cwd: String, pub tabs: Vec<Tab>, pub active: usize, pub next_tab: u32, pub next_pane: u32 }

#[derive(Default)]
pub struct World { pub ws: Vec<Workspace>, pub active: usize, pub next_ws: u32 }

pub fn ws_name(w: u32) -> String { format!("w{w}") }
pub fn tab_name(w: u32, t: u32) -> String { format!("w{w}:t{t}") }
pub fn pane_name(w: u32, n: u32) -> String { format!("w{w}:p{n}") }

/// "w2:p5" -> (2, 5); the same for tabs ("w2:t1") and a workspace ("w2").
pub fn parse_name(s: &str, kind: char) -> Option<(u32, u32)> {
    let s = s.strip_prefix('w')?;
    let (w, rest) = match s.split_once(':') { Some((w, r)) => (w, Some(r)), None => (s, None) };
    let w = w.parse().ok()?;
    match (kind, rest) {
        ('w', None) => Some((w, 0)),
        (k, Some(r)) if r.starts_with(k) => Some((w, r[1..].parse().ok()?)),
        _ => None,
    }
}

impl World {
    pub fn new_workspace(&mut self, label: &str, cwd: &str) -> usize {
        self.next_ws += 1;
        self.ws.push(Workspace { id: self.next_ws, label: label.to_string(), cwd: cwd.to_string(), tabs: Vec::new(), active: 0, next_tab: 1, next_pane: 1 });
        self.ws.len() - 1
    }
    /// A workspace with the id and counters it had (after a restart: the names of its panes stay the same).
    pub fn restore_workspace(&mut self, id: u32, label: &str, cwd: &str, next_tab: u32, next_pane: u32) -> usize {
        self.next_ws = self.next_ws.max(id);
        self.ws.push(Workspace { id, label: label.to_string(), cwd: cwd.to_string(), tabs: Vec::new(), active: 0, next_tab, next_pane });
        self.ws.len() - 1
    }
    pub fn ws_index(&self, id: u32) -> Option<usize> { self.ws.iter().position(|w| w.id == id) }
    pub fn current(&self) -> Option<(&Workspace, &Tab)> { let w = self.ws.get(self.active)?; Some((w, w.tabs.get(w.active)?)) }
    pub fn current_mut(&mut self) -> Option<&mut Tab> { let w = self.ws.get_mut(self.active)?; w.tabs.get_mut(w.active) }
    pub fn focused_pane(&self) -> Option<u32> { self.current().map(|(_, t)| t.focus) }

    /// Workspace and tab index of a tab by its names.
    pub fn find_tab(&self, ws: u32, tab: u32) -> Option<(usize, usize)> {
        let wi = self.ws_index(ws)?;
        let ti = self.ws[wi].tabs.iter().position(|t| t.id == tab)?;
        Some((wi, ti))
    }
    pub fn tab_of_pane(&self, pane: u32) -> Option<(usize, usize)> {
        for (wi, w) in self.ws.iter().enumerate() { for (ti, t) in w.tabs.iter().enumerate() { if t.root.contains(pane) || t.popup == Some(pane) { return Some((wi, ti)); } } }
        None
    }

    /// Shows the tab of a pane and gives the pane the focus.
    pub fn focus_pane(&mut self, pane: u32) -> bool {
        let Some((wi, ti)) = self.tab_of_pane(pane) else { return false };
        self.active = wi; self.ws[wi].active = ti; self.ws[wi].tabs[ti].focus = pane;
        true
    }

    /// Rectangles of all panes of a tab in an area (a zoomed pane takes it all).
    pub fn rects(tab: &Tab, area: Rect) -> (Vec<(u32, Rect)>, Vec<Divider>) {
        let (mut out, mut divs) = (Vec::new(), Vec::new());
        match tab.zoomed.filter(|z| tab.root.contains(*z)) {
            Some(z) => out.push((z, area)),
            None => tab.root.rects(area, &mut out, &mut divs),
        }
        (out, divs)
    }

    /// Where a popup is: 70% of the area, in the middle (the pane inside has a border of one cell).
    pub fn popup_rect(area: Rect) -> Rect {
        let (w, h) = ((area.w as u32 * 7 / 10).max(10).min(area.w as u32) as u16, (area.h as u32 * 7 / 10).max(5).min(area.h as u32) as u16);
        Rect { x: area.x + (area.w - w) / 2, y: area.y + (area.h - h) / 2, w, h }
    }

    /// Drops a pane from its tab; an empty tab goes, and with it an empty workspace. Returns false when the pane was in no tab.
    pub fn remove_pane(&mut self, pane: u32) -> bool {
        let Some((wi, ti)) = self.tab_of_pane(pane) else { return false };
        let tab = &mut self.ws[wi].tabs[ti];
        if tab.popup == Some(pane) { tab.popup = None; tab.focus = if tab.root.contains(tab.popup_prev) { tab.popup_prev } else { let mut l = Vec::new(); tab.root.leaves(&mut l); l[0] }; return true; }
        if tab.root.remove(pane) {
            if tab.zoomed == Some(pane) { tab.zoomed = None; }
            if tab.focus == pane { let mut l = Vec::new(); tab.root.leaves(&mut l); tab.focus = l[0]; }
            return true;
        }
        self.ws[wi].tabs.remove(ti);
        let w = &mut self.ws[wi];
        if w.active >= w.tabs.len() { w.active = w.tabs.len().saturating_sub(1); } else if ti < w.active { w.active -= 1; }
        if w.tabs.is_empty() {
            self.ws.remove(wi);
            if self.active >= self.ws.len() { self.active = self.ws.len().saturating_sub(1); } else if wi < self.active { self.active -= 1; }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(parse_name("w7:p5", 'p'), Some((7, 5)));
        assert_eq!(parse_name("w7:t1", 't'), Some((7, 1)));
        assert_eq!(parse_name("w7", 'w'), Some((7, 0)));
        assert_eq!(parse_name("w7:p5", 't'), None);
        assert_eq!(parse_name("p5", 'p'), None);
    }
    #[test]
    fn remove_pane_cascades() {
        let mut w = World::default();
        let a = w.new_workspace("a", "/");
        w.ws[a].tabs.push(Tab::new(1, "1".into(), 10));
        let b = w.new_workspace("b", "/");
        w.ws[b].tabs.push(Tab::new(1, "1".into(), 20));
        w.active = 1;
        assert!(w.remove_pane(20));
        assert_eq!((w.ws.len(), w.active), (1, 0));
        assert_eq!(w.focused_pane(), Some(10));
    }
}
