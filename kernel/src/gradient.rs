//! CSS `background-image`: `none`, or one `linear-gradient()` or
//! `radial-gradient()` (CSS Images 3 §3). @ref LLP 1066
//!
//! Stops are resolved to percentages when parsed (CSS's fix-up), so the row
//! holds what every host paints and `css()` is canonical. Geometry depends on
//! the box, so it is resolved by the host at paint time through
//! [`Gradient::geometry`]; a colour may be a `light-dark()` pair, resolved per
//! appearance like any colour row (LLP 1034).

use crate::style::{Color, ColorValue};
use std::fmt::Write as _;

/// Most stops one gradient takes: a bound on what crosses to a native host.
pub const MAX_STOPS: usize = 64;

/// The row: `none` (the initial value) or one gradient.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackgroundImage(Option<Gradient>);

/// One gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Linear or radial, and how it is placed in the box.
    pub kind: GradientKind,
    /// At least two; positions are percentages 0–100, nondecreasing.
    pub stops: Vec<Stop>,
}

/// A colour stop, positioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// The colour, possibly a `light-dark()` pair.
    pub color: ColorValue,
    /// Percent of the gradient line (linear) or ray (radial), 0–100.
    pub at: f32,
}

/// The gradient's shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientKind {
    /// `linear-gradient()`: CSS degrees (0 is up, clockwise), or the magic
    /// corner of `to <corner>`, whose angle depends on the box.
    Linear(Direction),
    /// `radial-gradient()`: a circle or an ellipse sized by an extent
    /// keyword, centred at a position in the box.
    Radial {
        /// `circle`, else `ellipse`.
        circle: bool,
        /// How far the ending shape reaches.
        extent: Extent,
        /// Its centre: horizontal, vertical.
        at: [Length; 2],
    },
}

/// A linear gradient's direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// Degrees; `to <side>` is one of 0, 90, 180, 270.
    Angle(f32),
    /// `to <corner>`: right (else left), bottom (else top).
    Corner {
        /// `right`, else `left`.
        right: bool,
        /// `bottom`, else `top`.
        bottom: bool,
    },
}

/// CSS `<radial-extent>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// `closest-side`.
    ClosestSide,
    /// `closest-corner`.
    ClosestCorner,
    /// `farthest-side`.
    FarthestSide,
    /// `farthest-corner`, the initial value.
    FarthestCorner,
}

/// A `<length-percentage>` of a position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// CSS pixels.
    Px(f32),
    /// Percent of the box's side, 0–100 (keywords are 0, 50, 100).
    Percent(f32),
}

impl Length {
    fn resolve(self, side: f32) -> f32 {
        match self {
            Length::Px(px) => px,
            Length::Percent(p) => side * p / 100.0,
        }
    }
}

/// Where a gradient sits in a box of a given size, in the box's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Geometry {
    /// Stop 0% at `start`, 100% at `end`; colours extend past both.
    Linear {
        /// The 0% point.
        start: (f32, f32),
        /// The 100% point.
        end: (f32, f32),
    },
    /// Stop 0% at `center`, 100% on the ellipse of `radii`; the last colour
    /// fills beyond it.
    Radial {
        /// The centre.
        center: (f32, f32),
        /// Horizontal and vertical radii (equal for a circle).
        radii: (f32, f32),
    },
}

/// The grammar, once linked ([`link`]). Only a wasm artifact reads it.
#[cfg(target_arch = "wasm32")]
static LINKED: std::sync::OnceLock<fn(&str) -> Option<BackgroundImage>> =
    std::sync::OnceLock::new();

/// Link `background-image`'s gradient grammar (LLP 1047 D2, linked by use):
/// ~3 KiB of a web core that none of Caltrain, RealWorld or the video player
/// paints with. A web artifact links it when its plan binds the row, and a
/// plan that binds it unlinked is refused at boot (D6), so an unlinked
/// artifact never parses one. Native artifacts and the compiler parse
/// without it.
pub fn link() {
    #[cfg(target_arch = "wasm32")]
    let _ = LINKED.set(|css| BackgroundImage::check(css).ok());
}

