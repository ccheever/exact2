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
    AlignContent, AlignItems, AlignSelf, BorderStyle, BoxSizing, Direction, Display, FlexDirection,
    FlexWrap, GridAutoFlow, JustifyContent, NodeType, Overflow, PositionType, StyleId, StyleMask,
    StyleProps,
};

/// Largest grid track list the closed grammar carries.
pub const MAX_GRID_TRACKS: usize = 32;

/// An edge of the viewport: which safe-area inset an `env()` length names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Edge {
    /// `safe-area-inset-top`.
    Top = 0,
    /// `safe-area-inset-right`.
    Right = 1,
    /// `safe-area-inset-bottom`.
    Bottom = 2,
    /// `safe-area-inset-left`.
    Left = 3,
}

impl Edge {
    /// Every edge, in wire order.
    pub const ALL: [Edge; 4] = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];

    /// The CSS name: `top`, `right`, `bottom`, `left`.
    pub fn name(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Right => "right",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
        }
    }

    /// The edge by CSS name.
    pub fn from_name(name: &str) -> Option<Edge> {
        Edge::ALL.iter().copied().find(|e| e.name() == name)
    }

    /// The edge by wire index (0–3).
    pub fn from_index(i: u8) -> Option<Edge> {
        Edge::ALL.get(i as usize).copied()
    }
}

/// The page's environment: what CSS's `env(safe-area-inset-*)` resolve to,
/// in points, set by the host with the viewport (a phone's status bar and
/// home indicator under `viewport-fit=cover`; zero everywhere else, as a
/// browser reports them for a page without it).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Env {
    /// `safe-area-inset-top`.
    pub top: f32,
    /// `safe-area-inset-right`.
    pub right: f32,
    /// `safe-area-inset-bottom`.
    pub bottom: f32,
    /// `safe-area-inset-left`.
    pub left: f32,
}

impl Env {
    /// The four insets, top right bottom left.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Env {
        Env {
            top,
            right,
            bottom,
            left,
        }
    }

    /// The inset at an edge.
    pub fn inset(&self, edge: Edge) -> f32 {
        match edge {
            Edge::Top => self.top,
            Edge::Right => self.right,
            Edge::Bottom => self.bottom,
            Edge::Left => self.left,
        }
    }

    /// Whether every inset is a finite number.
    pub fn is_finite(&self) -> bool {
        Edge::ALL.iter().all(|e| self.inset(*e).is_finite())
    }
}

/// A length: automatic, absolute points, a percentage of the parent (0–100),
/// or a safe-area inset of the viewport plus points — CSS's
/// `env(safe-area-inset-<edge>)` and `calc(env(safe-area-inset-<edge>) + <n>px)`,
/// resolved against the kernel's [`Env`] at layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dimension {
    /// Let the engine decide.
    #[default]
    Auto,
    /// Layout points.
    Points(f32),
    /// Percent of the containing block, authored as 0–100.
    Percent(f32),
    /// The viewport's safe-area inset at an edge, plus points (zero for a
    /// bare `env()`).
    Env(Edge, f32),
}

impl Dimension {
    /// Whether the value is a finite number (or `Auto`).
    pub fn is_finite(self) -> bool {
        match self {
            Dimension::Auto => true,
            Dimension::Points(v) | Dimension::Percent(v) | Dimension::Env(_, v) => v.is_finite(),
        }
    }

    /// An `env()` length by CSS's grammar, or `None` when the text is not one:
    /// `env(safe-area-inset-<edge>)`, or `calc(env(safe-area-inset-<edge>) + <n>px)`
    /// (`-` as well). No fallback argument: the host always defines the four
    /// insets, so CSS would never use one.
    pub fn parse_env(text: &str) -> Option<Dimension> {
        let t = text.trim();
        let edge_of = |inner: &str| -> Option<Edge> {
            let inner = inner.trim();
            let name = inner
                .strip_prefix("env(")?
                .strip_suffix(')')?
                .trim()
                .strip_prefix("safe-area-inset-")?;
            Edge::from_name(name)
        };
        if let Some(edge) = edge_of(t) {
            return Some(Dimension::Env(edge, 0.0));
        }
        let body = t.strip_prefix("calc(")?.strip_suffix(')')?.trim();
        // `env(...) ± <n>px`: the operator is the first `+`/`-` after the
        // closing paren of the `env(...)` term.
        let close = body.find(')')?;
        let (term, rest) = body.split_at(close + 1);
        let edge = edge_of(term)?;
        let rest = rest.trim();
        let (sign, number) = match rest.as_bytes().first() {
            Some(b'+') => (1.0, &rest[1..]),
            Some(b'-') => (-1.0, &rest[1..]),
            _ => return None,
        };
        let number = number.trim().strip_suffix("px")?.trim();
        let plus = exact_num::parse_f32(number).ok()?;
        plus.is_finite()
            .then_some(Dimension::Env(edge, sign * plus))
    }

