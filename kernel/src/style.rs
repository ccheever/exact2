//! Style value types and the one lowering onto Taffy.
//!
//! The generated `StyleProps` holds the rows; this module holds the value
//! grammars the rows use (dimensions, colors, grid tracks and placements) and
//! `to_taffy`, the single place where authored style becomes engine style.
//! Percentages are authored as points (0–100) on the wire and in storage and
//! are converted to Taffy's fraction exactly once, here.

use taffy::prelude::{auto, fr, length, line, max_content, min_content, percent, span};
use taffy::style::TrackSizingFunction;

use crate::arena::NodeArena;
use crate::error::StyleValueError;
use crate::generated::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Display, FlexDirection, FlexWrap, GridAutoFlow,
    JustifyContent, NodeType, Overflow, PositionType, StyleId, StyleProps,
};

/// Largest grid track list the closed grammar carries.
pub const MAX_GRID_TRACKS: usize = 32;

/// A length: automatic, absolute points, or a percentage of the parent (0–100).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dimension {
    /// Let the engine decide.
    #[default]
    Auto,
    /// Layout points.
    Points(f32),
    /// Percent of the containing block, authored as 0–100.
    Percent(f32),
}

impl Dimension {
    /// Whether the value is a finite number (or `Auto`).
    pub fn is_finite(self) -> bool {
        match self {
            Dimension::Auto => true,
            Dimension::Points(v) | Dimension::Percent(v) => v.is_finite(),
        }
    }

    fn to_taffy(self) -> taffy::style::Dimension {
        match self {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
        }
    }

    fn to_lpa(self) -> taffy::style::LengthPercentageAuto {
        match self {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
        }
    }

    /// Rows that do not admit `auto` (padding) lower it to zero; the decoder
    /// already refuses `auto` there, so this arm is unreachable from the wire.
    fn to_lp(self) -> taffy::style::LengthPercentage {
        match self {
            Dimension::Auto => length(0.0_f32),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
        }
    }
}

/// A packed RGBA color, `0xRRGGBBAA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color(pub u32);

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Color = Color(0);
    /// Opaque black.
    pub const BLACK: Color = Color(0x0000_00ff);
    /// Opaque white.
    pub const WHITE: Color = Color(0xffff_ffff);

    /// From channels.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32)
    }

    /// Red channel.
    pub const fn r(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// Green channel.
    pub const fn g(self) -> u8 {
        (self.0 >> 16) as u8
    }

    /// Blue channel.
    pub const fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Alpha channel.
    pub const fn a(self) -> u8 {
        self.0 as u8
    }
}

/// The `transition` row's type: CSS `transition` declarations, owned by
/// `exact-motion` so the evaluator and the kernel share one definition. The
/// kernel owns the bytes (`wire::codec`); the engine owns the semantics.
pub use exact_motion::Transitions;

/// An untyped style value from a producer that resolves rows by id — a plan
/// runner, a compiler lowering a literal, a TypeScript encoder. Exactly one
/// place turns it into a row: the generated `StyleProps::set_dynamic`.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleValue {
    /// A number: points for dimensions, the raw value for numeric rows, a
    /// packed `0xRRGGBBAA` for colors.
    Number(f64),
    /// Text: an enum value by name, or a color as `#rrggbb[aa]`.
    Text(String),
    /// A percentage, authored 0–100.
    Percent(f64),
    /// `auto`.
    Auto,
    /// Two numbers.
    Vec2(f32, f32),
}