impl BackgroundImage {
    /// The row's parse, as every codec's: `None` for anything not drawn.
    /// On the web, once linked ([`link`]).
    pub fn parse(css: &str) -> Option<Self> {
        #[cfg(target_arch = "wasm32")]
        return LINKED.get().and_then(|parse| parse(css));
        #[cfg(not(target_arch = "wasm32"))]
        Self::check(css).ok()
    }

    /// The parse with the reason a value is refused, for the compiler to say.
    pub fn check(css: &str) -> Result<Self, &'static str> {
        let css = css.trim();
        if css.eq_ignore_ascii_case("none") {
            return Ok(Self(None));
        }
        let lower = css.to_ascii_lowercase();
        for (prefix, why) in REFUSED {
            if lower.starts_with(prefix) {
                return Err(why);
            }
        }
        let open = css.find('(').ok_or(EXPECTED)?;
        let close = matching(css, open).ok_or(EXPECTED)?;
        if !css[close + 1..].trim().is_empty() {
            return Err(if css[close + 1..].trim_start().starts_with(',') {
                "one gradient per box: several background layers are not implemented"
            } else {
                EXPECTED
            });
        }
        let args = split_top(&css[open + 1..close], ',');
        let radial = match lower[..open].trim_end() {
            "linear-gradient" => false,
            "radial-gradient" => true,
            _ => return Err(EXPECTED),
        };
        let (kind, first) = if radial {
            radial_prelude(args[0])?
        } else {
            linear_prelude(args[0])?
        };
        let stops = stops(&args[usize::from(first)..])?;
        Ok(Self(Some(Gradient { kind, stops })))
    }

    /// The gradient, or `None` for `none`.
    pub fn gradient(&self) -> Option<&Gradient> {
        self.0.as_ref()
    }

    /// Canonical CSS, also the wire form: stop positions explicit, colours
    /// as `#rrggbbaa` or `light-dark()` of two.
    pub fn css(&self) -> String {
        let Some(g) = &self.0 else {
            return "none".into();
        };
        let mut out = String::new();
        match g.kind {
            GradientKind::Linear(Direction::Angle(deg)) => {
                let _ = write!(out, "linear-gradient({}deg", exact_num::Shortest32(deg));
            }
            GradientKind::Linear(Direction::Corner { right, bottom }) => {
                let _ = write!(
                    out,
                    "linear-gradient(to {} {}",
                    if bottom { "bottom" } else { "top" },
                    if right { "right" } else { "left" }
                );
            }
            GradientKind::Radial { circle, extent, at } => {
                let _ = write!(
                    out,
                    "radial-gradient({} {} at",
                    if circle { "circle" } else { "ellipse" },
                    EXTENTS[extent as usize].0
                );
                for length in at {
                    let _ = match length {
                        Length::Px(n) => write!(out, " {}px", exact_num::Shortest32(n)),
                        Length::Percent(n) => write!(out, " {}%", exact_num::Shortest32(n)),
                    };
                }
            }
        }
        for stop in &g.stops {
            out.push_str(", ");
            match stop.color {
                ColorValue::Fixed(c) => hex(&mut out, c),
                ColorValue::LightDark(light, dark) => {
                    out.push_str("light-dark(");
                    hex(&mut out, light);
                    out.push_str(", ");
                    hex(&mut out, dark);
                    out.push(')');
                }
            }
            let _ = write!(out, " {}%", exact_num::Shortest32(stop.at));
        }
        out.push(')');
        out
    }
}

impl Gradient {
    /// Whether any stop is a `light-dark()` pair: what says an appearance
    /// change must repaint it.
    pub fn is_scheme_aware(&self) -> bool {
        self.stops.iter().any(|s| s.color.is_scheme_aware())
    }

