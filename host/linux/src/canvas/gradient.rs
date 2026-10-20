//! Gradients for the reader to shade (`SHADER`, op 43): a box's
//! `background-image` and an SVG fill or stroke ink, each drawn by the next
//! shape op with an Android `Shader` in place of its colour (LLP 1115.003 D2).
//!
//! `[43, len, kind, spread, g×6, m×6, n, (offset, ARGB)×n]`, `len` the words
//! after it. `kind` 0 linear (`g` = x0 y0 x1 y1), 1 two-point radial (`g` =
//! the start circle's x y r, then the end circle's), 2 sweep (`g` = cx cy,
//! clockwise from +x as Android's `SweepGradient`); `spread` 0 pad, 1
//! repeat, 2 reflect (`Shader.TileMode` CLAMP, REPEAT, MIRROR). `m` (a b c d
//! e f, as `MATRIX`) maps gradient space to the shape's coordinates. The
//! next RRECT, PATH or STROKE paints with it and opaque black; its own colour
//! word is that black.
use super::{Recorder, Shape, RRECT};
use crate::paint::{GradientPaint, Ink};
use exact_kernel::gradient::{premultiplied_ramp, Geometry};
use exact_kernel::svg::server::{ServerKind, Spread};
use tiny_skia::Transform;

pub(super) const SHADER: u32 = 43;

/// The colour word of a shaded shape: its shader's colours at full alpha.
pub(super) const SHADED: u32 = 0xFF00_0000;

impl Recorder {
    /// A box gradient over `s`'s rounded rect.
    pub(super) fn gradient_rect(&mut self, s: &Shape, g: &GradientPaint, ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 || g.stops.is_empty() {
            return;
        }
        let bounds = super::clip::map(s.rect, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts);
        // Android, like tiny-skia, mixes stops unpremultiplied; CSS mixes
        // premultiplied (CSS Images 4 §3.5.2).
        let stops: Vec<(f32, [u8; 4])> = premultiplied_ramp(&g.stops)
            .into_iter()
            .map(|(at, c)| (at, crate::paint::rgba(c)))
            .collect();
        let spread = u32::from(g.repeat);
        match g.geometry {
            Geometry::Linear { start, end } => self.shader(
                0,
                spread,
                [start.0, start.1, end.0, end.1, 0.0, 0.0],
                IDENTITY,
                &stops,
            ),
            // The unit circle, scaled to the ellipse; CSS pads a radial.
            Geometry::Radial { center, radii } => self.shader(
                1,
                0,
                [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
                [radii.0, 0.0, 0.0, radii.1, center.0, center.1],
                &stops,
            ),
            // Turned so the sweep starts where CSS's `from` does (0 is up),
            // rather than started there, so it never wraps mid-turn.
            Geometry::Conic { center, from } => {
                let r = Transform::from_rotate_at(from - 90.0, center.0, center.1);
                let m = [r.sx, r.ky, r.kx, r.sy, r.tx, r.ty];
                self.shader(2, 0, [center.0, center.1, 0.0, 0.0, 0.0, 0.0], m, &stops)
            }
        }
        self.ops.extend([RRECT, SHADED]);
        self.rect_radii(s);
    }

    /// Record `ink` for the shape op that follows and return that op's
    /// colour word, or `None` when this host leaves the ink unpainted.
    pub(super) fn ink(&mut self, ink: &Ink<'_>) -> Option<u32> {
        match ink {
            Ink::Solid(c) => Some(Self::color(*c)),
            Ink::Gradient {
                server,
                stops,
                transform,
            } => {
                let spread = match server.spread {
                    Spread::Pad => 0,
                    Spread::Repeat => 1,
                    Spread::Reflect => 2,
                };
                let (kind, g) = match server.kind {
                    ServerKind::Linear { x1, y1, x2, y2 } => (0, [x1, y1, x2, y2, 0.0, 0.0]),
                    ServerKind::Radial {
                        cx,
                        cy,
                        r,
                        fx,
                        fy,
                        fr,
                    } => (1, [fx, fy, fr, cx, cy, r]),
                };
                self.shader(kind, spread, g, *transform, stops);
                Some(SHADED)
            }
            Ink::Pattern { .. } => {
                skipped_once("an SVG pattern");
                None
            }
        }
    }

    fn shader(
        &mut self,
        kind: u32,
        spread: u32,
        g: [f32; 6],
        m: [f32; 6],
        stops: &[(f32, [u8; 4])],
    ) {
        self.ops
            .extend([SHADER, (15 + 2 * stops.len()) as u32, kind, spread]);
        for v in g.into_iter().chain(m) {
            self.f(v);
        }
        self.ops.push(stops.len() as u32);
        for (at, c) in stops {
            self.f(*at);
            self.ops.push(Self::color(*c));
        }
    }
}

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Say once per process that a paint went undrawn here, rather than drawing
/// nothing silently.
pub(super) fn skipped_once(what: &'static str) {
    use std::collections::HashSet;
    use std::sync::Mutex;
    static SAID: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    if said.get_or_insert_with(HashSet::new).insert(what) {
        eprintln!("exact: the Android Canvas host does not paint {what}; it is skipped");
    }
}

#[cfg(test)]
mod tests {
    use super::super::stream::drawing;
    use super::*;
    use crate::paint::Backend;
    use exact_kernel::Color;

