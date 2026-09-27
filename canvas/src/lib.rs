//! Canvas 2D for exact2 (LLP 1056): Core Graphics' model by the web's name.
//!
//! @ref LLP 1056 D1 (the data module draws), D3 (the recorder), D4 (the
//! protocol's frame), D6 (coordinates)
//!
//! - [`context`]: the recorder, [`Context2d`], with web-sys's method names.
//! - [`list`]: the bytes every host replays, and the structural check.
//! - [`color`]: CSS Color 4's sRGB forms and the canvas serialisation.
//! - [`geom`]: the author matrix and f64 arc, `arcTo` and `roundRect`
//!   geometry, resolved at the call.
//!
//! A leaf: it depends on nothing, so the runner and every host can reach it.
//! An app's artifact carries it only if its data module draws.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod color;
pub mod context;
pub mod geom;
pub mod list;

pub use color::Rgba;
pub use context::{CanvasGradient, CanvasWindingRule, Context2d, DomException, DomMatrix, Style};
pub use geom::{Matrix, Radius};

/// `globalCompositeOperation`'s values; the list's `Composite` operand is
/// the index.
pub const COMPOSITE: [&str; 26] = [
    "source-over",
    "source-in",
    "source-out",
    "source-atop",
    "destination-over",
    "destination-in",
    "destination-out",
    "destination-atop",
    "lighter",
    "copy",
    "xor",
    "multiply",
    "screen",
    "overlay",
    "darken",
    "lighten",
    "color-dodge",
    "color-burn",
    "hard-light",
    "soft-light",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity",
];

/// Whether stage 1 draws operator `k`: every one that changes only covered
/// pixels. The five that reach outside the shape, within the clip
/// (`source-in`, `source-out`, `destination-in`, `destination-atop`,
/// `copy`), are stage 2 (LLP 1056 §3).
pub fn composite_supported(k: usize) -> bool {
    !matches!(k, 1 | 2 | 5 | 7 | 9) && k < COMPOSITE.len()
}

/// Why a draw was requested (LLP 1056 D4). Requests coalesce before they
/// run, so one draw can have several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Causes(pub u8);

impl Causes {
    /// The node was created.
    pub const MOUNT: Causes = Causes(1);
    /// Its surface arguments changed in an accepted commit.
    pub const ARGS: Causes = Causes(2);
    /// A new size generation.
    pub const SIZE: Causes = Causes(4);
    /// It asked for another frame, and the host presented one.
    pub const FRAME: Causes = Causes(8);
    /// An image it named decoded (stage 2).
    pub const IMAGE: Causes = Causes(16);
    /// A font it used became available (stage 2).
    pub const FONT: Causes = Causes(32);

    const NAMES: [&'static str; 6] = ["mount", "args", "size", "frame", "image", "font"];

    /// Whether `other`'s causes are all present.
    pub fn has(self, other: Causes) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    /// The union.
    pub fn with(self, other: Causes) -> Causes {
        Causes(self.0 | other.0)
    }

    /// Whether there are none.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// `frame.cause`: the first of mount, args, size, frame, image, font
    /// present.
    pub fn primary(self) -> &'static str {
        (0..6)
            .find(|i| self.0 & (1 << i) != 0)
            .map_or("frame", |i| Self::NAMES[i])
    }

    /// Every cause present, by name.
    pub fn names(self) -> Vec<&'static str> {
        (0..6)
            .filter(|i| self.0 & (1 << i) != 0)
            .map(|i| Self::NAMES[i])
            .collect()
    }
}

/// What one draw is told (LLP 1056 D1, D4, D5, D6). Coordinates are CSS px
/// of the content box by default; with an explicit bitmap size they are the
/// bitmap's pixels and `scale` is 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Frame {
    /// The presentable clock, ms: the agent's under an agent-owned clock.
    pub time: f64,
    /// When this canvas node was created, on the same clock.
    pub mounted: f64,
    /// Why this draw runs.
    pub cause: Causes,
    /// The coordinate space's width.
    pub width: f64,
    /// The coordinate space's height.
    pub height: f64,
    /// The backing store's width in pixels.
    pub pixel_width: u32,
    /// The backing store's height in pixels.
    pub pixel_height: u32,
    /// Backing pixels per coordinate unit.
    pub scale: f64,
}

/// A draw that failed: a thrown DOM exception, or the author's own error.
/// Either way the calls made before it are kept (LLP 1056 D4, r3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawError {
    /// A canvas method threw.
    Dom(DomException),
    /// The draw's own error.
    Message(String),
}

impl From<DomException> for DrawError {
    fn from(e: DomException) -> Self {
        DrawError::Dom(e)
    }
}

impl From<String> for DrawError {
    fn from(e: String) -> Self {
        DrawError::Message(e)
    }
}

impl From<&str> for DrawError {
    fn from(e: &str) -> Self {
        DrawError::Message(e.into())
    }
}

impl std::fmt::Display for DrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawError::Dom(e) => e.fmt(f),
            DrawError::Message(m) => f.write_str(m),
        }
    }
}
