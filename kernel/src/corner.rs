//! CSS `corner-shape` (CSS Borders 4) and `-apple-continuous` (LLP 1076 D1).
//!
//! Each corner is a superellipse parameter K, as CSS defines the keywords:
//! `round` 1, `squircle` 2, `bevel` 0, `scoop` -1, `square` ∞, `notch` -∞.
//! `-apple-continuous` is Apple's continuous corner curve, the one shape every
//! host draws for that name: iOS natively (`cornerCurve = .continuous`), the
//! others from [`outline`]. The outline is geometry every native host draws
//! the same way; the web draws CSS's keywords itself and approximates
//! Apple's curve with [`APPLE_ON_THE_WEB`].

use crate::svg::{Path, Seg};

/// One corner's shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Corner {
    /// `superellipse(K)`; the keywords are values of K.
    Superellipse(f32),
    /// `-apple-continuous`.
    AppleContinuous,
}

const ROUND: Corner = Corner::Superellipse(1.0);

/// The row: top-left, top-right, bottom-right, bottom-left, as CSS orders
/// the shorthand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerShape(pub [Corner; 4]);

impl Default for CornerShape {
    fn default() -> Self {
        Self([ROUND; 4])
    }
}

const KEYWORDS: [(&str, f32); 6] = [
    ("round", 1.0),
    ("squircle", 2.0),
    ("bevel", 0.0),
    ("scoop", -1.0),
    ("square", f32::INFINITY),
    ("notch", f32::NEG_INFINITY),
];

/// Apple's curve reaches this many radii along each side from the corner.
pub const APPLE_EXTENT: f32 = 1.528_664_8;

/// The web's stand-in for `-apple-continuous`: `superellipse(K)` with the
/// radius scaled. Fitted to Apple's curve; the worst distance between them
/// is 2.5% of the radius (0.4 pt at 16 pt), declared in LLP 1001.
pub const APPLE_ON_THE_WEB: (f32, f32) = (1.6, 1.52);

impl CornerShape {
    /// The row's parse, as every codec's: `None` for anything not drawn.
    pub fn parse(css: &str) -> Option<Self> {
        Self::check(css).ok()
    }

    /// One to four corner values, expanded as `border-radius` expands.
    pub fn check(css: &str) -> Result<Self, &'static str> {
        let mut values = Vec::new();
        let mut rest = css.trim();
        while !rest.is_empty() {
            let (word, after) = match rest.find('(') {
                Some(open) if !rest[..open].contains(char::is_whitespace) => {
                    let close = rest.find(')').ok_or("superellipse( needs its )")?;
                    (&rest[..=close], &rest[close + 1..])
                }
                _ => rest.split_at(rest.find(char::is_whitespace).unwrap_or(rest.len())),
            };
            values.push(corner(word)?);
            rest = after.trim_start();
        }
        let [tl, tr, br, bl] = match values[..] {
            [a] => [a, a, a, a],
            [a, b] => [a, b, a, b],
            [a, b, c] => [a, b, c, b],
            [a, b, c, d] => [a, b, c, d],
            _ => return Err("one to four corner shapes"),
        };
        Ok(Self([tl, tr, br, bl]))
    }

    /// Canonical CSS, also the wire form: one value when all four agree.
    pub fn css(&self) -> String {
        let one = |c: Corner| match c {
            Corner::AppleContinuous => "-apple-continuous".to_string(),
            Corner::Superellipse(k) => match KEYWORDS.iter().find(|(_, v)| *v == k) {
                Some((name, _)) => (*name).to_string(),
                None => exact_num::text!("superellipse({})", exact_num::Shortest32(k)),
            },
        };
        let [a, b, c, d] = self.0;
        if a == b && b == c && c == d {
            return one(a);
        }
        [a, b, c, d].map(one).join(" ")
    }

    /// Whether every corner is `round`: the hosts' arc path applies.
    pub fn is_round(&self) -> bool {
        self.0.iter().all(|c| *c == ROUND)
    }

    /// Whether every corner is `-apple-continuous`.
    pub fn is_apple_continuous(&self) -> bool {
        self.0.iter().all(|c| *c == Corner::AppleContinuous)
    }
}

