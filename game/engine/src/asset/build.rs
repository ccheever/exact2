//! Code-made geometry for [`World::generated`](crate::World::generated) and
//! [`Model::parts`](super::Model::parts): one builder, flat or smooth, so a
//! game keeps its shapes and palette and none re-derives vertex emission,
//! tessellation or bounds. Every number comes from the engine's portable math,
//! so the same calls make the same bytes, and the same identity, on every host.
use super::MeshData;
use crate::math;
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

/// A mesh being built, with every vertex painted an opaque colour.
///
/// A *smooth* builder keeps the vertices the shapes make and the normals they
/// give them. A *flat* one turns each triangle into a facet of its own three
/// vertices, normal to the face, each keeping its vertex's colour: the same
/// calls, faceted. Colours are linear unless [`squared`](Self::squared).
pub struct MeshBuilder {
    mesh: MeshData,
    /// A flat builder's vertices, which only its facets reach the mesh through.
    staged: Option<Vec<(Vec3, [f32; 4])>>,
    squared: bool,
}

/// A unit vector perpendicular to `axis`: where a [`MeshBuilder::tube`]'s frame
/// starts, and a side for anything else laid along an axis.
pub fn across(axis: Vec3) -> Vec3 {
    let up = if axis.y.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Y
    };
    axis.cross(up).normalize_or_zero()
}

impl MeshBuilder {
    /// Shapes keep their vertices and normals: shading blends across them.
    pub fn smooth() -> Self {
        Self {
            mesh: MeshData::default(),
            staged: None,
            squared: false,
        }
    }

    /// Every triangle a facet: its own vertices, normal to its face.
    pub fn flat() -> Self {
        Self {
            staged: Some(Vec::new()),
            ..Self::smooth()
        }
    }

    /// Colours are authored by eye and stored squared (gamma 2, a cheap
    /// stand-in for decoding sRGB that every host computes alike).
    pub fn squared(mut self) -> Self {
        self.squared = true;
        self
    }

    fn paint(&self, c: [f32; 3]) -> [f32; 4] {
        if self.squared {
            [c[0] * c[0], c[1] * c[1], c[2] * c[2], 1.0]
        } else {
            [c[0], c[1], c[2], 1.0]
        }
    }

    fn push(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        let i = (self.mesh.positions.len() / 3) as u32;
        self.mesh.positions.extend(p.to_array());
        self.mesh.normals.extend(n.to_array());
        self.mesh.uvs.extend([0.0, 0.0]);
        self.mesh.colors.extend(c);
        i
    }

