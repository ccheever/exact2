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

mod backdrop;
pub use backdrop::link as link_backdrop_filter;
pub(crate) mod effects;
pub use crate::gradient::link as link_gradients;
pub use effects::link as link_effects;
pub mod relative;
mod shadow;
pub use shadow::BoxShadow;

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
/// a percentage plus points — CSS's `calc(<p>% + <n>px)`, which the engine
/// resolves against the percentage's basis — or a safe-area inset of the
/// viewport plus points — `env(safe-area-inset-<edge>)` and
/// `calc(env(safe-area-inset-<edge>) + <n>px)`, resolved against the
/// kernel's [`Env`] at layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dimension {
    /// Let the engine decide.
    #[default]
    Auto,
    /// Layout points.
    Points(f32),
    /// Percent of the containing block, authored as 0–100.
    Percent(f32),
    /// Percent of the containing block (0–100) plus points: `calc(50% - 89px)`
    /// is `Calc(50.0, -89.0)`.
    Calc(f32, f32),
    /// The viewport's safe-area inset at an edge, plus points (zero for a
    /// bare `env()`).
    Env(Edge, f32),
}

/// The `calc()` pairs the engine holds by handle: Taffy keeps one opaque
/// pointer per length, so a pair is interned here and named by its index,
/// shifted past the three tag bits Taffy reserves. Equal pairs share a
/// handle, so an unchanged style still compares equal by value.
static CALC_PAIRS: std::sync::Mutex<Vec<(f32, f32)>> = std::sync::Mutex::new(Vec::new());

fn calc_handle(percent: f32, points: f32) -> *const () {
    let mut pairs = CALC_PAIRS.lock().unwrap_or_else(|e| e.into_inner());
    let index = pairs
        .iter()
        .position(|(p, x)| p.to_bits() == percent.to_bits() && x.to_bits() == points.to_bits())
        .unwrap_or_else(|| {
            pairs.push((percent, points));
            pairs.len() - 1
        });
    ((index + 1) << 3) as *const ()
}

/// The points a `calc()` handle resolves to against `basis`, the size its
/// percentage is a fraction of — what [`crate::LayoutTree`] gives Taffy.
pub(crate) fn resolve_calc(handle: *const (), basis: f32) -> f32 {
    let index = (handle as usize >> 3) - 1;
    let pairs = CALC_PAIRS.lock().unwrap_or_else(|e| e.into_inner());
    let (percent, points) = pairs[index];
    basis * percent / 100.0 + points
}

impl Dimension {
    /// Whether the value is a finite number (or `Auto`).
    pub fn is_finite(self) -> bool {
        match self {
            Dimension::Auto => true,
            Dimension::Points(v) | Dimension::Percent(v) | Dimension::Env(_, v) => v.is_finite(),
            Dimension::Calc(p, v) => p.is_finite() && v.is_finite(),
        }
    }