impl StyleValue {
    pub(crate) fn f32(&self, style: StyleId) -> Result<f32, StyleValueError> {
        match self {
            StyleValue::Number(n) if (*n as f32).is_finite() => Ok(*n as f32),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "number",
            }),
        }
    }

    pub(crate) fn int(&self, style: StyleId, min: f64, max: f64) -> Result<i64, StyleValueError> {
        match self {
            StyleValue::Number(n) if n.is_finite() && n.fract() == 0.0 => {
                if *n < min || *n > max {
                    Err(StyleValueError::OutOfRange { style })
                } else {
                    Ok(*n as i64)
                }
            }
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "integer",
            }),
        }
    }

    pub(crate) fn text(&self, style: StyleId) -> Result<&str, StyleValueError> {
        match self {
            StyleValue::Text(t) => Ok(t),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "text",
            }),
        }
    }

    pub(crate) fn dimension(
        &self,
        style: StyleId,
        admits_auto: bool,
    ) -> Result<Dimension, StyleValueError> {
        match self {
            StyleValue::Number(n) if (*n as f32).is_finite() => Ok(Dimension::Points(*n as f32)),
            StyleValue::Percent(p) if (*p as f32).is_finite() => Ok(Dimension::Percent(*p as f32)),
            StyleValue::Auto if admits_auto => Ok(Dimension::Auto),
            StyleValue::Auto => Err(StyleValueError::AutoNotAdmitted { style }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "number, percent, or auto",
            }),
        }
    }

    pub(crate) fn color(&self, style: StyleId) -> Result<Color, StyleValueError> {
        match self {
            StyleValue::Number(n)
                if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= u32::MAX as f64 =>
            {
                Ok(Color(*n as u32))
            }
            StyleValue::Text(t) => Color::parse_hex(t).ok_or(StyleValueError::BadColor { style }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "color",
            }),
        }
    }

    pub(crate) fn vec2(&self, style: StyleId) -> Result<Vec2, StyleValueError> {
        match self {
            StyleValue::Vec2(x, y) if x.is_finite() && y.is_finite() => Ok(Vec2 { x: *x, y: *y }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "vec2",
            }),
        }
    }
}

impl Color {
    /// Parse CSS hex notation: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`.
    pub fn parse_hex(text: &str) -> Option<Color> {
        let hex = text.strip_prefix('#')?;
        let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        let bytes = hex.as_bytes();
        let (r, g, b, a) = match bytes.len() {
            3 | 4 => {
                let mut v = [0u8; 4];
                for (i, c) in bytes.iter().enumerate() {
                    let d = digit(*c)?;
                    v[i] = d * 17;
                }
                (v[0], v[1], v[2], if bytes.len() == 4 { v[3] } else { 255 })
            }
            6 | 8 => {
                let mut v = [0u8; 4];
                for (i, pair) in bytes.chunks(2).enumerate() {
                    v[i] = digit(pair[0])? * 16 + digit(pair[1])?;
                }
                (v[0], v[1], v[2], if bytes.len() == 8 { v[3] } else { 255 })
            }
            _ => return None,
        };
        Some(Color::rgba(r, g, b, a))
    }
}

/// One style row's value, read by id — the read-side twin of [`StyleValue`]
/// for consumers that lower rows generically (a web host emitting CSS).
#[derive(Debug, Clone, PartialEq)]
pub enum RowValue<'a> {
    /// A dimension.
    Dimension(Dimension),
    /// A number (`f32`, `u8`, `u16`, `u32`, `i32` rows).
    Number(f64),
    /// A color.
    Color(Color),
    /// Two numbers.
    Vec2(Vec2),
    /// Two colors.
    Color2([Color; 2]),
    /// An enum value, by its declared (CSS) name.
    Enum(&'static str),
    /// Grid tracks.
    Tracks(&'a GridTracks),
    /// A grid placement.
    Placement(GridPlacement),
    /// The `transition` row.
    Transitions(&'a Transitions),
}

/// Two floats.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    /// Horizontal.
    pub x: f32,
    /// Vertical.
    pub y: f32,
}

/// One grid track under the closed portable grammar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    /// A flexible fraction of the free space.
    Fr(f32),
    /// Layout points.
    Points(f32),
    /// Percent of the grid container, authored as 0–100.
    Percent(f32),
    /// Auto-sized.
    Auto,
    /// Min-content.
    MinContent,
    /// Max-content.
    MaxContent,
}

/// A grid template: an ordered track list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridTracks(pub Vec<GridTrack>);

impl GridTracks {
    /// Whether every track size is a finite number.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|t| match *t {
            GridTrack::Fr(v) | GridTrack::Points(v) | GridTrack::Percent(v) => v.is_finite(),
            GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => true,
        })
    }

    /// `count` equal `1fr` tracks.
    pub fn equal(count: usize) -> Self {
        GridTracks(vec![GridTrack::Fr(1.0); count.min(MAX_GRID_TRACKS)])
    }
}

/// One edge of a grid placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridLine {
    /// Auto-placed.
    #[default]
    Auto,
    /// A 1-based line index (negative counts from the end).
    Line(i16),
    /// Span this many tracks.
    Span(u16),
}

/// An item's placement on one grid axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPlacement {
    /// Start edge.
    pub start: GridLine,
    /// End edge.
    pub end: GridLine,
}