fn corner(word: &str) -> Result<Corner, &'static str> {
    if word.eq_ignore_ascii_case("-apple-continuous") {
        return Ok(Corner::AppleContinuous);
    }
    if let Some((_, k)) = KEYWORDS
        .iter()
        .find(|(name, _)| word.eq_ignore_ascii_case(name))
    {
        return Ok(Corner::Superellipse(*k));
    }
    let lower = word.to_ascii_lowercase();
    let arg = lower
        .strip_prefix("superellipse(")
        .and_then(|a| a.strip_suffix(')'))
        .ok_or("round, squircle, square, bevel, scoop, notch, superellipse(<number>) or -apple-continuous")?
        .trim();
    let k = match arg {
        "infinity" => f32::INFINITY,
        "-infinity" => f32::NEG_INFINITY,
        n => n
            .parse::<f32>()
            .ok()
            .filter(|k| k.is_finite())
            .ok_or("superellipse() takes a number, infinity or -infinity")?,
    };
    Ok(Corner::Superellipse(k))
}

/// A box's outline with shaped corners, in the box's coordinates: the
/// rectangle `(x, y, w, h)` and each corner's used radii `(rx, ry)` (already
/// clamped as CSS clamps them), top-left first, clockwise from the top edge.
pub fn outline(rect: (f32, f32, f32, f32), radii: [(f32, f32); 4], shape: &CornerShape) -> Path {
    let (x, y, w, h) = rect;
    let mut path = Path(Vec::with_capacity(64));
    let limit = apple_radius_limit(w, h);
    // Each corner in a local frame from its corner point: `along` runs into
    // the edge that follows it clockwise, `down` into the edge before it.
    let corners = [
        ((x, y), (1.0, 0.0), (0.0, 1.0)),
        ((x + w, y), (0.0, 1.0), (-1.0, 0.0)),
        ((x + w, y + h), (-1.0, 0.0), (0.0, -1.0)),
        ((x, y + h), (0.0, -1.0), (1.0, 0.0)),
    ];
    for (i, ((cx, cy), along, down)) in corners.into_iter().enumerate() {
        let (mut rx, mut ry) = radii[i];
        if shape.0[i] == Corner::AppleContinuous {
            (rx, ry) = (rx.min(limit), ry.min(limit));
        }
        // The radius along the following edge, then along the one before:
        // a horizontal edge follows the top-left and bottom-right corners.
        let (ra, rd) = if i % 2 == 0 { (rx, ry) } else { (ry, rx) };
        let at = |a: f32, d: f32| (cx + along.0 * a + down.0 * d, cy + along.1 * a + down.1 * d);
        let mut points = Vec::new();
        corner_points(shape.0[i], ra, rd, &mut points);
        for (n, (a, d)) in points.into_iter().enumerate() {
            let (px, py) = at(a, d);
            path.0.push(if i == 0 && n == 0 {
                Seg::Move(px, py)
            } else {
                Seg::Line(px, py)
            });
        }
    }
    path.0.push(Seg::Close);
    path
}

/// The border's inner edge: the outline of the padding box, its radii the
/// outer radii less the border widths (CSS's rule for round corners, kept for
/// every shape), `widths` top, right, bottom, left.
pub fn inner_outline(
    rect: (f32, f32, f32, f32),
    radii: [(f32, f32); 4],
    widths: [f32; 4],
    shape: &CornerShape,
) -> Path {
    let (x, y, w, h) = rect;
    let [t, r, b, l] = widths;
    let inner = |(rx, ry): (f32, f32), side_x: f32, side_y: f32| {
        ((rx - side_x).max(0.0), (ry - side_y).max(0.0))
    };
    outline(
        (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0)),
        [
            inner(radii[0], l, t),
            inner(radii[1], r, t),
            inner(radii[2], r, b),
            inner(radii[3], l, b),
        ],
        shape,
    )
}

