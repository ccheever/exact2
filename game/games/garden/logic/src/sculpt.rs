//! Smooth-shaded model building for the alternative looks: analytic
//! ellipsoids, tapered tubes and curved double-sided sheets, each vertex
//! painted, so shading (contact darkening, sunlit tips, a painted highlight)
//! lives in the shared mesh instead of in per-entity state.
use exact_game::{asset::MeshData, *};

/// An authored sRGB-ish colour; vertices store it squared (linear).
pub type Rgb = [f32; 3];

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| v * k)
}

/// Pushes a colour away from its grey: `k > 1` saturates.
pub fn saturate(c: Rgb, k: f32) -> Rgb {
    let grey = (c[0] + c[1] + c[2]) / 3.;
    c.map(|v| (grey + (v - grey) * k).clamp(0., 1.))
}

/// A stable value in [0, 1) for scattering scenery.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A unit vector perpendicular to `axis`.
fn across(axis: Vec3) -> Vec3 {
    let up = if axis.y.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Y
    };
    axis.cross(up).normalize_or_zero()
}

/// A mesh being built, and the (length, width) every leaf is scaled by.
pub struct Sculpt(MeshData, (f32, f32));

impl Default for Sculpt {
    fn default() -> Self {
        Self(MeshData::default(), (1., 1.))
    }
}

impl Sculpt {
    /// Scales leaves added from now on: lusher foliage from the same layout.
    pub fn leafy(&mut self, length: f32, width: f32) {
        self.1 = (length, width);
    }

    pub fn vert(&mut self, p: Vec3, n: Vec3, c: Rgb) -> u32 {
        let i = (self.0.positions.len() / 3) as u32;
        self.0.positions.extend(p.to_array());
        self.0.normals.extend(n.normalize_or_zero().to_array());
        self.0.uvs.extend([0.0, 0.0]);
        self.0
            .colors
            .extend([c[0] * c[0], c[1] * c[1], c[2] * c[2], 1.0]);
        i
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.0.indices.extend([a, b, c]);
    }

    /// A box with flat faces, darker toward its bottom by `under`.
    pub fn slab(&mut self, at: Vec3, size: Vec3, rot: Quat, color: Rgb, under: f32) {
        let p = |x: f32, y: f32, z: f32| (at + rot * (size * Vec3::new(x, y, z) * 0.5), y);
        let shade = |y: f32| scale(color, 1. - under * (0.5 - y * 0.5));
        for face in [
            [
                p(-1., -1., 1.),
                p(1., -1., 1.),
                p(1., 1., 1.),
                p(-1., 1., 1.),
            ],
            [
                p(1., -1., -1.),
                p(-1., -1., -1.),
                p(-1., 1., -1.),
                p(1., 1., -1.),
            ],
            [
                p(1., -1., 1.),
                p(1., -1., -1.),
                p(1., 1., -1.),
                p(1., 1., 1.),
            ],
            [
                p(-1., -1., -1.),
                p(-1., -1., 1.),
                p(-1., 1., 1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., 1., 1.),
                p(1., 1., 1.),
                p(1., 1., -1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., -1., -1.),
                p(1., -1., -1.),
                p(1., -1., 1.),
                p(-1., -1., 1.),
            ],
        ] {
            let n = (face[1].0 - face[0].0).cross(face[2].0 - face[0].0);
            let ids = face.map(|(q, y)| self.vert(q, n, shade(y)));
            self.tri(ids[0], ids[1], ids[2]);
            self.tri(ids[0], ids[2], ids[3]);
        }
    }

