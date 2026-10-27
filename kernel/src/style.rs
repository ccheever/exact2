//! Style grammars (dimensions, colors, grid tracks/placements) and Taffy lowering.
//! Generated `StyleProps` holds rows; `to_taffy` converts authored to engine style.
//! Percentages use points (0–100) on the wire/in storage, becoming fractions here.

use std::borrow::Cow;
use taffy::prelude::{auto, length, percent};

use crate::arena::NodeArena;
use crate::error::StyleValueError;
use crate::generated::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Direction, Display, FlexDirection, FlexWrap,
    GridAutoFlow, JustifyContent, JustifyItems, NodeType, Overflow, PositionType, StyleId,
    StyleMask, StyleProps,
};

mod backdrop;
pub use backdrop::{link as link_backdrop_filter, BackdropFilter, BackdropOp};
mod border;
pub(crate) mod effects;
mod engine;
pub use crate::gradient::link as link_gradients;
pub use effects::link as link_effects;
mod grid;
pub use grid::link as link_grid;
pub use grid::{
    GridFitContent, GridLine, GridPlacement, GridRepeat, GridRepeatCount, GridTrack,
    GridTrackComponent, GridTrackMax, GridTrackMin, GridTracks,
};
pub mod calc;
mod floor;
mod handles;
pub use calc::Comparison;
use handles::calc_handle;
pub(crate) use handles::{calc_handle_in, resolve_calc, tree_handle, LayoutEnv};
pub mod env;
pub mod owner;
pub use env::link as link_segments;
pub use env::{uses_env, Edge, Env, EnvRefusal, Rect, Region, SegmentVar, TitleBarVar};
/// Link the wide colour forms (`lab()`, `lch()`, `oklab()`, `oklch()`,
/// `color()`) into every colour row's grammar: native hosts and the compiler
/// at start, a web artifact by use (LLP 1047 D2, LLP 1056 §8.2).
pub use exact_motion::color::css::link_wide as link_wide_colors;
pub use owner::{OwnerValues, OwnerVar};
mod viewport;
pub use viewport::ViewportUnit;
pub mod cells;
mod color_parse;
pub mod metrics;
pub mod profiled;
pub mod relative;
pub mod roles;
pub mod scale;
mod shadow;
pub mod sides;
pub mod space;
pub(crate) mod stroke;
pub mod symbols;
pub mod theme;
mod variations;
pub mod wide;
pub use shadow::{BoxShadow, BoxShadows, GlyphShadow, TextShadow};
pub use variations::FontVariations;

/// Largest explicit grid Taffy lays out on one axis.
pub const MAX_GRID_TRACKS: usize = 10_000;

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
    /// A layout region's inset at an edge plus points (LLP 1123 D10; [`env`]).
    Region(Region, Edge, f32),
    /// A viewport segment's length (LLP 1078 D3): `env(viewport-segment-<var>
    /// <x> <y>)`, plus points. Undefined on a viewport with one segment, or
    /// past its grid: the row's initial value then (CSS-ENV-1 §2.3).
    Segment(SegmentVar, u8, u8, f32),
    /// A content owner's value plus points (LLP 1123 D9): `var(--exact-…)`,
    /// resolved against the environment of the owner the node lies in.
    Owner(OwnerVar, f32),
    /// A percentage of a viewport dimension, resolved at layout.
    Viewport(ViewportUnit, f32),
    /// A math function the other forms do not hold exactly: its simplified
    /// calculation tree (CSS Values 4 §10.10), interned, so the row holds a
    /// handle; resolved as `env()` is, or at layout against its basis when
    /// it holds a percentage ([`calc`]).
    Compare(Comparison),
}

impl Dimension {
    /// Whether the value is a finite number (or `Auto`).
    pub fn is_finite(self) -> bool {
        match self {
            Dimension::Auto | Dimension::Compare(_) => true,
            Dimension::Points(v)
            | Dimension::Percent(v)
            | Dimension::Viewport(_, v)
            | Dimension::Env(_, v)
            | Dimension::Region(_, _, v)
            | Dimension::Segment(_, _, _, v)
            | Dimension::Owner(_, v) => v.is_finite(),
            Dimension::Calc(p, v) => p.is_finite() && v.is_finite(),
        }
    }

    /// Whether it resolves against a percentage basis: a percentage, alone
    /// or in a calculation.
    pub fn has_percent(self) -> bool {
        match self {
            Dimension::Percent(_) | Dimension::Calc(..) => true,
            Dimension::Compare(c) => c.has_percent(),
            _ => false,
        }
    }

    /// A math function as the dimension it simplifies to ([`calc`]), or `None`.
    pub fn parse_calc(text: &str) -> Option<Dimension> {
        calc::parse(text).ok().flatten()
    }

    /// An `env()` length by CSS's grammar, or `None` when the text is not one:
    /// `env(safe-area-inset-<edge>)`, `env(viewport-segment-<var> <x> <y>)`
    /// (LLP 1078 D3), either with a fallback, or a math function
    /// ([`Self::parse_calc`]). An undefined segment without a fallback takes
    /// the row's initial value. A text that names one of the variables
    /// wrongly is `None` too; [`env::parse`] says why.
    pub fn parse_env(text: &str) -> Option<Dimension> {
        env::parse(text)
            .ok()
            .flatten()
            .or_else(|| Self::parse_calc(text))
    }

