//! Material 3's filled text field (LLP 1115.001 D9: "a field's container is
//! Material's filled text field"), which Android's reader paints for a text
//! field round its `EditText` and for a field that shows a value: a select,
//! and a date or time control. The metrics and roles are the component's
//! (<https://m3.material.io/components/text-fields/specs>): a 56 dp
//! container in `surfaceContainerHighest`, its top corners 4 dp, its text
//! 16 dp in, an active indicator along its foot (1 dp `onSurfaceVariant`,
//! 2 dp `primary` while focused) and a 24 dp trailing icon 12 dp from its
//! end. Its states are layers of `onSurface` over the container: 8 % where a
//! pointer hovers, 10 % while a field that opens something is pressed. The
//! colours are the roles the shell reported (`presenter/palette.rs`).
//!
//! A field's `placeholder` is its label, as an `EditText`'s hint is in a
//! `TextInputLayout`: on the middle line in the field's own size while the
//! field is empty and unfocused, and otherwise floated above its text in
//! Material's body small (12 dp on a 16 dp line, 8 dp from the top), the
//! text below it on a 24 dp line. It moves between the two in 150 ms, at
//! once under Reduce Motion.
use super::{look, rgba, Rect4, Shape};
use exact_kernel::{Direction, FieldChrome, NodeRef, PropId, StyleId, StyleMask, ViewId};
use std::collections::{HashMap, HashSet};
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

/// A floated label's top, the line it is set on, and the top of the text
/// under it.
const LABEL_TOP: f32 = 8.0;
const LABEL_LINE: f32 = 16.0;
const LABELLED_TOP: f32 = 24.0;
/// How long a label takes to float or to rest, ms.
const LABEL_MS: f64 = 150.0;

/// A text field's chrome: the text 16 dp in on every side of a 56 dp line;
/// below a label, from 24 dp down to 8 dp above the foot.
pub(crate) fn chrome(labelled: bool) -> FieldChrome {
    let (top, bottom) = match labelled {
        true => (LABELLED_TOP, LABEL_TOP),
        false => (INSET, INSET),
    };
    FieldChrome {
        top,
        right: INSET,
        bottom,
        left: INSET,
        minimum_height: HEIGHT,
        provisional: false,
    }
}

/// A field's label: its `placeholder`, where it has one.
pub(crate) fn label<'a>(node: &NodeRef<'a>) -> Option<&'a str> {
    node.props
        .str(PropId::Placeholder)
        .filter(|p| !p.is_empty())
}

/// Where each field's label is, and where it is going.
#[derive(Default)]
pub(crate) struct Labels {
    at: HashMap<ViewId, Float>,
    painted: HashSet<ViewId>,
    /// The latest paint's clock, ms.
    now: f64,
}

#[derive(Clone, Copy)]
struct Float {
    up: bool,
    /// Where it was when it last turned, 0 resting and 1 floated.
    from: f32,
    since: f64,
}

impl Float {
    fn at(&self, now: f64) -> f32 {
        let t = ((now - self.since) / LABEL_MS).clamp(0.0, 1.0) as f32;
        // Material's standard easing, near enough: fast out, slow in.
        let e = 1.0 - (1.0 - t).powi(3);
        let to = if self.up { 1.0 } else { 0.0 };
        self.from + (to - self.from) * e
    }
}

impl Labels {
    /// A label still moving owes a frame.
    pub(crate) fn moving(&self) -> bool {
        (self.at.values())
            .any(|f| self.now - f.since < LABEL_MS && f.at(self.now) != f.at(f.since + LABEL_MS))
    }

    /// When the last label still moving ends, ms.
    pub(crate) fn settles_at(&self) -> Option<f64> {
        (self.at.values())
            .map(|f| f.since + LABEL_MS)
            .filter(|end| *end > self.now)
            .reduce(f64::max)
    }

    /// How far `id`'s label has floated now, given where it is going.
    fn float(&mut self, id: ViewId, up: bool, now: f64, still: bool) -> f32 {
        self.now = now;
        self.painted.insert(id);
        let f = self.at.entry(id).or_insert(Float {
            up,
            from: if up { 1.0 } else { 0.0 },
            since: now - LABEL_MS,
        });
        if f.up != up {
            let from = if still {
                f32::from(u8::from(up))
            } else {
                f.at(now)
            };
            *f = Float {
                up,
                from,
                since: if still { now - LABEL_MS } else { now },
            };
        }
        f.at(now)
    }

