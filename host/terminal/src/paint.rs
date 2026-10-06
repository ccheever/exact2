//! The painter: one walk of the kernel tree into cells.
//!
//! @ref LLP 1101 D7 — the kernel is the display list: frames, styles and
//! props are read where they live, snapped to the grid (D3: each edge to the
//! nearest cell boundary, so siblings tile), and drawn per §4's mapping.

use crate::grid::{CellRect, Grid, Rgb, Style};
use crate::measure::{columns, wrap};
use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::text::Paragraph;
use exact_kernel::{
    Color, ColorValue, Display, Kernel, NodeRef, NodeType, Overflow, PropId, StyleMask,
    TextDecorationLine, TextOverflow, ViewId, Visibility,
};
use std::collections::HashMap;

/// A painted frame: the grid, and the boxes a press can hit, in paint order.
pub struct Painted {
    /// The screen.
    pub grid: Grid,
    /// Interactive nodes and their visible cells; the last that contains a
    /// cell is the one on top.
    pub hits: Vec<(ViewId, CellRect)>,
    /// Scrollable nodes: their visible cells and how far their content reaches.
    pub scrollers: Vec<(ViewId, CellRect, f32)>,
}

/// What the walk needs from the host.
pub struct Scene<'a> {
    /// The kernel, laid out.
    pub kernel: &'a Kernel,
    /// The focused node.
    pub focus: Option<ViewId>,
    /// Each scroller's offset, in layout pixels.
    pub scroll: &'a HashMap<ViewId, f32>,
    /// The caret in the focused field, in clusters from its start.
    pub caret: usize,
    /// The appearance `light-dark()` resolves by.
    pub dark: bool,
}

/// Paint the roots into a grid of `cols` × `rows`.
pub fn paint(scene: &Scene<'_>, roots: &[ViewId], cols: usize, rows: usize) -> Painted {
    let mut out = Painted {
        grid: Grid::new(cols, rows),
        hits: Vec::new(),
        scrollers: Vec::new(),
    };
    let clip = out.grid.bounds();
    for &root in roots {
        node(scene, &mut out, root, clip, 0.0);
    }
    out
}

/// A pixel edge to the nearest cell boundary.
fn snap(px: f32, cell: f32) -> i32 {
    (px / cell).round() as i32
}

/// A frame (absolute, minus the scroll above it) as cells.
fn cells(x: f32, y: f32, w: f32, h: f32) -> CellRect {
    let (x0, y0) = (snap(x, COLUMN), snap(y, ROW));
    CellRect {
        x: x0,
        y: y0,
        w: snap(x + w, COLUMN) - x0,
        h: snap(y + h, ROW) - y0,
    }
}

fn rgb(c: Color) -> Option<Rgb> {
    (c.a() >= 128).then(|| Rgb(c.r(), c.g(), c.b()))
}

