//! Form controls, painted (LLP 1069.001 D7): Linux has no platform
//! controls, so a checkbox is Chrome's rounded square and a switch a pill
//! with a thumb, filled with `accent-color`. `appearance: none` draws
//! nothing here: the author's box is the whole look (D6).

use super::{rgba, Backend, Rect4, Shape};
use exact_kernel::{Appearance, NodeRef, PropId, StyleMask};
use tiny_skia::Transform;

/// Chrome's default accent, `#0075ff`, where `accent-color` is `auto`.
const ACCENT: [u8; 4] = [0x00, 0x75, 0xff, 0xff];

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
