//! Material 3's filled text field (LLP 1115.001 D9: "a field's container is
//! Material's filled text field"), which Android's reader paints for a text
//! field round its `EditText` and for a field that shows a value: a select,
//! and a date or time control. The metrics and roles are the component's
//! (<https://m3.material.io/components/text-fields/specs>): a 56 dp
//! container in `surfaceContainerHighest`, its top corners 4 dp, its text
//! 16 dp in, an active indicator along its foot (1 dp `onSurfaceVariant`,
//! 2 dp `primary` while focused) and a 24 dp trailing icon 12 dp from its
//! end. The colours are the roles the shell reported (`presenter/palette.rs`).
use super::{look, rgba, Rect4, Shape};
use exact_kernel::{Direction, FieldChrome, NodeRef, PropId, StyleId, StyleMask};
use tiny_skia::Transform;

/// The container's height.
pub(crate) const HEIGHT: f32 = 56.0;
/// The text's inset from either side, and the gap before a trailing icon.
const INSET: f32 = 16.0;
/// A trailing icon's size and its inset from the container's end.
const ICON: f32 = 24.0;
const ICON_INSET: f32 = 12.0;
/// What a field with a trailing icon has beside its text.
pub(crate) const WITH_ICON: f32 = INSET + INSET + ICON + ICON_INSET;

/// A text field's chrome: the text 16 dp in on every side of a 56 dp line.
pub(crate) fn chrome() -> FieldChrome {
    FieldChrome {
        top: INSET,
        right: INSET,
        bottom: INSET,
        left: INSET,
        minimum_height: HEIGHT,
        provisional: false,
    }
}

/// What a field showing a value has at its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    /// A select's: Material's `arrow_drop_down`.
    Menu,
    /// A date's: the calendar.
    Calendar,
    /// A time's: Material's `schedule`.
    Clock,
}

/// `arrow_drop_down`, filled.
const MENU: &str = "M7 10l5 5 5-5Z";
/// `schedule`'s face and hands, stroked as a symbol role is.
const CLOCK: &str = "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18ZM12 7v5l3.5 2";

fn disabled(node: &NodeRef<'_>) -> bool {
    node.props.bool(PropId::Disabled) == Some(true)
}

impl super::Painter {
    /// The container and its indicator in `rect`, the field's border box;
    /// returns the container's shape.
    pub(super) fn filled_field(
        &mut self,
        node: &NodeRef<'_>,
        rect: Rect4,
        focused: bool,
        ts: Transform,
    ) -> Shape {
        let dark = self.dark;
        let off = disabled(node);
        let shape = Shape::new(rect, [4.0, 4.0, 0.0, 0.0]);
        // Disabled: `onSurface` at 4 %, and the indicator at 38 %.
        let fill = match off {
            true => look::fade(look::role("CanvasText", dark), 0.04),
            false => look::role("Field", dark),
        };
        self.backend.fill(&shape, fill, ts);
        let (line, thick) = match (off, focused) {
            (true, _) => (look::role("GrayText", dark), 1.0),
            (false, true) => (super::control::ring(node, dark), 2.0),
            (false, false) => (look::role("-exact-secondary-label", dark), 1.0),
        };
        let (x, y, w, h) = rect;
        self.backend
            .fill(&Shape::rect((x, y + h - thick, w, thick)), line, ts);
        shape
    }

    /// A field that shows `text` and opens something when pressed: its
    /// container, the text on its middle line and its trailing `icon`.
    pub(super) fn filled_value(
        &mut self,
        node: &NodeRef<'_>,
        rect: Rect4,
        focused: bool,
        ts: Transform,
        text: &str,
        icon: Icon,
    ) {
        let dark = self.dark;
        let off = disabled(node);
        self.filled_field(node, rect, focused, ts);
        let style = node.computed_style(StyleMask::INHERITED);
        // An inherited `color` does not tint the field (D9); the node's own does.
        let ink = match (off, node.style.mask.has(StyleId::TextColor)) {
            (true, _) => look::role("GrayText", dark),
            (false, true) => rgba(node.text_color().resolve(dark)),
            (false, false) => look::role("FieldText", dark),
        };
        let rtl = style.direction == Direction::Rtl;
        let (x, y, w, h) = rect;
        let paragraph = self
            .text
            .borrow_mut()
            .paragraph(&super::text_spec(&style, text), None);
        let room = (w - WITH_ICON + INSET).max(0.0);
        let ox = if rtl {
            x + w - INSET - paragraph.width.min(room)
        } else {
            x + INSET
        };
        let oy = y + ((h - paragraph.height) / 2.0).max(0.0);
        let clip = if rtl {
            (x + w - INSET - room, y, room, h)
        } else {
            (x + INSET, y, room, h)
        };
        self.backend.push_clip(&Shape::rect(clip), ts);
        self.backend.text(
            &mut self.text.borrow_mut(),
            &paragraph,
            &[crate::text::RunPaint {
                color: ink,
                source: node.id,
            }],
            (ox, oy),
            ts,
        );
        self.backend.pop_clip();
        let ix = if rtl {
            x + ICON_INSET
        } else {
            x + w - ICON_INSET - ICON
        };
        let at = (ix, y + (h - ICON) / 2.0, ICON, ICON);
        let tint = match off {
            true => look::role("GrayText", dark),
            false => look::role("-exact-secondary-label", dark),
        };
        match icon {
            Icon::Calendar => self.symbol_face("calendar", node.style, at, tint, ts),
            Icon::Clock => self.glyph_face(CLOCK, false, node.style, at, tint, ts),
            Icon::Menu => self.glyph_face(MENU, true, node.style, at, tint, ts),
        }
    }
}
