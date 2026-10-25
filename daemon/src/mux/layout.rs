//! The split tree of a tab: panes are leaves, a split cuts its rectangle in two with a one-cell divider between.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir { Right, Down }

/// How a split is cut: by its ratio, or the first / second child has a fixed size in cells (the panels: a bigger window gives the room to the other side).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fixed { None, First(u16), Second(u16) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side { Left, Right, Up, Down }

impl Side {
    pub fn parse(s: &str) -> Option<Side> { match s { "left" => Some(Side::Left), "right" => Some(Side::Right), "up" => Some(Side::Up), "down" => Some(Side::Down), _ => None } }
    fn axis(self) -> Dir { match self { Side::Left | Side::Right => Dir::Right, _ => Dir::Down } }
    /// the divider lies behind the pane (right/down of it) when the pane is the first child
    fn forward(self) -> bool { matches!(self, Side::Right | Side::Down) }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect { pub x: u16, pub y: u16, pub w: u16, pub h: u16 }

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Leaf(u32),
    /// `ratio`: the share of the first child (of the space without the divider)
    Split { dir: Dir, ratio: f32, fixed: Fixed, a: Box<Node>, b: Box<Node> },
}

/// A divider: where it is, along which axis it runs and the path of its split in the tree (false = first child).
#[derive(Clone, Debug, PartialEq)]
pub struct Divider { pub dir: Dir, pub x: u16, pub y: u16, pub len: u16, pub path: Vec<bool>, pub rect: Rect, /// the size of the first child
    pub first: u16 }

/// The size of the first child of a split over `total` cells (the divider takes one).
fn cut(total: u16, ratio: f32, fixed: Fixed) -> u16 {
    let avail = total.saturating_sub(1);
    if avail < 2 { return avail; }
    let first = match fixed { Fixed::None => (avail as f32 * ratio).round() as u16, Fixed::First(n) => n, Fixed::Second(n) => avail.saturating_sub(n) };
    first.clamp(1, avail - 1)
}

impl Node {
    pub fn leaves(&self, out: &mut Vec<u32>) { match self { Node::Leaf(p) => out.push(*p), Node::Split { a, b, .. } => { a.leaves(out); b.leaves(out); } } }
    pub fn contains(&self, id: u32) -> bool { match self { Node::Leaf(p) => *p == id, Node::Split { a, b, .. } => a.contains(id) || b.contains(id) } }

    pub fn rects(&self, r: Rect, out: &mut Vec<(u32, Rect)>, divs: &mut Vec<Divider>) { self.walk(r, out, divs, &mut Vec::new()) }
    fn walk(&self, r: Rect, out: &mut Vec<(u32, Rect)>, divs: &mut Vec<Divider>, path: &mut Vec<bool>) {
        match self {
            Node::Leaf(p) => out.push((*p, r)),
            Node::Split { dir, ratio, fixed, a, b } => {
                let (ra, rb, div) = match dir {
                    Dir::Right => { let wa = cut(r.w, *ratio, *fixed); (Rect { w: wa, ..r }, Rect { x: r.x + wa + 1, w: r.w.saturating_sub(wa + 1), ..r }, (r.x + wa, r.y, r.h)) }
                    Dir::Down => { let ha = cut(r.h, *ratio, *fixed); (Rect { h: ha, ..r }, Rect { y: r.y + ha + 1, h: r.h.saturating_sub(ha + 1), ..r }, (r.x, r.y + ha, r.w)) }
                };
                divs.push(Divider { dir: *dir, x: div.0, y: div.1, len: div.2, path: path.clone(), rect: r, first: match dir { Dir::Right => ra.w, Dir::Down => ra.h } });
                path.push(false); a.walk(ra, out, divs, path); path.pop();
                path.push(true); b.walk(rb, out, divs, path); path.pop();
            }
        }
    }

    /// `target` becomes a split of itself (first child) and `new` (second child); `ratio` is what the old pane keeps.
    pub fn split(&mut self, target: u32, new: u32, dir: Dir, ratio: f32) -> bool {
        match self {
            Node::Leaf(p) if *p == target => { *self = Node::Split { dir, ratio: ratio.clamp(0.05, 0.95), fixed: Fixed::None, a: Box::new(Node::Leaf(target)), b: Box::new(Node::Leaf(new)) }; true }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => a.split(target, new, dir, ratio) || b.split(target, new, dir, ratio),
        }
    }

