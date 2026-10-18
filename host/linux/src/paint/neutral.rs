//! The neutral look's controls, painted (LLP 1115.000 D17). Each shape is
//! the one the page's sheet draws (`host/web/neutral.css`, and the shell's
//! where the look keeps it): its lengths are the look's rows of the schema's
//! `metrics` and its colours the role table's neutral column, so a page and
//! this painter show one designed look. What a browser's engine draws its
//! own way (a glyph's edge, a shadow's blur) is this painter's own.
use super::look::{self, fade, role};
use super::{border, Backend, Rect4, Shape};
use exact_kernel::generated::metric;
use exact_kernel::{Direction, NodeRef, PropId, StyleId};
use tiny_skia::Transform;

/// A control's font size: what the look's `em` lengths are of.
pub(super) fn em(node: &NodeRef<'_>) -> f32 {
    node.computed_row(StyleId::FontSize, |s| s.font_size)
}

fn rtl(node: &NodeRef<'_>) -> bool {
    node.computed_row(StyleId::Direction, |s| s.direction) == Direction::Rtl
}

fn disabled(node: &NodeRef<'_>) -> bool {
    node.props.bool(PropId::Disabled) == Some(true)
}

/// A line `width` thick just inside a shape's edge.
pub(super) fn edge(
    backend: &mut dyn Backend,
    shape: &Shape,
    width: f32,
    color: [u8; 4],
    ts: Transform,
) {
    for part in border::border_fills(shape, [width; 4], [color; 4]) {
        backend.fill_border(&part, ts);
    }
}

/// A stroke with round ends from `a` to `b`.
fn stroke(
    backend: &mut dyn Backend,
    a: (f32, f32),
    b: (f32, f32),
    width: f32,
    color: [u8; 4],
    ts: Transform,
) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let bar = Shape::new(
        (
            mx - (length + width) / 2.0,
            my - width / 2.0,
            length + width,
            width,
        ),
        [width / 2.0; 4],
    );
    let turn = Transform::from_rotate_at(dy.atan2(dx).to_degrees(), mx, my);
    backend.fill(&bar, color, ts.pre_concat(turn));
}

/// A checkbox or a radio: a box the line's height with the control's line
/// on the field's ground, 4 px corners or round; checked, the accent under
/// the ink that reads on it, a check or a dot. `hovered` darkens an
/// unchecked one's line to the label's ink.
pub(super) fn mark(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
    on: bool,
    hovered: bool,
) {
    let em = em(node);
    let radio = node.props.str(PropId::Type) == Some("radio");
    let (x, y, w, h) = content;
    let line = look::px(metric::CONTROL_LINE, em);
    let radius = if radio {
        w.min(h) / 2.0
    } else {
        look::px(metric::MARK_RADIUS, em)
    };
    let shape = Shape::new(content, [radius; 4]);
    let dim = disabled(node);
    if dim {
        backend.push_opacity(look::number(metric::MARK_DISABLED_ALPHA));
    }
    if on {
        let (accent, ink) = look::accents(super::control::accent(node, dark), dark);
        backend.fill(&shape, accent, ts);
        // The mark is drawn in the box inside the line, as the sheet's is.
        let (ix, iy, side) = (x + line, y + line, (w.min(h) - 2.0 * line).max(0.0));
        if radio {
            let d = side * look::number(metric::MARK_DOT);
            let dot = (ix + (side - d) / 2.0, iy + (side - d) / 2.0, d, d);
            backend.fill(&Shape::new(dot, [d / 2.0; 4]), ink, ts);
        } else {
            // The sheet's check, on its 16 unit grid: a 2 unit stroke from
            // (3.5, 8.5) by (6.5, 11.5) to (12.5, 4.5).
            let u = side / 16.0;
            let at = |px: f32, py: f32| (ix + px * u, iy + py * u);
            stroke(backend, at(3.5, 8.5), at(6.5, 11.5), 2.0 * u, ink, ts);
            stroke(backend, at(6.5, 11.5), at(12.5, 4.5), 2.0 * u, ink, ts);
        }
    } else {
        backend.fill(
            &shape,
            role("-exact-secondary-grouped-background", dark),
            ts,
        );
        let ink = if hovered && !dim {
            role("-exact-label", dark)
        } else {
            look::line(dark)
        };
        edge(backend, &shape, line, ink, ts);
    }
    if dim {
        backend.pop_opacity();
    }
}