/// One corner's points, `(a, d)`: `a` inward along the following edge, `d`
/// inward along the preceding one; from `(0, rd)` to `(ra, 0)`.
fn corner_points(corner: Corner, ra: f32, rd: f32, out: &mut Vec<(f32, f32)>) {
    if ra <= 0.0 || rd <= 0.0 {
        out.push((0.0, 0.0));
        return;
    }
    match corner {
        Corner::AppleContinuous => apple_points(ra, rd, out),
        Corner::Superellipse(k) if k == f32::INFINITY => {
            out.extend([(0.0, rd), (0.0, 0.0), (ra, 0.0)])
        }
        Corner::Superellipse(k) if k == f32::NEG_INFINITY => {
            out.extend([(0.0, rd), (ra, rd), (ra, 0.0)])
        }
        Corner::Superellipse(k) => {
            // X^p + Y^p = 1 about the corner box's inner point, p = 2^K, as
            // the point (ra - ra X, rd - rd Y). One half is sampled, small X
            // up to the diagonal, spaced so X^p is even when p < 1 (where
            // the curve meets the edges steeply); the other is its mirror.
            let p = 2f32.powf(k);
            let n = ((ra.max(rd) * 0.5).ceil() as usize).clamp(6, 48);
            let diagonal = 0.5f32.powf(1.0 / p);
            let q = if p < 1.0 { 1.0 / p } else { 1.0 };
            let half: Vec<(f32, f32)> = (0..=n)
                .map(|i| {
                    let x = diagonal * (i as f32 / n as f32).powf(q);
                    let y = (1.0 - x.powf(p)).max(0.0).powf(1.0 / p);
                    (x, y)
                })
                .collect();
            // From the edge before (X = 1) to the diagonal: the mirror.
            for &(x, y) in half.iter() {
                out.push((ra - ra * y, rd - rd * x));
            }
            // From the diagonal to the edge after (X = 0).
            for &(x, y) in half.iter().rev().skip(1) {
                out.push((ra - ra * x, rd - rd * y));
            }
        }
    }
}

/// Apple's continuous corner, three cubics, flattened. The curve reaches
/// [`APPLE_EXTENT`] radii along each side; a box too small for that takes a
/// smaller radius, as UIKit does.
fn apple_points(ra: f32, rd: f32, out: &mut Vec<(f32, f32)>) {
    // Control points in radii, `(a, d)`, from the edge before to the edge
    // after: the curve as measured from UIKit's (symmetric about the
    // diagonal; LLP 1076 D1 holds it to the simulator's pixels).
    const C: [[(f32, f32); 4]; 3] = [
        [
            (0.0, APPLE_EXTENT),
            (0.0, 1.088_493),
            (0.0, 0.868_407),
            (0.065_496, 0.669_935),
        ],
        [
            (0.065_496, 0.669_935),
            (0.187_932, 0.372_823),
            (0.372_821, 0.187_933),
            (0.669_934, 0.065_496),
        ],
        [
            (0.669_934, 0.065_496),
            (0.868_407, 0.0),
            (1.088_493, 0.0),
            (APPLE_EXTENT, 0.0),
        ],
    ];
    let steps = ((ra.max(rd) * 0.4).ceil() as usize).clamp(4, 24);
    out.push((0.0, rd * APPLE_EXTENT));
    for [p0, p1, p2, p3] in C {
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let a = b0 * p0.0 + b1 * p1.0 + b2 * p2.0 + b3 * p3.0;
            let d = b0 * p0.1 + b1 * p1.1 + b2 * p2.1 + b3 * p3.1;
            out.push((a * ra, d * rd));
        }
    }
}

/// The largest radius `-apple-continuous` draws in a box of this size:
/// Apple's curve needs [`APPLE_EXTENT`] radii on each side.
pub fn apple_radius_limit(width: f32, height: f32) -> f32 {
    width.min(height) / 2.0 / APPLE_EXTENT
}

#[cfg(test)]
mod tests;