    /// The points an `env()` length resolves to under `env`; any other
    /// dimension unchanged.
    pub fn resolve(self, env: &Env) -> Dimension {
        match self {
            Dimension::Env(edge, plus) => Dimension::Points(env.inset(edge) + plus),
            other => other,
        }
    }

    fn to_taffy(self, env: &Env) -> taffy::style::Dimension {
        match self.resolve(env) {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
            Dimension::Env(..) => unreachable!("resolved above"),
        }
    }

    fn to_lpa(self, env: &Env) -> taffy::style::LengthPercentageAuto {
        match self.resolve(env) {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
            Dimension::Env(..) => unreachable!("resolved above"),
        }
    }

    /// Rows that do not admit `auto` (padding) lower it to zero; the decoder
    /// already refuses `auto` there, so this arm is unreachable from the wire.
    fn to_lp(self, env: &Env) -> taffy::style::LengthPercentage {
        match self.resolve(env) {
            Dimension::Auto => length(0.0_f32),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
            Dimension::Env(..) => unreachable!("resolved above"),
        }
    }

    /// Whether [`Self::to_lp`] gives zero as the engine compares lengths, by
    /// bits: `auto`, or `+0` points or percent — never `-0`.
    fn lp_is_zero(self, env: &Env) -> bool {
        match self.resolve(env) {
            Dimension::Auto => true,
            Dimension::Points(v) => v.to_bits() == 0,
            Dimension::Percent(v) => (v / 100.0).to_bits() == 0,
            Dimension::Env(..) => unreachable!("resolved above"),
        }
    }
}

/// CSS line-height, preserved through inheritance and resolved per receiving font.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// The receiving font's natural metrics.
    Normal,
    /// A unitless multiple of the receiving font size.
    Number(f32),
    /// An absolute length in logical pixels.
    Length(f32),
}

impl LineHeight {
    /// Resolve after inheritance; `None` means normal, including distinct `Some(0)`.
    pub fn resolve(self, font_size: f32) -> Option<f32> {
        match self {
            Self::Normal => None,
            Self::Number(n) => Some(n * font_size),
            Self::Length(n) => Some(n),
        }
    }
    /// All stored numbers must be finite.
    pub fn is_finite(self) -> bool {
        match self {
            Self::Normal => true,
            Self::Number(n) | Self::Length(n) => n.is_finite(),
        }
    }
    /// CSS refuses negative line heights.
    pub fn is_valid(self) -> bool {
        self.is_finite()
            && match self {
                Self::Normal => true,
                Self::Number(n) | Self::Length(n) => n >= 0.0,
            }
    }
    /// The CSS spelling. Percentages and font-relative lengths are unsupported.
    pub fn css(self) -> String {
        match self {
            Self::Normal => "normal".into(),
            Self::Number(n) => exact_num::Shortest32(n).to_string(),
            Self::Length(n) => format!("{}px", exact_num::Shortest32(n)),
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
    pub(crate) fn line_height(&self, style: StyleId) -> Result<LineHeight, StyleValueError> {
        let value = match self {
            Self::Number(n) if *n >= 0.0 => Some(LineHeight::Number(*n as f32)),
            Self::Text(t) if t.trim().eq_ignore_ascii_case("normal") => Some(LineHeight::Normal),
            Self::Text(t) => t
                .trim()
                .strip_suffix("px")
                .and_then(|n| exact_num::parse_f64(n).ok())
                .filter(|n| *n >= 0.0)
                .map(|n| LineHeight::Length(n as f32)),
            _ => None,
        };
        value
            .filter(|v| v.is_valid())
            .ok_or(StyleValueError::WrongKind {
                style,
                expected:
                    "nonnegative finite number, px length, or normal (percent/em unsupported)",
            })
    }

    pub(crate) fn f32(&self, style: StyleId) -> Result<f32, StyleValueError> {
        // @ref LLP 1043.000 §3 D1 — shape-margin is a nonnegative CSS length.
        if style == StyleId::ShapeMargin {
            let value = match self {
                StyleValue::Number(n) => Some(*n as f32),
                StyleValue::Text(s) => s
                    .trim()
                    .strip_suffix("px")
                    .and_then(|s| exact_num::parse_f32(s).ok()),
                _ => None,
            };
            return value.filter(|n| n.is_finite() && *n >= 0.0).ok_or(StyleValueError::WrongKind {
                style, expected: "nonnegative finite length in points/px (percentage shape-margin is not implemented in exact2 v1)",
            });
        }
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
            StyleValue::Text(t) if Dimension::parse_env(t).is_some() => {
                Ok(Dimension::parse_env(t).unwrap_or_default())
            }
            StyleValue::Text(t) => {
                parse_pixel_length(t.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']))
                    .map(Dimension::Points)
                    .ok_or(StyleValueError::WrongKind {
                        style,
                        expected: "number, px length, percent, auto, or env(safe-area-inset-*)",
                    })
            }
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "number, percent, auto, or env(safe-area-inset-*)",
            }),
        }
    }