    /// After a paint: the labels the walk did not paint are gone.
    pub(super) fn painted(&mut self) {
        let painted = std::mem::take(&mut self.painted);
        self.at.retain(|id, _| painted.contains(id));
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
    /// returns the container's shape. `opens`: a press opens something (a
    /// menu, a picker), so it shows a pressed state.
    pub(super) fn filled_field(
        &mut self,
        node: &NodeRef<'_>,
        rect: Rect4,
        focused: bool,
        opens: bool,
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
        let layer = match (off, self.feedback) {
            (true, _) => 0.0,
            (_, (_, Some(id))) if opens && id == node.id => 0.10,
            (_, (Some(id), _)) if id == node.id => 0.08,
            _ => 0.0,
        };
        if layer > 0.0 {
            let state = look::fade(look::role("CanvasText", dark), layer);
            self.backend.fill(&shape, state, ts);
        }
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

    /// `node`'s label in its field's box `rect`, where it has one: resting
    /// while `empty` and not `focused`, else floated. Whether it has one.
    pub(super) fn field_label(
        &mut self,
        node: &NodeRef<'_>,
        rect: Rect4,
        focused: bool,
        empty: bool,
        ts: Transform,
    ) -> bool {
        let Some(text) = label(node) else {
            return false;
        };
        // A label moves: its row is drawn by the walk, never from a recording.
        self.row_refuse();
        let dark = self.dark;
        let (now, still) = (self.numerals.now, self.numerals.still);
        let up = self.labels.float(node.id, focused || !empty, now, still);
        let mut style = node.computed_style(StyleMask::INHERITED);
        let size = style.font_size;
        style.font_size = size + (size * 0.75 - size) * up;
        let paragraph = self
            .text
            .borrow_mut()
            .paragraph(&super::text_spec(&style, text), None);
        let (x, y, w, h) = rect;
        let rest = y + (h - paragraph.height) / 2.0;
        let floated = y + LABEL_TOP + (LABEL_LINE - paragraph.height) / 2.0;
        let oy = rest + (floated - rest) * up;
        let rtl = style.direction == Direction::Rtl;
        let ox = if rtl {
            x + w - INSET - paragraph.width
        } else {
            x + INSET
        };
        // An authored `::placeholder` colour wins (LLP 1104 D3).
        let ink = match super::control::authored_placeholder(node.style, dark) {
            Some(ink) => ink,
            None if disabled(node) => look::role("GrayText", dark),
            None if focused => super::control::ring(node, dark),
            None => look::role("-exact-secondary-label", dark),
        };
        self.backend.push_clip(
            &Shape::rect((x + INSET, y, (w - 2.0 * INSET).max(0.0), h)),
            ts,
        );
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
        true
    }

    /// A field that shows `text` and opens something when pressed: its
    /// container, its label, the text on its middle line (or under the
    /// label) and its trailing `icon`.
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
        self.filled_field(node, rect, focused, true, ts);
        let style = node.computed_style(StyleMask::INHERITED);
        // An inherited `color` does not tint the field (D9); the node's own does.
        let ink = match (off, node.style.mask.has(StyleId::TextColor)) {
            (true, _) => look::role("GrayText", dark),
            (false, true) => rgba(node.text_color().resolve(dark)),
            (false, false) => look::role("FieldText", dark),
        };
        let rtl = style.direction == Direction::Rtl;
        let (x, y, w, h) = rect;
        // The label's box ends at the icon, which RTL puts at the start.
        let beside = match rtl {
            true => (x + ICON + ICON_INSET, y, w - ICON - ICON_INSET, h),
            false => (x, y, w - ICON - ICON_INSET, h),
        };
        let labelled = self.field_label(node, beside, focused, text.is_empty(), ts);
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
        let oy = match labelled {
            true => y + LABELLED_TOP + (h - LABELLED_TOP - LABEL_TOP - paragraph.height) / 2.0,
            false => y + ((h - paragraph.height) / 2.0).max(0.0),
        };
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
