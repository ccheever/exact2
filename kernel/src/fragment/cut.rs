//! @ref LLP 1093 D3–D5 — the cut: a walk over the flow thread in document
//! order, as Chrome 154 breaks, and Chrome's column balancer over it.
//!
//! Each candidate break gets an *appeal* (Chrome's term), best first:
//! perfect; violating `orphans`/`widows`; violating `break-*: avoid`. When
//! the next line or box would cross the column's end, the walk breaks at the
//! latest candidate of the best appeal seen since the column began; a forced
//! break ends the column wherever it is. Every column takes at least one line
//! or box, so the walk always makes progress.

use super::build::{self, Flow, Para};
use super::{Column, Columns, Fragment, FragmentRefusal, Placement};
use crate::arena::NodeArena;
use crate::generated::{BoxSizing, ColumnFill, Direction, NodeType, StyleMask};
use crate::id::AxisOffer;
use crate::layout::LayoutTree;
use crate::text::{TextMeasureRequest, TextMeasurer, TextRun};
use crate::Dimension;

/// What one container's cut publishes.
#[derive(Debug, Default)]
pub(crate) struct Cut {
    pub used_height: Option<f32>,
    pub overflow: Option<taffy::Rect<f32>>,
    pub columns: Columns,
    pub placements: Vec<(u32, Placement)>,
    pub fragments: Vec<(u32, Vec<super::Fragment>)>,
    pub refusals: Vec<(u32, FragmentRefusal)>,
    /// Walks the cut took (LLP 1093 §3's probe).
    #[cfg(test)]
    pub walks: usize,
    /// The container's border box once its used height applies.
    pub size: (f32, f32),
    /// Whether an absolutely positioned box in the flow placed against the
    /// container's final size counts in its overflow.
    pub absolutes: bool,
}

// A unit of the walk: an atom, or one line box of an expanded paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Pos {
    pub atom: usize,
    pub line: usize,
}

// Chrome's break appeal, worst first.
const AVOID: u8 = 1;
const ORPHANS_WIDOWS: u8 = 2;
const PERFECT: u8 = 3;
const EPSILON: f32 = 0.01;

/// One walk at one column height.
#[derive(Debug, Default)]
pub(super) struct Walk {
    /// Where each column starts, and the flow-thread y it shows at its top.
    pub starts: Vec<(Pos, f32)>,
    /// The least addition to the column height that would let some column
    /// end later than it does; `None` when none would (forced breaks).
    pub shortage: Option<f32>,
    /// Kernel-only monolithic atoms that did not fit their column.
    pub refused: Vec<usize>,
}

pub(super) struct Cutter<'a> {
    pub flow: Flow,
    arena: &'a NodeArena,
    measurer: &'a mut dyn TextMeasurer,
    /// The first column's top: the container's content top.
    pub top: f32,
    /// Walks run, for the probe (LLP 1093 §3).
    #[cfg(test)]
    pub walks: usize,
}

impl<'a> Cutter<'a> {
    pub(super) fn new(
        flow: Flow,
        arena: &'a NodeArena,
        measurer: &'a mut dyn TextMeasurer,
        top: f32,
    ) -> Self {
        Cutter {
            flow,
            arena,
            measurer,
            top,
            #[cfg(test)]
            walks: 0,
        }
    }

    // A paragraph's line boxes, asked of the host once (D6): the walk asks
    // only when the paragraph would cross a column's end.
    fn expand(&mut self, para: usize) {
        let p = &self.flow.paras[para];
        if p.lines.is_some() {
            return;
        }
        let bottoms = lines(self.arena, self.measurer, p.slot, p.width).1;
        self.flow.paras[para].lines = Some(bottoms);
    }

    // Expand every paragraph whose lines are unknown among atoms
    // `from..to`; whether there was one.
    fn expand_between(&mut self, from: usize, to: usize) -> bool {
        let mut any = false;
        for atom in from..to {
            if let Some(p) = self.flow.atoms[atom].para {
                if self.flow.paras[p].lines.is_none() {
                    self.expand(p);
                    any = true;
                }
            }
        }
        any
    }