/// The switch: a pill in the control's line, the accent when on, and a
/// thumb in the ink that reads on each, at the inline start until on.
pub(super) fn switch(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
    on: bool,
) {
    let (x, y, w, h) = content;
    let dim = disabled(node);
    if dim {
        backend.push_opacity(look::number(metric::MARK_DISABLED_ALPHA));
    }
    let (track, thumb) = if on {
        look::accents(super::control::accent(node, dark), dark)
    } else {
        (look::line(dark), role("-exact-background", dark))
    };
    backend.fill(&Shape::new(content, [h / 2.0; 4]), track, ts);
    let d = h * look::number(metric::SWITCH_THUMB);
    let at_end = on != rtl(node);
    let cx = if at_end { x + w - h / 2.0 } else { x + h / 2.0 };
    let circle = (cx - d / 2.0, y + (h - d) / 2.0, d, d);
    backend.fill(&Shape::new(circle, [d / 2.0; 4]), thumb, ts);
    if dim {
        backend.pop_opacity();
    }
}

/// A range: a thin track in the control's line across the box, and a round
/// thumb in the accent, ringed in the field's ground, at `fraction` of the
/// way. Nothing marks the part before the thumb, as on the page.
pub(super) fn range(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
    fraction: f32,
) {
    let em = em(node);
    let (x, y, w, h) = content;
    let dim = disabled(node);
    if dim {
        backend.push_opacity(look::number(metric::MARK_DISABLED_ALPHA));
    }
    let t = look::px(metric::RANGE_TRACK, em).min(h);
    let cy = y + h / 2.0;
    let track = Shape::new((x, cy - t / 2.0, w, t), [t / 2.0; 4]);
    backend.fill(&track, look::line(dark), ts);
    let d = look::px(metric::MARK_SIZE, em).min(h).min(w);
    let along = if rtl(node) { 1.0 - fraction } else { fraction };
    let thumb = Shape::new((x + (w - d) * along, cy - d / 2.0, d, d), [d / 2.0; 4]);
    let (accent, _) = look::accents(super::control::accent(node, dark), dark);
    backend.fill(
        &thumb,
        role("-exact-secondary-grouped-background", dark),
        ts,
    );
    backend.fill(
        &thumb.inset(look::px(metric::RANGE_THUMB_LINE, em)),
        accent,
        ts,
    );
    if dim {
        backend.pop_opacity();
    }
}

/// An indeterminate `progress`: a ring that fades round to its ink: the
/// node's own `color` where written, else an authored accent, else the
/// secondary label's. One still frame: this host paints on change.
pub(super) fn spinner(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
) {
    let (x, y, w, h) = content;
    let s = w.min(h);
    if s <= 0.0 {
        return;
    }
    // Its own `color`, else an authored accent (LLP 1121 D9), else the
    // look's grey: an inherited colour is not the spinner's (LLP 1115 D4).
    let ink = if node.style.written.has(StyleId::TextColor) {
        super::rgba(node.text_color().resolve(dark))
    } else if let Some(accent) = super::control::authored_accent(node, dark) {
        accent
    } else {
        role("-exact-secondary-label", dark)
    };
    // The page's ring is the outer 24% of the radius, and its ink starts a
    // tenth of the way round.
    let (cx, cy, r) = (x + w / 2.0, y + h / 2.0, s / 2.0);
    let thick = r * 0.24;
    const STEPS: usize = 48;
    let arc = std::f32::consts::TAU * (r - thick / 2.0) / STEPS as f32;
    let piece = Shape::rect((cx - arc * 0.6, cy - r, arc * 1.2, thick));
    for i in 0..STEPS {
        let turn = i as f32 / STEPS as f32;
        let alpha = ((turn - 0.1) / 0.9).max(0.0);
        if alpha <= 0.0 {
            continue;
        }
        let rotate = Transform::from_rotate_at(360.0 * turn, cx, cy);
        backend.fill(&piece, fade(ink, alpha), ts.pre_concat(rotate));
    }
}

