//! Form controls, painted (LLP 1069.001 D7): Linux has no platform
//! controls, so a checkbox is Chrome's rounded square and a switch a pill
//! with a thumb, filled with `accent-color`. `appearance: none` draws
//! nothing here: the author's box is the whole look (D6).

use super::{rgba, Backend, Rect4, Shape};
use exact_kernel::{Appearance, NodeRef, PropId, StyleMask};
use tiny_skia::Transform;

/// Chrome's default accent, `#0075ff`, where `accent-color` is `auto`.
const ACCENT: [u8; 4] = [0x00, 0x75, 0xff, 0xff];

/// A node's `accent-color`, where it sets one.
pub(crate) fn accent(node: &NodeRef<'_>, dark: bool) -> Option<[u8; 4]> {
    node.computed_style(StyleMask::INHERITED)
        .accent_color
        .map(|c| rgba(c.resolve(dark)))
}

/// A select's open menu (LLP 1069.001 D7): its panel in viewport points,
/// one row per option, the chosen one marked with the accent.
#[derive(Debug, Clone)]
pub struct MenuPaint {
    /// The panel.
    pub rect: Rect4,
    /// Each row's height.
    pub row: f32,
    /// Each option's label and whether it is disabled.
    pub rows: Vec<(String, bool)>,
    /// The chosen row.
    pub chosen: Option<usize>,
    /// The select's inherited text style, which the rows are set in.
    pub style: exact_kernel::StyleProps,
    /// The select's accent, where it sets one.
    pub accent: Option<[u8; 4]>,
}

/// The menu's inset above its first row and below its last.
pub const MENU_PAD: f32 = 4.0;

impl super::Painter {
    /// A closed select, or a field that shows a value (LLP 1069.001 D7): a
    /// rounded box with the text, and a chevron when it opens a menu.
    pub(super) fn field_control(
        &mut self,
        node: &NodeRef<'_>,
        content: Rect4,
        ts: Transform,
        text: &str,
        chevron: bool,
    ) {
        if node.style.appearance == Appearance::None {
            return;
        }
        let dark = self.dark;
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let alpha = |mut c: [u8; 4]| {
            if disabled {
                c[3] /= 2;
            }
            c
        };
        let frame = Shape::new(content, [4.0; 4]);
        let (line, fill) = if dark {
            ([0x85, 0x85, 0x85, 0xff], [0x3b, 0x3b, 0x3b, 0xff])
        } else {
            ([0x76, 0x76, 0x76, 0xff], [0xff, 0xff, 0xff, 0xff])
        };
        self.backend.fill(&frame, alpha(line), ts);
        self.backend.fill(&frame.inset(1.0), alpha(fill), ts);
        let style = node.computed_style(StyleMask::INHERITED);
        let ink = alpha(rgba(node.text_color().resolve(dark)));
        let spec = super::text_spec(&style, text);
        let paragraph = self.text.borrow_mut().paragraph(&spec, None);
        let (x, y, w, h) = content;
        let oy = y + ((h - paragraph.height) / 2.0).max(0.0);
        {
            let mut engine = self.text.borrow_mut();
            let palette = [crate::text::RunPaint {
                color: ink,
                source: node.id,
            }];
            self.backend
                .text(&mut engine, &paragraph, &palette, (x + 6.0, oy), ts);
        }
        if chevron {
            // A "v": two bars from its point, below the middle at the right.
            let (px, py) = (x + w - 11.0, y + h / 2.0 + 2.5);
            for angle in [-135.0f32, -45.0] {
                let turn = Transform::from_rotate_at(angle, px, py);
                let bar = Shape::rect((px - 0.75, py - 0.75, 6.0, 1.5));
                self.backend.fill(&bar, ink, ts.pre_concat(turn));
            }
        }
    }