    fn lines_of(&self, atom: usize) -> Option<&[f32]> {
        let para = self.flow.atoms[atom].para?;
        self.flow.paras[para]
            .lines
            .as_deref()
            .filter(|l| l.len() > 1)
    }

    pub(super) fn units(&self, atom: usize) -> usize {
        self.lines_of(atom).map_or(1, <[f32]>::len)
    }

    fn unit(&self, pos: Pos) -> (f32, f32) {
        let atom = &self.flow.atoms[pos.atom];
        match self.lines_of(pos.atom) {
            Some(lines) => {
                let top = atom.para.map_or(0.0, |p| self.flow.paras[p].content_top);
                let start = if pos.line == 0 {
                    atom.top
                } else {
                    top + lines[pos.line - 1]
                };
                let end = if pos.line + 1 == lines.len() {
                    atom.bottom
                } else {
                    top + lines[pos.line]
                };
                (start, end)
            }
            None => (atom.top, atom.bottom),
        }
    }

    fn next(&self, pos: Pos) -> Option<Pos> {
        if pos.line + 1 < self.units(pos.atom) {
            Some(Pos {
                atom: pos.atom,
                line: pos.line + 1,
            })
        } else if pos.atom + 1 < self.flow.atoms.len() {
            Some(Pos {
                atom: pos.atom + 1,
                line: 0,
            })
        } else {
            None
        }
    }

    // The break before `pos`, in a column that began at `first`: its appeal,
    // whether it is forced, and where the next column would start (D3).
    fn candidate(&self, pos: Pos, first: Pos) -> (u8, bool, f32) {
        let atom = &self.flow.atoms[pos.atom];
        if pos.line == 0 {
            return if atom.forced {
                (PERFECT, true, atom.top - atom.margin)
            } else {
                (if atom.avoid { AVOID } else { PERFECT }, false, atom.top)
            };
        }
        let p: &Para = &self.flow.paras[atom.para.expect("a line is a paragraph's")];
        let total = self.units(pos.atom) as u32;
        let before = pos.line as u32;
        let here = if first.atom == pos.atom {
            before - first.line as u32
        } else {
            before
        };
        // Chrome 154 scores `widows` only when the paragraph has room for
        // both rules (L >= orphans + widows); `orphans` always (D4).
        let short =
            here < p.orphans || (total - before < p.widows && total >= p.orphans + p.widows);
        let mut appeal = if short { ORPHANS_WIDOWS } else { PERFECT };
        if p.avoid {
            appeal = appeal.min(AVOID);
        }
        (appeal, false, self.unit(pos).0)
    }

    /// Cut the flow thread into columns of height `h` (D4).
    pub(super) fn walk(&mut self, h: f32) -> Walk {
        #[cfg(test)]
        {
            self.walks += 1;
        }
        let mut out = Walk::default();
        if self.flow.atoms.is_empty() {
            return out;
        }
        let mut start = (Pos { atom: 0, line: 0 }, self.top);
        loop {
            out.starts.push(start);
            let (first, s) = start;
            let limit = s + h;
            let mut best: Option<(u8, Pos, f32)> = None;
            let mut pos = first;
            let next = loop {
                if pos != first {
                    let (appeal, forced, restart) = self.candidate(pos, first);
                    if forced {
                        break Some((pos, restart));
                    }
                    if best.is_none_or(|b| appeal >= b.0) {
                        best = Some((appeal, pos, restart));
                    }
                }
                let (_, bottom) = self.unit(pos);
                if bottom > limit + EPSILON {
                    let atom = &self.flow.atoms[pos.atom];
                    if let (Some(para), 0, None) = (
                        atom.para,
                        pos.line,
                        atom.para.and_then(|p| self.flow.paras[p].lines.as_ref()),
                    ) {
                        self.expand(para);
                        if self.units(pos.atom) > 1 {
                            continue;
                        }
                    }
                    let atom = &self.flow.atoms[pos.atom];
                    if atom.refusal.is_some() {
                        out.refused.push(pos.atom);
                    }
                    // A better candidate, or a later one of the best
                    // appeal, may lie between the lines of a paragraph this
                    // column holds whole: ask for those lines, and walk the
                    // column again.
                    if let Some((appeal, at, _)) = best {
                        let from = if appeal < PERFECT {
                            first.atom
                        } else {
                            at.atom
                        };
                        if self.expand_between(from, pos.atom) {
                            break Some(start);
                        }
                    }
                    let over = bottom - limit;
                    out.shortage = Some(out.shortage.map_or(over, |s: f32| s.min(over)));
                    match best {
                        Some((_, at, restart)) => break Some((at, restart)),
                        // The first unit is taller than the column: it
                        // starts the column and overflows it.
                        None => match self.next(pos) {
                            Some(n) => {
                                let (_, forced, restart) = self.candidate(n, first);
                                let _ = forced;
                                break Some((n, restart));
                            }
                            None => break None,
                        },
                    }
                }
                match self.next(pos) {
                    Some(n) => pos = n,
                    None => break None,
                }
            };
            match next {
                // Walked again, with more paragraphs' lines.
                Some(n) if n == start => {
                    out.starts.pop();
                }
                Some(n) => start = n,
                None => return out,
            }
        }
    }

