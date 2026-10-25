//! The screen of one client as a grid of cells, and the escape codes that turn one grid into the next.
use vt100::Color;

#[derive(Clone, PartialEq, Debug)]
pub struct Cell { pub s: String, pub fg: Color, pub bg: Color, pub bold: bool, pub dim: bool, pub italic: bool, pub underline: bool, pub inverse: bool, pub cont: bool }

impl Default for Cell {
    fn default() -> Cell { Cell { s: String::new(), fg: Color::Default, bg: Color::Default, bold: false, dim: false, italic: false, underline: false, inverse: false, cont: false } }
}

#[derive(Clone, Debug)]
pub struct Grid { pub cols: u16, pub rows: u16, pub cells: Vec<Cell>, pub cursor: Option<(u16, u16)> }

impl Grid {
    pub fn new(cols: u16, rows: u16) -> Grid { Grid { cols, rows, cells: vec![Cell::default(); cols as usize * rows as usize], cursor: None } }
    pub fn get(&self, x: u16, y: u16) -> &Cell { &self.cells[y as usize * self.cols as usize + x as usize] }
    fn set(&mut self, x: u16, y: u16, c: Cell) { if x < self.cols && y < self.rows { let i = y as usize * self.cols as usize + x as usize; self.cells[i] = c; } }

    /// The text of a row (for tests and `pane.read` of the whole frame).
    pub fn row_text(&self, y: u16) -> String {
        let mut s = String::new();
        for x in 0..self.cols { let c = self.get(x, y); if c.cont { continue; } if c.s.is_empty() { s.push(' ') } else { s.push_str(&c.s) } }
        s.trim_end().to_string()
    }

    pub fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, c: &Cell) {
        for yy in y..(y + h).min(self.rows) { for xx in x..(x + w).min(self.cols) { self.set(xx, yy, c.clone()); } }
    }

    /// Text at a position, one cell per character (wide ones take two); stops at the right edge.
    pub fn put(&mut self, x: u16, y: u16, text: &str, st: &Cell) {
        let mut xx = x;
        for ch in text.chars() {
            let w = if wide(ch) { 2 } else { 1 };
            if xx + w > self.cols { break; }
            let mut c = st.clone(); c.s = ch.to_string(); self.set(xx, y, c);
            if w == 2 { let mut k = st.clone(); k.cont = true; self.set(xx + 1, y, k); }
            xx += w;
        }
    }

    /// The (visible part of the) screen of a pane in the rectangle x,y,w,h.
    pub fn blit(&mut self, screen: &vt100::Screen, x: u16, y: u16, w: u16, h: u16) {
        for r in 0..h {
            for c in 0..w {
                let cell = match screen.cell(r, c) { Some(c) => c, None => { self.set(x + c, y + r, Cell::default()); continue; } };
                let mut o = Cell { s: cell.contents().to_string(), fg: cell.fgcolor(), bg: cell.bgcolor(), bold: cell.bold(), dim: cell.dim(), italic: cell.italic(), underline: cell.underline(), inverse: cell.inverse(), cont: cell.is_wide_continuation() };
                if cell.is_wide() && c + 1 >= w { o = Cell { bg: o.bg, ..Cell::default() }; }
                self.set(x + c, y + r, o);
            }
        }
    }
}

pub fn wide(c: char) -> bool { unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) == 2 }

fn sgr(c: &Cell) -> String {
    let mut s = String::from("\x1b[0");
    if c.bold { s.push_str(";1") }
    if c.dim { s.push_str(";2") }
    if c.italic { s.push_str(";3") }
    if c.underline { s.push_str(";4") }
    if c.inverse { s.push_str(";7") }
    match c.fg { Color::Default => {}, Color::Idx(n) => s.push_str(&format!(";38;5;{n}")), Color::Rgb(r, g, b) => s.push_str(&format!(";38;2;{r};{g};{b}")) }
    match c.bg { Color::Default => {}, Color::Idx(n) => s.push_str(&format!(";48;5;{n}")), Color::Rgb(r, g, b) => s.push_str(&format!(";48;2;{r};{g};{b}")) }
    s.push('m'); s
}

fn same_style(a: &Cell, b: &Cell) -> bool { a.fg == b.fg && a.bg == b.bg && a.bold == b.bold && a.dim == b.dim && a.italic == b.italic && a.underline == b.underline && a.inverse == b.inverse }

/// The bytes that turn `prev` (what the terminal shows) into `now`; without `prev` (or with another size) a full redraw.
/// The cursor is placed and shown or hidden at the end.
pub fn diff(prev: Option<&Grid>, now: &Grid) -> String {
    let mut out = String::new();
    let blank;
    let prev = match prev {
        Some(p) if p.cols == now.cols && p.rows == now.rows => p,
        _ => { out.push_str("\x1b[0m\x1b[2J"); blank = Grid::new(now.cols, now.rows); &blank }
    };
    let mut cur = Cell::default(); // the style the terminal has now
    let mut pos: Option<(u16, u16)> = None;
    out.push_str("\x1b[?25l");
    for y in 0..now.rows {
        for x in 0..now.cols {
            let c = now.get(x, y);
            if c.cont || c == prev.get(x, y) { continue; }
            if pos != Some((x, y)) { out.push_str(&format!("\x1b[{};{}H", y + 1, x + 1)); }
            if !same_style(&cur, c) { out.push_str(&sgr(c)); cur = c.clone(); }
            if c.s.is_empty() { out.push(' ') } else { out.push_str(&c.s) }
            let w = if c.s.chars().next().map_or(false, wide) { 2 } else { 1 };
            pos = if x + w >= now.cols { None } else { Some((x + w, y)) };
        }
    }
    out.push_str("\x1b[0m");
    if let Some((cx, cy)) = now.cursor { out.push_str(&format!("\x1b[{};{}H\x1b[?25h", cy + 1, cx + 1)); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn show(g: &Grid) -> vt100::Parser { let mut p = super::super::parser(g.rows, g.cols, 0); p.process(diff(None, g).as_bytes()); p }
    #[test]
    fn diff_round_trip() {
        let mut a = Grid::new(20, 5);
        let st = Cell { bold: true, fg: Color::Idx(2), ..Cell::default() };
        a.put(1, 1, "hello", &st);
        a.put(3, 2, "日本", &Cell { bg: Color::Rgb(1, 2, 3), ..Cell::default() });
        a.cursor = Some((4, 1));
        let mut p = show(&a);
        assert_eq!(p.screen().rows(0, 20).nth(1).unwrap(), " hello");
        assert_eq!(p.screen().rows(0, 20).nth(2).unwrap(), "   日本");
        let mut b = a.clone();
        b.put(1, 1, "J", &Cell::default()); b.put(19, 4, "z", &Cell::default());
        p.process(diff(Some(&a), &b).as_bytes());
        assert_eq!(p.screen().rows(0, 20).nth(1).unwrap(), " Jello");
        assert_eq!(p.screen().rows(0, 20).nth(4).unwrap(), "                   z");
        assert_eq!(p.screen().contents_formatted(), show(&b).screen().contents_formatted());
        assert_eq!(p.screen().cursor_position(), (1, 4));
    }
    #[test]
    fn no_change_is_nearly_empty() {
        let a = Grid::new(10, 3);
        assert!(diff(Some(&a), &a).len() < 20);
    }
}
