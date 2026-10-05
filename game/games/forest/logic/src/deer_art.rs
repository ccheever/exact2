//! A readable silhouette and saved attack pose for the night encounter.
use crate::creatures::{Deer, Mind, WINDUP};
use exact_game::{
    asset::MeshData,
    audio::{AudioListener, Spatial, Synth},
    *,
};

#[derive(Default)]
struct Model(MeshData);
impl Model {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let first = (self.0.positions.len() / 3) as u32;
        for p in [a, b, c] {
            self.0.positions.extend(p.to_array());
            self.0.normals.extend(normal.to_array());
            self.0.uvs.extend([0., 0.]);
            self.0.colors.extend([
                color[0] * color[0],
                color[1] * color[1],
                color[2] * color[2],
                1.,
            ]);
        }
        self.0.indices.extend([first, first + 1, first + 2]);
    }
    fn bone(&mut self, a: Vec3, b: Vec3, r0: f32, r1: f32, color: [f32; 3]) {
        let axis = (b - a).normalize();
        let side = axis
            .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::Z })
            .normalize();
        let up = axis.cross(side);
        let ring = |k: u32, p: Vec3, radius: f32| {
            let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / 6.);
            p + (side * c + up * s) * radius
        };
        for k in 0..6 {
            let (p, q, r, s) = (
                ring(k, a, r0),
                ring(k + 1, a, r0),
                ring(k, b, r1),
                ring(k + 1, b, r1),
            );
            self.tri(p, q, r, color);
            self.tri(q, s, r, color);
            self.tri(a, q, p, color);
            self.tri(b, r, s, color);
        }
    }
    fn globe(&mut self, at: Vec3, radii: Vec3, color: [f32; 3]) {
        let p = |j: u32, k: u32| {
            let (sy, cy) = math::sin_cos(j as f32 * std::f32::consts::PI / 6.);
            let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / 8.);
            at + radii * Vec3::new(c * sy, cy, s * sy)
        };
        for j in 0..6 {
            for k in 0..8 {
                let (a, b, c, d) = (p(j, k), p(j, k + 1), p(j + 1, k), p(j + 1, k + 1));
                if j > 0 {
                    self.tri(a, b, c, color);
                }
                if j < 5 {
                    self.tri(b, d, c, color);
                }
            }
        }
    }
    fn finish(mut self) -> MeshData {
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

pub fn body() -> MeshData {
    let mut m = Model::default();
    let fur = [0.31, 0.21, 0.13];
    let hoof = [0.12, 0.10, 0.085];
    m.globe(Vec3::new(0., 0.05, 0.), Vec3::new(0.49, 0.77, 0.37), fur);
    m.globe(
        Vec3::new(0., 0.48, 0.06),
        Vec3::new(0.57, 0.43, 0.38),
        [0.40, 0.29, 0.18],
    );
    for sign in [-1., 1.] {
        let v = |x: f32, y: f32, z: f32| Vec3::new(x * sign, y, z);
        m.bone(v(0.24, -0.42, 0.), v(0.33, -0.97, -0.13), 0.18, 0.10, fur);
        m.bone(
            v(0.33, -0.97, -0.13),
            v(0.30, -1.48, 0.10),
            0.10,
            0.065,
            fur,
        );
        m.globe(v(0.30, -1.49, 0.17), Vec3::new(0.14, 0.10, 0.22), hoof);
        m.bone(v(0.42, 0.42, 0.), v(0.70, -0.10, 0.05), 0.18, 0.10, fur);
        m.bone(v(0.70, -0.10, 0.05), v(0.78, -0.85, 0.30), 0.10, 0.065, fur);
        for finger in [-0.07, 0., 0.07] {
            m.bone(
                v(0.78 + finger, -0.83, 0.3),
                v(0.8 + finger, -1.03, 0.42),
                0.035,
                0.012,
                hoof,
            );
        }
    }
    m.finish()
}
pub fn head() -> MeshData {
    let mut m = Model::default();
    let bone = [0.82, 0.76, 0.60];
    let fur = [0.42, 0.31, 0.20];
    m.globe(Vec3::new(0., 0.35, 0.31), Vec3::new(0.33, 0.48, 0.37), fur);
    m.globe(Vec3::new(0., 0.25, 0.67), Vec3::new(0.23, 0.25, 0.52), bone);
    m.globe(
        Vec3::new(0., 0.20, 1.05),
        Vec3::new(0.18, 0.14, 0.11),
        [0.16, 0.12, 0.10],
    );
    for sign in [-1., 1.] {
        let v = |x: f32, y: f32, z: f32| Vec3::new(x * sign, y, z);
        m.bone(v(0.23, 0.52, 0.23), v(0.72, 0.87, 0.26), 0.15, 0.01, fur);
        m.bone(v(0.22, 0.70, 0.13), v(0.50, 1.15, 0.00), 0.095, 0.075, bone);
        m.bone(v(0.50, 1.15, 0.00), v(0.96, 1.44, -0.12), 0.075, 0.05, bone);
        m.bone(
            v(0.96, 1.44, -0.12),
            v(1.28, 1.73, -0.03),
            0.05,
            0.008,
            bone,
        );
        m.bone(v(0.43, 1.04, 0.04), v(0.35, 1.54, 0.19), 0.055, 0.008, bone);
        m.bone(
            v(0.71, 1.29, -0.06),
            v(0.72, 1.80, 0.09),
            0.055,
            0.008,
            bone,
        );
        m.bone(
            v(0.98, 1.46, -0.11),
            v(1.02, 1.93, -0.18),
            0.045,
            0.008,
            bone,
        );
    }
    m.finish()
}

pub fn sounds(w: &mut World) {
    w.sounds([
        (
            "deer-warning",
            Synth::saw(74.)
                .seconds(0.82)
                .attack(0.04)
                .decay(0.18)
                .sustain(0.65)
                .lowpass_hz(580.)
                .slide(-6.)
                .gain(0.23)
                .layer(
                    Synth::noise()
                        .seconds(0.82)
                        .attack(0.06)
                        .decay(0.55)
                        .lowpass_hz(1000.)
                        .gain(0.10),
                ),
        ),
        (
            "deer-stunned",
            Synth::triangle(520.)
                .seconds(0.36)
                .slide(-14.)
                .decay(0.28)
                .gain(0.17),
        ),
    ]);
    w.insert(w.named("camera").unwrap(), AudioListener);
}

pub fn present(w: &World, previous: Mind) {
    let deer = *w.require::<Deer>("deer");
    let pitch = match deer.mind {
        Mind::Windup => 0.15 + 0.65 * (1. - (deer.timer / WINDUP).clamp(0., 1.)),
        Mind::Chase => 0.8,
        Mind::Stunned => 0.95,
        _ => 0.,
    };
    let next = Transform {
        position: Vec3::new(0., 0.70, 0.10),
        rotation: Quat::from_rotation_x(pitch),
        ..Transform::default()
    };
    if *w.require::<Transform>("deer-head") != next {
        *w.require_mut::<Transform>("deer-head") = next;
    }
    if deer.mind != previous {
        let glow = match deer.mind {
            Mind::Windup | Mind::Chase => [7., 1.4, 0.1],
            Mind::Stunned => [0.15, 3., 4.],
            _ => [4., 0.25, 0.1],
        };
        for eye in ["deer-eye-l", "deer-eye-r"] {
            *w.require_mut::<Material>(eye) = Material::glow(glow);
        }
        let sound = match deer.mind {
            Mind::Windup => Some("deer-warning"),
            Mind::Stunned => Some("deer-stunned"),
            _ => None,
        };
        if let Some(sound) = sound {
            w.play(sound)
                .at("deer")
                .spatial(Spatial {
                    ref_distance: 6.,
                    ..Spatial::default()
                })
                .start();
        }
    }
}