    /// The stops under an appearance, positions as fractions 0–1, the
    /// first at 0 and the last at 1: the end colours repeated where CSS
    /// extends them, so no painter has to (Vello's ramp starts its first
    /// stretch at 0 whatever the first stop's offset).
    pub fn resolved(&self, dark: bool) -> Vec<(f32, Color)> {
        let mut out: Vec<(f32, Color)> = self
            .stops
            .iter()
            .map(|s| (s.at / 100.0, s.color.resolve(dark)))
            .collect();
        if let Some(&(_, c)) = out.first().filter(|s| s.0 > 0.0) {
            out.insert(0, (0.0, c));
        }
        if let Some(&(_, c)) = out.last().filter(|s| s.0 < 1.0) {
            out.push((1.0, c));
        }
        out
    }

    /// Placement in a `width` × `height` box — CSS's gradient box, the
    /// padding box under the initial `background-origin`.
    pub fn geometry(&self, width: f32, height: f32) -> Geometry {
        let (w, h) = (width.max(0.0), height.max(0.0));
        match self.kind {
            GradientKind::Linear(direction) => {
                // The corner's angle makes the 50% line join the other two
                // corners (CSS Images 3 §3.1.1).
                let (sin, cos) = match direction {
                    Direction::Angle(deg) => deg.to_radians().sin_cos(),
                    Direction::Corner { right, bottom } => {
                        let (x, y) = (if right { h } else { -h }, if bottom { w } else { -w });
                        let n = x.hypot(y);
                        if n == 0.0 {
                            (0.0, 1.0)
                        } else {
                            (x / n, -y / n)
                        }
                    }
                };
                // The line through the centre, long enough that its ends'
                // perpendiculars touch the box's corners.
                let half = (w * sin.abs() + h * cos.abs()) / 2.0;
                let (cx, cy) = (w / 2.0, h / 2.0);
                Geometry::Linear {
                    start: (cx - sin * half, cy + cos * half),
                    end: (cx + sin * half, cy - cos * half),
                }
            }
            GradientKind::Radial { circle, extent, at } => {
                let (cx, cy) = (at[0].resolve(w), at[1].resolve(h));
                let (dx, dy) = ((cx.abs(), (w - cx).abs()), (cy.abs(), (h - cy).abs()));
                let near = (dx.0.min(dx.1), dy.0.min(dy.1));
                let far = (dx.0.max(dx.1), dy.0.max(dy.1));
                let radii = match (circle, extent) {
                    (true, Extent::ClosestSide) => square(near.0.min(near.1)),
                    (true, Extent::FarthestSide) => square(far.0.max(far.1)),
                    (true, Extent::ClosestCorner) => square(corner(&dx, &dy, f32::min)),
                    (true, Extent::FarthestCorner) => square(corner(&dx, &dy, f32::max)),
                    (false, Extent::ClosestSide) => near,
                    (false, Extent::FarthestSide) => far,
                    // Through the corner, with the ratio the side form has.
                    (false, Extent::ClosestCorner) => scaled(near),
                    (false, Extent::FarthestCorner) => scaled(far),
                };
                Geometry::Radial {
                    center: (cx, cy),
                    radii,
                }
            }
        }
    }
}

/// Stops for an interpolator that mixes unpremultiplied colour (Core
/// Graphics, Core Animation, tiny-skia), made to look as CSS's premultiplied
/// mix does: a transparent stop takes each neighbour's hue on that side, so
/// `transparent → white` never passes through grey; a stretch between two
/// different partial alphas is sampled in premultiplied space.
pub fn premultiplied_ramp(stops: &[(f32, Color)]) -> Vec<(f32, Color)> {
    const STEPS: usize = 8;
    let rgb = |c: Color, a: u8| Color::rgba(c.r(), c.g(), c.b(), a);
    let mut out = Vec::with_capacity(stops.len() * 2);
    for (i, &(at, c)) in stops.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| stops[p].1);
        let next = stops.get(i + 1).map(|n| n.1);
        if c.a() == 0 {
            let left = prev.or(next).map_or(c, |n| rgb(n, 0));
            let right = next.map_or(left, |n| rgb(n, 0));
            out.push((at, left));
            if right != left {
                out.push((at, right));
            }
        } else {
            out.push((at, c));
        }
        let Some(&(end, n)) = stops.get(i + 1) else {
            continue;
        };
        if c.a() == 0 || n.a() == 0 || c.a() == n.a() || rgb(c, 0) == rgb(n, 0) {
            continue;
        }
        for k in 1..STEPS {
            let t = k as f32 / STEPS as f32;
            let a = c.a() as f32 + (n.a() as f32 - c.a() as f32) * t;
            let channel = |x: u8, y: u8| {
                let premul = x as f32 * c.a() as f32 * (1.0 - t) + y as f32 * n.a() as f32 * t;
                (premul / a).round().clamp(0.0, 255.0) as u8
            };
            out.push((
                at + (end - at) * t,
                Color::rgba(
                    channel(c.r(), n.r()),
                    channel(c.g(), n.g()),
                    channel(c.b(), n.b()),
                    a.round() as u8,
                ),
            ));
        }
    }
    out
}

