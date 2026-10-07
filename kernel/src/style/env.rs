//! The page's environment: what CSS's `env()` lengths resolve to — the four
//! safe-area insets (LLP 1001 §2) and, on a foldable, the viewport segments
//! (LLP 1078 D3; CSS-ENV-1 §2.3). The host sets both with the viewport; the
//! kernel resolves every `env()` length against them where the engine style
//! is derived. Text on a dimension row is parsed here, once, by CSS's grammar.

use super::Dimension;

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

/// A viewport segment variable (CSS-ENV-1 §2.3): which length of segment
/// `(x, y)` an `env(viewport-segment-<var> x y)` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SegmentVar {
    /// `viewport-segment-width`.
    Width = 0,
    /// `viewport-segment-height`.
    Height = 1,
    /// `viewport-segment-top`: the top edge, from the viewport's top.
    Top = 2,
    /// `viewport-segment-left`: the left edge, from the viewport's left.
    Left = 3,
    /// `viewport-segment-bottom`: the bottom edge, from the viewport's top
    /// (a `DOMRect`'s `bottom`, as Chromium resolves it).
    Bottom = 4,
    /// `viewport-segment-right`: the right edge, from the viewport's left.
    Right = 5,
}

impl SegmentVar {
    /// Every variable, in wire order (kinds 8–13).
    pub const ALL: [SegmentVar; 6] = [
        SegmentVar::Width,
        SegmentVar::Height,
        SegmentVar::Top,
        SegmentVar::Left,
        SegmentVar::Bottom,
        SegmentVar::Right,
    ];
    /// The largest index either axis takes (LLP 1078 D3: 15 is plenty).
    pub const MAX_INDEX: u8 = 15;

    /// The CSS name's tail: `width`, `height`, `top`, `left`, `bottom`, `right`.
    pub fn name(self) -> &'static str {
        match self {
            SegmentVar::Width => "width",
            SegmentVar::Height => "height",
            SegmentVar::Top => "top",
            SegmentVar::Left => "left",
            SegmentVar::Bottom => "bottom",
            SegmentVar::Right => "right",
        }
    }

    /// The variable by its name's tail.
    pub fn from_name(name: &str) -> Option<SegmentVar> {
        SegmentVar::ALL.iter().copied().find(|v| v.name() == name)
    }

    /// The variable by wire index (0–5).
    pub fn from_index(i: u8) -> Option<SegmentVar> {
        SegmentVar::ALL.get(i as usize).copied()
    }

    /// The length of a segment this variable names.
    pub fn of(self, rect: &Rect) -> f32 {
        match self {
            SegmentVar::Width => rect.width,
            SegmentVar::Height => rect.height,
            SegmentVar::Top => rect.y,
            SegmentVar::Left => rect.x,
            SegmentVar::Bottom => rect.y + rect.height,
            SegmentVar::Right => rect.x + rect.width,
        }
    }
}

/// One viewport segment, in the layout viewport's points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// The width.
    pub width: f32,
    /// The height.
    pub height: f32,
}

impl Rect {
    /// A rect from its origin and size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether every side is a finite number.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
    }
}

/// The page's environment: what CSS's `env()` lengths resolve to, in points,
/// set by the host with the viewport. The four safe-area insets (a phone's
/// status bar and home indicator under `viewport-fit=cover`; zero everywhere
/// else, as a browser reports them for a page without it), and the viewport
/// segments a fold makes (LLP 1078 D3): `cols × rows` rects, row-major, in
/// the layout viewport's coordinates — none on a viewport with one segment,
/// where CSS-ENV-1 defines no segment variable.
#[derive(Debug, Clone, PartialEq)]
pub struct Env {
    /// `safe-area-inset-top`.
    pub top: f32,
    /// `safe-area-inset-right`.
    pub right: f32,
    /// `safe-area-inset-bottom`.
    pub bottom: f32,
    /// `safe-area-inset-left`.
    pub left: f32,
    /// `horizontal-viewport-segments`: columns of segments, from 1.
    pub cols: u8,
    /// `vertical-viewport-segments`: rows of segments, from 1.
    pub rows: u8,
    /// The segments, row-major, `cols × rows` of them — empty for one segment.
    pub segments: Vec<Rect>,
    /// Layout viewport width in points.
    pub viewport_width: f32,
    /// Layout viewport height in points.
    pub viewport_height: f32,
    /// A terminal's border rule (LLP 1101 §4, LLP 1101.001 P13): a drawn
    /// side occupies one cell. Set by the terminal host on its own kernel;
    /// no other host sees it.
    pub cell_borders: bool,
}