    fn recorder() -> Recorder {
        let mut r = Recorder::new();
        r.begin(200.0, 200.0, 1.0);
        r
    }

    #[test]
    fn a_box_gradient_shades_its_rounded_rect() {
        let mut r = recorder();
        let g = GradientPaint {
            geometry: Geometry::Linear {
                start: (0.0, 0.0),
                end: (0.0, 40.0),
            },
            repeat: false,
            stops: vec![
                (0.0, Color::rgba(255, 0, 0, 255)),
                (1.0, Color::rgba(0, 0, 255, 255)),
            ],
        };
        r.fill_gradient(
            &Shape::new((0.0, 0.0, 80.0, 40.0), [6.0; 4]),
            &g,
            Transform::identity(),
        );
        let at = r.ops.iter().position(|&w| w == SHADER).expect("a shader");
        let len = r.ops[at + 1] as usize;
        assert_eq!(r.ops[at + 2..at + 4], [0, 0], "linear, padded");
        let n = r.ops[at + 16] as usize;
        assert_eq!(len, 15 + 2 * n);
        // Opaque stops need no premultiplied expansion.
        assert_eq!(n, 2);
        assert_eq!(r.ops[at + 18], 0xFFFF_0000);
        assert_eq!(r.ops[at + 2 + len..at + 4 + len], [RRECT, SHADED]);
        let (ops, definitions) = drawing(&r.ops).expect("a walkable stream");
        assert!(!definitions);
        assert_eq!(ops, r.ops);
    }

    #[test]
    fn a_fade_to_transparent_keeps_its_hue() {
        let mut r = recorder();
        let g = GradientPaint {
            geometry: Geometry::Linear {
                start: (0.0, 0.0),
                end: (0.0, 40.0),
            },
            repeat: false,
            stops: vec![
                (0.0, Color::rgba(255, 255, 255, 240)),
                (1.0, Color::TRANSPARENT),
            ],
        };
        r.fill_gradient(
            &Shape::rect((0.0, 0.0, 80.0, 40.0)),
            &g,
            Transform::identity(),
        );
        let at = r.ops.iter().position(|&w| w == SHADER).expect("a shader");
        // The transparent stop takes its neighbour's hue, so the fade never
        // passes through grey.
        assert_eq!(r.ops[at + 16], 2);
        assert_eq!(r.ops[at + 20], 0x00FF_FFFF);
    }

    #[test]
    fn a_pattern_ink_is_skipped_and_draws_nothing() {
        let mut r = recorder();
        let tile = std::sync::Arc::new(tiny_skia::Pixmap::new(2, 2).unwrap());
        let ink = Ink::Pattern {
            tile,
            transform: IDENTITY,
            opacity: 1.0,
        };
        assert_eq!(r.ink(&ink), None);
        assert!(!r.ops.contains(&SHADER));
    }
}