    /// A select's open menu, over everything (LLP 1069.001 D7).
    pub(super) fn menu(&mut self, menu: &MenuPaint) {
        let ts = Transform::identity();
        let dark = self.dark;
        let panel = Shape::new(menu.rect, [6.0; 4]);
        self.backend.fill(&panel, [0, 0, 0, 0x40], ts);
        let bg = if dark {
            [0x2c, 0x2c, 0x2e, 0xff]
        } else {
            [0xff, 0xff, 0xff, 0xff]
        };
        self.backend.fill(&panel.inset(1.0), bg, ts);
        let accent = menu.accent.unwrap_or(ACCENT);
        let (x, y, w, _) = menu.rect;
        for (i, (label, disabled)) in menu.rows.iter().enumerate() {
            let top = y + MENU_PAD + i as f32 * menu.row;
            let chosen = menu.chosen == Some(i);
            if chosen {
                let row = Shape::new((x + 4.0, top, w - 8.0, menu.row), [4.0; 4]);
                self.backend.fill(&row, accent, ts);
            }
            let mut ink = if chosen || dark {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [0x00, 0x00, 0x00, 0xff]
            };
            if *disabled {
                ink[3] = 0x66;
            }
            let spec = super::text_spec(&menu.style, label);
            let paragraph = self.text.borrow_mut().paragraph(&spec, None);
            let oy = top + ((menu.row - paragraph.height) / 2.0).max(0.0);
            let mut engine = self.text.borrow_mut();
            let palette = [crate::text::RunPaint {
                color: ink,
                source: 0,
            }];
            self.backend
                .text(&mut engine, &paragraph, &palette, (x + 12.0, oy), ts);
        }
    }
}

/// Paint `node`'s control in its content box; `held` is an unbound
/// checkbox's own state, which the host keeps as a browser does.
pub(super) fn paint(
    backend: &mut dyn Backend,
    node: &NodeRef<'_>,
    content: Rect4,
    ts: Transform,
    dark: bool,
    held: Option<bool>,
) {
    if node.style.appearance == Appearance::None {
        return;
    }
    let on = node.props.bool(PropId::Checked).or(held).unwrap_or(false);
    let disabled = node.props.bool(PropId::Disabled) == Some(true);
    let accent = node
        .computed_style(StyleMask::INHERITED)
        .accent_color
        .map_or(ACCENT, |c| rgba(c.resolve(dark)));
    let dim = |mut c: [u8; 4]| {
        if disabled {
            c[3] /= 2;
        }
        c
    };
    let (x, y, w, h) = content;
    if node.props.str(PropId::AccessibilityRole) == Some("switch") {
        let track = Shape::new(content, [h / 2.0; 4]);
        // UIKit's `systemFill` (Safari's off track): translucent, so it
        // reads on a grey surface as on white.
        let off = if dark {
            [0x78, 0x78, 0x80, 0x5c]
        } else {
            [0x78, 0x78, 0x80, 0x33]
        };
        backend.fill(&track, dim(if on { accent } else { off }), ts);
        let d = (h - 4.0).max(0.0);
        let tx = if on { x + w - 2.0 - d } else { x + 2.0 };
        let thumb = Shape::new((tx, y + 2.0, d, d), [d / 2.0; 4]);
        backend.fill(&thumb, dim([0xff, 0xff, 0xff, 0xff]), ts);
        return;
    }
    let square = Shape::new(content, [2.0; 4]);
    if on {
        backend.fill(&square, dim(accent), ts);
        // The check: two strokes, as rotated bars about their joint.
        let s = w.min(h);
        let (cx, cy) = (x + w * 0.42, y + h * 0.7);
        let bar = (s * 0.14).max(1.5);
        let white = dim([0xff, 0xff, 0xff, 0xff]);
        let short = Transform::from_rotate_at(45.0, cx, cy);
        backend.fill(
            &Shape::rect((cx - s * 0.3, cy - bar / 2.0, s * 0.3 + bar / 2.0, bar)),
            white,
            ts.pre_concat(short),
        );
        let long = Transform::from_rotate_at(-50.0, cx, cy);
        backend.fill(
            &Shape::rect((cx - bar / 2.0, cy - bar / 2.0, s * 0.62, bar)),
            white,
            ts.pre_concat(long),
        );
    } else {
        let (border, fill) = if dark {
            ([0x85, 0x85, 0x85, 0xff], [0x3b, 0x3b, 0x3b, 0xff])
        } else {
            ([0x76, 0x76, 0x76, 0xff], [0xff, 0xff, 0xff, 0xff])
        };
        backend.fill(&square, dim(border), ts);
        backend.fill(&square.inset(1.0), dim(fill), ts);
    }
}