/// A determinate `progress`: a 4 px pill across the box in the fill role,
/// the accent to the value from the inline start.
pub(super) fn progress_bar(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
) {
    let (x, y, w, h) = content;
    let t = look::px(metric::RANGE_TRACK, em(node)).min(h);
    if w <= 0.0 || t <= 0.0 {
        return;
    }
    let filled = w * exact_kernel::Progress::of(node.props).fraction() as f32;
    let bar = |from: f32, width: f32| Shape::new((from, y + (h - t) / 2.0, width, t), [t / 2.0; 4]);
    backend.fill(&bar(x, w), role("-exact-fill", dark), ts);
    if filled > 0.0 {
        let from = if rtl(node) { x + w - filled } else { x };
        let (accent, _) = look::accents(super::control::accent(node, dark), dark);
        backend.fill(&bar(from, filled), accent, ts);
    }
}

/// What a field draws under its text: the field's ground inside the
/// control's line, with a control's corners. Disabled, the raised ground
/// inside a divider; marked invalid, the line is the red.
pub(super) fn field(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    rect: Rect4,
    ts: Transform,
    dark: bool,
) -> Shape {
    let em = em(node);
    let shape = Shape::new(rect, [look::px(metric::CONTROL_RADIUS, em); 4]);
    let (ground, line) = if disabled(node) {
        (
            role("-exact-secondary-background", dark),
            role("-exact-separator", dark),
        )
    } else if node.props.str(PropId::AccessibilityInvalid) == Some("true") {
        (
            role("-exact-secondary-grouped-background", dark),
            role("-exact-system-red", dark),
        )
    } else {
        (
            role("-exact-secondary-grouped-background", dark),
            look::line(dark),
        )
    };
    backend.fill(&shape, ground, ts);
    edge(
        backend,
        &shape,
        look::px(metric::CONTROL_LINE, em),
        line,
        ts,
    );
    shape
}

/// A field's ink: the label's, the grey when disabled, and the node's own
/// `color` where its author wrote one. The page's `color` does not tint a
/// surface the host paints.
pub(super) fn field_ink(node: &NodeRef<'_>, dark: bool) -> [u8; 4] {
    if node.style.written.has(StyleId::TextColor) {
        super::rgba(node.text_color().resolve(dark))
    } else if disabled(node) {
        role("GrayText", dark)
    } else {
        role("-exact-label", dark)
    }
}

/// What a file field says: its button's words, and what stands beside it
/// until a file is chosen. A browser's own, in English.
pub(crate) const FILE_WORDS: (&str, &str) = ("Choose File", "No file chosen");

/// A file field's two texts in its own type, the button's at the look's
/// weight.
fn file_specs(node: &NodeRef<'_>, beside: &str) -> (crate::text::Spec, crate::text::Spec) {
    let style = node.computed_style(exact_kernel::StyleMask::INHERITED);
    let mut button = style.clone();
    button.font_weight = look::number(metric::CONTROL_WEIGHT) as u16;
    (
        super::text_spec(&button, FILE_WORDS.0),
        super::text_spec(&style, beside),
    )
}

/// A file field's size: an outline button the control's height, then the
/// words beside it.
pub(crate) fn file_size(engine: &mut crate::text::TextEngine, node: &NodeRef<'_>) -> (f32, f32) {
    let em = em(node);
    let (button, beside) = file_specs(node, FILE_WORDS.1);
    let wide = engine.paragraph(&button, None).width
        + 2.0 * look::px(metric::BUTTON_PADDING_INLINE, em)
        + 2.0 * look::px(metric::CONTROL_LINE, em)
        + look::px(metric::FILE_GAP, em)
        + engine.paragraph(&beside, None).width;
    (wide.ceil(), look::px(metric::CONTROL_HEIGHT, em))
}