    /// A smooth ellipsoid; `shade` paints each vertex from its unit direction
    /// in the ellipsoid's own frame (+Y up).
    pub fn ellipsoid(
        &mut self,
        at: Vec3,
        radius: Vec3,
        rot: Quat,
        detail: (u32, u32),
        shade: impl Fn(Vec3) -> Rgb,
    ) {
        let (rings, segs) = detail;
        let start = (self.0.positions.len() / 3) as u32;
        for j in 0..=rings {
            let (sy, cy) = math::sin_cos(j as f32 * std::f32::consts::PI / rings as f32);
            for k in 0..=segs {
                let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / segs as f32);
                let d = Vec3::new(c * sy, cy, s * sy);
                let n = rot * (d / radius);
                self.vert(at + rot * (radius * d), n, shade(d));
            }
        }
        let row = segs + 1;
        for j in 0..rings {
            for k in 0..segs {
                let (a, b) = (start + j * row + k, start + j * row + k + 1);
                let (c, d) = (a + row, b + row);
                if j > 0 {
                    self.tri(a, b, c);
                }
                if j + 1 < rings {
                    self.tri(b, d, c);
                }
            }
        }
    }

    /// A ball painted from `low` (underside) to `high` (top), with a soft
    /// painted highlight toward `light` when `gloss` > 0.
    pub fn ball(&mut self, at: Vec3, radius: Vec3, low: Rgb, high: Rgb, gloss: f32) {
        let light = Vec3::new(-0.45, 0.8, 0.4).normalize();
        self.ellipsoid(at, radius, Quat::IDENTITY, (8, 14), |d| {
            let c = mix(low, high, d.y * 0.5 + 0.5);
            let h = d.dot(light).max(0.);
            scale(c, 1. + gloss * h * h * h * h * h * h)
        });
    }

    /// A tube along `path` (point, radius), smooth around and along, painted
    /// by `shade(t along, angle around)`. A zero end radius closes to a point;
    /// otherwise `cap` closes the far end.
    pub fn tube(
        &mut self,
        path: &[(Vec3, f32)],
        segs: u32,
        cap: bool,
        shade: impl Fn(f32, f32) -> Rgb,
    ) {
        let n = path.len();
        let start = (self.0.positions.len() / 3) as u32;
        let mut side = across(path[1].0 - path[0].0);
        let mut last = (Vec3::ZERO, Vec3::ZERO);
        for (i, &(p, r)) in path.iter().enumerate() {
            let axis = if i + 1 < n {
                path[i + 1].0 - p
            } else {
                p - path[i - 1].0
            }
            .normalize_or_zero();
            let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
            // Carry the frame along the path so the tube does not twist.
            side = (side - axis * side.dot(axis)).normalize_or_zero();
            if side == Vec3::ZERO {
                side = across(axis);
            }
            let other = axis.cross(side);
            last = (side, other);
            let t = i as f32 / (n - 1) as f32;
            for k in 0..=segs {
                let a = k as f32 * std::f32::consts::TAU / segs as f32;
                let (s, c) = math::sin_cos(a);
                let radial = side * c + other * s;
                self.vert(p + radial * r, radial, shade(t, a));
            }
        }
        let row = segs + 1;
        for i in 0..n as u32 - 1 {
            for k in 0..segs {
                let a = start + i * row + k;
                let (b, c, d) = (a + 1, a + row, a + row + 1);
                self.tri(a, b, c);
                self.tri(b, d, c);
            }
        }
        let (end, r) = path[n - 1];
        if cap && r > 0. {
            let axis = (end - path[n - 2].0).normalize_or_zero();
            let mid = self.vert(end, axis, shade(1., 0.));
            let ring: Vec<u32> = (0..=segs)
                .map(|k| {
                    let a = k as f32 * std::f32::consts::TAU / segs as f32;
                    let (s, c) = math::sin_cos(a);
                    self.vert(end + (last.0 * c + last.1 * s) * r, axis, shade(1., a))
                })
                .collect();
            for k in 0..segs as usize {
                self.tri(ring[k], ring[k + 1], mid);
            }
        }
    }

    /// A solid of revolution about +Y through `at`: (height, radius) pairs
    /// from bottom to top (a zero radius closes an end), painted by
    /// `shade(height, angle)`.
    pub fn lathe(
        &mut self,
        at: Vec3,
        profile: &[(f32, f32)],
        segs: u32,
        shade: impl Fn(f32, f32) -> Rgb,
    ) {
        let n = profile.len();
        let start = (self.0.positions.len() / 3) as u32;
        for i in 0..n {
            let (y, r) = profile[i];
            let (y0, r0) = profile[i.saturating_sub(1)];
            let (y1, r1) = profile[(i + 1).min(n - 1)];
            let (dy, dr) = (y1 - y0, r1 - r0);
            for k in 0..=segs {
                let a = k as f32 * std::f32::consts::TAU / segs as f32;
                let (s, c) = math::sin_cos(a);
                let radial = Vec3::new(c, 0., s);
                let normal = radial * dy - Vec3::Y * dr;
                self.vert(at + radial * r + Vec3::Y * y, normal, shade(y, a));
            }
        }
        let row = segs + 1;
        for i in 0..n as u32 - 1 {
            for k in 0..segs {
                let a = start + i * row + k;
                let (b, c, d) = (a + 1, a + row, a + row + 1);
                self.tri(a, c, b);
                self.tri(b, c, d);
            }
        }
    }

    /// A smooth grid of rows of (point, colour). Normals come from the
    /// surface; `double` adds the back face with reversed normals.
    pub fn sheet(&mut self, rows: &[Vec<(Vec3, Rgb)>], double: bool) {
        let cols = rows[0].len();
        let normal = |r: usize, c: usize| -> Vec3 {
            let p = |r: usize, c: usize| rows[r][c].0;
            let du = p(r, (c + 1).min(cols - 1)) - p(r, c.saturating_sub(1));
            let dv = p((r + 1).min(rows.len() - 1), c) - p(r.saturating_sub(1), c);
            let n = du.cross(dv);
            if n.length_squared() < 1e-12 {
                Vec3::Y
            } else {
                n
            }
        };
        for back in [false, true] {
            if back && !double {
                break;
            }
            let start = (self.0.positions.len() / 3) as u32;
            for (r, row) in rows.iter().enumerate() {
                for (c, &(p, color)) in row.iter().enumerate() {
                    let n = normal(r, c);
                    let color = if back { scale(color, 0.8) } else { color };
                    self.vert(p, if back { -n } else { n }, color);
                }
            }
            let w = cols as u32;
            for r in 0..rows.len() as u32 - 1 {
                for c in 0..w - 1 {
                    let a = start + r * w + c;
                    let (b, d, e) = (a + 1, a + w, a + w + 1);
                    if back {
                        self.tri(a, d, b);
                        self.tri(b, d, e);
                    } else {
                        self.tri(a, b, d);
                        self.tri(b, e, d);
                    }
                }
            }
        }
    }

    /// A curved leaf from `root` toward `tip`: `round` 0 is a blade, 1 a
    /// paddle; `droop` bends the tip down; the two halves fold up by `fold`.
    #[allow(clippy::too_many_arguments)]
    pub fn leaf(
        &mut self,
        root: Vec3,
        tip: Vec3,
        width: f32,
        round: f32,
        droop: f32,
        fold: f32,
        base: Rgb,
        edge: Rgb,
    ) {
        let axis = (tip - root) * self.1 .0;
        let width = width * self.1 .1;
        let side = across(axis) * width;
        let up = side.cross(axis).normalize_or_zero();
        let up = if up.y < 0. { -up } else { up };
        let steps = 7;
        let rows: Vec<Vec<(Vec3, Rgb)>> = (0..=steps)
            .map(|i| {
                let u = i as f32 / steps as f32;
                let profile = math::sin(u * std::f32::consts::PI).max(0.);
                let w = profile * (1. - round) * math::sqrt(profile) + profile * round;
                let w = w * (1. - 0.25 * u * u * (1. - round));
                let mid = root + axis * u - Vec3::Y * (droop * axis.length() * u * u)
                    + up * (axis.length() * 0.12 * u * (1. - u));
                let tint = mix(base, edge, u * 0.85);
                [-1.0f32, -0.5, 0., 0.5, 1.]
                    .iter()
                    .map(|&s| {
                        let across = side * (s * w);
                        let lift = up * (fold * width * w * s * s);
                        let color = mix(tint, edge, s.abs() * 0.35);
                        let color = if s == 0. { scale(color, 1.12) } else { color };
                        (mid + across + lift, color)
                    })
                    .collect()
            })
            .collect();
        self.sheet(&rows, true);
    }

    /// One tapering grass blade, dark at the root.
    pub fn blade(&mut self, root: Vec3, tip: Vec3, width: f32, low: Rgb, high: Rgb) {
        let side = across(tip - root) * width;
        let bend = Vec3::new(tip.x - root.x, 0., tip.z - root.z) * 0.35;
        let rows: Vec<Vec<(Vec3, Rgb)>> = (0..=3)
            .map(|i| {
                let u = i as f32 / 3.;
                let mid = root.lerp(tip, u) + bend * (u * u - u);
                let w = 1. - u * 0.92;
                let c = mix(low, high, u);
                vec![(mid - side * w, c), (mid + side * w, c)]
            })
            .collect();
        self.sheet(&rows, true);
    }

    /// Nothing added yet: a model part with no geometry is left out.
    pub fn is_empty(&self) -> bool {
        self.0.positions.is_empty()
    }

    pub fn finish(mut self) -> MeshData {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in self.0.positions.chunks_exact(3) {
            let p = Vec3::new(p[0], p[1], p[2]);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        self.0.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
        self.0
    }
}