fn track(t: GridTrack) -> TrackSizingFunction {
    match t {
        GridTrack::Fr(v) => fr(v),
        GridTrack::Points(v) => length(v),
        GridTrack::Percent(v) => percent(v / 100.0),
        GridTrack::Auto => auto(),
        GridTrack::MinContent => min_content(),
        GridTrack::MaxContent => max_content(),
    }
}

fn grid_line(l: GridLine) -> taffy::style::GridPlacement {
    match l {
        GridLine::Auto => taffy::style::GridPlacement::Auto,
        GridLine::Line(i) => line(i),
        GridLine::Span(n) => span(n),
    }
}

fn flex_direction(v: FlexDirection) -> taffy::style::FlexDirection {
    match v {
        FlexDirection::Row => taffy::style::FlexDirection::Row,
        FlexDirection::Column => taffy::style::FlexDirection::Column,
        FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
        FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    }
}

fn flex_wrap(v: FlexWrap) -> taffy::style::FlexWrap {
    match v {
        FlexWrap::Nowrap => taffy::style::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
    }
}

fn justify_content(v: JustifyContent) -> taffy::style::JustifyContent {
    match v {
        JustifyContent::FlexStart => taffy::style::JustifyContent::FlexStart,
        JustifyContent::FlexEnd => taffy::style::JustifyContent::FlexEnd,
        JustifyContent::Center => taffy::style::JustifyContent::Center,
        JustifyContent::SpaceBetween => taffy::style::JustifyContent::SpaceBetween,
        JustifyContent::SpaceAround => taffy::style::JustifyContent::SpaceAround,
        JustifyContent::SpaceEvenly => taffy::style::JustifyContent::SpaceEvenly,
    }
}

fn align_items(v: AlignItems) -> taffy::style::AlignItems {
    match v {
        AlignItems::FlexStart => taffy::style::AlignItems::FlexStart,
        AlignItems::FlexEnd => taffy::style::AlignItems::FlexEnd,
        AlignItems::Center => taffy::style::AlignItems::Center,
        AlignItems::Baseline => taffy::style::AlignItems::Baseline,
        AlignItems::Stretch => taffy::style::AlignItems::Stretch,
    }
}

fn align_self(v: AlignSelf) -> Option<taffy::style::AlignSelf> {
    match v {
        AlignSelf::Auto => None,
        AlignSelf::FlexStart => Some(taffy::style::AlignSelf::FlexStart),
        AlignSelf::FlexEnd => Some(taffy::style::AlignSelf::FlexEnd),
        AlignSelf::Center => Some(taffy::style::AlignSelf::Center),
        AlignSelf::Baseline => Some(taffy::style::AlignSelf::Baseline),
        AlignSelf::Stretch => Some(taffy::style::AlignSelf::Stretch),
    }
}

fn align_content(v: AlignContent) -> taffy::style::AlignContent {
    match v {
        AlignContent::FlexStart => taffy::style::AlignContent::FlexStart,
        AlignContent::FlexEnd => taffy::style::AlignContent::FlexEnd,
        AlignContent::Center => taffy::style::AlignContent::Center,
        AlignContent::Stretch => taffy::style::AlignContent::Stretch,
        AlignContent::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
        AlignContent::SpaceAround => taffy::style::AlignContent::SpaceAround,
        AlignContent::SpaceEvenly => taffy::style::AlignContent::SpaceEvenly,
    }
}

fn overflow(v: Overflow) -> taffy::style::Overflow {
    match v {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Hidden => taffy::style::Overflow::Hidden,
        Overflow::Scroll => taffy::style::Overflow::Scroll,
    }
}

fn grid_auto_flow(v: GridAutoFlow) -> taffy::style::GridAutoFlow {
    match v {
        GridAutoFlow::Row => taffy::style::GridAutoFlow::Row,
        GridAutoFlow::Column => taffy::style::GridAutoFlow::Column,
        GridAutoFlow::RowDense => taffy::style::GridAutoFlow::RowDense,
        GridAutoFlow::ColumnDense => taffy::style::GridAutoFlow::ColumnDense,
    }
}

