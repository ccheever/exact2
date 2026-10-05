//! The alternative looks' art direction over the engine's smooth
//! [`MeshBuilder`]: each vertex painted, so shading (contact darkening, sunlit
//! tips, a painted highlight) lives in the shared mesh instead of in
//! per-entity state.
use exact_game::{
    asset::{across, MeshBuilder, MeshData},
    *,
};

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

/// A smooth mesh being built with authored colours, and the (length, width)
/// every leaf is scaled by. The shapes are the engine's
/// [`MeshBuilder`]; what is here is the garden's own: its leaves, blades,
/// shaded slabs and glossy balls.
pub struct Sculpt(MeshBuilder, (f32, f32));

impl Default for Sculpt {
    fn default() -> Self {
        Self(MeshBuilder::smooth().squared(), (1., 1.))
    }
}

impl std::ops::Deref for Sculpt {
    type Target = MeshBuilder;
    fn deref(&self) -> &MeshBuilder {
        &self.0
    }
}

impl std::ops::DerefMut for Sculpt {
    fn deref_mut(&mut self) -> &mut MeshBuilder {
        &mut self.0
    }
}

impl Sculpt {
    /// Scales leaves added from now on: lusher foliage from the same layout.
    pub fn leafy(&mut self, length: f32, width: f32) {
        self.1 = (length, width);
    }

    /// A box with flat faces, darker toward its bottom by `under`.
    pub fn slab(&mut self, at: Vec3, size: Vec3, rot: Quat, color: Rgb, under: f32) {
        self.cuboid(at, size, rot, |c| {
            scale(color, 1. - under * (0.5 - c.y * 0.5))
        });
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
        self.sheet(&rows, Some(0.8));
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
        self.sheet(&rows, Some(0.8));
    }

    pub fn finish(self) -> MeshData {
        self.0.finish()
    }
}