fn node(scene: &Scene<'_>, out: &mut Painted, id: ViewId, clip: CellRect, dy: f32) {
    let kernel = scene.kernel;
    let Some(n) = kernel.node(id) else {
        return;
    };
    if n.style.display == Display::None || n.is_inline_run() || n.node_type == NodeType::Head {
        return;
    }
    let f = n.frame;
    let rect = cells(f.x, f.y - dy, f.width, f.height);
    let visible =
        n.computed_row(exact_kernel::StyleId::Visibility, |s| s.visibility) == Visibility::Visible;
    let current = n.text_color();
    if visible {
        if let Some(bg) = n
            .style
            .background_color
            .map(|c| c.resolve(scene.dark))
            .and_then(rgb)
        {
            out.grid.fill(rect, clip, bg);
        }
        border(scene, out, &n, rect, clip, current);
    }
    let [bt, br, bb, bl] = n.style.border_widths();
    let (pl, pt, pr, pb) = kernel.resolved_padding(n.key).unwrap_or_default();
    let content = cells(
        f.x + bl + pl,
        f.y - dy + bt + pt,
        f.width - bl - br - pl - pr,
        f.height - bt - bb - pt - pb,
    );
    if matches!(
        n.node_type,
        NodeType::Pressable | NodeType::TextInput | NodeType::Control
    ) {
        out.hits.push((id, rect.intersect(clip)));
    }
    if visible {
        match n.node_type {
            NodeType::Text => text(scene, out, &n, content, clip, current),
            NodeType::TextInput => field(scene, out, &n, content, clip, current),
            _ => {}
        }
    }
    let scrolls = n.node_type == NodeType::ScrollView
        || matches!(n.style.overflow_y, Overflow::Scroll | Overflow::Auto);
    let clips = scrolls
        || n.style.overflow_x != Overflow::Visible
        || n.style.overflow_y != Overflow::Visible;
    let inner = cells(
        f.x + bl,
        f.y - dy + bt,
        f.width - bl - br,
        f.height - bt - bb,
    );
    let child_clip = if clips { clip.intersect(inner) } else { clip };
    let mut child_dy = dy;
    if scrolls {
        let reach = (n.content.1 - (f.height - bt - bb)).max(0.0);
        let offset = scene
            .scroll
            .get(&id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, reach);
        child_dy += offset;
        out.scrollers.push((id, inner.intersect(clip), reach));
        // A one-column indicator at the inner right edge (LLP 1101 §4).
        if reach > 0.0 && n.style.scrollbar_width != exact_kernel::ScrollbarWidth::None {
            let track = inner.h.max(1);
            let thumb = ((track as f32) * (f.height / (f.height + reach)))
                .ceil()
                .max(1.0) as i32;
            let top = ((track - thumb) as f32 * (offset / reach)).round() as i32;
            let x = inner.x + inner.w - 1;
            for i in 0..track {
                let on = i >= top && i < top + thumb;
                let style = Style {
                    faint: !on,
                    ..Style::default()
                };
                out.grid.put(
                    x,
                    inner.y + i,
                    if on { "┃" } else { "│" },
                    1,
                    style,
                    child_clip,
                );
            }
        }
    }
    if n.node_type != NodeType::Text {
        for child in n.children() {
            node(scene, out, child, child_clip, child_dy);
        }
    }
    if visible && scene.focus == Some(id) && n.node_type != NodeType::TextInput {
        out.grid.reverse(rect, clip);
    }
}

fn border(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    r: CellRect,
    clip: CellRect,
    current: ColorValue,
) {
    let widths = n.style.border_widths();
    if widths.iter().all(|w| *w <= 0.0) || r.w < 1 || r.h < 1 {
        return;
    }
    let colors = n.style.border_colors(current);
    let fg = |i: usize| rgb(colors[i].resolve(scene.dark));
    // Heavy from the width written (`medium` is 3 px), not the cell it takes.
    let s = n.style;
    let heavy = [
        s.border_width_top,
        s.border_width_right,
        s.border_width_bottom,
        s.border_width_left,
    ]
    .iter()
    .zip(widths)
    .any(|(w, on)| on > 0.0 && *w >= 3.0);
    let env = scene.kernel.env();
    let round = !heavy
        && [
            n.style.border_radius_top_left,
            n.style.border_radius_top_right,
            n.style.border_radius_bottom_right,
            n.style.border_radius_bottom_left,
        ]
        .iter()
        .any(|d| matches!(d.resolve(&env), exact_kernel::Dimension::Points(p) if p > 0.0));
    let (h, v) = if heavy {
        ("━", "┃")
    } else {
        ("─", "│")
    };
    let (tl, tr, br, bl) = match (heavy, round) {
        (true, _) => ("┏", "┓", "┛", "┗"),
        (false, true) => ("╭", "╮", "╯", "╰"),
        (false, false) => ("┌", "┐", "┘", "└"),
    };
    let [top, right, bottom, left] = widths.map(|w| w > 0.0);
    let (x1, y1) = (r.x + r.w - 1, r.y + r.h - 1);
    let style = |i| Style {
        fg: fg(i),
        ..Style::default()
    };
    for x in r.x..=x1 {
        if top {
            out.grid.put(x, r.y, h, 1, style(0), clip);
        }
        if bottom {
            out.grid.put(x, y1, h, 1, style(2), clip);
        }
    }
    for y in r.y..=y1 {
        if left {
            out.grid.put(r.x, y, v, 1, style(3), clip);
        }
        if right {
            out.grid.put(x1, y, v, 1, style(1), clip);
        }
    }
    if top && left {
        out.grid.put(r.x, r.y, tl, 1, style(0), clip);
    }
    if top && right {
        out.grid.put(x1, r.y, tr, 1, style(0), clip);
    }
    if bottom && right {
        out.grid.put(x1, y1, br, 1, style(2), clip);
    }
    if bottom && left {
        out.grid.put(r.x, y1, bl, 1, style(2), clip);
    }
}