impl StyleProps {
    /// Lower to engine style. `node_type` supplies the per-tag defaults the
    /// table does not carry: scroll containers scroll on their block axis
    /// unless the producer set `overflow_y`.
    #[allow(clippy::field_reassign_with_default)]
    pub fn to_taffy(&self, node_type: NodeType) -> taffy::style::Style {
        let mut s = taffy::style::Style::default();
        s.display = match self.display {
            Display::Block => taffy::style::Display::Block,
            Display::Flex => taffy::style::Display::Flex,
            Display::None => taffy::style::Display::None,
            Display::Grid => taffy::style::Display::Grid,
        };
        s.box_sizing = match self.box_sizing {
            BoxSizing::ContentBox => taffy::style::BoxSizing::ContentBox,
            BoxSizing::BorderBox => taffy::style::BoxSizing::BorderBox,
        };
        s.position = match self.position_type {
            PositionType::Relative => taffy::style::Position::Relative,
            PositionType::Absolute => taffy::style::Position::Absolute,
        };
        let overflow_y = if !self.mask.has(StyleId::OverflowY) && node_type.scrolls_by_default() {
            taffy::style::Overflow::Scroll
        } else {
            overflow(self.overflow_y)
        };
        // CSS Overflow §3: when one axis is not `visible`, a `visible` other
        // axis computes to `auto`. The schema has no `auto`; `scroll` is its
        // stand-in (Taffy's sizing is the same). Symmetric, either axis.
        let mut overflow_x = overflow(self.overflow_x);
        let mut overflow_y = overflow_y;
        use taffy::style::Overflow as O;
        if overflow_x == O::Visible && overflow_y != O::Visible {
            overflow_x = O::Scroll;
        } else if overflow_y == O::Visible && overflow_x != O::Visible {
            overflow_y = O::Scroll;
        }
        s.overflow = taffy::geometry::Point {
            x: overflow_x,
            y: overflow_y,
        };
        s.scrollbar_width = 0.0;

        s.size = taffy::geometry::Size {
            width: self.width.to_taffy(),
            height: self.height.to_taffy(),
        };
        s.min_size = taffy::geometry::Size {
            width: self.min_width.to_taffy(),
            height: self.min_height.to_taffy(),
        };
        s.max_size = taffy::geometry::Size {
            width: self.max_width.to_taffy(),
            height: self.max_height.to_taffy(),
        };
        s.aspect_ratio = if self.aspect_ratio > 0.0 && self.aspect_ratio.is_finite() {
            Some(self.aspect_ratio)
        } else {
            None
        };

        s.inset = taffy::geometry::Rect {
            top: self.top.to_lpa(),
            right: self.right.to_lpa(),
            bottom: self.bottom.to_lpa(),
            left: self.left.to_lpa(),
        };
        s.margin = taffy::geometry::Rect {
            top: self.margin_top.to_lpa(),
            right: self.margin_right.to_lpa(),
            bottom: self.margin_bottom.to_lpa(),
            left: self.margin_left.to_lpa(),
        };
        s.padding = taffy::geometry::Rect {
            top: self.padding_top.to_lp(),
            right: self.padding_right.to_lp(),
            bottom: self.padding_bottom.to_lp(),
            left: self.padding_left.to_lp(),
        };
        s.border = taffy::geometry::Rect {
            top: length(self.border_width_top),
            right: length(self.border_width_right),
            bottom: length(self.border_width_bottom),
            left: length(self.border_width_left),
        };

        s.flex_direction = flex_direction(self.flex_direction);
        s.flex_wrap = flex_wrap(self.flex_wrap);
        s.flex_basis = self.flex_basis.to_taffy();
        s.flex_grow = self.flex_grow;
        s.flex_shrink = self.flex_shrink;
        s.justify_content = Some(justify_content(self.justify_content));
        s.align_items = Some(align_items(self.align_items));
        s.align_self = align_self(self.align_self);
        s.align_content = Some(align_content(self.align_content));
        s.justify_items = Some(align_items(self.justify_items));
        s.gap = taffy::geometry::Size {
            width: length(self.column_gap),
            height: length(self.row_gap),
        };

        s.grid_auto_flow = grid_auto_flow(self.grid_auto_flow);
        s.grid_template_columns = self
            .grid_template_columns
            .0
            .iter()
            .map(|t| track(*t).into())
            .collect();
        s.grid_template_rows = self
            .grid_template_rows
            .0
            .iter()
            .map(|t| track(*t).into())
            .collect();
        s.grid_column = taffy::geometry::Line {
            start: grid_line(self.grid_column.start),
            end: grid_line(self.grid_column.end),
        };
        s.grid_row = taffy::geometry::Line {
            start: grid_line(self.grid_row.start),
            end: grid_line(self.grid_row.end),
        };
        s
    }
}