    /// The flow thread's height below the first column's top.
    pub(super) fn height(&self) -> f32 {
        self.flow
            .atoms
            .last()
            .map_or(0.0, |a| (a.bottom - self.top).max(0.0))
    }

    /// Chrome's balancer (D5): start at the content's height over `n` (each
    /// run between forced breaks shares the columns its height asks for),
    /// clamped to `cap` and kept no lower than the tallest unbreakable piece,
    /// then stretch by the walk's shortage while it needs more than `n`
    /// columns and the height is below the cap.
    pub(super) fn balance(&mut self, n: usize, cap: f32) -> (f32, Walk) {
        let mut runs = Vec::new();
        let mut from = self.top;
        let mut tallest = 0f32;
        for (i, atom) in self.flow.atoms.iter().enumerate() {
            if i > 0 && atom.forced {
                runs.push(self.flow.atoms[i - 1].bottom - from);
                from = atom.top - atom.margin;
            }
            if atom.para.is_none() {
                tallest = tallest.max(atom.bottom - atom.top);
            }
        }
        runs.push(self.flow.atoms.last().map_or(0.0, |a| a.bottom) - from);
        let mut breaks = vec![0usize; runs.len()];
        for _ in runs.len()..n {
            let mut at = 0;
            for i in 1..runs.len() {
                if runs[i] / (breaks[i] + 1) as f32 > runs[at] / (breaks[at] + 1) as f32 {
                    at = i;
                }
            }
            breaks[at] += 1;
        }
        let guess = runs
            .iter()
            .zip(&breaks)
            .map(|(r, b)| r / (*b + 1) as f32)
            .fold(0f32, f32::max);
        let mut h = guess.max(tallest).min(cap);
        loop {
            let walk = self.walk(h);
            match walk.shortage {
                Some(more) if walk.starts.len() > n && h < cap - EPSILON && more > 0.0 => {
                    h = (h + more).min(cap);
                }
                _ => return (h, walk),
            }
        }
    }

    pub(super) fn column_of(walk: &Walk, pos: Pos) -> usize {
        walk.starts.partition_point(|(p, _)| *p <= pos).max(1) - 1
    }
}

/// A paragraph's metrics and line-box bottoms at a content width, from the
/// host's engine (D6); no bottoms when it answers none.
pub(super) fn lines(
    arena: &NodeArena,
    measurer: &mut dyn TextMeasurer,
    slot: u32,
    width: f32,
) -> (f32, Vec<f32>) {
    let mut bottoms = Vec::new();
    let mut runs: Vec<TextRun<'_>> = Vec::new();
    arena.text_runs(slot, &mut runs);
    let Some(stamp) = arena.paragraph_stamp(slot) else {
        return (0.0, bottoms);
    };
    if runs.is_empty() {
        return (0.0, bottoms);
    }
    let request = TextMeasureRequest {
        runs: &runs,
        paragraph: arena.paragraph(slot),
        width: AxisOffer::Definite(width.max(0.0)),
        height: AxisOffer::MaxContent,
        exclusions: &[],
    };
    if request.paragraph.markup != crate::text::Markup::Markdown {
        measurer.lines(&stamp, &request, &mut bottoms);
    }
    // The height its lines take, or the measure's when it gives none.
    let height = match bottoms.last() {
        Some(&last) => last,
        None => measurer.measure_identified(&stamp, &request).height,
    };
    (height, bottoms)
}