/// The decoration a node draws with: its own, or the nearest ancestor's
/// (CSS propagates `text-decoration` to descendants' text).
fn decoration(kernel: &Kernel, n: &NodeRef<'_>) -> (bool, bool) {
    let mut at = Some(n.id);
    let (mut under, mut strike) = (false, false);
    while let Some(id) = at {
        let Some(node) = kernel.node(id) else { break };
        match node.style.text_decoration_line {
            TextDecorationLine::Underline => under = true,
            TextDecorationLine::LineThrough => strike = true,
            TextDecorationLine::UnderlineLineThrough => (under, strike) = (true, true),
            TextDecorationLine::None => {}
        }
        at = node.parent;
    }
    (under, strike)
}

fn base_style(scene: &Scene<'_>, n: &NodeRef<'_>, current: ColorValue) -> Style {
    let (underline, strike) = decoration(scene.kernel, n);
    Style {
        fg: rgb(current.resolve(scene.dark)),
        underline,
        strike,
        ..Style::default()
    }
}

fn text(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    content: CellRect,
    clip: CellRect,
    current: ColorValue,
) {
    let computed = n.computed_style(StyleMask::INHERITED);
    let paragraph = Paragraph::from_style(&computed);
    let runs = n.text_runs();
    let width = content.w.max(0) as usize;
    let mut lines = wrap(&runs, &paragraph, Some(width), false);
    let ellipsis = n.style.text_overflow == TextOverflow::Ellipsis;
    let base = base_style(scene, n, current);
    let inner = clip.intersect(content);
    for (row, line) in lines.iter_mut().enumerate() {
        if ellipsis && line.cols() > width && width > 0 {
            while line.cols() > width - 1 {
                line.glyphs.pop();
            }
            let run = line.glyphs.last().map_or(0, |g| g.run);
            line.glyphs.push(crate::measure::Glyph {
                text: "…".into(),
                cols: 1,
                run,
            });
        }
        let slack = width.saturating_sub(line.cols()) as i32;
        let mut x = content.x
            + match paragraph.text_align {
                exact_kernel::TextAlign::Right => slack,
                exact_kernel::TextAlign::Center => slack / 2,
                _ => 0,
            };
        for glyph in &line.glyphs {
            let ts = runs[glyph.run].style;
            let style = Style {
                bold: ts.font_weight >= 600,
                faint: ts.font_weight <= 300,
                italic: ts.font_style != exact_kernel::FontStyle::Normal,
                ..base
            };
            out.grid.put(
                x,
                content.y + row as i32,
                &glyph.text,
                glyph.cols,
                style,
                inner,
            );
            x += glyph.cols as i32;
        }
    }
}

/// A text field, host-drawn: the value (or a faint placeholder) on one line,
/// scrolled to keep the caret in view, and the terminal's cursor at the caret.
fn field(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    content: CellRect,
    clip: CellRect,
    current: ColorValue,
) {
    let value = n.props.str(PropId::Value).unwrap_or("");
    let focused = scene.focus == Some(n.id);
    let mut style = base_style(scene, n, current);
    let shown: Vec<(String, usize)> = if value.is_empty() {
        style.faint = true;
        n.props
            .str(PropId::Placeholder)
            .unwrap_or("")
            .graphemes_cells()
    } else {
        value.graphemes_cells()
    };
    let inner = clip.intersect(content);
    let width = content.w.max(1) as usize;
    let caret_cols: usize = if value.is_empty() {
        0
    } else {
        shown.iter().take(scene.caret).map(|g| g.1).sum()
    };
    let skip = caret_cols.saturating_sub(width - 1);
    let mut x = content.x - skip as i32;
    for (text, cols) in &shown {
        out.grid.put(x, content.y, text, *cols, style, inner);
        x += *cols as i32;
    }
    if focused {
        let cx = content.x + (caret_cols - skip) as i32;
        if inner.contains(cx, content.y) {
            out.grid.cursor = Some((cx as usize, content.y as usize));
        }
    }
}

trait Clusters {
    fn graphemes_cells(&self) -> Vec<(String, usize)>;
}

impl Clusters for str {
    fn graphemes_cells(&self) -> Vec<(String, usize)> {
        use unicode_segmentation::UnicodeSegmentation;
        use unicode_width::UnicodeWidthStr;
        self.graphemes(true)
            .map(|g| {
                if g.chars().any(char::is_control) {
                    ("\u{fffd}".to_string(), 1)
                } else {
                    (g.to_string(), g.width().max(1))
                }
            })
            .collect()
    }
}

/// Columns in a pixel width, for callers outside the walk.
pub fn width_in_columns(px: f32) -> usize {
    columns(px)
}
