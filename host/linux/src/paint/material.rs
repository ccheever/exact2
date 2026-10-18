//! Material 3's filled text field (LLP 1115.001 D9: "a field's container is
//! Material's filled text field"), which Android's reader paints for a text
//! field round its `EditText` and for a field that shows a value: a select,
//! and a date or time control. Its numbers are the schema's Material column
//! (the `field-` and `state-` rows), Compose Material 3's at the pinned
//! version (LLP 1115.001.001 D2; `host/android/reference/rows.json` names
//! each one's symbol): a 56 dp container in `surfaceContainerHighest`, its
//! top corners 4 dp, its text 16 dp in, an active indicator along its foot
//! (1 dp `onSurfaceVariant`, 2 dp `primary` while focused) and a 24 dp
//! trailing icon 12 dp from its end. Its states are layers of `onSurface`
//! over the container: 8 % where a pointer hovers, 10 % while a field that
//! opens something is pressed. The colours are the roles the shell reported
//! (`presenter/palette.rs`).
//!
//! A field's `placeholder` is its label, as an `EditText`'s hint is in a
//! `TextInputLayout`: on the middle line in the field's own size while the
//! field is empty and unfocused, and otherwise floated above its text in
//! Material's body small (12 dp on a 16 dp line, 8 dp from the top), the
//! text below it on a 24 dp line. It moves between the two in 150 ms, at
//! once under Reduce Motion: that is Exact's, where Material 3 1.4 moves it
//! by its motion scheme's fast spatial spring (`MotionSchemeKeyTokens
//! .FastSpatial` in `TextFieldImpl.kt`), owed to LLP 1115.001.001 O3.
use super::{look, rgba, Rect4, Shape};
use exact_kernel::generated::metric;
use exact_kernel::style::metrics::{self, Column};
use exact_kernel::{Direction, FieldChrome, NodeRef, PropId, StyleId, StyleMask, ViewId};
use std::collections::{HashMap, HashSet};
use tiny_skia::Transform;

/// A length of the Material look, dp, whatever look the process shows: the
/// Material look reads its own column.
fn dp(index: usize) -> f32 {
    metrics::length_in(index, Column::Material, 0.0, 0.0).expect("a Material row")
}

/// An opacity of the Material look.
fn opacity(index: usize) -> f32 {
    metrics::number_in(index, Column::Material).expect("a Material row")
}

/// The container's height.
pub(crate) fn height() -> f32 {
    dp(metric::FIELD_HEIGHT)
}

/// The text's inset from either side, and the gap before a trailing icon.
fn inset() -> f32 {
    dp(metric::FIELD_PADDING)
}

/// A trailing icon's size.
fn icon_size() -> f32 {
    dp(metric::FIELD_ICON_SIZE)
}

/// A trailing icon's inset from the container's end.
fn icon_inset() -> f32 {
    dp(metric::FIELD_ICON_INSET)
}

/// What a field with a trailing icon has beside its text.
pub(crate) fn with_icon() -> f32 {
    inset() + inset() + icon_size() + icon_inset()
}

/// A floated label's top, and the line it is set on.
fn label_top() -> f32 {
    dp(metric::FIELD_LABEL_TOP)
}

fn label_line() -> f32 {
    dp(metric::FIELD_LABEL_LINE)
}

/// The top of the text under a floated label.
fn labelled_top() -> f32 {
    label_top() + label_line()
}

/// How long a label takes to float or to rest, ms (Exact's: see above).
const LABEL_MS: f64 = 150.0;