impl Default for Env {
    fn default() -> Env {
        Env::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl Env {
    /// The four insets, top right bottom left; one segment.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Env {
        Env {
            top,
            right,
            bottom,
            left,
            cols: 1,
            rows: 1,
            segments: Vec::new(),
            viewport_width: 0.0,
            viewport_height: 0.0,
            cell_borders: false,
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

    /// Whether every inset and every segment side is a finite number.
    pub fn is_finite(&self) -> bool {
        Edge::ALL.iter().all(|e| self.inset(*e).is_finite())
            && self.segments.iter().all(Rect::is_finite)
    }

    /// Whether the grid is well formed: both counts at least 1, and exactly
    /// `cols × rows` segments — none when that is 1 (CSS-ENV-1 §2.3: the
    /// variables exist only with at least two segments).
    pub fn segments_consistent(&self) -> bool {
        let count = usize::from(self.cols) * usize::from(self.rows);
        self.cols >= 1
            && self.rows >= 1
            && if count == 1 {
                self.segments.is_empty()
            } else {
                self.segments.len() == count
            }
    }

    /// The segment at column `x`, row `y` — `None` on a viewport with one
    /// segment, or past the grid.
    pub fn segment(&self, x: u8, y: u8) -> Option<&Rect> {
        if x >= self.cols || y >= self.rows {
            return None;
        }
        self.segments
            .get(usize::from(y) * usize::from(self.cols) + usize::from(x))
    }

    /// This environment with its segment grid replaced.
    pub fn with_segments(&self, cols: u8, rows: u8, segments: Vec<Rect>) -> Env {
        Env {
            cols,
            rows,
            segments,
            ..self.clone()
        }
    }

    /// This environment with the terminal's border rule on or off.
    pub fn with_cell_borders(&self, on: bool) -> Env {
        Env {
            cell_borders: on,
            ..self.clone()
        }
    }

    /// This environment with its insets replaced, the grid kept.
    pub fn with_insets(&self, top: f32, right: f32, bottom: f32, left: f32) -> Env {
        Env {
            top,
            right,
            bottom,
            left,
            ..self.clone()
        }
    }
}

/// Why an `env()` text that names one of the kernel's variables is not a
/// length (LLP 1078 D10): refused by name at the bake, never silently zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvRefusal {
    /// `env(safe-area-inset-top, 0px)`: the host always defines the insets
    /// and the segment variables take the row's initial value when
    /// undefined; a fallback argument is refused (LLP 1001 §2).
    Fallback,
    /// `viewport-segment-*` takes exactly two indices, `x` then `y`.
    IndexCount,
    /// An index is a non-negative integer.
    IndexForm,
    /// An index is at most 15.
    IndexRange,
    /// Not one of the kernel's variables.
    UnknownVariable,
    /// `viewport-segment-*` is linked by use (LLP 1047 D2): this artifact's
    /// plan never names a segment, so its grammar was not linked.
    Unlinked,
}

impl EnvRefusal {
    /// The reason, as the bake prints it.
    pub fn reason(self) -> &'static str {
        match self {
            EnvRefusal::Fallback => "env() takes no fallback argument here: the host defines safe-area-inset-* and an undefined viewport-segment-* takes the row's initial value",
            EnvRefusal::IndexCount => "env(viewport-segment-*) takes exactly two indices, x then y: env(viewport-segment-width 0 0)",
            EnvRefusal::IndexForm => "a viewport segment index is a non-negative integer",
            EnvRefusal::IndexRange => "a viewport segment index is at most 15",
            EnvRefusal::UnknownVariable => "env() names safe-area-inset-top/right/bottom/left or viewport-segment-width/height/top/left/bottom/right x y",
            EnvRefusal::Unlinked => "env(viewport-segment-*) is not linked into this artifact (LLP 1078 D3; linked by use, LLP 1047 D2)",
        }
    }
}

/// The variable an `env(...)` term names, with its points added (zero).
/// The segment half of the grammar, linked by use (LLP 1047 D2): the
/// `viewport-segment-*` term, a segment length's resolution, its CSS text
/// and its wire decode. Until a host links it (`link`), a text naming a
/// segment is refused as unlinked, a kind 8–13 on the wire is unknown, and
/// a segment length resolves to `auto`; the linker drops what only these
/// reach. The compiler and the native hosts link at start; a web artifact
/// links it when its plan names a segment (`Capability::Segments`).
struct Hooks {
    term: fn(&str, &[&str]) -> Result<Dimension, EnvRefusal>,
    resolve: fn(SegmentVar, u8, u8, f32, &Env) -> Dimension,
    css: fn(SegmentVar, u8, u8, f32, &mut String),
    decode: fn(u8, f32, u8, u8) -> Option<Dimension>,
}

static LINKED: std::sync::OnceLock<Hooks> = std::sync::OnceLock::new();

/// Link the viewport segment grammar, its resolution, CSS text and wire
/// decode (LLP 1078 D3).
pub fn link() {
    let _ = LINKED.set(Hooks {
        term: segment_term,
        resolve: resolve_segment,
        css: segment_css,
        decode: decode_segment,
    });
}

/// Whether the segment grammar is linked.
pub fn linked() -> bool {
    LINKED.get().is_some()
}

/// A segment length's points under `env`, or `Auto` when its segment is
/// undefined — or when the grammar is not linked (the row then takes its
/// initial value, as it would for an undefined segment).
pub(crate) fn resolve(var: SegmentVar, x: u8, y: u8, plus: f32, env: &Env) -> Dimension {
    LINKED
        .get()
        .map_or(Dimension::Auto, |h| (h.resolve)(var, x, y, plus, env))
}

/// A segment length as CSS-ENV-1's text, for a host whose browser resolves
/// it (LLP 1078 D6); nothing when the grammar is not linked.
pub fn css(var: SegmentVar, x: u8, y: u8, plus: f32, out: &mut String) {
    if let Some(h) = LINKED.get() {
        (h.css)(var, x, y, plus, out);
    }
}

/// A wire kind 8–13 as a segment length; `None` when the grammar is not
/// linked (the kind is then unknown to this artifact).
pub(crate) fn decode(kind: u8, plus: f32, x: u8, y: u8) -> Option<Dimension> {
    LINKED.get().and_then(|h| (h.decode)(kind, plus, x, y))
}

fn resolve_segment(var: SegmentVar, x: u8, y: u8, plus: f32, env: &Env) -> Dimension {
    match env.segment(x, y) {
        Some(rect) => Dimension::Points(var.of(rect) + plus),
        None => Dimension::Auto,
    }
}

fn segment_css(var: SegmentVar, x: u8, y: u8, plus: f32, out: &mut String) {
    use std::fmt::Write as _;
    out.push_str(if plus == 0.0 { "env(" } else { "calc(env(" });
    out.push_str("viewport-segment-");
    out.push_str(var.name());
    let _ = write!(out, " {x} {y})");
    if plus != 0.0 {
        out.push_str(if plus < 0.0 { " - " } else { " + " });
        let _ = write!(out, "{}px)", exact_num::Shortest(f64::from(plus.abs())));
    }
}

fn decode_segment(kind: u8, plus: f32, x: u8, y: u8) -> Option<Dimension> {
    let var = SegmentVar::from_index(kind.checked_sub(8)?)?;
    if x > SegmentVar::MAX_INDEX || y > SegmentVar::MAX_INDEX {
        return None;
    }
    Some(Dimension::Segment(var, x, y, plus))
}

/// `viewport-segment-<var> <x> <y>`: the variable's tail and its indices.
fn segment_term(tail: &str, indices: &[&str]) -> Result<Dimension, EnvRefusal> {
    let var = SegmentVar::from_name(tail).ok_or(EnvRefusal::UnknownVariable)?;
    if indices.len() != 2 {
        return Err(EnvRefusal::IndexCount);
    }
    let index = |s: &str| -> Result<u8, EnvRefusal> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(EnvRefusal::IndexForm);
        }
        let n: u32 = s.parse().map_err(|_| EnvRefusal::IndexRange)?;
        if n > u32::from(SegmentVar::MAX_INDEX) {
            return Err(EnvRefusal::IndexRange);
        }
        Ok(n as u8)
    };
    Ok(Dimension::Segment(
        var,
        index(indices[0])?,
        index(indices[1])?,
        0.0,
    ))
}