impl super::Painter {
    /// A select's open menu, as the look draws a menu: a raised surface (a
    /// surface's corners, a divider round it and a short shadow under it),
    /// its rows in the label's ink with a control's corners, the chosen one
    /// on a neutral fill and a disabled one grey.
    pub(super) fn neutral_menu(&mut self, menu: &super::control::MenuPaint, dark: bool) {
        let ts = Transform::identity();
        let em = menu.style.font_size;
        let (x, y, w, _) = menu.rect;
        let panel = Shape::new(menu.rect, [look::px(metric::SURFACE_RADIUS, em); 4]);
        let shadow = Shape {
            rect: (
                x,
                y + look::px(metric::RAISED_SHADOW_Y, em),
                menu.rect.2,
                menu.rect.3,
            ),
            ..panel
        };
        let ink = fade([0, 0, 0, 255], look::number(metric::RAISED_SHADOW_ALPHA));
        // The page's blur radius is twice a Gaussian's deviation.
        let sigma = look::px(metric::RAISED_SHADOW_BLUR, em) / 2.0;
        self.backend.blurred_shadow(&shadow, ink, sigma, &panel, ts);
        self.backend.fill(
            &panel,
            role("-exact-secondary-grouped-background", dark),
            ts,
        );
        let line = look::px(metric::CONTROL_LINE, em);
        edge(
            self.backend.as_mut(),
            &panel,
            line,
            role("-exact-separator", dark),
            ts,
        );
        let pad = look::px(metric::MENU_PADDING, em);
        let inset = look::px(metric::MENU_ROW_PADDING_INLINE, em);
        for (i, (label, disabled)) in menu.rows.iter().enumerate() {
            let top = y + super::control::MENU_PAD + i as f32 * menu.row;
            if menu.chosen == Some(i) {
                let row = Shape::new(
                    (x + pad, top, w - 2.0 * pad, menu.row),
                    [look::px(metric::CONTROL_RADIUS, em); 4],
                );
                self.backend
                    .fill(&row, role("-exact-tertiary-fill", dark), ts);
            }
            let color = role(
                if *disabled {
                    "GrayText"
                } else {
                    "-exact-label"
                },
                dark,
            );
            let spec = super::text_spec(&menu.style, label);
            let paragraph = self.text.borrow_mut().paragraph(&spec, None);
            let oy = top + ((menu.row - paragraph.height) / 2.0).max(0.0);
            let palette = [crate::text::RunPaint { color, source: 0 }];
            self.backend.text(
                &mut self.text.borrow_mut(),
                &paragraph,
                &palette,
                (x + pad + inset, oy),
                ts,
            );
        }
    }

    /// A file field: the outline button, and beside it the words for no
    /// file, in the secondary label's ink. This host's picker hands the
    /// files to the app and keeps no name to show.
    pub(super) fn file_control(&mut self, node: &NodeRef<'_>, content: Rect4, ts: Transform) {
        let (dark, em) = (self.dark, em(node));
        let (x, y, w, h) = content;
        let dim = disabled(node);
        if dim {
            self.backend
                .push_opacity(look::number(metric::MARK_DISABLED_ALPHA));
        }
        let (button, beside) = file_specs(node, FILE_WORDS.1);
        let (button, beside) = {
            let mut engine = self.text.borrow_mut();
            (
                engine.paragraph(&button, None),
                engine.paragraph(&beside, None),
            )
        };
        let line = look::px(metric::CONTROL_LINE, em);
        let pad = look::px(metric::BUTTON_PADDING_INLINE, em);
        let wide = button.width + 2.0 * (pad + line);
        let flip = rtl(node);
        let left = if flip { x + w - wide } else { x };
        let shape = Shape::new(
            (left, y, wide, h),
            [look::px(metric::CONTROL_RADIUS, em); 4],
        );
        if !dim && self.feedback.0 == Some(node.id) {
            self.backend
                .fill(&shape, role("-exact-quaternary-fill", dark), ts);
        }
        edge(self.backend.as_mut(), &shape, line, look::line(dark), ts);
        let gap = look::px(metric::FILE_GAP, em);
        let texts = [
            (&button, left + line + pad, role("-exact-label", dark)),
            (
                &beside,
                if flip {
                    left - gap - beside.width
                } else {
                    left + wide + gap
                },
                role("-exact-secondary-label", dark),
            ),
        ];
        self.backend.push_clip(&Shape::rect(content), ts);
        for (paragraph, at, color) in texts {
            let palette = [crate::text::RunPaint {
                color,
                source: node.id,
            }];
            let oy = y + ((h - paragraph.height) / 2.0).max(0.0);
            self.backend.text(
                &mut self.text.borrow_mut(),
                paragraph,
                &palette,
                (at, oy),
                ts,
            );
        }
        self.backend.pop_clip();
        if dim {
            self.backend.pop_opacity();
        }
    }
}