    /// The points an `env()` length resolves to under `env`; any other
    /// dimension unchanged. A segment length whose segment `env` does not
    /// define is `Auto` — the stand-in for the row's initial value, which
    /// [`StyleProps::to_taffy`] substitutes exactly before lowering.
    pub fn resolve(self, env: &Env) -> Dimension {
        match self {
            Dimension::Env(..) | Dimension::Region(..) => env::resolve_inset(self, env),
            Dimension::Viewport(unit, n) => Dimension::Points(calc::narrow(
                f64::from(unit.basis(env)) * f64::from(n) / 100.0,
            )),
            Dimension::Segment(var, x, y, plus) => env::resolve(var, x, y, plus, env),
            Dimension::Owner(var, plus) => var.value(env).map_or(Dimension::Auto, |base| {
                Dimension::Points(calc::narrow(base + f64::from(plus)))
            }),
            Dimension::Compare(c) if c.undefined(env) => Dimension::Auto,
            Dimension::Compare(c) => match c.substituted(env) {
                // A tree needs a basis only where it reads a percentage
                Some(Dimension::Compare(t)) if t == c => c
                    .value(env, None)
                    .map_or_else(|| Dimension::Compare(c.bound(env)), Dimension::Points),
                // A text substituted is the row's value it reads as
                Some(d) => d.resolve(env),
                None => Dimension::Auto,
            },
            other => other,
        }
    }

    /// The points this length is against `basis`, the size its percentage is
    /// a fraction of, under `env`; zero for `auto` or an undefined segment.
    /// What a host that sizes a box itself (a radius, a native control's
    /// padding) reads.
    pub fn at_basis(self, env: &Env, basis: f32) -> f32 {
        let basis = f64::from(basis);
        match self.resolve(env) {
            Dimension::Points(v) => v,
            Dimension::Percent(p) => calc::narrow(basis * f64::from(p) / 100.0),
            Dimension::Calc(p, x) => calc::narrow(basis * f64::from(p) / 100.0 + f64::from(x)),
            Dimension::Compare(c) => calc::narrow(c.wide(env, Some(basis)).unwrap_or(0.0)),
            _ => 0.0,
        }
    }

    /// Whether this is a segment length `env` does not define (LLP 1078 D3),
    /// or a calculation that reads one or an undefined title-bar area.
    fn undefined_segment(self, env: &Env) -> bool {
        match self {
            Dimension::Segment(_, x, y, _) => env.segment(x, y).is_none(),
            Dimension::Owner(var, _) => var.value(env).is_none(),
            Dimension::Compare(c) => c.undefined(env),
            _ => false,
        }
    }

    fn to_lpa(self, env: &Env) -> taffy::style::LengthPercentageAuto {
        match self.resolve(env) {
            Dimension::Auto => auto(),
            Dimension::Points(v) => length(v),
            Dimension::Percent(v) => percent(v / 100.0),
            Dimension::Calc(p, v) => taffy::style::LengthPercentageAuto::calc(calc_handle(p, v)),
            Dimension::Compare(c) => {
                taffy::style::LengthPercentageAuto::calc(tree_handle(c, false, 0.0))
            }
            _ => unreachable!("resolved above"),
        }
    }