    fn face(&mut self, [(a, ca), (b, cb), (c, cc)]: [(Vec3, [f32; 4]); 3]) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = self.push(a, n, ca);
        self.push(b, n, cb);
        self.push(c, n, cc);
        self.mesh.indices.extend([i, i + 1, i + 2]);
    }

    /// A vertex at `p` with normal `n` (normalized here), for
    /// [`triangle`](Self::triangle). A flat builder holds it until a triangle
    /// uses it, and ignores `n`.
    pub fn vertex(&mut self, p: Vec3, n: Vec3, color: [f32; 3]) -> u32 {
        let c = self.paint(color);
        match &mut self.staged {
            Some(staged) => {
                staged.push((p, c));
                staged.len() as u32 - 1
            }
            None => self.push(p, n.normalize_or_zero(), c),
        }
    }

    /// A triangle over three [`vertex`](Self::vertex) indices, counter-clockwise
    /// seen from its front.
    pub fn triangle(&mut self, a: u32, b: u32, c: u32) {
        match &self.staged {
            Some(staged) => {
                let face = [a, b, c].map(|i| staged[i as usize]);
                self.face(face);
            }
            None => self.mesh.indices.extend([a, b, c]),
        }
    }

    /// One flat triangle in a single colour, whatever the builder's shading.
    pub fn facet(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        let c4 = self.paint(color);
        self.face([(a, c4), (b, c4), (c, c4)]);
    }

    /// A box of `size` centred on `at`, turned by `rot`, its faces flat.
    /// `shade` paints each corner from its place in the box (each axis ±1).
    pub fn cuboid(&mut self, at: Vec3, size: Vec3, rot: Quat, shade: impl Fn(Vec3) -> [f32; 3]) {
        let p = |x: f32, y: f32, z: f32| {
            let corner = Vec3::new(x, y, z);
            (at + rot * (size * corner * 0.5), corner)
        };
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
            let ids = face.map(|(q, corner)| self.vertex(q, n, shade(corner)));
            self.triangle(ids[0], ids[1], ids[2]);
            self.triangle(ids[0], ids[2], ids[3]);
        }
    }

    /// An ellipsoid of `rings` from pole to pole and `segs` around, centred on
    /// `at` and turned by `rot`. `shade` paints each vertex from its unit
    /// direction in the ellipsoid's own frame (+Y up).
    pub fn ellipsoid(
        &mut self,
        at: Vec3,
        radius: Vec3,
        rot: Quat,
        (rings, segs): (u32, u32),
        shade: impl Fn(Vec3) -> [f32; 3],
    ) {
        let mut grid = Vec::with_capacity(((rings + 1) * (segs + 1)) as usize);
        for j in 0..=rings {
            let (sy, cy) = math::sin_cos(j as f32 * PI / rings as f32);
            for k in 0..=segs {
                let (s, c) = math::sin_cos(k as f32 * TAU / segs as f32);
                let d = Vec3::new(c * sy, cy, s * sy);
                let n = rot * (d / radius);
                grid.push(self.vertex(at + rot * (radius * d), n, shade(d)));
            }
        }
        let row = (segs + 1) as usize;
        for j in 0..rings as usize {
            for k in 0..segs as usize {
                let (a, b) = (grid[j * row + k], grid[j * row + k + 1]);
                let (c, d) = (grid[(j + 1) * row + k], grid[(j + 1) * row + k + 1]);
                if j > 0 {
                    self.triangle(a, b, c);
                }
                if j + 1 < rings as usize {
                    self.triangle(b, d, c);
                }
            }
        }
    }

    /// A tube along `path` (point, radius), `segs` around, its frame carried
    /// along so it does not twist; painted by `shade(t along, angle around)`.
    /// A zero end radius closes to a point; otherwise `cap` closes the far end.
    pub fn tube(
        &mut self,
        path: &[(Vec3, f32)],
        segs: u32,
        cap: bool,
        shade: impl Fn(f32, f32) -> [f32; 3],
    ) {
        let n = path.len();
        let mut rings = Vec::with_capacity(n * (segs as usize + 1));
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
            side = (side - axis * side.dot(axis)).normalize_or_zero();
            if side == Vec3::ZERO {
                side = across(axis);
            }
            let other = axis.cross(side);
            last = (side, other);
            let t = i as f32 / (n - 1) as f32;
            for k in 0..=segs {
                let a = k as f32 * TAU / segs as f32;
                let (s, c) = math::sin_cos(a);
                let radial = side * c + other * s;
                rings.push(self.vertex(p + radial * r, radial, shade(t, a)));
            }
        }
        let row = segs as usize + 1;
        for i in 0..n - 1 {
            for k in 0..segs as usize {
                let a = i * row + k;
                let (a, b, c, d) = (rings[a], rings[a + 1], rings[a + row], rings[a + row + 1]);
                self.triangle(a, b, c);
                self.triangle(b, d, c);
            }
        }
        let (end, r) = path[n - 1];
        if cap && r > 0. {
            let axis = (end - path[n - 2].0).normalize_or_zero();
            let mid = self.vertex(end, axis, shade(1., 0.));
            let ring: Vec<u32> = (0..=segs)
                .map(|k| {
                    let a = k as f32 * TAU / segs as f32;
                    let (s, c) = math::sin_cos(a);
                    self.vertex(end + (last.0 * c + last.1 * s) * r, axis, shade(1., a))
                })
                .collect();
            for k in 0..segs as usize {
                self.triangle(ring[k], ring[k + 1], mid);
            }
        }
    }

    /// A solid of revolution about +Y through `at`: (height, radius) pairs from
    /// bottom to top (a zero radius closes an end), `segs` around, painted by
    /// `shade(height, angle)`.
    pub fn lathe(
        &mut self,
        at: Vec3,
        profile: &[(f32, f32)],
        segs: u32,
        shade: impl Fn(f32, f32) -> [f32; 3],
    ) {
        let n = profile.len();
        let mut rings = Vec::with_capacity(n * (segs as usize + 1));
        for i in 0..n {
            let (y, r) = profile[i];
            let (y0, r0) = profile[i.saturating_sub(1)];
            let (y1, r1) = profile[(i + 1).min(n - 1)];
            let (dy, dr) = (y1 - y0, r1 - r0);
            for k in 0..=segs {
                let a = k as f32 * TAU / segs as f32;
                let (s, c) = math::sin_cos(a);
                let radial = Vec3::new(c, 0., s);
                let normal = radial * dy - Vec3::Y * dr;
                rings.push(self.vertex(at + radial * r + Vec3::Y * y, normal, shade(y, a)));
            }
        }
        let row = segs as usize + 1;
        for i in 0..n - 1 {
            for k in 0..segs as usize {
                let a = i * row + k;
                let (a, b, c, d) = (rings[a], rings[a + 1], rings[a + row], rings[a + row + 1]);
                self.triangle(a, c, b);
                self.triangle(b, c, d);
            }
        }
    }

    /// A grid of rows of (point, colour), its normals from the surface. `back`
    /// adds the reverse face, its colours scaled by that much.
    pub fn sheet(&mut self, rows: &[Vec<(Vec3, [f32; 3])>], back: Option<f32>) {
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
        for shade in [None, back] {
            let reverse = shade.is_some();
            let mut grid = Vec::with_capacity(rows.len() * cols);
            for (r, row) in rows.iter().enumerate() {
                for (c, &(p, color)) in row.iter().enumerate() {
                    let n = normal(r, c);
                    let color = shade.map_or(color, |k| color.map(|v| v * k));
                    grid.push(self.vertex(p, if reverse { -n } else { n }, color));
                }
            }
            for r in 0..rows.len() - 1 {
                for c in 0..cols - 1 {
                    let a = r * cols + c;
                    let (a, b, d, e) = (grid[a], grid[a + 1], grid[a + cols], grid[a + cols + 1]);
                    if reverse {
                        self.triangle(a, d, b);
                        self.triangle(b, d, e);
                    } else {
                        self.triangle(a, b, d);
                        self.triangle(b, e, d);
                    }
                }
            }
            if back.is_none() {
                break;
            }
        }
    }

    /// Nothing added yet: a model part with no geometry is left out.
    pub fn is_empty(&self) -> bool {
        self.mesh.positions.is_empty()
    }

    /// The mesh, bounded by its vertices.
    pub fn finish(mut self) -> MeshData {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in self.mesh.positions.chunks_exact(3) {
            let p = Vec3::new(p[0], p[1], p[2]);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        self.mesh.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
        self.mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(m: &MeshData, i: u32) -> Vec3 {
        let i = i as usize * 3;
        Vec3::from_slice(&m.positions[i..i + 3])
    }
    fn normal(m: &MeshData, i: u32) -> Vec3 {
        let i = i as usize * 3;
        Vec3::from_slice(&m.normals[i..i + 3])
    }
    fn color(m: &MeshData, i: u32) -> [f32; 4] {
        let i = i as usize * 4;
        m.colors[i..i + 4].try_into().unwrap()
    }
    fn egg(m: &mut MeshBuilder) {
        let rot = Quat::from_rotation_z(0.3);
        m.ellipsoid(
            Vec3::new(1., 2., 3.),
            Vec3::new(0.5, 1., 0.25),
            rot,
            (5, 7),
            |d| [0.5 + d.y * 0.5, 0.25, 1.],
        );
    }

    #[test]
    fn flat_is_the_smooth_mesh_faceted() {
        let (mut smooth, mut flat) = (MeshBuilder::smooth(), MeshBuilder::flat());
        egg(&mut smooth);
        egg(&mut flat);
        let (smooth, flat) = (smooth.finish(), flat.finish());
        // Five rings: the polar rows are one triangle a segment, the rest two.
        assert_eq!(smooth.indices.len(), 3 * 7 * (2 * 5 - 2));
        assert_eq!(flat.indices.len(), smooth.indices.len());
        assert_eq!(flat.positions.len(), flat.indices.len() * 3);
        assert!(flat.indices.iter().enumerate().all(|(k, &i)| i == k as u32));
        for (t, tri) in smooth.indices.chunks_exact(3).enumerate() {
            let f = [0, 1, 2].map(|k| (3 * t + k) as u32);
            for k in 0..3 {
                assert_eq!(at(&flat, f[k]).to_array(), at(&smooth, tri[k]).to_array());
                assert_eq!(color(&flat, f[k]), color(&smooth, tri[k]));
            }
            let face = (at(&flat, f[1]) - at(&flat, f[0]))
                .cross(at(&flat, f[2]) - at(&flat, f[0]))
                .normalize();
            for k in f {
                assert_eq!(normal(&flat, k), face);
            }
            // The smooth normals point the same way as the facet.
            assert!(normal(&smooth, tri[0]).dot(face) > 0.);
        }
        assert_eq!(flat.bounds, smooth.bounds);
        assert_eq!(flat.uvs.len(), flat.positions.len() / 3 * 2);
    }

    #[test]
    fn colours_are_linear_unless_squared() {
        let mut linear = MeshBuilder::smooth();
        let mut squared = MeshBuilder::smooth().squared();
        for m in [&mut linear, &mut squared] {
            m.facet(Vec3::ZERO, Vec3::X, Vec3::Y, [0.5, 0.2, 1.]);
        }
        assert_eq!(color(&linear.finish(), 0), [0.5, 0.2, 1., 1.]);
        assert_eq!(color(&squared.finish(), 2), [0.25, 0.2 * 0.2, 1., 1.]);
    }

    #[test]
    fn a_cuboid_is_closed_and_faces_out() {
        let centre = Vec3::new(0., 1., 0.);
        let mut m = MeshBuilder::smooth();
        m.cuboid(
            centre,
            Vec3::new(2., 1., 0.5),
            Quat::from_rotation_y(0.7),
            |c| [c.y * 0.5 + 0.5; 3],
        );
        let m = m.finish();
        assert_eq!((m.positions.len() / 3, m.indices.len()), (24, 36));
        for tri in m.indices.chunks_exact(3) {
            let (a, b, c) = (at(&m, tri[0]), at(&m, tri[1]), at(&m, tri[2]));
            let out = (a + b + c) / 3. - centre;
            assert!((b - a).cross(c - a).dot(out) > 0.);
            assert!(normal(&m, tri[0]).dot(out) > 0.);
        }
        // Shaded by height in the box's own frame: tops white, bottoms black.
        assert!(m.colors.chunks_exact(4).all(|c| c[0] == 0. || c[0] == 1.));
        assert!((m.bounds[1] - 0.5).abs() < 1e-6 && (m.bounds[4] - 1.5).abs() < 1e-6);
    }

    #[test]
    fn tubes_lathes_and_two_sided_sheets() {
        let mut m = MeshBuilder::smooth();
        let path = [
            (Vec3::ZERO, 0.2),
            (Vec3::new(0., 1., 0.2), 0.15),
            (Vec3::new(0.3, 2., 0.), 0.1),
        ];
        m.tube(&path, 6, true, |t, _| [t; 3]);
        let tube = 3 * 7 + 1 + 7;
        m.lathe(Vec3::X, &[(0., 0.), (0.5, 0.4), (1., 0.)], 8, |y, _| [y; 3]);
        let lathe = 3 * 9;
        let rows = vec![vec![(Vec3::ZERO, [1.; 3]), (Vec3::X, [1.; 3])]; 3]
            .into_iter()
            .enumerate()
            .map(|(r, row)| {
                row.into_iter()
                    .map(|(p, c)| (p + Vec3::Z * r as f32, c))
                    .collect()
            })
            .collect::<Vec<Vec<_>>>();
        m.sheet(&rows, Some(0.5));
        let m = m.finish();
        assert_eq!(m.positions.len() / 3, tube + lathe + 2 * 6);
        let count = m.indices.len() / 3;
        assert_eq!(count, 2 * 2 * 6 + 6 + 2 * 2 * 8 + 2 * 2 * 2);
        assert!(m
            .indices
            .iter()
            .all(|&i| (i as usize) < m.positions.len() / 3));
        assert!(m
            .normals
            .chunks_exact(3)
            .all(|n| (Vec3::from_slice(n).length() - 1.).abs() < 1e-5));
        // The sheet's back: the same points, reversed normals, scaled colours.
        let front = (tube + lathe) as u32;
        let back = front + 6;
        assert_eq!(at(&m, front), at(&m, back));
        assert_eq!(normal(&m, front), -normal(&m, back));
        assert_eq!(color(&m, back), [0.5, 0.5, 0.5, 1.]);
    }

    /// Games' saved model identities are hashes of these bytes: a change here
    /// moves their pins. Change it knowingly.
    #[test]
    fn equal_calls_make_equal_bytes() {
        let make = || {
            let mut m = MeshBuilder::flat().squared();
            egg(&mut m);
            m.cuboid(Vec3::ZERO, Vec3::ONE, Quat::IDENTITY, |_| [0.3, 0.6, 0.9]);
            m.finish()
        };
        let digest = crate::hash::of(&make());
        assert_eq!(digest, crate::hash::of(&make()));
        assert_eq!(digest, 0x1388_7efc_c598_3a74);
    }
}