    /// A `calc()` of one percentage and one pixel length by CSS's grammar —
    /// `calc(<p>% + <n>px)` or `calc(<n>px - <p>%)`, either order, the
    /// operator set off by whitespace as CSS requires — or `None` when the
    /// text is not one. Two of a kind, a bare number, or `*` and `/` are not.
    pub fn parse_calc(text: &str) -> Option<Dimension> {
        let body = text.trim().strip_prefix("calc(")?.strip_suffix(')')?;
        let b = body.as_bytes();
        let op = (1..b.len().saturating_sub(1)).find(|&i| {
            matches!(b[i], b'+' | b'-')
                && b[i - 1].is_ascii_whitespace()
                && b[i + 1].is_ascii_whitespace()
        })?;
        let sign = if b[op] == b'-' { -1.0 } else { 1.0 };
        let term = |t: &str| -> Option<(bool, f32)> {
            let t = t.trim();
            let px = t.len() > 2
                && t.get(t.len() - 2..)
                    .is_some_and(|u| u.eq_ignore_ascii_case("px"));
            match t.strip_suffix('%') {
                Some(p) => exact_num::parse_f32(p).ok().map(|v| (true, v)),
                None if px => parse_pixel_length(t).map(|v| (false, v)),
                None => None,
            }
        };
        let dim = match (term(&body[..op])?, term(&body[op + 1..])?) {
            ((true, percent), (false, points)) => Dimension::Calc(percent, sign * points),
            ((false, points), (true, percent)) => Dimension::Calc(sign * percent, points),
            _ => return None,
        };
        dim.is_finite().then_some(dim)
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
            Dimension::Calc(p, v) => taffy::style::Dimension::calc(calc_handle(p, v)),
            Dimension::Env(..) => unreachable!("resolved above"),
        }
    }

    fn to_lpa(self, env: &Env) -> taffy::style::LengthPercentageAuto {
        match self.resolve(env) {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
            Dimension::Calc(p, v) => taffy::style::LengthPercentageAuto::calc(calc_handle(p, v)),
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
            Dimension::Calc(p, v) => taffy::style::LengthPercentage::calc(calc_handle(p, v)),
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
            // A calc() is a handle the engine resolves, never its zero length.
            Dimension::Calc(..) => false,
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
            Self::Number(n) => exact_num::text!("{}", exact_num::Shortest32(n)),
            Self::Length(n) => exact_num::text!("{}px", exact_num::Shortest32(n)),
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
pub use exact_motion::{link_animations, Animations, Transitions};

/// An untyped style value from a producer that resolves rows by id — a plan
/// runner, a compiler lowering a literal, a TypeScript encoder. Exactly one
/// place turns it into a row: the generated `StyleProps::set_dynamic`.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleValue {
    /// A number: points for dimensions, the raw value for numeric rows, a
    /// packed `0xRRGGBBAA` for colors.
    Number(f64),
    /// Text: an enum value by name, or a color as `#rrggbb[aa]` or `rgb()`.
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
                "nonnegative finite number, px, rem or em length, or normal (percent unsupported)",
        })
    }

    /// A `box-shadow` given to one of its four rows: that row's part.
    /// @ref LLP 1064 D1
    fn box_shadow(&self, style: StyleId) -> Option<Result<BoxShadow, StyleValueError>> {
        match self {
            StyleValue::Text(t) => Some(
                BoxShadow::parse(t)
                    .map_err(|reason| StyleValueError::BadBoxShadow { style, reason }),
            ),
            _ => None,
        }
    }

    pub(crate) fn f32(&self, style: StyleId) -> Result<f32, StyleValueError> {
        if matches!(style, StyleId::ShadowRadius | StyleId::ShadowOpacity) {
            if let Some(shadow) = self.box_shadow(style) {
                let shadow = shadow?;
                return Ok(if style == StyleId::ShadowRadius {
                    shadow.blur
                } else {
                    shadow.opacity
                });
            }
        }
        // @ref LLP 1053.000 D1 — CSS `backdrop-filter`: `none` or one `blur()`.
        if style == StyleId::BackdropBlur {
            return match self {
                StyleValue::Text(t) => backdrop::parse_linked(t)
                    .map_err(|reason| StyleValueError::BadBackdropFilter { style, reason }),
                StyleValue::Number(n) if (*n as f32).is_finite() && *n >= 0.0 => Ok(*n as f32),
                _ => Err(StyleValueError::BadBackdropFilter {
                    style,
                    reason: "`backdrop-filter` is `none` or `blur(<length>)` (LLP 1053.000 D1)",
                }),
            };
        }
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
        // @ref LLP 1053 G3 — CSS refuses a negative flex factor.
        if matches!(style, StyleId::FlexGrow | StyleId::FlexShrink) {
            return match self {
                StyleValue::Number(n) if (*n as f32).is_finite() && *n >= 0.0 => Ok(*n as f32),
                _ => Err(StyleValueError::WrongKind {
                    style,
                    expected: "nonnegative number",
                }),
            };
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

    /// A CSS-valued row's text: text as given, a number (`aspect-ratio: 2`),
    /// a percentage (`transform-origin: 25%`) or `auto` as CSS spells it.
    pub(crate) fn css_text(&self, style: StyleId) -> Result<String, StyleValueError> {
        match self {
            StyleValue::Number(n) => Ok(exact_num::Shortest(*n).to_string()),
            StyleValue::Percent(p) => Ok(format!("{}%", exact_num::Shortest(*p))),
            StyleValue::Auto => Ok("auto".into()),
            _ => self.text(style).map(str::to_string),
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
            StyleValue::Text(t) => Dimension::parse_env(t)
                .or_else(|| Dimension::parse_calc(t))
                .or_else(|| {
                    parse_pixel_length(t.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']))
                        .map(Dimension::Points)
                })
                // CSS's absolute units (96 px to the inch) and a percentage
                // written as text (LLP 1055.000 D4).
                .or_else(|| absolute_length(t.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])))
                .ok_or(StyleValueError::WrongKind {
                    style,
                    expected: "number, px, rem or em length, percent, auto, calc(<percent> ± <px>), or env(safe-area-inset-*)",
                }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "number, percent, auto, calc(<percent> ± <px>), or env(safe-area-inset-*)",
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
            if style == StyleId::ShadowColor && Color::parse(t).is_none() {
                return self.box_shadow(style).expect("text").map(|s| s.color);
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
            StyleValue::Text(t) => Color::parse(t).ok_or(StyleValueError::BadColor { style }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "color",
            }),
        }
    }

    pub(crate) fn vec2(&self, style: StyleId) -> Result<Vec2, StyleValueError> {
        match self {
            StyleValue::Vec2(x, y) if x.is_finite() && y.is_finite() => Ok(Vec2 { x: *x, y: *y }),
            StyleValue::Text(_) if style == StyleId::ShadowOffset => {
                self.box_shadow(style).expect("text").map(|s| s.offset)
            }
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
/// A CSS length in an absolute unit (96 px to the inch), or a percentage
/// written as text, with the CSS number grammar `px` lengths use (LLP
/// 1055.000 D4). Unitless text stays refused.
fn absolute_length(token: &str) -> Option<Dimension> {
    let number = |n: &str| parse_pixel_length(&format!("{n}px"));
    if let Some(n) = token.strip_suffix('%') {
        return number(n).map(Dimension::Percent);
    }
    let split = token.len().checked_sub(2)?;
    let (n, unit) = (token.get(..split)?, token.get(split..)?);
    let scale = match unit.to_ascii_lowercase().as_str() {
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        _ => return None,
    };
    number(n).map(|v| Dimension::Points(v * scale))
}

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
        // The comma between the two colours, not one inside an `rgb()`.
        let mut depth = 0;
        let comma = inner.find(|c| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => return true,
                _ => {}
            }
            false
        })?;
        Some(ColorValue::LightDark(
            Color::parse(&inner[..comma])?,
            Color::parse(&inner[comma + 1..])?,
        ))
    }
}