/// The engine style for a live slot.
pub fn taffy_style(arena: &NodeArena, slot: u32) -> taffy::style::Style {
    let mut s = arena.style(slot).to_taffy(arena.node_type(slot));
    // A root with `width: auto` fills what it is offered, as a `<div>` fills
    // the body: CSS's block rule, which Taffy does not apply to a root.
    // Height stays auto — as tall as its content, the page a viewport scrolls.
    // A replaced element keeps its intrinsic ratio unless a row sets one:
    // CSS sizes an `<img>` with one dimension given from the other by ratio.
    if arena.node_type(slot) == NodeType::Image && !arena.style(slot).mask.has(StyleId::AspectRatio)
    {
        if let Some((w, h)) = arena.intrinsic(slot) {
            if w > 0.0 && h > 0.0 {
                s.aspect_ratio = Some(w / h);
            }
        }
    }
    if arena.is_root(slot)
        && s.size.width.is_auto()
        && s.position != taffy::style::Position::Absolute
    {
        // CSS block `width: auto`: the border box fills the containing block
        // (padding and border inside it), which is `100%` under border-box
        // sizing. Margins on a root are not subtracted (LLP 1010 §1).
        s.size.width = taffy::style::Dimension::percent(1.0);
        s.box_sizing = taffy::style::BoxSizing::BorderBox;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_converts_exactly_once() {
        let d = Dimension::Percent(50.0);
        assert_eq!(d.to_taffy(), percent(0.5_f32));
        assert_eq!(d.to_lpa(), percent(0.5_f32));
        assert_eq!(d.to_lp(), percent(0.5_f32));
    }

    #[test]
    fn defaults_are_the_css_defaults() {
        let s = StyleProps::default().to_taffy(NodeType::View);
        assert_eq!(s.display, taffy::style::Display::Block);
        assert_eq!(s.box_sizing, taffy::style::BoxSizing::ContentBox);
        assert_eq!(s.flex_direction, taffy::style::FlexDirection::Row);
        assert_eq!(s.flex_shrink, 1.0);
        assert_eq!(s.align_items, Some(taffy::style::AlignItems::Stretch));
        assert_eq!(s.position, taffy::style::Position::Relative);
        assert_eq!(s.overflow.y, taffy::style::Overflow::Visible);
    }

    #[test]
    fn scroll_containers_scroll_on_the_block_axis_by_default() {
        let s = StyleProps::default().to_taffy(NodeType::ScrollView);
        assert_eq!(s.overflow.y, taffy::style::Overflow::Scroll);
        // CSS Overflow §3: a `visible` axis beside a non-visible one computes
        // to `auto` — `scroll` here — so a scroll container clips both axes.
        assert_eq!(s.overflow.x, taffy::style::Overflow::Scroll);
        let plain = StyleProps::default().to_taffy(NodeType::View);
        assert_eq!(plain.overflow.x, taffy::style::Overflow::Visible);
        // Symmetric: a hidden x makes an unset y scrollable, not hidden.
        let mut hidden_x = StyleProps::default();
        hidden_x.overflow_x = Overflow::Hidden;
        hidden_x.mask.set(StyleId::OverflowX);
        let t = hidden_x.to_taffy(NodeType::View);
        assert_eq!(
            (t.overflow.x, t.overflow.y),
            (
                taffy::style::Overflow::Hidden,
                taffy::style::Overflow::Scroll
            )
        );
        let mut explicit = StyleProps::default();
        explicit.overflow_y = Overflow::Hidden;
        explicit.mask.set(StyleId::OverflowY);
        assert_eq!(
            explicit.to_taffy(NodeType::ScrollView).overflow.y,
            taffy::style::Overflow::Hidden
        );
    }

    #[test]
    fn color_channels() {
        let c = Color::rgba(0x12, 0x34, 0x56, 0x78);
        assert_eq!(c.0, 0x1234_5678);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x12, 0x34, 0x56, 0x78));
    }

    #[test]
    fn grid_tracks_lower_to_engine_tracks() {
        let mut p = StyleProps::default();
        p.display = Display::Grid;
        p.grid_template_columns = GridTracks(vec![
            GridTrack::Fr(1.0),
            GridTrack::Points(40.0),
            GridTrack::Auto,
        ]);
        p.grid_row = GridPlacement {
            start: GridLine::Line(1),
            end: GridLine::Span(2),
        };
        let s = p.to_taffy(NodeType::View);
        assert_eq!(s.grid_template_columns.len(), 3);
        assert_eq!(s.grid_row.start, line(1));
        assert_eq!(s.grid_row.end, span(2));
    }
}
