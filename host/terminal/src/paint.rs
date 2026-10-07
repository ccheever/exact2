//! The painter: one walk of the kernel tree into cells.
//!
//! @ref LLP 1101 D7 — the kernel is the display list: frames, styles and
//! props are read where they live, snapped to the grid (D3: each edge to the
//! nearest cell boundary, so siblings tile), and drawn per §4's mapping.
//! Open dialogs and popovers paint last, over a faint backdrop.

use crate::grid::{CellRect, Grid, Rgb, Style};
use crate::measure::{columns, wrap};
use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::text::Paragraph;
use exact_kernel::{
    Color, ColorValue, Dimension, Display, Kernel, NodeRef, NodeType, Overflow, PropId, StyleMask,
    TextDecorationLine, TextOverflow, ViewId, Visibility,
};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A painted frame: the grid, and the boxes a press can hit, in paint order.
pub struct Painted {
    /// The screen.
    pub grid: Grid,
    /// Interactive nodes and their visible cells; the last that contains a
    /// cell is the one on top.
    pub hits: Vec<(ViewId, CellRect)>,
    /// Scrollable nodes: their visible cells and how far their content reaches.
    pub scrollers: Vec<(ViewId, CellRect, f32)>,
    /// Image nodes and the cells they occupy (unclipped), for a writer that
    /// draws them with a terminal's image protocol.
    pub images: Vec<(ViewId, CellRect)>,
    /// The open top layer's cells, when one is painted.
    pub layer: Option<CellRect>,
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
    /// Open dialogs and popovers, bottom to top.
    pub layers: Vec<ViewId>,
    /// Decoded images.
    pub images: &'a crate::image::Images,
    /// Centre a dialog with no insets in the grid (full screen).
    pub center: bool,
    /// The kernel's environment: its border rule among it.
    pub env: exact_kernel::Env,
}

/// Paint document rows `top..top + rows` of the roots into a grid of
/// `cols` × `rows`.
pub fn paint(scene: &Scene<'_>, roots: &[ViewId], cols: usize, rows: usize, top: usize) -> Painted {
    let mut out = Painted {
        grid: Grid::new(cols, rows),
        hits: Vec::new(),
        scrollers: Vec::new(),
        images: Vec::new(),
        layer: None,
    };
    let clip = out.grid.bounds();
    let dy = top as f32 * ROW;
    for &root in roots {
        node(scene, &mut out, root, clip, 0.0, dy, false);
    }
    for &layer in &scene.layers {
        // A modal layer: what is beneath is faint and takes no pointer.
        out.grid.dim();
        out.hits.clear();
        out.scrollers.clear();
        let (dx, ldy) = placement(scene, layer, cols, rows, dy);
        if let Some(n) = scene.kernel.node(layer) {
            let f = n.frame;
            out.layer = Some(cells(f.x - dx, f.y - ldy, f.width, f.height));
        }
        node(scene, &mut out, layer, clip, dx, ldy, true);
    }
    out
}

/// Where an open layer paints: where it was laid out, or centred in the
/// grid when it names no inset (as the Mac host centres a dialog).
fn placement(scene: &Scene<'_>, layer: ViewId, cols: usize, rows: usize, dy: f32) -> (f32, f32) {
    let Some(n) = scene.kernel.node(layer) else {
        return (0.0, dy);
    };
    let s = n.style;
    let f = n.frame;
    let h = (f.height / ROW).round();
    if !scene.center {
        // Inline: anchored to the bottom of the region painted, which the
        // writer has made tall enough (it grows down into new rows, never
        // up over what is already printed).
        let gy = (rows as f32 - h).max(0.0);
        return (0.0, f.y - gy * ROW);
    }
    let auto = [s.top, s.right, s.bottom, s.left]
        .iter()
        .all(|d| *d == Dimension::Auto);
    if !auto {
        return (0.0, dy);
    }
    let w = (f.width / COLUMN).round();
    let gx = ((cols as f32 - w) / 2.0).floor().max(0.0);
    let gy = ((rows as f32 - h) / 2.0).floor().max(0.0);
    (f.x - gx * COLUMN, f.y - gy * ROW)
}

/// A pixel edge to the nearest cell boundary.
fn snap(px: f32, cell: f32) -> i32 {
    (px / cell).round() as i32
}

/// A frame (absolute, minus the offsets above it) as cells.
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