    /// Takes a pane out; its sibling takes the place of their split. False when it is the only pane (or not there).
    pub fn remove(&mut self, target: u32) -> bool {
        match self {
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => {
                if matches!(**a, Node::Leaf(p) if p == target) { *self = (**b).clone(); return true; }
                if matches!(**b, Node::Leaf(p) if p == target) { *self = (**a).clone(); return true; }
                a.remove(target) || b.remove(target)
            }
        }
    }

    pub fn swap(&mut self, x: u32, y: u32) {
        match self {
            Node::Leaf(p) => { if *p == x { *p = y } else if *p == y { *p = x } }
            Node::Split { a, b, .. } => { a.swap(x, y); b.swap(x, y); }
        }
    }

    fn at_mut(&mut self, path: &[bool]) -> Option<&mut Node> {
        let mut n = self;
        for &second in path { match n { Node::Split { a, b, .. } => n = if second { b } else { a }, _ => return None } }
        Some(n)
    }

    /// The splits above a pane, root first: (path, axis, the pane lies in the first child).
    fn ancestors(&self, target: u32) -> Vec<(Vec<bool>, Dir, bool)> {
        let mut out = Vec::new();
        let mut path = Vec::new();
        let mut n = self;
        loop {
            match n {
                Node::Leaf(_) => return out,
                Node::Split { dir, a, b, .. } => {
                    let in_a = a.contains(target);
                    if !in_a && !b.contains(target) { return Vec::new(); }
                    out.push((path.clone(), *dir, in_a));
                    path.push(!in_a);
                    n = if in_a { a } else { b };
                }
            }
        }
    }

    /// Moves a divider of the pane by `amount` (a share of that split) toward `side`. The divider on that side of the pane
    /// is taken; a pane at the edge moves its inner divider that way (the nearest split along the axis).
    pub fn resize(&mut self, target: u32, side: Side, amount: f32, area: Rect) -> bool {
        let anc = self.ancestors(target);
        let pick = anc.iter().rev().find(|(_, d, in_a)| *d == side.axis() && *in_a == side.forward()).or_else(|| anc.iter().rev().find(|(_, d, _)| *d == side.axis())).cloned();
        let Some((path, _, _)) = pick else { return false };
        let (mut out, mut divs) = (Vec::new(), Vec::new());
        self.rects(area, &mut out, &mut divs);
        let Some(d) = divs.iter().find(|d| d.path == path) else { return false };
        let total = match d.dir { Dir::Right => d.rect.w, Dir::Down => d.rect.h }.saturating_sub(1).max(1);
        let delta = ((amount * total as f32).round() as i32).max(1);
        let pos = d.first as i32 + if side.forward() { delta } else { -delta };
        self.set_divider(&path, d.rect, pos.clamp(1, total as i32 - 1) as u16);
        true
    }

    /// The divider at `path` is put `pos` cells from the start of `rect` (a split with a fixed size keeps that size).
    pub fn set_divider(&mut self, path: &[bool], rect: Rect, pos: u16) {
        if let Some(Node::Split { dir, ratio, fixed, .. }) = self.at_mut(path) {
            let total = match dir { Dir::Right => rect.w, Dir::Down => rect.h }.saturating_sub(1).max(1);
            let pos = pos.clamp(1, total.saturating_sub(1).max(1));
            *fixed = match *fixed { Fixed::None => Fixed::None, Fixed::First(_) => Fixed::First(pos), Fixed::Second(_) => Fixed::Second(total - pos) };
            *ratio = (pos as f32 / total as f32).clamp(0.05, 0.95);
        }
    }

    /// The split that holds this pane gets a fixed size for it (cells): its side keeps that size when the window changes.
    pub fn fix_size_of(&mut self, pane: u32, size: u16) -> bool {
        match self {
            Node::Leaf(_) => false,
            Node::Split { fixed, a, b, .. } => {
                if matches!(**a, Node::Leaf(p) if p == pane) { *fixed = Fixed::First(size); return true; }
                if matches!(**b, Node::Leaf(p) if p == pane) { *fixed = Fixed::Second(size); return true; }
                a.fix_size_of(pane, size) || b.fix_size_of(pane, size)
            }
        }
    }
}