/// The `env()` variable families `term` reads, each with its kind (LLP 1081
/// D8): both are CSS Environment Variables 1's.
pub const ENV_NAMES: [(&str, &str); 2] = [
    ("safe-area-inset-", "css CSS Environment Variables 1"),
    ("viewport-segment-", "css CSS Environment Variables 1"),
];

fn term(inner: &str) -> Result<Dimension, EnvRefusal> {
    let inner = inner.trim();
    let body = inner
        .strip_prefix("env(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or(EnvRefusal::UnknownVariable)?
        .trim();
    if body.contains(',') {
        return Err(EnvRefusal::Fallback);
    }
    let mut words = body.split_whitespace();
    let name = words.next().ok_or(EnvRefusal::UnknownVariable)?;
    if let Some(edge) = name.strip_prefix(ENV_NAMES[0].0) {
        let edge = Edge::from_name(edge).ok_or(EnvRefusal::UnknownVariable)?;
        return if words.next().is_some() {
            Err(EnvRefusal::UnknownVariable)
        } else {
            Ok(Dimension::Env(edge, 0.0))
        };
    }
    let tail = name
        .strip_prefix(ENV_NAMES[1].0)
        .ok_or(EnvRefusal::UnknownVariable)?;
    let hooks = LINKED.get().ok_or(EnvRefusal::Unlinked)?;
    let indices: Vec<&str> = words.collect();
    (hooks.term)(tail, &indices)
}

/// An `env()` length by CSS's grammar: `env(safe-area-inset-<edge>)`,
/// `env(viewport-segment-<var> <x> <y>)`, or either inside
/// `calc(env(…) ± <n>px)` or `calc(<n>px + env(…))` (addition commutes;
/// `<n>px - env(…)` negates the variable, which no row can hold). `Ok(None)` when the text is not an `env()` form
/// at all (a `calc()` of percent and points, a plain length); `Err` when it
/// names one of the kernel's variables wrongly (LLP 1078 D10).
pub fn parse(text: &str) -> Result<Option<Dimension>, EnvRefusal> {
    let t = text.trim();
    if t.starts_with("env(") {
        return term(t).map(Some);
    }
    let Some(body) = t.strip_prefix("calc(").and_then(|s| s.strip_suffix(')')) else {
        return Ok(None);
    };
    let body = body.trim();
    if !body.starts_with("env(") {
        return leading_length(body);
    }
    // `env(...) ± <n>px`: the operator is the first `+`/`-` after the
    // closing paren of the `env(...)` term.
    let Some(close) = body.find(')') else {
        return Ok(None);
    };
    let (env, rest) = body.split_at(close + 1);
    let dim = term(env)?;
    let rest = rest.trim();
    let (sign, number) = match rest.as_bytes().first() {
        Some(b'+') => (1.0, &rest[1..]),
        Some(b'-') => (-1.0, &rest[1..]),
        _ => return Ok(None),
    };
    let Some(number) = number.trim().strip_suffix("px") else {
        return Ok(None);
    };
    let Ok(plus) = exact_num::parse_f32(number.trim()) else {
        return Ok(None);
    };
    if !plus.is_finite() {
        return Ok(None);
    }
    Ok(Some(match dim {
        Dimension::Env(edge, _) => Dimension::Env(edge, sign * plus),
        Dimension::Segment(var, x, y, _) => Dimension::Segment(var, x, y, sign * plus),
        other => other,
    }))
}

/// `<n>px + env(…)`, a calc body whose length comes first: the same
/// dimension as `env(…) + <n>px`. By CSS's grammar: whitespace on both
/// sides of the `+`, none between the number and `px`. `Ok(None)` for
/// anything else.
fn leading_length(body: &str) -> Result<Option<Dimension>, EnvRefusal> {
    let Some(at) = body.find("env(") else {
        return Ok(None);
    };
    let (head, env) = body.split_at(at);
    let Some(head) = head
        .strip_suffix(|c: char| c.is_ascii_whitespace())
        .map(str::trim_end)
        .and_then(|h| h.strip_suffix('+'))
        .and_then(|h| h.strip_suffix(|c: char| c.is_ascii_whitespace()))
    else {
        return Ok(None);
    };
    let Some(number) = head.trim_end().strip_suffix("px") else {
        return Ok(None);
    };
    let Ok(plus) = exact_num::parse_f32(number) else {
        return Ok(None);
    };
    if !plus.is_finite() || env.find(')') != Some(env.len() - 1) {
        return Ok(None);
    }
    Ok(Some(match term(env)? {
        Dimension::Env(edge, _) => Dimension::Env(edge, plus),
        Dimension::Segment(var, x, y, _) => Dimension::Segment(var, x, y, plus),
        other => other,
    }))
}

/// Whether a style reads a viewport or environment length.
pub fn uses_env(style: &crate::StyleProps) -> bool {
    style.mask.iter().any(|id| {
        matches!(
            style.get(id),
            crate::RowValue::Dimension(
                Dimension::Env(..) | Dimension::Segment(..) | Dimension::Viewport(..)
            )
        )
    })
}

impl Env {
    pub(crate) fn with_viewport(&self, offer: crate::Offer) -> Self {
        use crate::AxisOffer::Definite;
        if !matches!((offer.width, offer.height), (Definite(w), Definite(h)) if w >= 0.0 && h >= 0.0)
        {
            return self.clone();
        }
        Self {
            viewport_width: if let Definite(v) = offer.width {
                v
            } else {
                self.viewport_width
            },
            viewport_height: if let Definite(v) = offer.height {
                v
            } else {
                self.viewport_height
            },
            ..self.clone()
        }
    }
}
