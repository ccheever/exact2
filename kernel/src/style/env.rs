//! The page's environment: what CSS's `env()` lengths resolve to — the four
//! safe-area insets (LLP 1001 §2), on a foldable the viewport segments
//! (LLP 1078 D3; CSS-ENV-1 §2.3), and on a desktop window the title-bar
//! area (Window Controls Overlay). The host sets them with the viewport; the
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

/// A layout region a native host reports beside the safe area (LLP 1123
/// D10): UIKit's and AppKit's `LayoutRegion`s, each an inset per edge from
/// the layout viewport, which `env(-exact-<region>-inset-<edge>)` names.
/// The `Corners` regions are the same region with the host's corner
/// adaptation: left and right adapt on the horizontal axis, top and bottom
/// on the vertical one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Region {
    /// `-exact-safe-area-corners-inset-`: the safe area kept clear of the
    /// display's or window's corners.
    SafeAreaCorners = 0,
    /// `-exact-layout-margins-inset-`: the platform's layout margins, at
    /// least the safe area.
    LayoutMargins = 1,
    /// `-exact-layout-margins-corners-inset-`: the layout margins kept clear
    /// of the corners.
    LayoutMarginsCorners = 2,
    /// `-exact-readable-content-inset-`: a width comfortable for reading,
    /// inside the margins.
    ReadableContent = 3,
    /// `-exact-readable-content-corners-inset-`: the readable content kept
    /// clear of the corners.
    ReadableContentCorners = 4,
}

impl Region {
    /// Every region, in wire order.
    pub const ALL: [Region; 5] = [
        Region::SafeAreaCorners,
        Region::LayoutMargins,
        Region::LayoutMarginsCorners,
        Region::ReadableContent,
        Region::ReadableContentCorners,
    ];

    /// The variable's name before its edge: `-exact-layout-margins-inset-`.
    pub fn prefix(self) -> &'static str {
        match self {
            Region::SafeAreaCorners => "-exact-safe-area-corners-inset-",
            Region::LayoutMargins => "-exact-layout-margins-inset-",
            Region::LayoutMarginsCorners => "-exact-layout-margins-corners-inset-",
            Region::ReadableContent => "-exact-readable-content-inset-",
            Region::ReadableContentCorners => "-exact-readable-content-corners-inset-",
        }
    }

    /// The region by wire index (0–4).
    pub fn from_index(i: u8) -> Option<Region> {
        Region::ALL.get(i as usize).copied()
    }

    /// The region and edge a variable's name names:
    /// `-exact-layout-margins-inset-left`.
    pub fn from_name(name: &str) -> Option<(Region, Edge)> {
        Region::ALL.iter().find_map(|r| {
            name.strip_prefix(r.prefix())
                .and_then(Edge::from_name)
                .map(|e| (*r, e))
        })
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

/// A title-bar area variable (Window Controls Overlay §4.1,
/// <https://wicg.github.io/window-controls-overlay/>): one side of the
/// rectangle of the title bar an app may draw in, beside the window's
/// controls, which `env(titlebar-area-<var>)` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TitleBarVar {
    /// `titlebar-area-x`: the left edge, from the viewport's left.
    X = 0,
    /// `titlebar-area-y`: the top edge, from the viewport's top.
    Y = 1,
    /// `titlebar-area-width`.
    Width = 2,
    /// `titlebar-area-height`.
    Height = 3,
}

impl TitleBarVar {
    /// Every variable, in wire order.
    pub const ALL: [TitleBarVar; 4] = [
        TitleBarVar::X,
        TitleBarVar::Y,
        TitleBarVar::Width,
        TitleBarVar::Height,
    ];
    /// The names' common start.
    pub const PREFIX: &'static str = "titlebar-area-";