const EXPECTED: &str =
    "expected none, linear-gradient(…) or radial-gradient(…) with at least two colour stops";

const REFUSED: [(&str, &str); 7] = [
    (
        "repeating-linear-gradient(",
        "repeating-linear-gradient() is not implemented; a gradient paints once",
    ),
    (
        "repeating-radial-gradient(",
        "repeating-radial-gradient() is not implemented; a gradient paints once",
    ),
    (
        "repeating-conic-gradient(",
        "conic gradients are not implemented; use linear-gradient() or radial-gradient()",
    ),
    (
        "conic-gradient(",
        "conic gradients are not implemented; use linear-gradient() or radial-gradient()",
    ),
    (
        "url(",
        "an image as a background is not implemented; `background-image` takes a gradient",
    ),
    (
        "image-set(",
        "image-set() is not implemented; `background-image` takes a gradient",
    ),
    (
        "cross-fade(",
        "cross-fade() is not implemented; `background-image` takes a gradient",
    ),
];

const EXTENTS: [(&str, Extent); 4] = [
    ("closest-side", Extent::ClosestSide),
    ("closest-corner", Extent::ClosestCorner),
    ("farthest-side", Extent::FarthestSide),
    ("farthest-corner", Extent::FarthestCorner),
];

fn square(r: f32) -> (f32, f32) {
    (r, r)
}

fn scaled((x, y): (f32, f32)) -> (f32, f32) {
    (x * std::f32::consts::SQRT_2, y * std::f32::consts::SQRT_2)
}

fn corner(dx: &(f32, f32), dy: &(f32, f32), pick: fn(f32, f32) -> f32) -> f32 {
    let d = [
        dx.0.hypot(dy.0),
        dx.1.hypot(dy.0),
        dx.0.hypot(dy.1),
        dx.1.hypot(dy.1),
    ];
    d.into_iter().reduce(pick).unwrap_or(0.0)
}

fn hex(out: &mut String, c: Color) {
    let _ = write!(out, "#{:08x}", c.0);
}