/// A select's caret: a small triangle in the secondary label's ink, its
/// point down, `inset` from the box's inline end.
pub(super) fn caret(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    rect: Rect4,
    ts: Transform,
    dark: bool,
) {
    let (x, y, w, h) = rect;
    let inset = look::px(metric::FIELD_PADDING_INLINE, em(node));
    // The sheet's is 8 by 4.
    let left = if rtl(node) {
        x + inset
    } else {
        x + w - inset - 8.0
    };
    let top = y + h / 2.0 - 2.0;
    backend.push_clip(&Shape::rect((left, top, 8.0, 4.0)), ts);
    let side = 4.0 * std::f32::consts::SQRT_2;
    let square = Shape::rect((left + 4.0 - side / 2.0, top - side / 2.0, side, side));
    let turn = Transform::from_rotate_at(45.0, left + 4.0, top);
    backend.fill(
        &square,
        role("-exact-secondary-label", dark),
        ts.pre_concat(turn),
    );
    backend.pop_clip();
}

/// A list's row under the neutral look (LLP 1115.000 D16), as the page's
/// sheet draws it: over the row's own ground, the wash of what presses in
/// it (where a pointer hovers, and a stronger one while it is held), then,
/// under each row but the last, one line that only divides, inside the
/// row's box. Which node is a row, and whether its author wrote that edge,
/// is the kernel's to say (`Kernel::row_separator`); which node shows the
/// wash is the presenter's (`presenter/feedback.rs`). A control keeps its
/// own states.
pub(super) fn row(
    backend: &mut dyn Backend,
    kernel: &exact_kernel::Kernel,
    node: &NodeRef<'_>,
    outer: &Shape,
    ts: Transform,
    dark: bool,
    feedback: (Option<exact_kernel::ViewId>, Option<exact_kernel::ViewId>),
) {
    if !look::neutral() {
        return;
    }
    if node.node_type != exact_kernel::NodeType::Control {
        let wash = if feedback.1 == Some(node.id) {
            Some("-exact-tertiary-fill")
        } else if feedback.0 == Some(node.id) {
            Some("-exact-quaternary-fill")
        } else {
            None
        };
        if let Some(name) = wash {
            backend.fill(outer, role(name, dark), ts);
        }
    }
    if kernel.row_separator(node.id) {
        let (x, y, w, h) = outer.rect;
        let line = look::px(metric::CONTROL_LINE, em(node));
        // On whole device pixels, as a browser snaps a box's edge: a row
        // that layout put at half a pixel would else show the line as two
        // faint ones. Only where the row is drawn upright.
        let (top, line) = if ts.kx == 0.0 && ts.ky == 0.0 && ts.sy > 0.0 {
            let thick = (line * ts.sy).round().max(1.0);
            let bottom = (ts.sy * (y + h) + ts.ty).round();
            ((bottom - thick - ts.ty) / ts.sy, thick / ts.sy)
        } else {
            (y + h - line, line)
        };
        backend.fill(
            &Shape::rect((x, top, w, line)),
            role("-exact-separator", dark),
            ts,
        );
    }
}