    /// The name's tail: `x`, `y`, `width`, `height`.
    pub fn name(self) -> &'static str {
        match self {
            TitleBarVar::X => "x",
            TitleBarVar::Y => "y",
            TitleBarVar::Width => "width",
            TitleBarVar::Height => "height",
        }
    }

    /// The variable by its whole name, `titlebar-area-x`.
    pub fn from_name(name: &str) -> Option<TitleBarVar> {
        let tail = name.strip_prefix(Self::PREFIX)?;
        TitleBarVar::ALL.iter().copied().find(|v| v.name() == tail)
    }

    /// The variable by wire index (0–3).
    pub fn from_index(i: u8) -> Option<TitleBarVar> {
        TitleBarVar::ALL.get(i as usize).copied()
    }

    /// The side of the area this variable names.
    pub fn of(self, rect: &Rect) -> f32 {
        match self {
            TitleBarVar::X => rect.x,
            TitleBarVar::Y => rect.y,
            TitleBarVar::Width => rect.width,
            TitleBarVar::Height => rect.height,
        }
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
    /// Device pixels per point when borders occupy whole device pixels.
    pub border_scale: Option<f32>,
    /// A terminal's space rule (LLP 1101 §4, LLP 1121 stage 11 (c)): a
    /// padding, margin or gap in points resolves to whole cells on its axis,
    /// rows down and columns across, to the nearest with a tie toward zero.
    /// Set by the terminal host on its own kernel; no other host sees it.
    pub cell_spaces: bool,
    /// Platform control fonts (LLP 1104 D4). None preserves page inheritance.
    pub control_text_styles: Option<crate::ControlTextStyles>,
    /// Optional per-size button fonts. Separate from `ControlTextStyles` so
    /// existing hosts' control-font struct literals remain source compatible.
    pub button_fonts: Option<crate::ButtonFonts>,
    /// The window's own size, whatever is presented in it, on a host that
    /// lays the page out in a smaller viewport while a sheet is up (iOS,
    /// LLP 1075.003 §9.11): every viewport unit resolves against it
    /// everywhere, root and every sheet, as CSS's resolve against the
    /// viewport and never a dialog, so no length follows a sheet's size.
    /// `None`: the layout viewport.
    pub screen: Option<(f32, f32)>,
    /// The layout regions' insets (LLP 1123 D10), indexed by [`Region`],
    /// each top right bottom left, from the layout viewport. `None` on a
    /// host that reports none: every region is then the safe area.
    pub regions: Option<[[f32; 4]; 5]>,
    /// The title-bar area beside the window's controls, in the layout
    /// viewport's points (Window Controls Overlay): `None` where the window
    /// has no such area to give, where every `titlebar-area-*` is undefined
    /// and a fallback applies, as in a browser tab; all zero while the area
    /// is hidden (a window in full screen, or one whose toolbar fills the
    /// title bar).
    pub titlebar_area: Option<Rect>,
    /// The values of the content owner a node lies in (LLP 1123 D9), for the
    /// environment a node's rows resolve against; `None` for the document's
    /// own, outside every owner. `env()` never reads them.
    pub owner: Option<std::sync::Arc<super::owner::OwnerValues>>,
    /// The width and height of the nearest content owner's content root,
    /// the container units' basis (LLP 1123 D12); `None` outside every
    /// owner. Unlike `owner` it holds through a top-layer root, as CSS keeps
    /// a dialog's query container.
    pub container: Option<(f32, f32)>,
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
            border_scale: None,
            cell_spaces: false,
            control_text_styles: None,
            button_fonts: None,
            screen: None,
            regions: None,
            titlebar_area: None,
            owner: None,
            container: None,
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

    /// A layout region's inset at an edge: the host's, else the safe area's
    /// (LLP 1123 D10).
    pub fn region_inset(&self, region: Region, edge: Edge) -> f32 {
        self.regions
            .map_or_else(|| self.inset(edge), |r| r[region as usize][edge as usize])
    }

    /// Whether every inset and every segment side is a finite number.
    pub fn is_finite(&self) -> bool {
        self.button_fonts
            .as_ref()
            .is_none_or(crate::ButtonFonts::is_valid)
            && Edge::ALL.iter().all(|e| self.inset(*e).is_finite())
            && self
                .regions
                .is_none_or(|r| r.iter().flatten().all(|v| v.is_finite()))
            && self.segments.iter().all(Rect::is_finite)
            && self.titlebar_area.as_ref().is_none_or(Rect::is_finite)
            && self
                .screen
                .is_none_or(|(w, h)| w.is_finite() && h.is_finite() && w >= 0.0 && h >= 0.0)
            && self
                .control_text_styles
                .as_ref()
                .is_none_or(crate::ControlTextStyles::is_valid)
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

    /// This environment with the terminal's space rule on or off.
    pub fn with_cell_spaces(&self, on: bool) -> Env {
        Env {
            cell_spaces: on,
            ..self.clone()
        }
    }

    /// This environment with the window's size the viewport units resolve
    /// against (`None`: the layout viewport).
    pub fn with_screen(&self, screen: Option<(f32, f32)>) -> Env {
        Env {
            screen,
            ..self.clone()
        }
    }

    /// This environment with its layout regions' insets replaced (`None`:
    /// every region is the safe area).
    pub fn with_regions(&self, regions: Option<[[f32; 4]; 5]>) -> Env {
        Env {
            regions,
            ..self.clone()
        }
    }

    /// This environment with its title-bar area replaced (`None`: undefined).
    pub fn with_titlebar_area(&self, area: Option<Rect>) -> Env {
        Env {
            titlebar_area: area,
            ..self.clone()
        }
    }

    /// A title-bar area variable's points, or `None` where the area is
    /// undefined.
    pub fn titlebar(&self, var: TitleBarVar) -> Option<f32> {
        self.titlebar_area.as_ref().map(|r| var.of(r))
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
            EnvRefusal::IndexCount => "env(viewport-segment-*) takes exactly two indices, x then y: env(viewport-segment-width 0 0)",
            EnvRefusal::IndexForm => "a viewport segment index is a non-negative integer",
            EnvRefusal::IndexRange => "a viewport segment index is at most 15",
            EnvRefusal::UnknownVariable => "env() names safe-area-inset-top/right/bottom/left, viewport-segment-width/height/top/left/bottom/right x y, titlebar-area-x/y/width/height, or a layout region's inset, -exact-<region>-inset-top/right/bottom/left for safe-area-corners, layout-margins, layout-margins-corners, readable-content or readable-content-corners",
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
    resolve: fn(SegmentVar, u8, u8, &Env) -> Option<f64>,
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

/// An inset length's points under `env`: a safe-area inset's, or a layout
/// region's (the safe area's on a host that reports none), plus its own.
pub(crate) fn resolve_inset(d: Dimension, env: &Env) -> Dimension {
    let (base, plus) = match d {
        Dimension::Env(edge, plus) => (env.inset(edge), plus),
        Dimension::Region(region, edge, plus) => (env.region_inset(region, edge), plus),
        other => return other,
    };
    Dimension::Points(super::calc::narrow(f64::from(base) + f64::from(plus)))
}

/// A segment length's points under `env`, or `Auto` when its segment is
/// undefined — or when the grammar is not linked (the row then takes its
/// initial value, as it would for an undefined segment).
pub(crate) fn resolve(var: SegmentVar, x: u8, y: u8, plus: f32, env: &Env) -> Dimension {
    resolve_wide(var, x, y, env).map_or(Dimension::Auto, |base| {
        Dimension::Points(super::calc::narrow(base + f64::from(plus)))
    })
}

/// Preserve derived edges through the enclosing calculation.
pub(crate) fn resolve_wide(var: SegmentVar, x: u8, y: u8, env: &Env) -> Option<f64> {
    LINKED.get().and_then(|h| (h.resolve)(var, x, y, env))
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

fn resolve_segment(var: SegmentVar, x: u8, y: u8, env: &Env) -> Option<f64> {
    let rect = env.segment(x, y)?;
    Some(match var {
        SegmentVar::Right => f64::from(rect.x) + f64::from(rect.width),
        SegmentVar::Bottom => f64::from(rect.y) + f64::from(rect.height),
        _ => f64::from(var.of(rect)),
    })
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
        // CSS's integer: digits, a `+` before them allowed
        let s = s.strip_prefix('+').unwrap_or(s);
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
/// D8): CSS Environment Variables 1's, Window Controls Overlay's, and
/// Exact's layout regions (LLP 1123 D10), whose names begin `-exact-`.
pub const ENV_NAMES: [(&str, &str); 8] = [
    ("safe-area-inset-", "css CSS Environment Variables 1"),
    ("viewport-segment-", "css CSS Environment Variables 1"),
    ("-exact-safe-area-corners-inset-", "exact LLP 1123 D10"),
    ("-exact-layout-margins-inset-", "exact LLP 1123 D10"),
    ("-exact-layout-margins-corners-inset-", "exact LLP 1123 D10"),
    ("-exact-readable-content-inset-", "exact LLP 1123 D10"),
    (
        "-exact-readable-content-corners-inset-",
        "exact LLP 1123 D10",
    ),
    (TitleBarVar::PREFIX, "css Window Controls Overlay"),
];

pub(super) fn term(inner: &str) -> Result<Dimension, EnvRefusal> {
    let inner = inner.trim();
    let body = inner
        .strip_prefix("env(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or(EnvRefusal::UnknownVariable)?
        .trim();
    let mut words = body.split_whitespace();
    let name = words.next().ok_or(EnvRefusal::UnknownVariable)?;
    named(name, &words.collect::<Vec<_>>())
}

/// A variable by its name, as CSS decodes it, and its indices' text.
pub(super) fn named(name: &str, indices: &[&str]) -> Result<Dimension, EnvRefusal> {
    if let Some(edge) = name.strip_prefix(ENV_NAMES[0].0) {
        let edge = Edge::from_name(edge).ok_or(EnvRefusal::UnknownVariable)?;
        return if indices.is_empty() {
            Ok(Dimension::Env(edge, 0.0))
        } else {
            Err(EnvRefusal::UnknownVariable)
        };
    }
    if let Some(var) = TitleBarVar::from_name(name) {
        return if indices.is_empty() {
            Ok(super::calc::lone_env(super::calc::EnvTerm::TitleBar(var)))
        } else {
            Err(EnvRefusal::UnknownVariable)
        };
    }
    if let Some((region, edge)) = Region::from_name(name) {
        return if indices.is_empty() {
            Ok(Dimension::Region(region, edge, 0.0))
        } else {
            Err(EnvRefusal::UnknownVariable)
        };
    }
    let tail = name
        .strip_prefix(ENV_NAMES[1].0)
        .ok_or(EnvRefusal::UnknownVariable)?;
    let hooks = LINKED.get().ok_or(EnvRefusal::Unlinked)?;
    (hooks.term)(tail, indices)
}

/// A bare `env()` length by CSS's grammar: `env(safe-area-inset-<edge>)`,
/// `env(viewport-segment-<var> <x> <y>)`, `env(titlebar-area-<var>)` (a
/// calculation, since it may be undefined) or a layout region's
/// `env(-exact-<region>-inset-<edge>)` (LLP 1123 D10). `Ok(None)` when the text is not
/// one (a math function, or one with a fallback, is [`super::calc`]'s); `Err`
/// when it names one of the
/// kernel's variables wrongly (LLP 1078 D10).
pub fn parse(text: &str) -> Result<Option<Dimension>, EnvRefusal> {
    let t = text.trim();
    if t.starts_with("env(") {
        return term(t).map(Some);
    }
    Ok(None)
}

/// A calculation's CSS text for a browser, which has no layout regions (LLP
/// 1123 D10): each region's name is the safe area's, which is what the
/// region resolves to on a host that reports none. Names are matched as CSS
/// identifiers, escapes decoded, and the rest of the text is kept as written.
pub fn browser_css(c: super::Comparison, out: &mut String) {
    let mut text = String::new();
    c.css(&mut text);
    if !text.contains("-exact-") && !text.contains('\\') {
        out.push_str(&text);
        return;
    }
    let mut input = cssparser::ParserInput::new(&text);
    rename_regions(&mut cssparser::Parser::new(&mut input), out);
}

fn rename_regions(p: &mut cssparser::Parser<'_, '_>, out: &mut String) {
    use cssparser::Token;
    loop {
        let start = p.position();
        let Ok(token) = p.next_including_whitespace_and_comments().cloned() else {
            return;
        };
        let close = match token {
            Token::Ident(ref name) if Region::from_name(name).is_some() => {
                let (_, edge) = Region::from_name(name).expect("matched");
                out.push_str(ENV_NAMES[0].0);
                out.push_str(edge.name());
                continue;
            }
            Token::Function(_) | Token::ParenthesisBlock => ')',
            Token::SquareBracketBlock => ']',
            Token::CurlyBracketBlock => '}',
            _ => {
                out.push_str(p.slice_from(start));
                continue;
            }
        };
        out.push_str(p.slice_from(start));
        let _ = p.parse_nested_block(|p| {
            rename_regions(p, out);
            Ok::<(), cssparser::ParseError<'_, ()>>(())
        });
        out.push(close);
    }
}

/// Whether a style reads a viewport or environment length.
pub fn uses_env(style: &crate::StyleProps) -> bool {
    style.mask.iter().any(|id| {
        matches!(
            style.get(id),
            crate::RowValue::Dimension(
                Dimension::Env(..)
                    | Dimension::Region(..)
                    | Dimension::Segment(..)
                    | Dimension::Owner(..)
                    | Dimension::Viewport(..)
                    | Dimension::Compare(..)
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