    /// Whether a padding lowers to zero as the engine compares lengths, by
    /// bits: `auto`, or `+0` points or percent — never `-0`.
    fn lp_is_zero(self, env: &Env) -> bool {
        match self.resolve(env) {
            Dimension::Auto => true,
            Dimension::Points(v) => v.to_bits() == 0,
            Dimension::Percent(v) => (v / 100.0).to_bits() == 0,
            // A calc() is a handle the engine resolves, never its zero length.
            Dimension::Calc(..) | Dimension::Compare(_) => false,
            _ => unreachable!("resolved above"),
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
pub enum StyleValue<'a> {
    /// A number: dimensions in points, raw numeric rows, or colors as `0xRRGGBBAA`.
    Number(f64),
    /// CSS text borrowed while parsing; parsed rows own any text they retain.
    Text(Cow<'a, str>),
    /// A percentage, authored 0–100.
    Percent(f64),
    /// `auto`.
    Auto,
    /// Two numbers.
    Vec2(f32, f32),
}

impl StyleValue<'_> {
    /// Whether this value leaves row `style` unset — inherited, else
    /// initial — which is what clearing the row does: an empty string, which
    /// declares nothing (CSSOM's `setProperty(p, "")`), `unset`, and `inherit`
    /// on an inherited row (`color: currentcolor` is `inherit`). `inherit` on
    /// any other row needs its parent's value, which no row holds, so it stays refused (feed F1).
    pub fn unsets(&self, style: StyleId) -> bool {
        let StyleValue::Text(t) = self else {
            return false;
        };
        Self::text_unsets(t, style)
    }

    /// [`Self::unsets`] for borrowed text; dynamic bindings ask it of every changed row.
    pub fn text_unsets(t: &str, style: StyleId) -> bool {
        // A metric with no row on this platform (LLP 1115.002 D6).
        let t = metrics::text(t).unwrap_or(t).trim();
        t.is_empty()
            || t.eq_ignore_ascii_case("unset")
            || (t.eq_ignore_ascii_case("inherit") && StyleMask::INHERITED.has(style))
            || (t.eq_ignore_ascii_case("currentcolor") && style == StyleId::TextColor)
            // @ref LLP 1034 §8: a bound `normal` follows the surrounding
            // scheme, as leaving the row off does (CSS's own `normal` means
            // the page's schemes, which no host here has).
            || (t.eq_ignore_ascii_case("normal") && style == StyleId::ColorScheme)
    }

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

    pub(crate) fn f32(&self, style: StyleId) -> Result<f32, StyleValueError> {
        // @ref LLP 1077 D7, D8, D11 — the rows that take CSS text or a
        // range of their own.
        if let Some(value) = space::f32_row(self, style) {
            return value;
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
        // @ref LLP 1053 G3 — CSS refuses a negative flex factor, gap or
        // border width.
        if calc::nonnegative_row(style) {
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
    pub(crate) fn css_text(&self, style: StyleId) -> Result<Cow<'_, str>, StyleValueError> {
        match self {
            StyleValue::Number(n) => Ok(exact_num::Shortest(*n).to_string().into()),
            StyleValue::Percent(p) => Ok(format!("{}%", exact_num::Shortest(*p)).into()),
            StyleValue::Auto => Ok("auto".into()),
            _ => self.text(style).map(Cow::Borrowed),
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
        let value = match self {
            StyleValue::Number(n) if (*n as f32).is_finite() => Ok(Dimension::Points(*n as f32)),
            StyleValue::Percent(p) if (*p as f32).is_finite() => Ok(Dimension::Percent(*p as f32)),
            StyleValue::Auto if admits_auto => Ok(Dimension::Auto),
            StyleValue::Auto => Err(StyleValueError::AutoNotAdmitted { style }),
            // CSS's initial maximum, the unbounded one `auto` already names here (LLP 1102 §3.11).
            StyleValue::Text(t)
                if matches!(style, StyleId::MaxWidth | StyleId::MaxHeight)
                    && t.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])
                        .eq_ignore_ascii_case("none") =>
            {
                Ok(Dimension::Auto)
            }
            // An `env()` with a fallback is substituted before this row's
            // grammar reads the value (CSS Environment Variables 1 §3): held
            // as written, its syntax checked
            StyleValue::Text(t)
                if calc::substitutes(t).map_err(|r| match r {
                    calc::Refused::Env(refusal) => StyleValueError::BadEnv { style, refusal },
                    calc::Refused::Comparison(reason) => {
                        StyleValueError::BadComparison { style, reason }
                    }
                })? =>
            {
                let text = t.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
                // Refuse a text the wire's string cannot carry
                if text.len() > crate::wire::codec::MAX_STRING_BYTES as usize {
                    return Err(StyleValueError::BadComparison {
                        style,
                        reason: calc::refusal::TOO_LONG,
                    });
                }
                return Ok(Dimension::Compare(Comparison::substitution(text, style)));
            }
            StyleValue::Text(t) => match calc::parse(t) {
                Err(calc::Refused::Comparison(reason)) => {
                    Err(StyleValueError::BadComparison { style, reason })
                }
                Err(calc::Refused::Env(refusal)) => Err(StyleValueError::BadEnv { style, refusal }),
                Ok(Some(d)) => Ok(Some(d)),
                Ok(None) => env::parse(t).map_err(|refusal| StyleValueError::BadEnv { style, refusal }),
            }?
            .or_else(|| owner::parse(t).map(|var| Dimension::Owner(var, 0.0)))
            .or_else(|| viewport::parse(t))
                .or_else(|| {
                    parse_pixel_length(t.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']))
                        .map(Dimension::Points)
                })
                // CSS's absolute units (96 px to the inch) and a percentage
                // written as text (LLP 1055.000 D4).
                .or_else(|| absolute_length(t.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])))
                .ok_or(StyleValueError::WrongKind {
                    style,
                    expected: "number, px, rem or em length, viewport length (vw/vh/vmin/vmax/svw/svh/lvw/lvh/dvw/dvh), percent, auto, env(safe-area-inset-*), env(viewport-segment-* x y), a calc() of these (+ and - between lengths, * and / by a number), or min()/max()/clamp() of these",
                }),
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "number, percent, auto, env(safe-area-inset-*), env(viewport-segment-* x y), a calc() of these (+ and - between lengths, * and / by a number), or min()/max()/clamp()",
            }),
        }?;
        // A row CSS gives no percentage refuses one inside a math function
        // too: there is no basis to resolve it against (CSS Values 4 §10.9)
        if style.refuses_percent() && value.has_percent() {
            return Err(StyleValueError::WrongKind {
                style,
                expected: "a length or auto: CSS takes no percentage here, nor one inside calc(), min(), max() or clamp()",
            });
        }
        // CSS clamps a math function to the row's range (CSS Values 4 §9.1)
        let math = matches!(self, StyleValue::Text(t) if calc::names_math(t) && !calc::is_env(t));
        calc::signed(style, value, math)
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
            // @ref LLP 1077 D7 — a colour, else the shorthand's colour part.
            StyleValue::Text(t) if style == StyleId::TextStrokeColor => {
                self.color_value(style).map(Some).or_else(|_| {
                    stroke::parse(t)
                        .map(|(_, c)| c)
                        .map_err(|reason| StyleValueError::BadTextStroke { style, reason })
                })
            }
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
            // @ref LLP 1077 D8 — the one `translate` text sets its lengths,
            // its percentages (chess diary #4) and its z, each row its part.
            StyleValue::Text(t)
                if style == StyleId::Translate || style == StyleId::TranslatePercent =>
            {
                space::translate(t)
                    .map(|(px, pct)| if style == StyleId::Translate { px } else { pct })
                    .ok_or(StyleValueError::WrongKind {
                        style,
                        expected: "one or two lengths in px or percentages of the box (unitless zero allowed)",
                    })
            }
            // `translate: -50%`: one percentage, the x's, which the
            // compiler and the runner pass as a percentage.
            StyleValue::Percent(p)
                if p.is_finite()
                    && (style == StyleId::Translate || style == StyleId::TranslatePercent) =>
            {
                let x = if style == StyleId::Translate { 0.0 } else { *p as f32 };
                Ok(Vec2 { x, y: 0.0 })
            }
            _ => Err(StyleValueError::WrongKind {
                style,
                expected: "vec2",
            }),
        }
    }
}