    /// A colour as a row holds it. `light-dark(a, b)` is the one text a
    /// colour row takes beyond a hex, exactly as `env()` is the one text a
    /// dimension row takes (LLP 1034 D1).
    pub(crate) fn color_value(&self, style: StyleId) -> Result<ColorValue, StyleValueError> {
        if let StyleValue::Text(t) = self {
            if let Some(pair) = ColorValue::parse_light_dark(t) {
                return Ok(pair);
            }
        }
        self.color(style).map(ColorValue::Fixed)
    }

    /// A colour keyword remains distinct from transparent paint.
    pub(crate) fn keyword_color(
        &self,
        style: StyleId,
        keyword: &str,
    ) -> Result<Option<ColorValue>, StyleValueError> {
        match self {
            StyleValue::Auto if keyword == "auto" => Ok(None),
            StyleValue::Text(t) if t.eq_ignore_ascii_case(keyword) => Ok(None),
            _ => self.color_value(style).map(Some),
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
            StyleValue::Text(t) if style == StyleId::Translate => {
                parse_translate(t).ok_or(StyleValueError::WrongKind {
                    style,
                    expected: "one or two pixel lengths (unitless zero allowed)",
                })
            }
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "vec2",
            }),
        }
    }
}

// CSS pixel length or unitless zero, shared by dimensions and translation.
fn parse_pixel_length(token: &str) -> Option<f32> {
    let pixels = token
        .get(token.len().saturating_sub(2)..)
        .is_some_and(|unit| unit.eq_ignore_ascii_case("px"));
    let number = if pixels {
        &token[..token.len() - 2]
    } else {
        token
    };
    // Rust floats accept spellings outside CSS number tokens. Check the
    // decimal/exponent grammar before the range-preserving conversion.
    let unsigned = number.strip_prefix(['+', '-']).unwrap_or(number);
    let (mantissa, exponent) = unsigned.find(['e', 'E']).map_or((unsigned, None), |i| {
        (&unsigned[..i], Some(&unsigned[i + 1..]))
    });
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let valid_mantissa = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole.is_empty() || digits(whole)) && digits(fraction),
        None => digits(mantissa),
    };
    if !valid_mantissa || exponent.is_some_and(|e| !digits(e.strip_prefix(['+', '-']).unwrap_or(e)))
    {
        return None;
    }
    let value = exact_num::parse_f64(number).ok()?;
    if !value.is_finite()
        || value.abs() > f32::MAX as f64
        || (!pixels && mantissa.bytes().any(|b| b.is_ascii_digit() && b != b'0'))
    {
        return None;
    }
    Some(value as f32)
}

