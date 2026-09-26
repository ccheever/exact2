//! A box's `background-image` gradient (LLP 1056): captured for one
//! appearance with the box, placed in its padding box (CSS's gradient box
//! under the initial `background-origin`), and filled over the border box
//! with its radii, over the background colour and under the border.

use super::{Rect4, Shape};
use exact_kernel::gradient::Geometry;
use exact_kernel::{Color, StyleProps};

/// A gradient for one appearance: stops resolved, placement still the box's.
pub(super) struct Captured {
    gradient: exact_kernel::gradient::Gradient,
    stops: Vec<(f32, Color)>,
}

/// What a backend fills: placement in the shape's coordinates, and the
/// stops as CSS gives them (fractions 0–1, unexpanded; a backend that mixes
/// unpremultiplied expands them with `premultiplied_ramp`).
#[derive(Debug, Clone, PartialEq)]
pub struct GradientPaint {
    /// Where stop 0 and stop 1 fall.
    pub geometry: Geometry,
    /// Positions and colours.
    pub stops: Vec<(f32, Color)>,
}

impl Captured {
    pub(super) fn capture(style: &StyleProps, dark: bool) -> Option<Captured> {
        let gradient = style.background_image.gradient()?.clone();
        let stops = gradient.resolved(dark);
        Some(Captured { gradient, stops })
    }

    /// The paint for a padding box. A radial gradient whose ending shape has
    /// no area is its last colour everywhere, as CSS draws it.
    pub(super) fn place(&self, padding: Rect4) -> Result<GradientPaint, [u8; 4]> {
        let (x, y, w, h) = padding;
        let geometry = match self.gradient.geometry(w, h) {
            Geometry::Linear { start, end } => Geometry::Linear {
                start: (start.0 + x, start.1 + y),
                end: (end.0 + x, end.1 + y),
            },
            Geometry::Radial { center, radii } => {
                if radii.0 <= 0.0 || radii.1 <= 0.0 {
                    let last = self.stops.last().map_or(Color::TRANSPARENT, |s| s.1);
                    return Err(super::rgba(last));
                }
                Geometry::Radial {
                    center: (center.0 + x, center.1 + y),
                    radii,
                }
            }
        };
        Ok(GradientPaint {
            geometry,
            stops: self.stops.clone(),
        })
    }
}

/// Fill `shape` with the gradient through a backend's two primitives.
pub(super) fn paint(
    captured: &Captured,
    outer: &Shape,
    widths: [f32; 4],
    backend: &mut dyn super::Backend,
    ts: tiny_skia::Transform,
) {
    let (x, y, w, h) = outer.rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let padding = (
        x + widths[3],
        y + widths[0],
        (w - widths[1] - widths[3]).max(0.0),
        (h - widths[0] - widths[2]).max(0.0),
    );
    match captured.place(padding) {
        Ok(gradient) => backend.fill_gradient(outer, &gradient, ts),
        Err(color) => backend.fill(outer, color, ts),
    }
}