/// The byte index of the `)` closing the `(` at `open`.
fn matching(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split at `sep` outside parentheses, pieces trimmed; whitespace splits
/// drop empty pieces.
fn split_top(text: &str, sep: char) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if depth == 0 && (c == sep || (sep == ' ' && c.is_ascii_whitespace())) => {
                out.push(text[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    if sep == ' ' {
        out.retain(|s| !s.is_empty());
    }
    out
}

fn angle(token: &str) -> Option<f32> {
    let lower = token.to_ascii_lowercase();
    let (number, per_degree) = [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f32::consts::PI),
        ("turn", 360.0),
    ]
    .into_iter()
    .find_map(|(unit, scale)| lower.strip_suffix(unit).map(|n| (n, scale)))
    .unwrap_or((lower.as_str(), 0.0));
    let n = exact_num::parse_f32(number).ok()?;
    // A unitless angle is only zero.
    let deg = if per_degree == 0.0 {
        (n == 0.0).then_some(0.0)?
    } else {
        n * per_degree
    };
    deg.is_finite().then_some(deg)
}

/// A linear gradient's direction, and whether the first argument was one.
fn linear_prelude(first: &str) -> Result<(GradientKind, bool), &'static str> {
    let words = split_top(first, ' ');
    if words.first().is_some_and(|w| w.eq_ignore_ascii_case("to")) {
        let (mut x, mut y) = (None, None);
        for w in &words[1..] {
            match w.to_ascii_lowercase().as_str() {
                "left" if x.is_none() => x = Some(false),
                "right" if x.is_none() => x = Some(true),
                "top" if y.is_none() => y = Some(false),
                "bottom" if y.is_none() => y = Some(true),
                _ => return Err(
                    "a direction is `to` and a side or corner, as `to top` or `to bottom right`",
                ),
            }
        }
        let direction =
            match (x, y) {
                (Some(right), Some(bottom)) => Direction::Corner { right, bottom },
                (Some(right), None) => Direction::Angle(if right { 90.0 } else { 270.0 }),
                (None, Some(bottom)) => Direction::Angle(if bottom { 180.0 } else { 0.0 }),
                (None, None) => return Err(
                    "a direction is `to` and a side or corner, as `to top` or `to bottom right`",
                ),
            };
        return Ok((GradientKind::Linear(direction), true));
    }
    match (words.len(), words.first().and_then(|w| angle(w))) {
        (1, Some(deg)) => Ok((GradientKind::Linear(Direction::Angle(deg)), true)),
        _ => Ok((GradientKind::Linear(Direction::Angle(180.0)), false)),
    }
}

/// A radial gradient's shape, extent and centre, and whether the first
/// argument was them.
fn radial_prelude(first: &str) -> Result<(GradientKind, bool), &'static str> {
    let words = split_top(first, ' ');
    let keyword = |w: &str| {
        let w = w.to_ascii_lowercase();
        matches!(w.as_str(), "circle" | "ellipse" | "at") || EXTENTS.iter().any(|(k, _)| *k == w)
    };
    let (mut circle, mut extent, mut at) = (None, None, [Length::Percent(50.0); 2]);
    if !words.first().is_some_and(|w| keyword(w)) {
        let kind = GradientKind::Radial {
            circle: false,
            extent: Extent::FarthestCorner,
            at,
        };
        return Ok((kind, false));
    }
    let mut rest = words.iter();
    while let Some(w) = rest.next() {
        let w = w.to_ascii_lowercase();
        match w.as_str() {
            "circle" | "ellipse" if circle.is_none() => circle = Some(w == "circle"),
            "at" => {
                at = position(rest.as_slice())?;
                break;
            }
            _ => match EXTENTS.iter().find(|(k, _)| *k == w) {
                Some((_, e)) if extent.is_none() => extent = Some(*e),
                _ if w.ends_with("px") || w.ends_with('%') => {
                    return Err("an explicit radial size is not implemented; use circle/ellipse with closest-side, closest-corner, farthest-side or farthest-corner")
                }
                _ => return Err("a radial gradient starts with circle or ellipse, an extent keyword, and `at` a position"),
            },
        }
    }
    let kind = GradientKind::Radial {
        circle: circle.unwrap_or(false),
        extent: extent.unwrap_or(Extent::FarthestCorner),
        at,
    };
    Ok((kind, true))
}