// Fixed 2D CSS subset for Contract text authoring. `none` is deliberately not
// zero: CSS gives those different containing-block/stacking semantics. Percent,
// calc and a third axis need a richer row, not a lossy conversion to this Vec2.
fn parse_translate(text: &str) -> Option<Vec2> {
    // CSS whitespace is TAB, LF, FF, CR and SPACE; ASCII VT is not included.
    let mut parts = text
        .split(['\t', '\n', '\u{c}', '\r', ' '])
        .filter(|s| !s.is_empty());
    let x = parse_pixel_length(parts.next()?)?;
    let y = match parts.next() {
        Some(s) => parse_pixel_length(s)?,
        None => 0.0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(Vec2 { x, y })
}

/// A colour as authored, which may not be a single colour yet.
///
/// The deferred-value shape `Dimension` already has: the row stores what was
/// written and something else resolves it later. A length resolves in the
/// kernel because layout depends on it; a colour resolves in the **host**,
/// because nothing about layout depends on a colour and because the browser
/// is the one that should resolve `light-dark()` — handed the function it
/// does so per element against the inherited `color-scheme`, with no work of
/// ours. @ref LLP 1034 D1/D2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    /// One colour, whatever the appearance.
    Fixed(Color),
    /// CSS `light-dark(a, b)`: the first under a light scheme, the second
    /// under a dark one.
    LightDark(Color, Color),
}

impl Default for ColorValue {
    fn default() -> ColorValue {
        ColorValue::Fixed(Color(0))
    }
}

impl ColorValue {
    /// The colour under an appearance. A host that paints calls this; the web
    /// host does not, because it hands the pair to the browser.
    pub const fn resolve(self, dark: bool) -> Color {
        match self {
            ColorValue::Fixed(c) => c,
            ColorValue::LightDark(light, night) => {
                if dark {
                    night
                } else {
                    light
                }
            }
        }
    }

    /// Whether this is a pair — what a host asks before deciding whether an
    /// appearance change is anything to it.
    pub const fn is_scheme_aware(self) -> bool {
        matches!(self, ColorValue::LightDark(..))
    }

    /// `light-dark(<color>, <color>)`, CSS's own spelling and nothing else.
    /// Whitespace is free; anything that is not two parseable colours is not
    /// this function, and falls through to the plain colour parse.
    pub fn parse_light_dark(text: &str) -> Option<ColorValue> {
        let inner = text.trim().strip_prefix("light-dark(")?.strip_suffix(')')?;
        let (light, night) = inner.split_once(',')?;
        Some(ColorValue::LightDark(
            Color::parse_hex(light.trim())?,
            Color::parse_hex(night.trim())?,
        ))
    }
}