impl From<Color> for ColorValue {
    fn from(c: Color) -> ColorValue {
        ColorValue::Fixed(c)
    }
}

impl Color {
    /// A CSS colour: hex or `rgb()` notation, or the keyword `transparent`
    /// (any ASCII case, as CSS keywords are: transparent black), whitespace
    /// around it free.
    pub fn parse(text: &str) -> Option<Color> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("transparent") {
            return Some(Color::rgba(0, 0, 0, 0));
        }
        Color::parse_hex(text).or_else(|| Color::parse_rgb(text))
    }

    /// CSS `rgb()` / `rgba()` (one function under two names, as in CSS
    /// Color 4): `rgb(255, 0, 0)`, `rgba(255, 0, 0, 0.5)`, `rgb(255 0 0 / 50%)`.
    /// A channel is a number 0–255 or a percentage; alpha is a number 0–1 or
    /// a percentage; out-of-range values clamp, as on the web.
    fn parse_rgb(text: &str) -> Option<Color> {
        let inner = text
            .strip_prefix("rgba(")
            .or_else(|| text.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = if inner.contains(',') {
            inner.split(',').map(str::trim).collect()
        } else {
            let (rgb, alpha) = match inner.split_once('/') {
                Some((rgb, alpha)) => (rgb, Some(alpha.trim())),
                None => (inner, None),
            };
            rgb.split_whitespace().chain(alpha).collect()
        };
        let ([r, g, b], alpha) = match parts[..] {
            [r, g, b] => ([r, g, b], None),
            [r, g, b, a] => ([r, g, b], Some(a)),
            _ => return None,
        };
        // A value as a byte: a percentage of 255, or a number in `unit`s of
        // a byte (1 for a channel, 255 for alpha).
        let byte = |s: &str, unit: f32| -> Option<u8> {
            let v = match s.strip_suffix('%') {
                Some(p) => exact_num::parse_f32(p).ok()? / 100.0 * 255.0,
                None => exact_num::parse_f32(s).ok()? * unit,
            };
            v.is_finite().then(|| v.round().clamp(0.0, 255.0) as u8)
        };
        Some(Color::rgba(
            byte(r, 1.0)?,
            byte(g, 1.0)?,
            byte(b, 1.0)?,
            alpha.map_or(Some(255), |a| byte(a, 255.0))?,
        ))
    }

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
    /// CSS `aspect-ratio` as authored (LLP 1053 G1).
    AspectRatio(&'a crate::ratio::AspectRatio),
    /// `drag-timeline` (LLP 1057.003).
    DragTimeline(&'a crate::timeline::DragTimeline),
    /// CSS `animation-timeline` (LLP 1057.003).
    AnimationTimeline(&'a crate::timeline::AnimationTimeline),
    /// CSS `animation-range` (LLP 1057.003).
    AnimationRange(&'a crate::timeline::AnimationRange),
    /// CSS `timeline-scope` (LLP 1057.003 D4).
    TimelineScope(&'a crate::timeline::TimelineScope),
    /// SVG paint (LLP 1055 D2).
    Paint(&'a crate::svg::Paint),
    /// SVG `stroke-dasharray` (LLP 1055 D2).
    DashArray(&'a crate::svg::DashArray),
    /// CSS `transform` on an SVG element (LLP 1055.000 D5).
    Transform(&'a crate::svg::TransformList),
    /// CSS `transform-origin` (LLP 1055.000 D5).
    TransformOrigin(&'a crate::svg::TransformOrigin),
    /// SVG `paint-order` (LLP 1055.000 D7).
    PaintOrder(&'a crate::svg::PaintOrder),
    /// SVG `marker-start`, `marker-mid`, `marker-end` (LLP 1055.000 D9).
    Marker(&'a crate::svg::MarkerRef),
    /// CSS `filter` on SVG elements (LLP 1055.000 D14).
    Filter(&'a crate::svg::filter::FilterList),
    /// CSS `background-image`: `none` or one gradient (LLP 1066).
    BackgroundImage(&'a crate::gradient::BackgroundImage),
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
    /// The `animation` row (LLP 1055 D5).
    Animations(&'a Animations),
}

impl RowValue<'_> {
    /// Whether every number the row carries is finite. Rows without floats
    /// (and the CSS values, which parse to finite numbers) are.
    pub(crate) fn is_finite(&self) -> bool {
        match self {
            RowValue::LineHeight(v) => v.is_finite(),
            RowValue::Dimension(v) => v.is_finite(),
            RowValue::Number(v) => v.is_finite(),
            RowValue::Vec2(v) => v.x.is_finite() && v.y.is_finite(),
            RowValue::Tracks(v) => v.is_finite(),
            RowValue::Transitions(v) => v.is_finite(),
            RowValue::Animations(v) => v.is_finite(),
            RowValue::DashArray(v) => v.0.iter().all(|n| n.is_finite()),
            RowValue::Transform(v) => v.is_finite(),
            RowValue::TransformOrigin(v) => v.is_finite(),
            RowValue::AnimationRange(v) => v.0.is_none_or(|[a, b]| a.is_finite() && b.is_finite()),
            RowValue::Paint(_)
            | RowValue::PaintOrder(_)
            | RowValue::Marker(_)
            | RowValue::Filter(_)
            | RowValue::ClipPath(_)
            | RowValue::ShapeOutside(_)
            | RowValue::AspectRatio(_)
            | RowValue::DragTimeline(_)
            | RowValue::AnimationTimeline(_)
            | RowValue::TimelineScope(_)
            | RowValue::BackgroundImage(_)
            | RowValue::Color(_)
            | RowValue::ColorValue(_)
            | RowValue::Color2(_)
            | RowValue::Enum(_)
            | RowValue::Placement(_) => true,
        }
    }
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
        // A text field in a block container keeps its own width, 20
        // characters or its content's (`field-sizing`), where a `<div>`
        // stretches: an `<input>` or `<textarea>` at `display: block`. Flex
        // and grid still stretch it, and insets still size it, as Chrome
        // does. Taffy's block layout skips stretch for tables and replaced
        // elements; a replaced element would also stop the insets, so the
        // field takes the table's exemption (a leaf: nothing else follows).
        s.item_is_table = node_type == NodeType::TextInput;
        s.box_sizing = match self.box_sizing {
            BoxSizing::ContentBox => taffy::style::BoxSizing::ContentBox,
            BoxSizing::BorderBox => taffy::style::BoxSizing::BorderBox,
        };
        s.position = match self.position_type {
            PositionType::Static => taffy::style::Position::Static,
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
        s.aspect_ratio = self.aspect_ratio.preferred();
        s.aspect_ratio_content_box = self.aspect_ratio.content_box();

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
    // A replaced element keeps its natural ratio under `aspect-ratio: auto`
    // (with or without a fallback ratio, or a degenerate one): CSS sizes an
    // `<img>` with one dimension given from the other by that ratio. Only a
    // plain `<ratio>` overrides it; natural ratios are of the content box.
    if arena.node_type(slot).is_replaced() && arena.style(slot).aspect_ratio.defers_to_natural() {
        if let Some((w, h)) = arena.intrinsic(slot) {
            if w > 0.0 && h > 0.0 {
                s.aspect_ratio = Some(w / h);
                s.aspect_ratio_content_box = true;
            }
        } else if let Some(ratio) = crate::svg::natural_ratio(arena, slot) {
            // @ref LLP 1055.000 D4 — an `svg`'s view box is its natural ratio.
            s.aspect_ratio = Some(ratio);
            s.aspect_ratio_content_box = true;
        } else if let Some(((w, h), true)) = arena.node_type(slot).default_object_size() {
            // A canvas's natural size gives it a natural ratio.
            s.aspect_ratio = Some(w / h);
            s.aspect_ratio_content_box = true;
        }
    }
    // A native tab bar fills the tablist's box. Its measured height supplies
    // the automatic minimum, so even a short authored row reserves the bar.
    // An explicit CSS min-height still owns that constraint; no natural ratio
    // or preferred width is inferred from this container measurement.
    if !arena.node_type(slot).is_replaced()
        && arena.node_type(slot) != NodeType::Control
        && s.min_size.height.is_auto()
    {
        if let Some((_, height)) = arena.intrinsic(slot) {
            s.min_size.height = length(height);
        }
    }
    // @ref LLP 1074 T1 — a root is the containing block of every absolutely
    // positioned box no positioned ancestor holds, on every host: a static
    // root is `relative` (its insets apply, as on the web's root element).
    if arena.is_root(slot) && s.position == taffy::style::Position::Static {
        s.position = taffy::style::Position::Relative;
    }
    s
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod finite_tests;

impl crate::generated::TouchAction {
    /// Whether the value leaves pinch zoom to the platform: `auto`,
    /// `manipulation` or any value naming `pinch-zoom` (LLP 1057.001 §2).
    pub fn pinch_zoom(self) -> bool {
        matches!(self, Self::Auto | Self::Manipulation) || self.name().ends_with("pinch-zoom")
    }
    /// The same value's pan axes alone: `pinch-zoom` dropped, which leaves
    /// `none` when it named nothing else. What a pan decides by.
    pub fn pans(self) -> Self {
        match self.name().strip_suffix("pinch-zoom") {
            Some("") => Self::None,
            Some(rest) => Self::from_name(rest.trim_end()).unwrap_or(Self::None),
            None => self,
        }
    }
}

#[cfg(test)]
mod touch_action_tests;