/// CSS `<position>`'s one- and two-value forms.
fn position(words: &[&str]) -> Result<[Length; 2], &'static str> {
    const WHY: &str = "a position is one or two of left/center/right, top/center/bottom, px or %";
    // (horizontal?, vertical?) each keyword may stand for.
    let one = |w: &str| -> Option<(Length, bool, bool)> {
        Some(match w.to_ascii_lowercase().as_str() {
            "left" => (Length::Percent(0.0), true, false),
            "right" => (Length::Percent(100.0), true, false),
            "top" => (Length::Percent(0.0), false, true),
            "bottom" => (Length::Percent(100.0), false, true),
            "center" => (Length::Percent(50.0), true, true),
            other => {
                let length = if let Some(p) = other.strip_suffix('%') {
                    Length::Percent(exact_num::parse_f32(p).ok()?)
                } else if let Some(px) = other.strip_suffix("px") {
                    Length::Px(exact_num::parse_f32(px).ok()?)
                } else {
                    (exact_num::parse_f32(other).ok()? == 0.0).then_some(Length::Px(0.0))?
                };
                match length {
                    Length::Px(n) | Length::Percent(n) if !n.is_finite() => return None,
                    _ => (length, true, true),
                }
            }
        })
    };
    let center = Length::Percent(50.0);
    match words {
        [a] => {
            let (a, x, _) = one(a).ok_or(WHY)?;
            Ok(if x { [a, center] } else { [center, a] })
        }
        [a, b] => {
            let (a, ax, ay) = one(a).ok_or(WHY)?;
            let (b, bx, by) = one(b).ok_or(WHY)?;
            if ax && by {
                Ok([a, b])
            } else if ay && bx {
                Ok([b, a])
            } else {
                Err(WHY)
            }
        }
        _ => Err("a position of three or four values is not implemented; give one or two"),
    }
}

/// Stops with CSS's fix-up (CSS Images 3 §3.5.3): the first defaults to
/// 0%, the last to 100%, a position before an earlier one moves up to it,
/// and the rest share their gaps evenly.
fn stops(args: &[&str]) -> Result<Vec<Stop>, &'static str> {
    let mut authored: Vec<(ColorValue, Option<f32>)> = Vec::new();
    for arg in args {
        let words = split_top(arg, ' ');
        let mut split = words.len();
        while split > 0 && words.len() - split < 2 && is_position(words[split - 1]) {
            split -= 1;
        }
        if split == 0 {
            return Err("colour hints (a lone position between stops) are not implemented");
        }
        let color = words[..split].join(" ");
        let color = ColorValue::parse_light_dark(&color)
            .or_else(|| Color::parse(&color).map(ColorValue::Fixed))
            .ok_or("a stop's colour is `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `transparent` or `light-dark(a, b)`")?;
        if split == words.len() {
            authored.push((color, None));
        }
        for w in &words[split..] {
            let n = w
                .strip_suffix('%')
                .and_then(|p| exact_num::parse_f32(p).ok())
                .ok_or("a stop's position is a percentage; lengths are not implemented")?;
            if !(0.0..=100.0).contains(&n) {
                return Err("a stop's position is from 0% to 100%; positions past the ends are not implemented");
            }
            authored.push((color, Some(n)));
        }
    }
    if authored.len() < 2 {
        return Err("a gradient needs at least two colour stops");
    }
    if authored.len() > MAX_STOPS {
        return Err("a gradient takes at most 64 colour stops");
    }
    let last = authored.len() - 1;
    let mut at: Vec<Option<f32>> = authored.iter().map(|s| s.1).collect();
    at[0] = at[0].or(Some(0.0));
    at[last] = at[last].or(Some(100.0));
    let mut high = 0.0f32;
    for p in at.iter_mut().flatten() {
        high = high.max(*p);
        *p = high;
    }
    let mut i = 0;
    while i < last {
        let from = i;
        i += 1;
        while at[i].is_none() {
            i += 1;
        }
        let (a, b) = (at[from].unwrap_or(0.0), at[i].unwrap_or(100.0));
        let gap = (i - from) as f32;
        for (k, p) in at.iter_mut().enumerate().take(i).skip(from + 1) {
            *p = Some(a + (b - a) * (k - from) as f32 / gap);
        }
    }
    Ok(authored
        .into_iter()
        .zip(at)
        .map(|((color, _), at)| Stop {
            color,
            at: at.unwrap_or(0.0),
        })
        .collect())
}

fn is_position(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let number = lower
        .strip_suffix('%')
        .or_else(|| lower.strip_suffix("px"))
        .unwrap_or(&lower);
    exact_num::parse_f32(number).is_ok()
}

#[cfg(test)]
mod tests;