/// The pane next to `target` on a side: the nearest one that touches its edge with an overlap (the first in reading order on a tie).
pub fn neighbor(rects: &[(u32, Rect)], target: u32, side: Side) -> Option<u32> {
    let t = rects.iter().find(|(p, _)| *p == target)?.1;
    let (t_end_x, t_end_y) = (t.x + t.w, t.y + t.h);
    let mut best: Option<(u32, u32, u16)> = None; // pane, distance, overlap
    for (p, r) in rects {
        if *p == target { continue; }
        let (dist, overlap) = match side {
            Side::Left if r.x + r.w <= t.x => (t.x - (r.x + r.w), ov(t.y, t_end_y, r.y, r.y + r.h)),
            Side::Right if r.x >= t_end_x => (r.x - t_end_x, ov(t.y, t_end_y, r.y, r.y + r.h)),
            Side::Up if r.y + r.h <= t.y => (t.y - (r.y + r.h), ov(t.x, t_end_x, r.x, r.x + r.w)),
            Side::Down if r.y >= t_end_y => (r.y - t_end_y, ov(t.x, t_end_x, r.x, r.x + r.w)),
            _ => continue,
        };
        if overlap == 0 { continue; }
        let d = dist as u32;
        if best.map_or(true, |(_, bd, bo)| d < bd || (d == bd && overlap > bo)) { best = Some((*p, d, overlap)); }
    }
    best.map(|b| b.0)
}
fn ov(a0: u16, a1: u16, b0: u16, b1: u16) -> u16 { a1.min(b1).saturating_sub(a0.max(b0)) }

#[cfg(test)]
mod tests {
    use super::*;
    fn rects(n: &Node, w: u16, h: u16) -> Vec<(u32, Rect)> { let (mut o, mut d) = (Vec::new(), Vec::new()); n.rects(Rect { x: 0, y: 0, w, h }, &mut o, &mut d); o }

    #[test]
    fn split_and_rects() {
        let mut n = Node::Leaf(1);
        assert!(n.split(1, 2, Dir::Right, 0.5));
        let r = rects(&n, 81, 24);
        assert_eq!(r[0], (1, Rect { x: 0, y: 0, w: 40, h: 24 }));
        assert_eq!(r[1], (2, Rect { x: 41, y: 0, w: 40, h: 24 }));
        n.split(2, 3, Dir::Down, 0.5);
        let r = rects(&n, 81, 25);
        assert_eq!(r[1].1, Rect { x: 41, y: 0, w: 40, h: 12 });
        assert_eq!(r[2].1, Rect { x: 41, y: 13, w: 40, h: 12 });
        // the rectangles plus the dividers cover the area: no overlap
        let mut seen = std::collections::HashSet::new();
        for (_, q) in &r { for x in q.x..q.x + q.w { for y in q.y..q.y + q.h { assert!(seen.insert((x, y))); } } }
    }

    #[test]
    fn remove_swap_neighbor() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right, 0.5); n.split(2, 3, Dir::Down, 0.5);
        let r = rects(&n, 81, 25);
        assert_eq!(neighbor(&r, 1, Side::Right), Some(2));
        assert_eq!(neighbor(&r, 3, Side::Up), Some(2));
        assert_eq!(neighbor(&r, 3, Side::Left), Some(1));
        assert_eq!(neighbor(&r, 1, Side::Left), None);
        n.swap(1, 3);
        assert_eq!(rects(&n, 81, 25)[0].0, 3);
        assert!(n.remove(2));
        let mut l = Vec::new(); n.leaves(&mut l); assert_eq!(l, vec![3, 1]);
        assert!(n.remove(3)); assert_eq!(n, Node::Leaf(1));
        assert!(!n.remove(1));
    }

    #[test]
    fn a_fixed_size_stays_when_the_window_grows() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right, 0.5);
        n.fix_size_of(2, 30);   // pane 2 (second child) is 30 cells
        assert_eq!(rects(&n, 101, 10)[1].1.w, 30);
        assert_eq!(rects(&n, 201, 10)[1].1.w, 30);
        assert_eq!(rects(&n, 201, 10)[0].1.w, 170);
        let area = Rect { x: 0, y: 0, w: 201, h: 10 };
        assert!(n.resize(2, Side::Left, 0.05, area)); // grows by about 10 cells
        assert!(rects(&n, 201, 10)[1].1.w >= 38);
        let w = rects(&n, 201, 10)[1].1.w;
        assert_eq!(rects(&n, 301, 10)[1].1.w, w, "the new size is kept, too");
    }

    #[test]
    fn resize_moves_the_divider() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right, 0.5);
        let area = Rect { x: 0, y: 0, w: 81, h: 24 };
        assert!(n.resize(2, Side::Left, 0.25, area)); // pane 2 grows to the left
        match &n { Node::Split { ratio, .. } => assert!((ratio - 0.25).abs() < 0.02, "{ratio}"), _ => panic!() }
        assert!(n.resize(1, Side::Left, 0.05, area)); // pane 1 sits at the edge: its inner divider still moves left
        match &n { Node::Split { ratio, .. } => assert!((ratio - 0.20).abs() < 0.03, "{ratio}"), _ => panic!() }
        assert!(!n.resize(1, Side::Up, 0.1, area));
    }
}