fn is_layer(n: &NodeRef<'_>) -> bool {
    n.props.str(PropId::SemanticTag) == Some("dialog") || n.props.contains(PropId::Popover)
}

fn node(
    scene: &Scene<'_>,
    out: &mut Painted,
    id: ViewId,
    clip: CellRect,
    dx: f32,
    dy: f32,
    layer: bool,
) {
    let kernel = scene.kernel;
    let Some(n) = kernel.node(id) else {
        return;
    };
    if n.style.display == Display::None || n.is_inline_run() || n.node_type == NodeType::Head {
        return;
    }
    // A dialog or popover paints only as an open layer, after everything.
    if is_layer(&n) && !layer {
        return;
    }
    let f = n.frame;
    let rect = cells(f.x - dx, f.y - dy, f.width, f.height);
    // A box wholly above or below the rows asked for shows nothing, nor do
    // its flow children: a long transcript paints only the rows in view.
    if rect.h > 0 && (rect.y + rect.h <= clip.y || rect.y >= clip.y + clip.h) {
        return;
    }
    let visible =
        n.computed_row(exact_kernel::StyleId::Visibility, |s| s.visibility) == Visibility::Visible;
    let current = n.text_color();
    let native_field =
        n.node_type == NodeType::TextInput && n.style.appearance == exact_kernel::Appearance::Auto;
    if visible {
        if native_field {
            let fill = if scene.dark {
                Rgb(48, 48, 48)
            } else {
                Rgb(228, 228, 228)
            };
            out.grid.fill(rect, clip, fill);
        }
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
    let [bt, br, bb, bl] = n.style.border_widths_in(&scene.env);
    let (pl, pt, pr, pb) = kernel.resolved_padding(n.key).unwrap_or_default();
    let content = n.field_content_rect().map_or_else(
        || {
            cells(
                f.x - dx + bl + pl,
                f.y - dy + bt + pt,
                f.width - bl - br - pl - pr,
                f.height - bt - bb - pt - pb,
            )
        },
        |c| cells(f.x - dx + c.x, f.y - dy + c.y, c.width, c.height),
    );
    if matches!(
        n.node_type,
        NodeType::Pressable | NodeType::TextInput | NodeType::Control
    ) {
        out.hits.push((id, rect.intersect(clip)));
    }
    if visible {
        match n.node_type {
            NodeType::Text => {
                // Wrap at the columns the measurer was offered (the content width
                // before snapping), so the lines drawn are the lines measured.
                let px = f.width - bl - br - pl - pr;
                text(scene, out, &n, content, columns(px), clip, current)
            }
            NodeType::TextInput => field(scene, out, &n, content, clip, current),
            NodeType::Image => image(scene, out, &n, content, clip),
            _ => {}
        }
    }
    let scrolls = n.node_type == NodeType::ScrollView
        || matches!(n.style.overflow_y, Overflow::Scroll | Overflow::Auto);
    let clips = scrolls
        || n.style.overflow_x != Overflow::Visible
        || n.style.overflow_y != Overflow::Visible;
    let inner = cells(
        f.x - dx + bl,
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
            node(scene, out, child, child_clip, dx, child_dy, false);
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
    let widths = n.style.border_widths_in(&scene.env);
    if widths.iter().all(|w| *w <= 0.0) || r.w < 1 || r.h < 1 {
        return;
    }
    let colors = n.style.border_colors(current);
    let fg = |i: usize| rgb(colors[i].resolve(scene.dark));
    // Heavy only for `thick` (5 px) as written: CSS's default width is
    // `medium`, and an unadorned border is the ordinary light line.
    let s = n.style;
    let heavy = [
        s.border_width_top,
        s.border_width_right,
        s.border_width_bottom,
        s.border_width_left,
    ]
    .iter()
    .zip(widths)
    .any(|(w, on)| on > 0.0 && *w >= 5.0);
    let env = scene.kernel.env();
    let round = !heavy
        && [
            s.border_radius_top_left,
            s.border_radius_top_right,
            s.border_radius_bottom_right,
            s.border_radius_bottom_left,
        ]
        .iter()
        .any(|d| matches!(d.resolve(&env), Dimension::Points(p) if p > 0.0));
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
fn decoration(kernel: &Kernel, id: ViewId) -> (bool, bool) {
    let mut at = Some(id);
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
    let (underline, strike) = decoration(scene.kernel, n.id);
    Style {
        fg: rgb(current.resolve(scene.dark)),
        underline,
        strike,
        faint: faded(scene.kernel, n.id),
        ..Style::default()
    }
}

/// A cell has no alpha: a see-through box's text, or a native disabled
/// field, is faint instead (LLP 1104 D7).
fn faded(kernel: &Kernel, id: ViewId) -> bool {
    let mut at = kernel.node(id);
    while let Some(n) = at {
        if n.style.opacity < 1.0
            || (n.node_type == NodeType::TextInput
                && n.style.appearance == exact_kernel::Appearance::Auto
                && n.props.bool(PropId::Disabled) == Some(true))
        {
            return true;
        }
        at = n.parent.and_then(|p| kernel.node(p));
    }
    false
}

/// The leaves a paragraph's runs come from, in run order: a node with its
/// own text is one run; otherwise its inline `text` children are.
fn leaves(kernel: &Kernel, id: ViewId, out: &mut Vec<ViewId>) {
    let Some(n) = kernel.node(id) else { return };
    if n.props.str(PropId::Text).is_some() {
        out.push(id);
        return;
    }
    for child in n.children() {
        if kernel
            .node(child)
            .is_some_and(|c| c.node_type == NodeType::Text)
        {
            leaves(kernel, child, out);
        }
    }
}

/// Each run's own style: its colour, the background of the nearest inline
/// box under the paragraph, its decorations, weight and slant.
fn run_styles(
    scene: &Scene<'_>,
    n: &NodeRef<'_>,
    runs: &[exact_kernel::TextRun<'_>],
    base: Style,
) -> Vec<Style> {
    let kernel = scene.kernel;
    let mut ids = Vec::new();
    leaves(kernel, n.id, &mut ids);
    runs.iter()
        .enumerate()
        .map(|(i, run)| {
            let mut style = Style {
                bold: run.style.font_weight >= 600,
                faint: base.faint || run.style.font_weight <= 300,
                italic: run.style.font_style != exact_kernel::FontStyle::Normal,
                ..base
            };
            if ids.len() == runs.len() {
                let leaf = ids[i];
                if let Some(l) = kernel.node(leaf) {
                    style.fg = rgb(l.text_color().resolve(scene.dark));
                }
                let mut at = Some(leaf);
                while let Some(a) = at.filter(|a| *a != n.id) {
                    let Some(node) = kernel.node(a) else { break };
                    if let Some(bg) = node
                        .style
                        .background_color
                        .map(|c| c.resolve(scene.dark))
                        .and_then(rgb)
                    {
                        style.bg = Some(bg);
                        break;
                    }
                    at = node.parent;
                }
                (style.underline, style.strike) = decoration(kernel, leaf);
            }
            style
        })
        .collect()
}

fn text(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    content: CellRect,
    width: usize,
    clip: CellRect,
    current: ColorValue,
) {
    let computed = n.computed_style(StyleMask::INHERITED);
    let paragraph = Paragraph::from_style(&computed);
    let runs = n.text_runs();
    let mut lines = wrap(&runs, &paragraph, Some(width), false);
    let ellipsis = n.style.text_overflow == TextOverflow::Ellipsis;
    let mut styles = run_styles(scene, n, &runs, base_style(scene, n, current));
    // Each run's link, from its leaf's `href` (or the paragraph's own).
    let mut ids = Vec::new();
    leaves(scene.kernel, n.id, &mut ids);
    let own = n.props.str(PropId::Href).filter(|h| !h.is_empty());
    for (i, style) in styles.iter_mut().enumerate() {
        let href = ids
            .get(i)
            .filter(|_| ids.len() == runs.len())
            .and_then(|id| scene.kernel.node(*id))
            .and_then(|l| l.props.str(PropId::Href).filter(|h| !h.is_empty()))
            .or(own);
        if let Some(url) = href.filter(|u| !u.chars().any(|c| c.is_control())) {
            style.link = out.grid.link(url);
        }
    }
    let inner = clip.intersect(content);
    for (row, line) in lines.iter_mut().enumerate() {
        let y = content.y + row as i32;
        if y < inner.y || y >= inner.y + inner.h {
            continue;
        }
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
            let style = styles.get(glyph.run).copied().unwrap_or_default();
            out.grid.put(x, y, &glyph.text, glyph.cols, style, inner);
            x += glyph.cols as i32;
        }
    }
}

/// An image as half-blocks, the protocol-free form every writer can show;
/// a writer with an image protocol draws over these cells.
fn image(scene: &Scene<'_>, out: &mut Painted, n: &NodeRef<'_>, content: CellRect, clip: CellRect) {
    out.images.push((n.id, content));
    let inner = clip.intersect(content);
    match scene.images.of(n.id) {
        Some(decoded) if content.w > 0 && content.h > 0 => {
            let cells = decoded.half_blocks(content.w as usize, content.h as usize);
            for (row, line) in cells.iter().enumerate() {
                for (col, (top, bottom)) in line.iter().enumerate() {
                    let style = Style {
                        fg: Some(*top),
                        bg: Some(*bottom),
                        ..Style::default()
                    };
                    out.grid.put(
                        content.x + col as i32,
                        content.y + row as i32,
                        "▀",
                        1,
                        style,
                        inner,
                    );
                }
            }
        }
        _ => {
            let alt = n.props.str(PropId::AccessibilityLabel).unwrap_or("image");
            let style = Style {
                faint: true,
                ..Style::default()
            };
            for (x, g) in (content.x..).zip(format!("[{alt}]").graphemes(true)) {
                out.grid.put(x, content.y, g, 1, style, inner);
            }
        }
    }
}

/// A text field, host-drawn: the value (or a faint placeholder), wrapped in
/// a `textarea` and on one scrolled line otherwise, the caret kept in view
/// and the terminal's cursor placed at it.
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
    let multiline = n.props.str(PropId::SemanticTag) == Some("textarea");
    let mut style = base_style(scene, n, current);
    if n.style.appearance == exact_kernel::Appearance::Auto
        && !n.style.mask.has(exact_kernel::StyleId::TextColor)
    {
        style.fg = Some(if scene.dark {
            Rgb(255, 255, 255)
        } else {
            Rgb(0, 0, 0)
        });
    }
    let placeholder = value.is_empty();
    let shown = if placeholder {
        style.faint = true;
        clusters(n.props.str(PropId::Placeholder).unwrap_or(""))
    } else if n.props.str(PropId::Type) == Some("password") {
        // A password field shows a bullet per character, never the text.
        clusters(value)
            .into_iter()
            .map(|_| ("•".to_string(), 1))
            .collect()
    } else {
        clusters(value)
    };
    let inner = clip.intersect(content);
    let width = content.w.max(1) as usize;
    // Lay the clusters out as (row, col), breaking at newlines and, in a
    // textarea, at the width; the caret's own position too.
    let caret_at = if placeholder {
        0
    } else {
        scene.caret.min(shown.len())
    };
    let mut placed = Vec::with_capacity(shown.len());
    let (mut row, mut col) = (0usize, 0usize);
    let mut caret = (0, 0);
    for (i, (text, cols)) in shown.iter().enumerate() {
        if i == caret_at {
            caret = (row, col);
        }
        if text == "\n" {
            if multiline {
                row += 1;
                col = 0;
            }
            continue;
        }
        if multiline && col + cols > width {
            row += 1;
            col = 0;
        }
        placed.push((row, col, text.as_str(), *cols));
        col += cols;
    }
    if caret_at >= shown.len() {
        caret = if multiline && col >= width {
            (row + 1, 0)
        } else {
            (row, col)
        };
    }
    let height = content.h.max(1) as usize;
    let (skip_rows, skip_cols) = if multiline {
        (caret.0.saturating_sub(height - 1), 0)
    } else {
        (0, caret.1.saturating_sub(width - 1))
    };
    for (r, c, text, cols) in placed {
        if r < skip_rows {
            continue;
        }
        let x = content.x + c as i32 - skip_cols as i32;
        let y = content.y + (r - skip_rows) as i32;
        out.grid.put(x, y, text, cols, style, inner);
    }
    if focused {
        let cx = content.x + (caret.1 - skip_cols) as i32;
        let cy = content.y + (caret.0 - skip_rows) as i32;
        if inner.contains(cx, cy) {
            out.grid.cursor = Some((cx as usize, cy as usize));
        }
    }
}

fn clusters(s: &str) -> Vec<(String, usize)> {
    s.graphemes(true)
        .map(|g| {
            if g == "\n" || g == "\r\n" {
                ("\n".to_string(), 0)
            } else if g.chars().any(char::is_control) {
                ("\u{fffd}".to_string(), 1)
            } else {
                (g.to_string(), g.width().max(1))
            }
        })
        .collect()
}

/// Columns in a pixel width, for callers outside the walk.
pub fn width_in_columns(px: f32) -> usize {
    columns(px)
}