/// Whether `set_dynamic` reads any text on row `style` besides the CSS-wide
/// keywords [`StyleValue::unsets`] clears a row with. A number row with no
/// text arm (`opacity`, `flex-grow`, `z-index`, `font-weight`) refuses every
/// computed string, where a browser applies one (`opacity: 0.5`), so the
/// compiler refuses a string-typed binding of such a row as it refuses the
/// literal (LLP 1017 P1a). The rows listed are those whose conversion has a
/// text arm: `rem`/`em` and `px` lengths ([`relative`]), SVG's stroke
/// lengths, and the CSS forms [`StyleValue::f32`] reads.
pub fn takes_text(style: StyleId) -> bool {
    use crate::generated::StyleCodec;
    use StyleId::*;
    match style.codec() {
        StyleCodec::F32 => {
            relative::admits_relative(style)
                || matches!(
                    style,
                    StrokeWidth
                        | StrokeDashoffset
                        | Rotate
                        | TranslateZ
                        | Perspective
                        | SymbolValue
                        | TextStrokeWidth
                        | ShapeMargin
                        | FontStretch
                )
        }
        StyleCodec::U16 | StyleCodec::U32 | StyleCodec::I32 => false,
        _ => true,
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
    let value = parse_css_number(number)?;
    let mantissa = number.split(['e', 'E']).next()?;
    if value.abs() > f32::MAX as f64
        || (!pixels && mantissa.bytes().any(|b| b.is_ascii_digit() && b != b'0'))
    {
        return None;
    }
    Some(value as f32)
}

/// A finite CSS number token, before a property's unit conversion and range check.
fn parse_css_number(number: &str) -> Option<f64> {
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
    value.is_finite().then_some(value)
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
    /// A colour role (LLP 1095 D2), by id into `COLOR_ROLES`: the platform's
    /// own colour where a host has it, the role's pair everywhere else.
    Role(u8),
    /// A `-exact-platform-color()` (LLP 1095 D3), by id into the interned table.
    Platform(u16),
    /// A colour in its own space, or a `light-dark()` with one (LLP 1100
    /// D2), by id into `style::wide`.
    Wide(u16),
    /// A paint-motion frame (LLP 1100 D2): extended linear sRGB in
    /// 1/2048ths and 8-bit alpha. Never authored; not interned.
    Moving([i16; 3], u8),
    /// A colour in a profile's space (LLP 1100 D3), by id into
    /// `style::profiled`.
    Profiled(u16),
}

impl Default for ColorValue {
    fn default() -> ColorValue {
        ColorValue::Fixed(Color(0))
    }
}

impl ColorValue {
    /// The colour under an appearance. A host that paints calls this; the web
    /// host does not, because it hands the pair to the browser.
    pub fn resolve(self, dark: bool) -> Color {
        if let Some(c) = roles::painted_accent(self, dark) {
            return c;
        }
        // @ref LLP 1095 D4 — a role the theme replaces is what replaces it.
        if let Some(o) = roles::painted_override(self) {
            return o.resolve(dark);
        }
        // @ref LLP 1095 D1 — a reference is what the host reported, else
        // its fallback pair.
        if let Some(c) = roles::reported(self, dark) {
            return c;
        }
        match self.fallback() {
            ColorValue::LightDark(light, night) => {
                if dark {
                    night
                } else {
                    light
                }
            }
            ColorValue::Fixed(c) => c,
            ColorValue::Role(_)
            | ColorValue::Platform(_)
            | ColorValue::Wide(_)
            | ColorValue::Moving(..)
            | ColorValue::Profiled(_) => Color::TRANSPARENT,
        }
    }

    /// Whether this depends on the appearance: a pair or a reference — what
    /// a host asks before deciding whether an appearance change is anything to it.
    pub fn is_scheme_aware(self) -> bool {
        match self {
            ColorValue::Fixed(_) | ColorValue::Moving(..) | ColorValue::Profiled(_) => false,
            ColorValue::Wide(id) => wide::wide(id).is_some_and(|w| w.dark.is_some()),
            _ => true,
        }
    }

    /// A moving frame from extended linear sRGB and alpha, held to ±16.
    pub fn moving(linear: [f64; 3], alpha: f64) -> ColorValue {
        let q = |v: f64| (v.clamp(-16.0, 15.999) * 2048.0).round() as i16;
        ColorValue::Moving(linear.map(q), (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// A moving frame's extended linear sRGB and alpha.
    pub fn moving_linear(self) -> Option<([f64; 3], f64)> {
        match self {
            ColorValue::Moving(c, a) => {
                Some((c.map(|v| f64::from(v) / 2048.0), f64::from(a) / 255.0))
            }
            _ => None,
        }
    }

    /// A role (LLP 1095 D2), a `-exact-platform-color()` (D3), a colour in a
    /// profile's or its own space (LLP 1100), or `light-dark(<color>,
    /// <color>)`. Anything else falls through to the plain colour parse.
    pub fn parse_light_dark(text: &str) -> Option<ColorValue> {
        let start = text.trim_start();
        // A hex colour cannot name a role, a profile or another colour
        // space. Leave it to the plain parser without scanning those
        // vocabularies (animated terminal runs use hex on every frame).
        if start.starts_with('#') {
            return None;
        }
        // @ref LLP 1095 D4 — a theme name, or a role the theme overrides,
        // reads the colour its entry stands for.
        if let Some(value) = theme::resolved(text, |value| {
            ColorValue::parse_light_dark(value)
                .or_else(|| Color::parse(value).map(ColorValue::Fixed))
        }) {
            return value;
        }
        if let Some(role) = roles::role(text) {
            return Some(ColorValue::Role(role));
        }
        if start.starts_with("-exact-platform-color(") {
            return roles::parse_platform(text);
        }
        if start
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("color(--"))
        {
            return profiled::parse_profiled(text);
        }
        ColorValue::parse_pair(text).or_else(|| wide::parse_wide(text))
    }

    /// `light-dark()` of two legacy colours; a reference is not valid inside
    /// one (LLP 1095 D1).
    pub(crate) fn parse_pair(text: &str) -> Option<ColorValue> {
        let inner = text.trim().strip_prefix("light-dark(")?.strip_suffix(')')?;
        let comma = top_level_comma(inner)?;
        Some(ColorValue::LightDark(
            Color::parse(&inner[..comma])?,
            Color::parse(&inner[comma + 1..])?,
        ))
    }
}

/// The comma between two colours, not one inside an `rgb()`.
pub(crate) fn top_level_comma(inner: &str) -> Option<usize> {
    let mut depth = 0;
    inner.find(|c| {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
        false
    })
}

impl From<Color> for ColorValue {
    fn from(c: Color) -> ColorValue {
        ColorValue::Fixed(c)
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
    /// Ordered invoker-popover flip attempts (LLP 1021 D2).
    PositionTryFallbacks(&'a crate::position_try::PositionTryFallbacks),
    /// `-exact-drag-timeline` (LLP 1057.003).
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
    /// CSS `box-shadow`: `none` or a list (LLP 1077 D4).
    BoxShadow(&'a BoxShadows),
    /// CSS `backdrop-filter`, in authored order (LLP 1053.000 D1).
    BackdropFilter(&'a BackdropFilter),
    /// CSS `text-shadow` (LLP 1077 D3).
    TextShadow(&'a TextShadow),
    /// CSS `font-variation-settings` (LLP 1019 D9).
    FontVariations(&'a FontVariations),
    /// CSS `mask-image`: `none` or one gradient (LLP 1077 D2).
    MaskImage(&'a crate::gradient::BackgroundImage),
    /// CSS `corner-shape` (LLP 1077 D1).
    CornerShape(&'a crate::corner::CornerShape),
    /// CSS `rotate`'s axis (LLP 1077 D8).
    RotateAxis(&'a space::RotateAxis),
    /// A symbol's palette (LLP 1077 D10).
    SymbolPalette(&'a symbols::SymbolPalette),
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
    Placement(&'a GridPlacement),
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
            RowValue::BackdropFilter(v) => v.is_finite(),
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
            | RowValue::PositionTryFallbacks(_)
            | RowValue::DragTimeline(_)
            | RowValue::AnimationTimeline(_)
            | RowValue::TimelineScope(_)
            | RowValue::BackgroundImage(_)
            | RowValue::TextShadow(_)
            | RowValue::FontVariations(_)
            | RowValue::BoxShadow(_)
            | RowValue::MaskImage(_)
            | RowValue::CornerShape(_)
            | RowValue::RotateAxis(_)
            | RowValue::SymbolPalette(_)
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
        JustifyContent::Start => taffy::style::JustifyContent::START,
        JustifyContent::End => taffy::style::JustifyContent::END,
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
        AlignItems::Start => taffy::style::AlignItems::START,
        AlignItems::End => taffy::style::AlignItems::END,
        AlignItems::SelfStart => taffy::style::AlignItems::SELF_START,
        AlignItems::SelfEnd => taffy::style::AlignItems::SELF_END,
        AlignItems::FlexStart => taffy::style::AlignItems::FLEX_START,
        AlignItems::FlexEnd => taffy::style::AlignItems::FLEX_END,
        AlignItems::Center => taffy::style::AlignItems::CENTER,
        AlignItems::Baseline => taffy::style::AlignItems::BASELINE,
        AlignItems::Stretch => taffy::style::AlignItems::STRETCH,
    })
}

fn justify_items(v: JustifyItems, direction: Direction) -> Option<taffy::style::AlignItems> {
    use taffy::style::AlignItems as T;
    Some(match v {
        JustifyItems::Normal => return None,
        JustifyItems::Stretch => T::STRETCH,
        JustifyItems::Baseline => T::BASELINE,
        JustifyItems::Center | JustifyItems::UnsafeCenter => T::CENTER,
        JustifyItems::Start | JustifyItems::UnsafeStart => T::START,
        JustifyItems::End | JustifyItems::UnsafeEnd => T::END,
        JustifyItems::SelfStart | JustifyItems::UnsafeSelfStart => T::SELF_START,
        JustifyItems::SelfEnd | JustifyItems::UnsafeSelfEnd => T::SELF_END,
        JustifyItems::FlexStart | JustifyItems::UnsafeFlexStart => T::FLEX_START,
        JustifyItems::FlexEnd | JustifyItems::UnsafeFlexEnd => T::FLEX_END,
        JustifyItems::Left | JustifyItems::UnsafeLeft => match direction {
            Direction::Ltr => T::START,
            Direction::Rtl => T::END,
        },
        JustifyItems::Right | JustifyItems::UnsafeRight => match direction {
            Direction::Ltr => T::END,
            Direction::Rtl => T::START,
        },
        JustifyItems::SafeCenter => T::SAFE_CENTER,
        JustifyItems::SafeStart => T::SAFE_START,
        JustifyItems::SafeEnd => T::SAFE_END,
        JustifyItems::SafeSelfStart => T::SAFE_SELF_START,
        JustifyItems::SafeSelfEnd => T::SAFE_SELF_END,
        JustifyItems::SafeFlexStart => T::SAFE_FLEX_START,
        JustifyItems::SafeFlexEnd => T::SAFE_FLEX_END,
        JustifyItems::SafeLeft => match direction {
            Direction::Ltr => T::SAFE_START,
            Direction::Rtl => T::SAFE_END,
        },
        JustifyItems::SafeRight => match direction {
            Direction::Ltr => T::SAFE_END,
            Direction::Rtl => T::SAFE_START,
        },
    })
}

fn align_self(v: AlignSelf) -> Option<taffy::style::AlignSelf> {
    match v {
        AlignSelf::Auto => None,
        AlignSelf::Start => Some(taffy::style::AlignSelf::START),
        AlignSelf::End => Some(taffy::style::AlignSelf::END),
        AlignSelf::SelfStart => Some(taffy::style::AlignSelf::SELF_START),
        AlignSelf::SelfEnd => Some(taffy::style::AlignSelf::SELF_END),
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
        AlignContent::Start => taffy::style::AlignContent::START,
        AlignContent::End => taffy::style::AlignContent::END,
        AlignContent::FlexStart => taffy::style::AlignContent::FLEX_START,
        AlignContent::FlexEnd => taffy::style::AlignContent::FLEX_END,
        AlignContent::Center => taffy::style::AlignContent::CENTER,
        AlignContent::Stretch => taffy::style::AlignContent::STRETCH,
        AlignContent::SpaceBetween => taffy::style::AlignContent::SPACE_BETWEEN,
        AlignContent::SpaceAround => taffy::style::AlignContent::SPACE_AROUND,
        AlignContent::SpaceEvenly => taffy::style::AlignContent::SPACE_EVENLY,
    })
}

impl Overflow {
    /// CSS Overflow 3 §3.1: visible/clip compute to auto/hidden beside
    /// a scrolling axis; together they remain non-scrolling.
    pub fn computed(self, other: Self) -> Self {
        match self {
            Self::Visible if !matches!(other, Self::Visible | Self::Clip) => Self::Auto,
            Self::Clip if !matches!(other, Self::Visible | Self::Clip) => Self::Hidden,
            value => value,
        }
    }
}

pub(crate) fn set_grid_dynamic(
    style: &mut StyleProps,
    id: StyleId,
    value: &StyleValue,
) -> Result<(), StyleValueError> {
    grid::set_dynamic(style, id, value)
}

pub(crate) fn decode_grid_rows(
    style: &mut StyleProps,
    mask: StyleMask,
    reader: &mut crate::wire::codec::Reader<'_>,
) -> Result<(), crate::error::DecodeError> {
    grid::decode(style, mask, reader)
}

pub(crate) fn encode_grid_rows(
    style: &StyleProps,
    mask: StyleMask,
    writer: &mut crate::wire::codec::Writer,
) {
    grid::encode(style, mask, writer);
}

impl StyleProps {
    /// Whether every padding and border width reaches layout as zero, read
    /// without building the engine's style: a kernel that mirrors no engine
    /// tree checks content regions too (LLP 1047 §10).
    pub(crate) fn unpadded(&self, env: &Env) -> bool {
        let (down, across) = (cells::ROW, cells::COLUMN);
        [
            self.padding_top.spaced(env, down),
            self.padding_right.spaced(env, across),
            self.padding_bottom.spaced(env, down),
            self.padding_left.spaced(env, across),
        ]
        .into_iter()
        .all(|p| match p.resolve(env) {
            Dimension::Points(v) => v <= 0.0,
            Dimension::Percent(v) => v / 100.0 <= 0.0,
            _ => p.lp_is_zero(env),
        }) && self.border_widths().into_iter().all(|w| w.to_bits() == 0)
    }

    /// Lower to engine style. `node_type` supplies the per-tag defaults the
    /// table does not carry: scroll containers scroll on their block axis
    /// unless the producer set `overflow_y`.
    /// `env` resolves the `env()` lengths (LLP 1001 §2: the kernel's
    /// environment, set by the host with the viewport).
    #[allow(clippy::field_reassign_with_default)]
    pub fn to_taffy(&self, node_type: NodeType, env: &Env) -> taffy::style::Style {
        // A segment length the environment does not define is invalid at
        // computed-value time (CSS-ENV-1 §2.3): the row takes its initial
        // value, which is the table's default (LLP 1078 D3).
        self.env_resolved(env).lower(node_type, env)
    }

    /// This style with every segment length `env` does not define replaced
    /// by its row's initial value (CSS-ENV-1 §2.3: invalid at computed-value
    /// time; LLP 1078 D3) — borrowed when there is none to replace, a copy
    /// otherwise. Every reader of a style's dimensions goes through this
    /// before resolving them, so no reader sees the `Auto` stand-in on a row
    /// that does not admit it.
    pub fn env_resolved(&self, env: &Env) -> std::borrow::Cow<'_, StyleProps> {
        if self.has_undefined_segment(env) {
            std::borrow::Cow::Owned(self.with_initial_segments(env))
        } else {
            std::borrow::Cow::Borrowed(self)
        }
    }

    fn has_undefined_segment(&self, env: &Env) -> bool {
        self.mask
            .iter()
            .any(|id| matches!(self.get(id), RowValue::Dimension(d) if d.undefined_segment(env)))
    }

    /// A copy whose undefined segment rows hold the table's defaults.
    fn with_initial_segments(&self, env: &Env) -> StyleProps {
        let defaults = StyleProps::default();
        let mut out = self.clone();
        for id in self.mask.iter() {
            let RowValue::Dimension(d) = self.get(id) else {
                continue;
            };
            if !d.undefined_segment(env) {
                continue;
            }
            let RowValue::Dimension(initial) = defaults.get(id) else {
                continue;
            };
            let value = match initial {
                Dimension::Auto => StyleValue::Auto,
                Dimension::Percent(p) => StyleValue::Percent(f64::from(p)),
                Dimension::Points(v) => StyleValue::Number(f64::from(v)),
                _ => StyleValue::Number(0.0),
            };
            // The default fits its own row; nothing to refuse.
            let _ = out.set_dynamic(id, &value);
        }
        out
    }

    #[allow(clippy::field_reassign_with_default)]
    fn lower(&self, node_type: NodeType, env: &Env) -> taffy::style::Style {
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
        // A form control in a block container keeps its preferred width,
        // where a `<div>` stretches: an `<input>`, `<textarea>` or `<select>`
        // at `display: block`. Flex columns and grids still stretch it, and
        // absolute insets still size it, as Chrome does. Taffy's block layout
        // skips stretch for tables and replaced elements; a replaced element
        // would also stop the insets, so these measured leaves take the
        // table's exemption (nothing else follows from that marker).
        s.item_is_table = matches!(node_type, NodeType::TextInput | NodeType::Control);
        s.box_sizing = match self.box_sizing {
            BoxSizing::ContentBox => taffy::style::BoxSizing::ContentBox,
            BoxSizing::BorderBox => taffy::style::BoxSizing::BorderBox,
        };
        s.position = match self.position_type {
            PositionType::Static => taffy::style::Position::Static,
            PositionType::Relative => taffy::style::Position::Relative,
            PositionType::Absolute => taffy::style::Position::Absolute,
            // @ref LLP 1083 D1 — a sticky box lays out as a relative one
            // whose insets are not offsets: the host moves it as its
            // scroller scrolls (`sticky.rs`).
            PositionType::Sticky => taffy::style::Position::Relative,
        };
        let y = if !self.mask.has(StyleId::OverflowY) && node_type.scrolls_by_default() {
            Overflow::Auto
        } else {
            self.overflow_y
        };
        let x = self.overflow_x;
        s.overflow = taffy::geometry::Point {
            x: engine::overflow(x.computed(y)),
            y: engine::overflow(y.computed(x)),
        };
        s.scrollbar_width = 0.0;

        s.size = taffy::geometry::Size {
            width: self.width.floor_taffy(env),
            height: self.height.floor_taffy(env),
        };
        s.min_size = taffy::geometry::Size {
            width: self.min_width.floor_lpa(env),
            height: self.min_height.floor_lpa(env),
        };
        s.max_size = taffy::geometry::Size {
            width: self.max_width.floor_lpa(env),
            height: self.max_height.floor_lpa(env),
        };
        s.aspect_ratio = self.rare.aspect_ratio.preferred();
        s.aspect_ratio_content_box = self.rare.aspect_ratio.content_box();

        s.inset = if self.position_type == PositionType::Sticky {
            let auto = taffy::style::LengthPercentageAuto::auto();
            taffy::geometry::Rect {
                top: auto,
                right: auto,
                bottom: auto,
                left: auto,
            }
        } else {
            taffy::geometry::Rect {
                top: self.top.to_lpa(env),
                right: self.right.to_lpa(env),
                bottom: self.bottom.to_lpa(env),
                left: self.left.to_lpa(env),
            }
        };
        // Under a terminal's space rule, each in whole cells on its axis.
        let (down, across) = (cells::ROW, cells::COLUMN);
        s.margin = taffy::geometry::Rect {
            top: self.margin_top.spaced(env, down).to_lpa(env),
            right: self.margin_right.spaced(env, across).to_lpa(env),
            bottom: self.margin_bottom.spaced(env, down).to_lpa(env),
            left: self.margin_left.spaced(env, across).to_lpa(env),
        };
        s.padding = taffy::geometry::Rect {
            top: self.padding_top.spaced(env, down).floor_lp(env, 0.0),
            right: self.padding_right.spaced(env, across).floor_lp(env, 0.0),
            bottom: self.padding_bottom.spaced(env, down).floor_lp(env, 0.0),
            left: self.padding_left.spaced(env, across).floor_lp(env, 0.0),
        };
        let [top, right, bottom, left] = self.border_widths_in(env);
        s.border = taffy::geometry::Rect {
            top: length(top),
            right: length(right),
            bottom: length(bottom),
            left: length(left),
        };

        s.flex_direction = flex_direction(self.flex_direction);
        s.flex_wrap = flex_wrap(self.flex_wrap);
        s.flex_basis = self.flex_basis.floor_taffy(env);
        s.flex_grow = self.flex_grow;
        s.flex_shrink = self.flex_shrink;
        s.justify_content = justify_content(self.justify_content);
        s.align_items = align_items(self.align_items);
        s.align_self = align_self(self.align_self);
        s.align_content = align_content(self.align_content);
        s.justify_items = justify_items(self.justify_items, self.direction);
        s.gap = taffy::geometry::Size {
            width: length(cells::gap(env, self.column_gap, across)),
            height: length(cells::gap(env, self.row_gap, down)),
        };

        s.grid_auto_flow = engine::grid_auto_flow(self.grid_auto_flow);
        s.rare = engine::rare(self);
        s
    }
}

/// The engine style for a live slot, its `env()` lengths resolved against
/// the arena's environment.
pub fn taffy_style(arena: &NodeArena, slot: u32) -> taffy::style::Style {
    let mut s = arena
        .effective_style(slot)
        .to_taffy(arena.node_type(slot), arena.env_for(slot));
    arena.alert_layout(slot, &mut s);
    // HTML's button layout, which Exact's reset of a `<button>` keeps: its
    // automatic inline size is shrink-to-fit, and a block button's content
    // sits in an anonymous flow-root box centred safely in the block axis,
    // whatever `align-content` says (Chrome 154; LLP 1001 §1). A flex or
    // grid button lays out as any flex or grid container.
    if arena.is_button(slot) {
        s.item_is_table = true;
        if s.display == taffy::Display::Block {
            s.align_content = Some(taffy::style::AlignContent::SAFE_CENTER);
        }
    }
    if let Some(height) = arena.neutral_tab_min_height(slot) {
        s.min_size.height = length(height);
    }
    // The page reset makes a checkbox border-box for both `appearance:auto`
    // and `none`. With native appearance Chrome additionally ignores its
    // padding; with `none` the authored padding remains in that border box.
    // A select's native UA default is border-box unless the author overrides
    // it, and the web page's field look makes a date, time or local date-time
    // control one too (LLP 1116.001 D1). Other controls keep the reset's
    // content-box semantics.
    match crate::ControlKind::of(arena.node_type(slot), arena.props(slot)) {
        Some(crate::ControlKind::Checkbox | crate::ControlKind::Switch) => {
            s.box_sizing = taffy::style::BoxSizing::BorderBox;
            if arena.style(slot).appearance == crate::Appearance::Auto {
                s.padding = taffy::geometry::Rect {
                    top: length(0.0),
                    right: length(0.0),
                    bottom: length(0.0),
                    left: length(0.0),
                };
            }
        }
        Some(
            crate::ControlKind::Select
            | crate::ControlKind::Date
            | crate::ControlKind::Time
            | crate::ControlKind::DateTimeLocal,
        ) if arena.style(slot).appearance == crate::Appearance::Auto
            && !arena.style(slot).mask.has(StyleId::BoxSizing) =>
        {
            s.box_sizing = taffy::style::BoxSizing::BorderBox;
        }
        // A determinate `progress` is a block of intrinsic size `10em` by
        // `1em` (LLP 1116 D8; LLP 1115.000 D10.2).
        Some(crate::ControlKind::ProgressBar) => crate::control::progress_box(arena, slot, &mut s),
        // A percentage-wide range is compressible (LLP 1115.000 D10.2).
        Some(crate::ControlKind::Range) => crate::control::range_box(arena, slot, &mut s),
        _ => {}
    }
    let direction = arena.computed_source(slot, StyleId::Direction).direction;
    s.direction = match direction {
        Direction::Ltr => taffy::style::Direction::Ltr,
        Direction::Rtl => taffy::style::Direction::Rtl,
    };
    s.justify_items = justify_items(arena.style(slot).justify_items, direction);
    if let Some(multicol) = crate::fragment::taffy_multicol(arena, slot) {
        s.rare.multicol = Some(multicol);
    }
    // A root with `width: auto` fills what it is offered, as a `<div>` fills
    // the body: CSS's block rule, which Taffy does not apply to a root.
    // Height stays auto — as tall as its content, the page a viewport scrolls.
    // A replaced element keeps its natural ratio under `aspect-ratio: auto`
    // (with or without a fallback ratio, or a degenerate one): CSS sizes an
    // `<img>` with one dimension given from the other by that ratio. Only a
    // plain `<ratio>` overrides it; natural ratios are of the content box.
    if arena.node_type(slot).is_replaced()
        && arena.style(slot).rare.aspect_ratio.defers_to_natural()
    {
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
    crate::kernel::apply_projection(arena, slot, &mut s);
    // @ref LLP 1075.003 §3.5 — what a native container covers of the box.
    crate::kernel::apply_cover(arena, slot, &mut s);
    // A grouped list's card and cell margins, as the platform's list has them.
    crate::grouped::apply_margins(arena, slot, &mut s);
    crate::rows::apply_margins(arena, slot, &mut s);
    crate::replaced::apply_zoom(arena, slot, &mut s);
    if direction == Direction::Rtl {
        sides::mirror_unsaid(arena.style(slot), &mut s);
    }
    // @ref LLP 1115.002 D8.2 — the containing block a host gave the subtree.
    crate::kernel::apply_block(arena, slot, &mut s);
    // @ref LLP 1074 T1 — a root is the containing block of every absolutely
    // positioned box no positioned ancestor holds, on every host: a static
    // root is `relative` (its insets apply, as on the web's root element).
    if arena.is_root(slot) && s.position == taffy::style::Position::Static {
        s.position = taffy::style::Position::Relative;
    }
    s
}

#[cfg(test)]
mod finite_tests;
#[cfg(test)]
mod tests;
pub(crate) mod top_layer;
// A `touch-action` value's pinch and pan parts.
mod decoration;
#[cfg(test)]
mod decoration_tests;
mod touch_action;
#[cfg(test)]
mod touch_action_tests;