/// Cut `slot`, a laid-out multi-column container (D3–D7).
pub(crate) fn container(
    arena: &NodeArena,
    tree: &LayoutTree,
    measurer: &mut dyn TextMeasurer,
    slot: u32,
) -> Cut {
    let Some(node) = arena.taffy(slot) else {
        return Cut::default();
    };
    let Some(m) = super::taffy_multicol(arena, slot) else {
        return Cut::default();
    };
    let s = arena.style(slot);
    let env = arena.env();
    let l = tree.layout(node);
    let (p, b) = (l.padding, l.border);
    let (x0, y0) = (p.left + b.left, p.top + b.top);
    let inset_v = p.top + p.bottom + b.top + b.bottom;
    let u = (l.size.width - p.left - p.right - b.left - b.right - l.scrollbar_size.width).max(0.0);
    let (n, w) = m.columns(u);
    let n = n as usize;
    let gap = m.gap;
    let rtl = arena.computed_style(slot, StyleMask::INHERITED).direction == Direction::Rtl;
    let left = |k: usize| {
        let at = k as f32 * (w + gap);
        if rtl {
            u - w - at
        } else {
            at
        }
    };
    let text = arena.node_type(slot) == NodeType::Text;
    let flow = if text {
        let (height, _) = lines(arena, measurer, slot, w);
        build::text(arena, slot, y0, w, height)
    } else {
        build::flow(arena, tree, slot)
    };
    let mut cutter = Cutter::new(flow, arena, measurer, y0);
    let points = |d: Dimension| match d.resolve(env) {
        Dimension::Points(v) => Some(v),
        _ => None,
    };
    let adjust = if s.box_sizing == BoxSizing::ContentBox {
        0.0
    } else {
        inset_v
    };
    let definite = s.height != Dimension::Auto;
    let max = points(s.max_height).map(|v| (v - adjust).max(0.0));
    let content = (l.size.height - inset_v - l.scrollbar_size.height).max(0.0);
    let (h, walk, used) = match (definite, max, s.column_fill) {
        (true, _, ColumnFill::Auto) => {
            let walk = cutter.walk(content);
            (content, walk, None)
        }
        (true, _, ColumnFill::Balance) => {
            let (h, walk) = cutter.balance(n, content);
            (h, walk, None)
        }
        (false, Some(cap), ColumnFill::Auto) => {
            // As an automatic height while the content fits the cap.
            let walk = cutter.walk(cap);
            let h = if walk.starts.len() > 1 {
                cap
            } else {
                tallest_column(&cutter, &walk)
            };
            (h, walk, None)
        }
        (false, Some(cap), ColumnFill::Balance) => {
            let (h, walk) = cutter.balance(n, cap);
            (h, walk, Some(h))
        }
        (false, None, ColumnFill::Auto) => {
            // One column as tall as the content, but for forced breaks.
            let walk = cutter.walk(f32::INFINITY);
            let tallest = tallest_column(&cutter, &walk);
            let used = (walk.starts.len() > 1).then_some(tallest);
            (tallest, walk, used)
        }
        (false, None, ColumnFill::Balance) => {
            let cap = cutter.height();
            let (h, walk) = cutter.balance(n, cap);
            (h, walk, Some(h))
        }
    };
    let h = if h.is_finite() {
        h
    } else {
        tallest_column(&cutter, &walk)
    };
    let used = if cutter.flow.atoms.is_empty() && !text {
        None
    } else {
        used
    };
    publish(
        &cutter,
        &walk,
        Geometry {
            x0,
            y0,
            w,
            h,
            n,
            gap,
            left: &left,
        },
        slot,
        text,
        used,
        Outer {
            width: l.size.width,
            height: match used {
                Some(columns) => {
                    let mut outer = columns + inset_v;
                    if let Some(v) = points(s.max_height) {
                        outer = outer.min(v + inset_v - adjust);
                    }
                    if let Some(v) = points(s.min_height) {
                        outer = outer.max(v + inset_v - adjust);
                    }
                    outer
                }
                None => l.size.height,
            },
            border: b,
            padding: p,
            scrolls: s.overflow_x != crate::Overflow::Visible
                || s.overflow_y != crate::Overflow::Visible,
        },
        tree,
    )
}

