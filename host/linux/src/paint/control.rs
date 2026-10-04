//! Form controls, painted (LLP 1069.001 D7): Linux has no platform
//! controls, so a checkbox is Chrome's rounded square and a switch a pill
//! with a thumb, filled with `accent-color`. `appearance: none` draws
//! nothing here: the author's box is the whole look (D6).

use super::{rgba, Backend, Rect4, Shape};
use exact_kernel::{Appearance, NodeRef, PropId, StyleMask};
use tiny_skia::Transform;

/// Chrome's default accent, `#0075ff`, where `accent-color` is `auto`.
const ACCENT: [u8; 4] = [0x00, 0x75, 0xff, 0xff];

/// What a date control shows: the person's choice while its bound value is
/// the one it had when they chose (the web build writes an input's `value`
/// only when the bound value changes; x2apps kanban2 #5), else the bound
/// value, HTML's placeholder form when empty.
pub(super) fn date_text<'a>(node: &NodeRef<'a>, chosen: Option<&'a (String, String)>) -> &'a str {
    let bound = node.props.str(PropId::Value).unwrap_or("");
    let value = match chosen {
        Some((choice, at)) if at == bound => choice.as_str(),
        _ => bound,
    };
    match (value, node.props.str(PropId::Type)) {
        ("", Some("date")) => "yyyy-mm-dd",
        ("", Some("time")) => "--:--",
        ("", _) => "yyyy-mm-ddT--:--",
        (v, _) => v,
    }
}

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

    /// A range (LLP 1069.001 D7): Chrome's track, filled with the accent to
    /// a 16 px thumb at the value.
    pub(super) fn range_control(&mut self, node: &NodeRef<'_>, content: Rect4, ts: Transform) {
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
        let range = exact_kernel::Range::of(node.props);
        let value = range.shown(node.props);
        let t = if range.max > range.min {
            ((value - range.min) / (range.max - range.min)) as f32
        } else {
            0.0
        };
        let (x, y, w, h) = content;
        let d = 16f32.min(h).min(w);
        let cy = y + h / 2.0;
        let tx = x + (w - d) * t;
        let track =
            |from: f32, to: f32| Shape::new((from, cy - 2.0, (to - from).max(0.0), 4.0), [2.0; 4]);
        let rest = if dark {
            [0x78, 0x78, 0x80, 0x5c]
        } else {
            [0x78, 0x78, 0x80, 0x33]
        };
        self.backend.fill(&track(x, x + w), alpha(rest), ts);
        let accent = accent(node, dark).unwrap_or(ACCENT);
        self.backend
            .fill(&track(x, tx + d / 2.0), alpha(accent), ts);
        let thumb = Shape::new((tx, cy - d / 2.0, d, d), [d / 2.0; 4]);
        self.backend.fill(&thumb, alpha(accent), ts);
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

/// A native button's look on Linux and its metrics (LLP 1069.011 D2, D6): the
/// `buttonStyles` row's web/Linux look, as the web's stylesheet draws it.
/// `ua` is Chrome's own button; the others a pill padded 7/12. Both set the
/// title in Chrome's 13.33 px button font, without inherited typography.
pub(crate) fn button_look(node: &NodeRef<'_>) -> &'static str {
    let name = node.props.str(PropId::ButtonStyle).unwrap_or("bordered");
    exact_kernel::generated::button_style(name)
        .or_else(|| exact_kernel::generated::button_style("bordered"))
        .map_or("ua", |s| s.look)
}

/// A native button's title as the look sets it: Chrome's button font, no
/// inherited typography.
pub(crate) fn button_text_style() -> exact_kernel::StyleProps {
    exact_kernel::StyleProps {
        font_size: 13.333,
        // One line, as the web's face (`white-space: nowrap`) and the
        // platforms' titles are; its end is ellipsized where it is too wide.
        white_space: exact_kernel::WhiteSpace::Nowrap,
        ..Default::default()
    }
}

/// A look's padding around its title: (horizontal, vertical), each side.
pub(crate) fn button_padding(look: &str) -> (f32, f32) {
    if look == "ua" {
        (7.0, 2.0)
    } else {
        (12.0, 7.0)
    }
}

impl super::Painter {
    /// A native button (LLP 1069.011): its look's fill and its title in
    /// the look's ink. Linux draws no symbols (LLP 1035.004 D4).
    pub(super) fn button_control(
        &mut self,
        node: &NodeRef<'_>,
        content: Rect4,
        ts: Transform,
        title: &str,
    ) {
        let dark = self.dark;
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let dim = |mut c: [u8; 4]| {
            if disabled {
                c[3] = (c[3] as f32 * 0.45) as u8;
            }
            c
        };
        let accent = accent(node, dark).unwrap_or(ACCENT);
        let soft = |a: u8| [accent[0], accent[1], accent[2], a];
        let white = [0xff, 0xff, 0xff, 0xff];
        let label = if dark { white } else { [0, 0, 0, 0xff] };
        let look = button_look(node);
        let (_, _, _, h) = content;
        let pill = Shape::new(content, [h / 2.0; 4]);
        let ink = match look {
            "text" => accent,
            "soft" => {
                self.backend.fill(&pill, dim(soft(0x26)), ts);
                accent
            }
            "fill" => {
                self.backend.fill(&pill, dim(accent), ts);
                white
            }
            "glass" => {
                let tint = if dark {
                    [0x26, 0x26, 0x29, 0xb3]
                } else {
                    [0xff, 0xff, 0xff, 0xb3]
                };
                self.backend.fill(&pill, dim(tint), ts);
                label
            }
            "glass-fill" => {
                self.backend.fill(&pill, dim(soft(0xd9)), ts);
                white
            }
            _ => {
                // Chrome's own button: a 1 px border round a grey fill.
                let frame = Shape::new(content, [4.0; 4]);
                let (line, fill) = if dark {
                    ([0x85, 0x85, 0x85, 0xff], [0x6b, 0x6b, 0x6b, 0xff])
                } else {
                    ([0x76, 0x76, 0x76, 0xff], [0xef, 0xef, 0xef, 0xff])
                };
                self.backend.fill(&frame, dim(line), ts);
                self.backend.fill(&frame.inset(1.0), dim(fill), ts);
                label
            }
        };
        if title.is_empty() {
            return;
        }
        // One line, ending in "…" where the look's padding leaves too little
        // room, and clipped to the button, as the platforms draw a title.
        let spec = super::text_spec(&button_text_style(), title);
        let full = self.text.borrow_mut().paragraph(&spec, None);
        let (x, y, w, h) = content;
        let room = (w - 2.0 * button_padding(look).0).max(0.0);
        let paragraph = full.ellipsized(room).unwrap_or(full);
        let origin = (
            x + ((w - paragraph.width) / 2.0).max(0.0),
            y + ((h - paragraph.height) / 2.0).max(0.0),
        );
        let palette = [crate::text::RunPaint {
            color: dim(ink),
            source: node.id,
        }];
        self.backend.push_clip(&Shape::rect(content), ts);
        let mut engine = self.text.borrow_mut();
        self.backend
            .text(&mut engine, &paragraph, &palette, origin, ts);
        drop(engine);
        self.backend.pop_clip();
    }
}