/// A text field's chrome: the text 16 dp in on every side of a 56 dp line;
/// below a label, from 24 dp down to 8 dp above the foot.
pub(crate) fn chrome(labelled: bool) -> FieldChrome {
    let (top, bottom) = match labelled {
        true => (labelled_top(), label_top()),
        false => (inset(), inset()),
    };
    FieldChrome {
        top,
        right: inset(),
        bottom,
        left: inset(),
        minimum_height: height(),
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
        let corner = dp(metric::FIELD_CORNER);
        let shape = Shape::new(rect, [corner, corner, 0.0, 0.0]);
        // Disabled, the container is the enabled one's, as the library draws
        // it (`TextFieldDefaults.colors`: `disabledContainerColor` is
        // `ContainerColor`, not the token's `onSurface` at 4 %), and the
        // indicator `onSurface` at 38 %.
        self.backend.fill(&shape, look::role("Field", dark), ts);
        let layer = match (
            off,
            (self.feedback.0, self.presses(node.id).then_some(node.id)),
        ) {
            (true, _) => 0.0,
            (_, (_, Some(id))) if opens && id == node.id => opacity(metric::STATE_PRESSED_OPACITY),
            (_, (Some(id), _)) if id == node.id => opacity(metric::STATE_HOVER_OPACITY),
            _ => 0.0,
        };
        if layer > 0.0 {
            let state = look::fade(look::role("CanvasText", dark), layer);
            self.backend.fill(&shape, state, ts);
        }
        let (thin, thick) = (
            dp(metric::FIELD_INDICATOR),
            dp(metric::FIELD_INDICATOR_FOCUSED),
        );
        let (line, thick) = match (off, focused) {
            (true, _) => (look::role("GrayText", dark), thin),
            (false, true) => (super::control::ring(node, dark), thick),
            (false, false) => (look::role("-exact-secondary-label", dark), thin),
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
        let floated = y + label_top() + (label_line() - paragraph.height) / 2.0;
        let oy = rest + (floated - rest) * up;
        let rtl = style.direction == Direction::Rtl;
        let ox = if rtl {
            x + w - inset() - paragraph.width
        } else {
            x + inset()
        };
        // An authored `::placeholder` colour wins (LLP 1104 D3). Enabled and
        // unfocused, resting or floated, the label is Material's
        // onSurfaceVariant (`-exact-secondary-label`), or the design's
        // `placeholder` where it overrides that role (LLP 1121 D9).
        let ink = match super::control::authored_placeholder(node.style, dark) {
            Some(ink) => ink,
            None if disabled(node) => look::role("GrayText", dark),
            None if focused => super::control::ring(node, dark),
            None => resting_label(dark),
        };
        self.backend.push_clip(
            &Shape::rect((x + inset(), y, (w - 2.0 * inset()).max(0.0), h)),
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
        let (inset, size, icon_inset) = (inset(), icon_size(), icon_inset());
        // The label's box ends at the icon, which RTL puts at the start.
        let beside = match rtl {
            true => (x + size + icon_inset, y, w - size - icon_inset, h),
            false => (x, y, w - size - icon_inset, h),
        };
        let labelled = self.field_label(node, beside, focused, text.is_empty(), ts);
        let paragraph = self
            .text
            .borrow_mut()
            .paragraph(&super::text_spec(&style, text), None);
        let room = (w - with_icon() + inset).max(0.0);
        let ox = if rtl {
            x + w - inset - paragraph.width.min(room)
        } else {
            x + inset
        };
        let (top, label_top) = (labelled_top(), label_top());
        let oy = match labelled {
            true => y + top + (h - top - label_top - paragraph.height) / 2.0,
            false => y + ((h - paragraph.height) / 2.0).max(0.0),
        };
        let clip = if rtl {
            (x + w - inset - room, y, room, h)
        } else {
            (x + inset, y, room, h)
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
            x + icon_inset
        } else {
            x + w - icon_inset - size
        };
        let at = (ix, y + (h - size) / 2.0, size, size);
        let tint = match off {
            true => look::role("GrayText", dark),
            false => look::role("-exact-secondary-label", dark),
        };
        match icon {
            Icon::Calendar => self.symbol_face("calendar", node.style, at, tint, ts),
            Icon::Clock => self.glyph_face(role_path(CLOCK, false), node.style, at, tint, ts),
            Icon::Menu => self.glyph_face(role_path(MENU, true), node.style, at, tint, ts),
        }
    }
}

/// A 24-unit path drawn as a role's is: stroked, or filled even-odd.
fn role_path(d: &str, filled: bool) -> exact_kernel::SymbolPath<'_> {
    exact_kernel::SymbolPath {
        d,
        filled,
        even_odd: true,
        view_box: "0 0 24 24",
    }
}

/// An enabled, unfocused label's ink: the design's `placeholder` where it
/// overrides that role (LLP 1121 D9), else Material's onSurfaceVariant.
fn resting_label(dark: bool) -> [u8; 4] {
    match placeholder_overridden() {
        true => look::role("-exact-placeholder", dark),
        false => look::role("-exact-secondary-label", dark),
    }
}

/// Whether the design overrides the `placeholder` role (LLP 1095 D4).
fn placeholder_overridden() -> bool {
    exact_kernel::style::roles::role("-exact-placeholder")
        .is_some_and(|id| exact_kernel::style::roles::overridden(id).is_some())
}

#[cfg(test)]
mod placeholder_tests {
    use exact_kernel::style::roles;
    use exact_kernel::ColorValue;

    /// The label's role: `placeholder` only where the design overrides it,
    /// else Material's onSurfaceVariant as before (stage 8 C's gap, review 1).
    #[test]
    fn the_label_reads_placeholder_only_where_the_design_overrides_it() {
        roles::set_overrides(Vec::new());
        assert!(!super::placeholder_overridden());
        // No override: secondary-label's ink, never placeholder's fainter default.
        for dark in [false, true] {
            assert_eq!(
                super::resting_label(dark),
                super::look::role("-exact-secondary-label", dark)
            );
            assert_ne!(
                super::resting_label(dark),
                super::look::role("-exact-placeholder", dark)
            );
        }
        let id = roles::role("-exact-placeholder").unwrap();
        roles::set_overrides(vec![(
            id,
            ColorValue::LightDark(
                exact_kernel::Color(0xd948_0fff),
                exact_kernel::Color(0xd948_0fff),
            ),
        )]);
        assert!(super::placeholder_overridden());
        // The override's own colour, in both schemes.
        for dark in [false, true] {
            assert_eq!(super::resting_label(dark), [0xd9, 0x48, 0x0f, 0xff]);
        }
        roles::set_overrides(Vec::new());
    }
}