fn tallest_column(cutter: &Cutter<'_>, walk: &Walk) -> f32 {
    let mut tallest = 0f32;
    for (k, &(_, s)) in walk.starts.iter().enumerate() {
        let end = walk.starts.get(k + 1).map(|(p, _)| *p);
        let mut pos = walk.starts[k].0;
        let mut bottom = s;
        loop {
            bottom = bottom.max(cutter.unit(pos).1);
            match cutter.next(pos) {
                Some(n) if Some(n) != end => pos = n,
                _ => break,
            }
        }
        tallest = tallest.max(bottom - s);
    }
    tallest
}

struct Geometry<'a> {
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    n: usize,
    gap: f32,
    left: &'a dyn Fn(usize) -> f32,
}

struct Outer {
    width: f32,
    height: f32,
    scrolls: bool,
    border: taffy::Rect<f32>,
    padding: taffy::Rect<f32>,
}

#[allow(clippy::too_many_arguments)]
fn publish(
    cutter: &Cutter<'_>,
    walk: &Walk,
    g: Geometry<'_>,
    slot: u32,
    text: bool,
    used: Option<f32>,
    outer: Outer,
    tree: &LayoutTree,
) -> Cut {
    let mut cut = Cut {
        used_height: used,
        #[cfg(test)]
        walks: cutter.walks,
        size: (outer.width, outer.height),
        absolutes: !cutter.flow.absolutes.is_empty(),
        ..Cut::default()
    };
    let flow = &cutter.flow;
    let cols = walk.starts.len();
    let shift = |k: usize| ((g.left)(k), g.y0 - walk.starts[k].1);
    let mut right = 0f32;
    let mut bottom = 0f32;
    let mut leftmost = 0f32;
    let mut extend = |x: f32, y: f32, w: f32, h: f32| {
        leftmost = leftmost.min(x);
        right = right.max(x + w);
        bottom = bottom.max(y + h);
    };
    for k in 0..cols.max(g.n) {
        let column = Column {
            x: g.x0 + (g.left)(k),
            y: g.y0,
            width: g.w,
            height: g.h,
            holds: k < cols,
        };
        if column.holds {
            extend(column.x, column.y, column.width, column.height);
        }
        cut.columns.columns.push(column);
    }
    cut.columns.gap = g.gap;
    // Each paragraph's line range in column `k`.
    let range = |atom: usize, k: usize| -> (u32, u32) {
        let total = cutter.units(atom);
        if total <= 1 {
            return (0, 0);
        }
        let start = match walk.starts[k].0 {
            p if p.atom == atom => p.line,
            _ => 0,
        };
        let end = match walk.starts.get(k + 1) {
            Some((p, _)) if p.atom == atom => p.line,
            _ => total,
        };
        (start as u32, end as u32)
    };
    if text && cols > 1 {
        let frags = (0..cols)
            .map(|k| {
                let (dx, dy) = shift(k);
                Fragment {
                    x: g.x0 + (g.left)(k),
                    y: g.y0,
                    width: g.w,
                    height: g.h,
                    lines: range(0, k),
                    dx,
                    dy,
                }
            })
            .collect();
        cut.fragments.push((slot, frags));
    }
    for b in &flow.boxes {
        let cf = Cutter::column_of(
            walk,
            Pos {
                atom: b.first,
                line: 0,
            },
        );
        let last = Pos {
            atom: b.last,
            line: cutter.units(b.last) - 1,
        };
        let cl = Cutter::column_of(walk, last);
        if cf == cl {
            let (dx, dy) = shift(cf);
            extend(b.x + dx, b.top + dy, b.width, b.bottom - b.top);
            cut.placements
                .push((b.slot, Placement { dx, dy, size: None }));
            continue;
        }
        let mut rects = Vec::with_capacity(cl - cf + 1);
        for k in cf..=cl {
            let s = walk.starts[k].1;
            let top = if k == cf { b.top } else { s };
            let end = if k == cl { b.bottom } else { s + g.h };
            let (dx, dy) = shift(k);
            rects.push((b.x + dx, top + dy, (end - top).max(0.0), k));
        }
        let ux = rects.iter().map(|r| r.0).fold(f32::INFINITY, f32::min);
        let uy = rects.iter().map(|r| r.1).fold(f32::INFINITY, f32::min);
        let ur = rects
            .iter()
            .map(|r| r.0 + b.width)
            .fold(f32::NEG_INFINITY, f32::max);
        let ub = rects
            .iter()
            .map(|r| r.1 + r.2)
            .fold(f32::NEG_INFINITY, f32::max);
        extend(ux, uy, ur - ux, ub - uy);
        let para = flow.atoms[b.first].para.filter(|_| b.first == b.last);
        let frags = rects
            .iter()
            .map(|&(x, y, h, k)| {
                let (dx, dy) = shift(k);
                Fragment {
                    x: x - ux,
                    y: y - uy,
                    width: b.width,
                    height: h,
                    lines: para.map_or((0, 0), |_| range(b.first, k)),
                    dx: b.x + dx - ux,
                    dy: b.top + dy - uy,
                }
            })
            .collect();
        cut.placements.push((
            b.slot,
            Placement {
                dx: ux - b.x,
                dy: uy - b.top,
                size: Some((ur - ux, ub - uy)),
            },
        ));
        cut.fragments.push((b.slot, frags));
    }
    // D10.4: an absolutely positioned box takes the column holding its own
    // flow-thread top, past the content's last column too.
    for a in &flow.absolutes {
        let (dx, dy) = if a.in_flow && cols > 0 {
            let mut k = walk.starts.partition_point(|(_, s)| *s <= a.top).max(1) - 1;
            let mut s = walk.starts[k].1;
            if k + 1 == cols && g.h > 0.0 && a.top >= s + g.h {
                let more = ((a.top - s) / g.h).floor();
                k += more as usize;
                s += more * g.h;
            }
            ((g.left)(k), g.y0 - s)
        } else {
            (0.0, 0.0)
        };
        if let Some(node) = cutter.arena.taffy(a.slot) {
            let size = tree.layout(node).size;
            extend(a.x + dx, a.top + dy, size.width, size.height);
        }
        // Its column is part of the overflow too, as in Chrome.
        if a.in_flow && cols > 0 {
            extend(g.x0 + dx, g.y0, g.w, g.h);
        }
        cut.placements
            .push((a.slot, Placement { dx, dy, size: None }));
    }
    for &atom in &walk.refused {
        if let Some((s, why)) = flow.atoms[atom].refusal {
            cut.refusals.push((s, why));
        }
    }
    // The scrollable overflow (D7): the columns and every translated box,
    // a scroll container's end padding (as Taffy and Chrome count it), and
    // at least the client box, measured from the
    // padding box's corner as Taffy measures it. Its left edge is kept but
    // only the right and bottom are published (D10.8).
    let (b, p) = (outer.border, outer.padding);
    let (pr, pb) = if outer.scrolls {
        (p.right, p.bottom)
    } else {
        (0.0, 0.0)
    };
    let right = (right + pr).max(outer.width - b.right) - b.left;
    let bottom = (bottom + pb).max(outer.height - b.bottom) - b.top;
    cut.overflow = Some(taffy::Rect {
        left: (leftmost - b.left).min(0.0),
        right,
        top: 0.0,
        bottom,
    });
    cut
}