impl From<Color> for ColorValue {
    fn from(c: Color) -> ColorValue {
        ColorValue::Fixed(c)
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
    /// CSS line-height, retaining its kind.
    LineHeight(LineHeight),
    /// A validated CSS clipping path.
    ClipPath(&'a crate::clip::ClipPath),
    /// CSS shape-outside, resolved after layout (LLP 1043.000 D1).
    ShapeOutside(&'a exact_textflow::ShapeOutside),
    /// A dimension.
    Dimension(Dimension),
    /// A number (`f32`, `u8`, `u16`, `u32`, `i32` rows).
    Number(f64),
    /// A color.
    Color(Color),
    /// A colour a row holds: fixed, or a `light-dark()` pair a host resolves
    /// (LLP 1034 D1).
    ColorValue(ColorValue),
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

impl GridLine {
    pub(crate) fn is_valid(self) -> bool {
        !matches!(self, GridLine::Span(0))
    }
}

/// An item's placement on one grid axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPlacement {
    /// Start edge.
    pub start: GridLine,
    /// End edge.
    pub end: GridLine,
}

impl GridPlacement {
    pub(crate) fn is_valid(self) -> bool {
        self.start.is_valid() && self.end.is_valid()
    }
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

// CSS `normal` (the initial value of all four rows below) lowers to `None`,
// which is Taffy's `normal`: each algorithm resolves it for its display type
// as CSS Box Alignment §5.1/§6.1 does. Flex: `justify-content` is
// `flex-start`, `align-content` and `align-items` are `stretch`. Grid: auto
// tracks stretch into the free space on both axes, and an item stretches
// unless it has a definite size or an aspect ratio on that axis (then it
// starts). Block: `align-content` is `start` without establishing an
// independent formatting context, which every other value does.

fn justify_content(v: JustifyContent) -> Option<taffy::style::JustifyContent> {
    Some(match v {
        JustifyContent::Normal => return None,
        JustifyContent::FlexStart => taffy::style::JustifyContent::FLEX_START,
        JustifyContent::FlexEnd => taffy::style::JustifyContent::FLEX_END,
        JustifyContent::Center => taffy::style::JustifyContent::CENTER,
        JustifyContent::SpaceBetween => taffy::style::JustifyContent::SPACE_BETWEEN,
        JustifyContent::SpaceAround => taffy::style::JustifyContent::SPACE_AROUND,
        JustifyContent::SpaceEvenly => taffy::style::JustifyContent::SPACE_EVENLY,
    })
}

fn align_items(v: AlignItems) -> Option<taffy::style::AlignItems> {
    Some(match v {
        AlignItems::Normal => return None,
        AlignItems::FlexStart => taffy::style::AlignItems::FLEX_START,
        AlignItems::FlexEnd => taffy::style::AlignItems::FLEX_END,
        AlignItems::Center => taffy::style::AlignItems::CENTER,
        AlignItems::Baseline => taffy::style::AlignItems::BASELINE,
        AlignItems::Stretch => taffy::style::AlignItems::STRETCH,
    })
}

fn align_self(v: AlignSelf) -> Option<taffy::style::AlignSelf> {
    match v {
        AlignSelf::Auto => None,
        AlignSelf::FlexStart => Some(taffy::style::AlignSelf::FLEX_START),
        AlignSelf::FlexEnd => Some(taffy::style::AlignSelf::FLEX_END),
        AlignSelf::Center => Some(taffy::style::AlignSelf::CENTER),
        AlignSelf::Baseline => Some(taffy::style::AlignSelf::BASELINE),
        AlignSelf::Stretch => Some(taffy::style::AlignSelf::STRETCH),
    }
}

fn align_content(v: AlignContent) -> Option<taffy::style::AlignContent> {
    Some(match v {
        AlignContent::Normal => return None,
        AlignContent::FlexStart => taffy::style::AlignContent::FLEX_START,
        AlignContent::FlexEnd => taffy::style::AlignContent::FLEX_END,
        AlignContent::Center => taffy::style::AlignContent::CENTER,
        AlignContent::Stretch => taffy::style::AlignContent::STRETCH,
        AlignContent::SpaceBetween => taffy::style::AlignContent::SPACE_BETWEEN,
        AlignContent::SpaceAround => taffy::style::AlignContent::SPACE_AROUND,
        AlignContent::SpaceEvenly => taffy::style::AlignContent::SPACE_EVENLY,
    })
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
    /// CSS effective border widths: none and hidden occupy no border area.
    pub fn border_widths(&self) -> [f32; 4] {
        [
            (self.border_style_top, self.border_width_top),
            (self.border_style_right, self.border_width_right),
            (self.border_style_bottom, self.border_width_bottom),
            (self.border_style_left, self.border_width_left),
        ]
        .map(|(style, width)| {
            if style == BorderStyle::Solid {
                width.max(0.0)
            } else {
                0.0
            }
        })
    }

    /// Whether every padding and border width reaches layout as zero, read
    /// without building the engine's style: a kernel that mirrors no engine
    /// tree checks content regions too (LLP 1047 §10).
    pub(crate) fn unpadded(&self, env: &Env) -> bool {
        [
            self.padding_top,
            self.padding_right,
            self.padding_bottom,
            self.padding_left,
        ]
        .into_iter()
        .all(|p| p.lp_is_zero(env))
            && self.border_widths().into_iter().all(|w| w.to_bits() == 0)
    }

    /// Border colours after resolving currentColor against this node's computed colour.
    pub fn border_colors(&self, current: ColorValue) -> [ColorValue; 4] {
        [
            self.border_color_top,
            self.border_color_right,
            self.border_color_bottom,
            self.border_color_left,
        ]
        .map(|color| color.unwrap_or(current))
    }

    /// Lower to engine style. `node_type` supplies the per-tag defaults the
    /// table does not carry: scroll containers scroll on their block axis
    /// unless the producer set `overflow_y`.
    /// `env` resolves the `env()` lengths (LLP 1001 §2: the kernel's
    /// environment, set by the host with the viewport).
    #[allow(clippy::field_reassign_with_default)]
    pub fn to_taffy(&self, node_type: NodeType, env: &Env) -> taffy::style::Style {
        let mut s = taffy::style::Style::default();
        s.display = match self.display {
            // A document's metadata takes no space (LLP 1048.003 D1).
            _ if node_type.is_metadata() => taffy::style::Display::None,
            Display::Block => taffy::style::Display::Block,
            Display::Flex => taffy::style::Display::Flex,
            Display::None => taffy::style::Display::None,
            Display::Grid => taffy::style::Display::Grid,
        };
        // Replaced pixels have ink overflow, never scrollable overflow (CSS
        // Overflow §2.1). This also identifies images for grid intrinsic sizing.
        s.direction = match self.direction {
            Direction::Ltr => taffy::style::Direction::Ltr,
            Direction::Rtl => taffy::style::Direction::Rtl,
        };
        s.item_is_replaced = node_type.is_replaced();
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
            width: self.width.to_taffy(env),
            height: self.height.to_taffy(env),
        };
        s.min_size = taffy::geometry::Size {
            width: self.min_width.to_lpa(env),
            height: self.min_height.to_lpa(env),
        };
        s.max_size = taffy::geometry::Size {
            width: self.max_width.to_lpa(env),
            height: self.max_height.to_lpa(env),
        };
        s.aspect_ratio = if self.aspect_ratio > 0.0 && self.aspect_ratio.is_finite() {
            Some(self.aspect_ratio)
        } else {
            None
        };

        s.inset = taffy::geometry::Rect {
            top: self.top.to_lpa(env),
            right: self.right.to_lpa(env),
            bottom: self.bottom.to_lpa(env),
            left: self.left.to_lpa(env),
        };
        s.margin = taffy::geometry::Rect {
            top: self.margin_top.to_lpa(env),
            right: self.margin_right.to_lpa(env),
            bottom: self.margin_bottom.to_lpa(env),
            left: self.margin_left.to_lpa(env),
        };
        s.padding = taffy::geometry::Rect {
            top: self.padding_top.to_lp(env),
            right: self.padding_right.to_lp(env),
            bottom: self.padding_bottom.to_lp(env),
            left: self.padding_left.to_lp(env),
        };
        let [top, right, bottom, left] = self.border_widths();
        s.border = taffy::geometry::Rect {
            top: length(top),
            right: length(right),
            bottom: length(bottom),
            left: length(left),
        };

        s.flex_direction = flex_direction(self.flex_direction);
        s.flex_wrap = flex_wrap(self.flex_wrap);
        s.flex_basis = self.flex_basis.to_taffy(env);
        s.flex_grow = self.flex_grow;
        s.flex_shrink = self.flex_shrink;
        s.justify_content = justify_content(self.justify_content);
        s.align_items = align_items(self.align_items);
        s.align_self = align_self(self.align_self);
        s.align_content = align_content(self.align_content);
        s.justify_items = align_items(self.justify_items);
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

/// Whether any set dimension row of `style` is an `env()` length — the
/// rows a change of the kernel's environment re-derives.
pub fn uses_env(style: &StyleProps) -> bool {
    style
        .mask
        .iter()
        .any(|id| matches!(style.get(id), RowValue::Dimension(Dimension::Env(..))))
}

/// The engine style for a live slot, its `env()` lengths resolved against
/// the arena's environment.
pub fn taffy_style(arena: &NodeArena, slot: u32) -> taffy::style::Style {
    let mut s = arena
        .style(slot)
        .to_taffy(arena.node_type(slot), arena.env());
    s.direction = match arena.computed_style(slot, StyleMask::INHERITED).direction {
        Direction::Ltr => taffy::style::Direction::Ltr,
        Direction::Rtl => taffy::style::Direction::Rtl,
    };
    // A root with `width: auto` fills what it is offered, as a `<div>` fills
    // the body: CSS's block rule, which Taffy does not apply to a root.
    // Height stays auto — as tall as its content, the page a viewport scrolls.
    // A replaced element keeps its intrinsic ratio unless a row sets one:
    // CSS sizes an `<img>` with one dimension given from the other by ratio.
    if arena.node_type(slot).is_replaced() && !arena.style(slot).mask.has(StyleId::AspectRatio) {
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
        let env = Env::default();
        assert_eq!(d.to_taffy(&env), percent(0.5_f32));
        assert_eq!(d.to_lpa(&env), percent(0.5_f32));
        assert_eq!(d.to_lp(&env), percent(0.5_f32));
    }

    #[test]
    fn env_lengths_parse_by_the_css_grammar_and_resolve_against_the_environment() {
        assert_eq!(
            Dimension::parse_env("env(safe-area-inset-top)"),
            Some(Dimension::Env(Edge::Top, 0.0))
        );
        assert_eq!(
            Dimension::parse_env(" env( safe-area-inset-left ) "),
            Some(Dimension::Env(Edge::Left, 0.0))
        );
        assert_eq!(
            Dimension::parse_env("calc(env(safe-area-inset-bottom) + 12px)"),
            Some(Dimension::Env(Edge::Bottom, 12.0))
        );
        assert_eq!(
            Dimension::parse_env("calc(env(safe-area-inset-right)-2.5px)"),
            Some(Dimension::Env(Edge::Right, -2.5))
        );
        for bad in [
            "env(safe-area-inset-middle)",
            "env(keyboard-inset-height)",
            "calc(env(safe-area-inset-top) + 12)",
            "calc(env(safe-area-inset-top) * 2)",
            "calc(12px + env(safe-area-inset-top))",
            "env(safe-area-inset-top, 0px)",
            "12px",
            "auto",
        ] {
            assert_eq!(Dimension::parse_env(bad), None, "{bad}");
        }
        let env = Env::new(62.0, 0.0, 34.0, 0.0);
        assert_eq!(Dimension::Env(Edge::Top, 0.0).to_lp(&env), length(62.0_f32));
        assert_eq!(
            Dimension::Env(Edge::Bottom, 12.0).to_lpa(&env),
            length(46.0_f32)
        );
        assert_eq!(
            Dimension::Env(Edge::Left, 8.0).to_taffy(&env),
            length(8.0_f32)
        );
        // Environment and explicit pixel lengths share dimension decoding.
        let mut s = StyleProps::default();
        s.set_dynamic(
            StyleId::PaddingTop,
            &StyleValue::Text("env(safe-area-inset-top)".into()),
        )
        .unwrap();
        assert_eq!(s.padding_top, Dimension::Env(Edge::Top, 0.0));
        assert!(uses_env(&s));
        s.set_dynamic(StyleId::PaddingTop, &StyleValue::Text("12px".into()))
            .unwrap();
        assert_eq!(s.padding_top, Dimension::Points(12.0));
        assert!(!uses_env(&s));
        assert!(!uses_env(&StyleProps::default()));
    }

    #[test]
    fn defaults_are_the_css_defaults() {
        let s = StyleProps::default().to_taffy(NodeType::View, &Env::default());
        assert_eq!(s.display, taffy::style::Display::Block);
        assert_eq!(s.box_sizing, taffy::style::BoxSizing::ContentBox);
        assert_eq!(s.flex_direction, taffy::style::FlexDirection::Row);
        assert_eq!(s.flex_shrink, 1.0);
        // `normal`, which each layout mode resolves (flex: stretch).
        assert_eq!(s.align_items, None);
        assert_eq!(s.justify_content, None);
        assert_eq!(s.align_content, None);
        assert_eq!(s.justify_items, None);
        assert_eq!(s.position, taffy::style::Position::Relative);
        assert_eq!(s.overflow.y, taffy::style::Overflow::Visible);
    }

    #[test]
    fn scroll_containers_scroll_on_the_block_axis_by_default() {
        let s = StyleProps::default().to_taffy(NodeType::ScrollView, &Env::default());
        assert_eq!(s.overflow.y, taffy::style::Overflow::Scroll);
        // CSS Overflow §3: a `visible` axis beside a non-visible one computes
        // to `auto` — `scroll` here — so a scroll container clips both axes.
        assert_eq!(s.overflow.x, taffy::style::Overflow::Scroll);
        let plain = StyleProps::default().to_taffy(NodeType::View, &Env::default());
        assert_eq!(plain.overflow.x, taffy::style::Overflow::Visible);
        // Symmetric: a hidden x makes an unset y scrollable, not hidden.
        let mut hidden_x = StyleProps::default();
        hidden_x.overflow_x = Overflow::Hidden;
        hidden_x.mask.set(StyleId::OverflowX);
        let t = hidden_x.to_taffy(NodeType::View, &Env::default());
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
            explicit
                .to_taffy(NodeType::ScrollView, &Env::default())
                .overflow
                .y,
            taffy::style::Overflow::Hidden
        );
    }

    #[test]
    fn a_colour_row_holds_a_light_dark_pair_and_the_host_resolves_it() {
        // CSS's spelling, and only it (LLP 1034 D1).
        let pair = ColorValue::parse_light_dark("light-dark(#ffffff, #000000)").unwrap();
        assert_eq!(
            pair,
            ColorValue::LightDark(
                Color::parse_hex("#ffffff").unwrap(),
                Color::parse_hex("#000000").unwrap()
            )
        );
        assert_eq!(pair.resolve(false), Color::parse_hex("#ffffff").unwrap());
        assert_eq!(pair.resolve(true), Color::parse_hex("#000000").unwrap());
        assert!(pair.is_scheme_aware());
        // Whitespace is free.
        assert_eq!(
            ColorValue::parse_light_dark("  light-dark( #fff , #000 )  "),
            Some(ColorValue::LightDark(
                Color::parse_hex("#fff").unwrap(),
                Color::parse_hex("#000").unwrap()
            ))
        );
        // Anything that is not two colours is not this function.
        for text in [
            "#ffffff",
            "light-dark(#fff)",
            "light-dark(#fff, nope)",
            "dark-light(#fff, #000)",
            "light-dark(#fff, #000",
        ] {
            assert_eq!(ColorValue::parse_light_dark(text), None, "{text}");
        }
        // A fixed colour resolves to itself under either appearance.
        let one = ColorValue::Fixed(Color::parse_hex("#abcdef").unwrap());
        assert_eq!(one.resolve(false), one.resolve(true));
        assert!(!one.is_scheme_aware());
    }

    #[test]
    fn a_colour_row_takes_a_pair_dynamically_as_a_dimension_takes_env() {
        let mut s = StyleProps::default();
        s.set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text("light-dark(#ffffff, #17181b)".into()),
        )
        .expect("a colour row takes CSS's own function");
        assert_eq!(
            s.background_color,
            ColorValue::LightDark(
                Color::parse_hex("#ffffff").unwrap(),
                Color::parse_hex("#17181b").unwrap()
            )
        );
        // And still takes a plain colour, which is the common case.
        s.set_dynamic(StyleId::TextColor, &StyleValue::Text("#112233".into()))
            .expect("a hex is still a colour");
        assert_eq!(
            s.text_color,
            ColorValue::Fixed(Color::parse_hex("#112233").unwrap())
        );
        // A text that is neither is refused, not silently taken.
        assert!(s
            .set_dynamic(
                StyleId::TextColor,
                &StyleValue::Text("light-dark(#fff)".into())
            )
            .is_err());
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
        let s = p.to_taffy(NodeType::View, &Env::default());
        assert_eq!(s.grid_template_columns.len(), 3);
        assert_eq!(s.grid_row.start, line(1));
        assert_eq!(s.grid_row.end, span(2));
    }

    /// Content regions check padding and border without the engine's style;
    /// the answer is the one the engine's style gives.
    #[test]
    fn unpadded_reads_what_the_engine_style_would() {
        let env = Env::new(62.0, 0.0, 0.0, 0.0);
        let zero = |x: taffy::style::LengthPercentage| x == length(0.) || x == percent(0.);
        let engine = |p: &StyleProps| {
            let s = p.to_taffy(NodeType::View, &env);
            [s.padding, s.border]
                .iter()
                .all(|r| [r.left, r.right, r.top, r.bottom].into_iter().all(zero))
        };
        for d in [
            Dimension::Auto,
            Dimension::Points(0.0),
            Dimension::Points(-0.0),
            Dimension::Points(1.0),
            Dimension::Percent(0.0),
            Dimension::Percent(-0.0),
            Dimension::Percent(1e-45),
            Dimension::Percent(3.0),
            Dimension::Env(Edge::Right, 0.0),
            Dimension::Env(Edge::Right, -0.0),
            Dimension::Env(Edge::Top, 0.0),
            Dimension::Env(Edge::Top, -62.0),
        ] {
            let mut p = StyleProps::default();
            p.padding_bottom = d;
            assert_eq!(p.unpadded(&env), engine(&p), "{d:?}");
        }
        for (style, width) in [
            (BorderStyle::Solid, 0.0),
            (BorderStyle::Solid, -0.0),
            (BorderStyle::Solid, -2.0),
            (BorderStyle::Solid, 1.0),
            (BorderStyle::None, 3.0),
        ] {
            let mut p = StyleProps::default();
            p.border_style_left = style;
            p.border_width_left = width;
            assert_eq!(p.unpadded(&env), engine(&p), "{style:?} {width}");
        }
    }
}
