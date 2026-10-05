//! The looks' character, tools, crops and fruit.
use crate::{
    crops::CROPS,
    looks::{dir, Look},
    sculpt::*,
};
use exact_game::{
    asset::{MaterialData, MeshData, Model},
    *,
};
use std::f32::consts::{PI, TAU};

pub(crate) fn gardener(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let toy = l.toy;
    let (head_at, head_r) = if toy {
        (Vec3::new(0., 0.47, 0.), Vec3::new(0.32, 0.31, 0.30))
    } else {
        (Vec3::new(0., 0.44, 0.), Vec3::new(0.25, 0.27, 0.24))
    };
    let body = |c: Rgb| move |d: Vec3| l.soft(c, d);
    // Shirt, then the overalls over its lower half, bib and straps.
    let (torso_at, torso_r) = if toy {
        (Vec3::new(0., -0.06, 0.), Vec3::new(0.28, 0.29, 0.22))
    } else {
        (Vec3::new(0., -0.02, 0.), Vec3::new(0.30, 0.33, 0.21))
    };
    m.ellipsoid(torso_at, torso_r, Quat::IDENTITY, (12, 18), body(l.shirt));
    m.ellipsoid(
        Vec3::new(0., -0.17, 0.),
        Vec3::new(torso_r.x + 0.015, 0.21, torso_r.z + 0.015),
        Quat::IDENTITY,
        (10, 18),
        body(l.denim),
    );
    m.slab(
        Vec3::new(0., 0.0, torso_r.z - 0.02),
        Vec3::new(0.30, 0.27, 0.05),
        Quat::from_rotation_x(-0.12),
        l.denim,
        0.25,
    );
    m.slab(
        Vec3::new(0., -0.01, torso_r.z + 0.008),
        Vec3::new(0.13, 0.09, 0.012),
        Quat::from_rotation_x(-0.12),
        scale(l.denim, 1.18),
        0.1,
    );
    for x in [-0.11f32, 0.11] {
        m.slab(
            Vec3::new(x, 0.18, 0.08),
            Vec3::new(0.06, 0.05, 0.24),
            Quat::from_rotation_x(0.35),
            l.denim,
            0.2,
        );
        m.ball(
            Vec3::new(x, 0.11, torso_r.z + 0.03),
            Vec3::splat(0.026),
            scale(l.metal, 0.7),
            l.metal,
            0.6,
        );
    }
    if toy {
        // A neckerchief.
        m.ellipsoid(
            Vec3::new(0., 0.19, 0.06),
            Vec3::new(0.21, 0.06, 0.17),
            Quat::IDENTITY,
            (6, 14),
            body(l.band),
        );
        m.ellipsoid(
            Vec3::new(0., 0.13, 0.19),
            Vec3::new(0.07, 0.08, 0.03),
            Quat::from_rotation_z(PI / 4.),
            (6, 8),
            body(l.band),
        );
    }
    // Head: skin with painted cheeks.
    let cheek = l.cheek;
    let skin = l.skin;
    m.ellipsoid(head_at, head_r, Quat::IDENTITY, (14, 20), |d| {
        let blush = if d.z > 0.2 {
            let dx = d.x.abs() - 0.55;
            let dy = d.y + 0.2;
            math::exp(-(dx * dx + dy * dy) * if toy { 14. } else { 20. })
        } else {
            0.
        };
        l.soft(mix(skin, cheek, blush * if toy { 0.9 } else { 0.5 }), d)
    });
    let front = head_at.z + head_r.z;
    let at = |x: f32, y: f32, inset: f32| Vec3::new(x, head_at.y + y, front - inset);
    // Hair, showing below the hat at the sides and back.
    m.ellipsoid(
        head_at + Vec3::new(0., 0.06, -0.04),
        head_r * Vec3::new(1.04, 0.74, 1.04),
        Quat::IDENTITY,
        (8, 16),
        body(l.hair),
    );
    // Nose, ears.
    m.ball(
        at(0., -0.02, 0.0),
        Vec3::new(0.05, 0.045, 0.045) * if toy { 0.9 } else { 1. },
        scale(skin, 0.85),
        mix(skin, cheek, 0.35),
        0.3,
    );
    for s in [-1f32, 1.] {
        m.ball(
            head_at + Vec3::new(s * head_r.x * 0.98, -0.01, 0.),
            Vec3::new(0.035, 0.06, 0.045),
            scale(skin, 0.8),
            skin,
            0.,
        );
    }
    // Eyes with a catch-light.
    let (eye_x, eye_r) = if toy {
        (0.115, Vec3::new(0.042, 0.062, 0.03))
    } else {
        (0.085, Vec3::new(0.026, 0.034, 0.02))
    };
    for s in [-1f32, 1.] {
        let eye = at(s * eye_x, 0.045, if toy { 0.035 } else { 0.022 });
        m.ball(eye, eye_r, [0.10, 0.08, 0.07], [0.18, 0.14, 0.12], 0.);
        m.ball(
            eye + Vec3::new(0.012, 0.018, eye_r.z * 0.9),
            Vec3::splat(eye_r.x * 0.38),
            [1.0; 3],
            [1.2; 3],
            0.,
        );
        if !toy {
            m.slab(
                at(s * 0.09, 0.10, 0.03),
                Vec3::new(0.08, 0.022, 0.025),
                Quat::from_rotation_z(-s * 0.18),
                l.hair,
                0.,
            );
        }
    }
    if toy {
        m.slab(
            at(0., -0.10, 0.03),
            Vec3::new(0.07, 0.016, 0.015),
            Quat::IDENTITY,
            [0.55, 0.22, 0.20],
            0.,
        );
    } else {
        // A moustache.
        m.ellipsoid(
            at(0., -0.065, 0.012),
            Vec3::new(0.085, 0.024, 0.028),
            Quat::IDENTITY,
            (6, 12),
            body(l.hair),
        );
    }
    // Straw hat: a woven brim, crown and band.
    let brim_y = head_at.y + head_r.y * 0.82;
    let straw = l.straw;
    let weave = move |d: Vec3| {
        let r = math::sqrt(d.x * d.x + d.z * d.z);
        let ring = 0.92 + 0.08 * math::sin(r * 40.);
        l.soft(scale(straw, ring), d)
    };
    m.ellipsoid(
        Vec3::Y * brim_y,
        Vec3::new(0.50, 0.03, 0.50) * if toy { 1.06 } else { 1. },
        Quat::from_rotation_x(-0.08),
        (6, 28),
        weave,
    );
    let crown = Vec3::new(head_r.x * 1.0, 0.16, head_r.z * 1.0);
    m.ellipsoid(
        Vec3::Y * (brim_y + 0.10),
        crown,
        Quat::IDENTITY,
        (8, 20),
        weave,
    );
    m.ellipsoid(
        Vec3::Y * (brim_y + 0.045),
        Vec3::new(crown.x + 0.012, 0.05, crown.z + 0.012),
        Quat::IDENTITY,
        (6, 20),
        body(l.band),
    );
    if toy {
        // A daisy tucked in the band.
        let c = Vec3::new(crown.x * 0.8, brim_y + 0.06, crown.z * 0.6);
        for i in 0..6 {
            let d = dir(i as f32 * TAU / 6.);
            m.ball(
                c + Vec3::new(d.x * 0.045, d.z * 0.045, 0.02),
                Vec3::splat(0.03),
                [0.9; 3],
                [1.0; 3],
                0.,
            );
        }
        m.ball(
            c + Vec3::Z * 0.035,
            Vec3::splat(0.028),
            [1.0, 0.7, 0.2],
            [1.0, 0.85, 0.3],
            0.,
        );
    }
    // Satchel on the right hip.
    m.slab(
        Vec3::new(0.32, -0.16, -0.21),
        Vec3::new(0.25, 0.32, 0.22),
        Quat::from_rotation_y(0.2),
        l.bag,
        0.35,
    );
    m.slab(
        Vec3::new(0.33, -0.05, -0.19),
        Vec3::new(0.27, 0.14, 0.25),
        Quat::from_rotation_y(0.2),
        scale(l.bag, 1.12),
        0.2,
    );
    m.finish()
}

pub(crate) fn arm(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let sleeve = l.shirt;
    let soft = |c: Rgb| move |_: f32, a: f32| scale(c, 0.85 + 0.15 * math::cos(a));
    m.ball(
        Vec3::ZERO,
        Vec3::splat(0.115),
        scale(sleeve, 0.8),
        sleeve,
        0.,
    );
    m.tube(
        &[(Vec3::ZERO, 0.11), (Vec3::new(0., -0.30, 0.01), 0.092)],
        12,
        false,
        soft(sleeve),
    );
    m.ellipsoid(
        Vec3::new(0., -0.30, 0.01),
        Vec3::new(0.10, 0.035, 0.10),
        Quat::IDENTITY,
        (5, 12),
        |d| l.soft(scale(sleeve, 1.12), d),
    );
    m.tube(
        &[
            (Vec3::new(0., -0.30, 0.01), 0.075),
            (Vec3::new(0., -0.44, 0.02), 0.07),
        ],
        10,
        false,
        soft(l.skin),
    );
    let hand = if l.toy { 0.11 } else { 0.088 };
    m.ellipsoid(
        Vec3::new(0., -0.49, 0.02),
        Vec3::new(hand, hand * 1.08, hand * 0.95),
        Quat::IDENTITY,
        (8, 12),
        |d| l.soft(l.glove, d),
    );
    m.finish()
}

pub(crate) fn leg(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let soft = |c: Rgb| move |_: f32, a: f32| scale(c, 0.85 + 0.15 * math::cos(a));
    m.tube(
        &[
            (Vec3::new(0., 0.04, 0.), 0.13),
            (Vec3::new(0., -0.38, 0.01), 0.118),
        ],
        12,
        false,
        soft(l.denim),
    );
    m.ellipsoid(
        Vec3::new(0., -0.37, 0.01),
        Vec3::new(0.128, 0.035, 0.128),
        Quat::IDENTITY,
        (5, 12),
        |d| l.soft(scale(l.denim, 1.2), d),
    );
    m.ellipsoid(
        Vec3::new(0., -0.49, 0.06),
        Vec3::new(0.14, 0.10, 0.21),
        Quat::IDENTITY,
        (8, 14),
        |d| l.soft(l.boot, d),
    );
    m.slab(
        Vec3::new(0., -0.575, 0.06),
        Vec3::new(0.27, 0.04, 0.41),
        Quat::IDENTITY,
        scale(l.boot, 0.45),
        0.,
    );
    m.finish()
}

pub(crate) fn watering_can(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let can = l.can;
    let metal = move |_: f32, a: f32| {
        let glint = math::powi(math::cos(a - 2.2).max(0.), 12);
        scale(can, 0.8 + 0.2 * math::cos(a - 2.2).max(0.) + 0.5 * glint)
    };
    m.lathe(
        Vec3::new(0., -0.20, 0.),
        &[
            (0., 0.),
            (0., 0.19),
            (0.015, 0.2),
            (0.03, 0.2),
            (0.27, 0.18),
            (0.29, 0.172),
            (0.30, 0.15),
            (0.30, 0.),
        ],
        20,
        metal,
    );
    for y in [-0.17f32, 0.06] {
        m.lathe(
            Vec3::new(0., y, 0.),
            &[(0., 0.198), (0.02, 0.2), (0.04, 0.197)],
            20,
            |_, _| l.trim,
        );
    }
    m.tube(
        &[
            (Vec3::new(0., -0.12, 0.15), 0.032),
            (Vec3::new(0., -0.02, 0.30), 0.026),
            (Vec3::new(0., 0.06, 0.44), 0.022),
        ],
        8,
        false,
        |_, a| scale(can, 0.85 + 0.15 * math::cos(a)),
    );
    m.ellipsoid(
        Vec3::new(0., 0.075, 0.465),
        Vec3::new(0.06, 0.028, 0.06),
        Quat::from_rotation_x(0.75),
        (5, 12),
        |d| scale(l.trim, 0.85 + 0.25 * d.y.max(0.)),
    );
    m.tube(
        &[
            (Vec3::new(0., 0.09, -0.13), 0.022),
            (Vec3::new(0., 0.24, -0.09), 0.022),
            (Vec3::new(0., 0.29, 0.0), 0.022),
            (Vec3::new(0., 0.24, 0.08), 0.022),
            (Vec3::new(0., 0.10, 0.10), 0.022),
        ],
        8,
        false,
        |_, a| scale(l.trim, 0.85 + 0.15 * math::cos(a)),
    );
    m.finish()
}

// ------------------------------------------------------------------ crops

/// The hilled soil every plant stands in; it grows with the plant.
fn mound(m: &mut Sculpt, l: &Look, base: f32, radius: f32) {
    let (low, high) = l.mound;
    m.ellipsoid(
        Vec3::Y * base,
        Vec3::new(radius, 0.07 + radius * 0.06, radius),
        Quat::IDENTITY,
        (5, 14),
        |d| mix(low, high, d.y * 0.8),
    );
}

/// A fan of leaves around a point, golden-angle spaced.
#[allow(clippy::too_many_arguments)]
fn rosette(
    m: &mut Sculpt,
    root: Vec3,
    count: u32,
    reach: f32,
    lift: f32,
    width: f32,
    shape: (f32, f32, f32),
    tones: (Rgb, Rgb),
    phase: f32,
) {
    for i in 0..count {
        let a = phase + i as f32 * 2.39996;
        let k = 0.8 + 0.4 * hash(i * 7 + (phase * 100.) as u32);
        let tip = root + dir(a) * reach * k + Vec3::Y * lift * k;
        m.leaf(
            root,
            tip,
            width * k,
            shape.0,
            shape.1,
            shape.2,
            tones.0,
            tones.1,
        );
    }
}

fn stalk(m: &mut Sculpt, from: Vec3, to: Vec3, r: (f32, f32), color: Rgb, segs: u32) {
    let mid = from.lerp(to, 0.5) + Vec3::new(0.02, 0., -0.015);
    m.tube(
        &[(from, r.0), (mid, (r.0 + r.1) * 0.5), (to, r.1)],
        segs,
        true,
        move |t, a| {
            scale(
                color,
                (0.78 + 0.22 * t) * (0.88 + 0.12 * math::cos(a - 2.0)),
            )
        },
    );
}

pub(crate) fn plant(kind: usize, l: &Look) -> MeshData {
    let c = &CROPS[kind];
    let mut m = Sculpt::default();
    let h = c.height;
    let base = -h * 0.5;
    let at = |y: f32| Vec3::Y * (base + y);
    let tones = l.leaf_tones(c);
    let stem = scale(tones.0, 0.95);
    let toy = l.toy;
    // Chunkier, rounder leaves in the storybook look.
    let round = |r: f32| if toy { (r + 0.35).min(1.) } else { r };
    let wide = if toy { 1.35 } else { 1. };
    if toy {
        m.leafy(1.35, 2.0);
    } else {
        m.leafy(1.3, 2.1);
    }
    match c.id {
        "carrot" => {
            mound(&mut m, l, base, 0.30);
            let n = if toy { 7 } else { 11 };
            rosette(
                &mut m,
                at(0.03),
                n,
                0.14,
                h * 0.85,
                0.035 * wide,
                (round(0.15), 0.25, 0.45),
                tones,
                0.3,
            );
        }
        "strawberry" => {
            mound(&mut m, l, base, 0.42);
            for i in 0..7u32 {
                let a = i as f32 * 2.39996;
                let end = at(h * (0.45 + 0.3 * hash(i))) + dir(a) * 0.26;
                stalk(&mut m, at(0.04), end, (0.012, 0.009), stem, 5);
                for j in 0..3 {
                    let b = a + (j as f32 - 1.) * 0.75;
                    let tip = end + dir(b) * 0.15 + Vec3::Y * 0.03;
                    m.leaf(
                        end,
                        tip,
                        0.065 * wide,
                        round(0.85),
                        0.2,
                        0.3,
                        tones.0,
                        tones.1,
                    );
                }
            }
        }
        "blueberry" => {
            mound(&mut m, l, base, 0.40);
            for i in 0..5u32 {
                let a = i as f32 * TAU / 5. + 0.4;
                let top = at(h * (0.8 + 0.2 * hash(i + 3))) + dir(a) * 0.22;
                stalk(&mut m, at(0.02), top, (0.022, 0.01), l.bark, 6);
                for j in 0..7u32 {
                    let t = 0.35 + 0.65 * j as f32 / 6.;
                    let p = at(0.02).lerp(top, t);
                    let b = a + j as f32 * 2.1;
                    let tip = p + dir(b) * 0.11 + Vec3::Y * 0.05;
                    m.leaf(p, tip, 0.04 * wide, round(0.6), 0.1, 0.2, tones.0, tones.1);
                }
            }
        }
        "tomato" => {
            mound(&mut m, l, base, 0.40);
            m.slab(
                at(h * 0.52) + Vec3::new(-0.12, 0., -0.1),
                Vec3::new(0.035, h * 1.04, 0.035),
                Quat::IDENTITY,
                l.wood,
                0.35,
            );
            let top = at(h * 0.95);
            stalk(&mut m, at(0.02), top, (0.03, 0.015), stem, 7);
            for i in 0..9u32 {
                let t = 0.2 + 0.8 * i as f32 / 8.;
                let p = at(0.02).lerp(top, t);
                let a = i as f32 * 2.39996;
                let tip = p + dir(a) * (0.30 - 0.12 * t) + Vec3::Y * 0.04;
                m.leaf(
                    p,
                    tip,
                    0.08 * wide,
                    round(0.5),
                    0.45,
                    0.35,
                    tones.0,
                    tones.1,
                );
            }
        }
        "corn" => {
            mound(&mut m, l, base, 0.32);
            let top = at(h * 0.96);
            stalk(&mut m, at(0.), top, (0.045, 0.02), stem, 8);
            for i in 0..8u32 {
                let t = 0.12 + 0.75 * i as f32 / 7.;
                let p = at(0.).lerp(top, t);
                let a = i as f32 * PI + hash(i) * 0.6;
                let tip = p + dir(a) * (0.6 - 0.25 * t) + Vec3::Y * 0.35;
                m.leaf(
                    p,
                    tip,
                    0.055 * wide,
                    round(0.05),
                    0.9,
                    0.5,
                    tones.0,
                    tones.1,
                );
            }
            let tassel = scale(l.straw, 0.9);
            for i in 0..6u32 {
                let a = i as f32 * TAU / 6.;
                let tip = top + dir(a) * 0.12 + Vec3::Y * 0.2;
                m.leaf(top, tip, 0.012, 0., 0.3, 0., scale(tassel, 0.8), tassel);
            }
        }
        "watermelon" | "pumpkin" => {
            mound(&mut m, l, base, 0.5);
            for i in 0..4u32 {
                let a = i as f32 * TAU / 4. + 0.3;
                let p = |r: f32, y: f32| at(y) + dir(a + r * 1.2) * r;
                m.tube(
                    &[
                        (p(0., 0.04), 0.02),
                        (p(0.3, 0.05), 0.016),
                        (p(0.6, 0.03), 0.012),
                    ],
                    6,
                    true,
                    |_, _| stem,
                );
            }
            rosette(
                &mut m,
                at(0.04),
                7,
                0.48,
                0.08,
                0.17 * wide,
                (1., 0.35, 0.25),
                tones,
                0.1,
            );
            rosette(
                &mut m,
                at(0.06),
                5,
                0.22,
                h * 0.8,
                0.13 * wide,
                (1., 0.2, 0.3),
                tones,
                1.3,
            );
        }
        "apple" | "mango" => {
            mound(&mut m, l, base, 0.5);
            let crown = at(h * 0.62);
            stalk(&mut m, at(0.), crown, (0.11, 0.065), l.bark, 10);
            let lobes = if toy { 3 } else { c.slots as u32 };
            for i in 0..lobes {
                // Between the fruit slots, so hanging fruit shows.
                let a = (i as f32 + 0.5) / lobes as f32 * TAU;
                let centre = at(h * 0.78) + dir(a) * if toy { 0.38 } else { 0.45 };
                m.tube(&[(crown, 0.05), (centre, 0.025)], 6, false, |_, _| l.bark);
                let tone = l.canopy[(i % 3) as usize];
                let tone = mix(tone, saturate(c.leaf, l.sat), 0.5);
                let r = if toy { 0.5 } else { 0.36 };
                m.ellipsoid(
                    centre,
                    Vec3::new(r, r * 0.82, r),
                    Quat::from_rotation_y(a),
                    (8, 12),
                    |d| l.foliage(tone, d),
                );
            }
            let tone = mix(l.canopy[1], saturate(c.leaf, l.sat), 0.5);
            m.ellipsoid(
                at(h + 0.05),
                Vec3::new(0.48, 0.36, 0.48) * if toy { 1.15 } else { 1. },
                Quat::IDENTITY,
                (10, 14),
                |d| l.foliage(tone, d),
            );
            if !toy {
                // Leaves breaking the silhouette.
                for i in 0..14u32 {
                    let a = i as f32 * 2.39996;
                    let p = at(h * (0.75 + 0.3 * hash(i + 40))) + dir(a) * 0.55;
                    let tip = p + dir(a + 0.4) * 0.2 + Vec3::Y * 0.02;
                    m.leaf(p, tip, 0.06, 0.55, 0.3, 0.3, tones.0, tones.1);
                }
            }
        }
        "coconut" => {
            mound(&mut m, l, base, 0.45);
            let pts: Vec<(Vec3, f32)> = (0..=8)
                .map(|i| {
                    let t = i as f32 / 8.;
                    (at(h * 0.92 * t) + Vec3::X * (0.25 * t * t), 0.11 - 0.05 * t)
                })
                .collect();
            let bark = l.bark;
            m.tube(&pts, 10, true, move |t, _| {
                let ring = 0.82 + 0.18 * math::sin(t * 60.).abs();
                scale(bark, ring * (0.8 + 0.3 * t))
            });
            let top = pts[8].0;
            rosette(
                &mut m,
                top,
                9,
                1.1,
                -0.15,
                0.13 * wide,
                (round(0.1), 0.75, 0.55),
                tones,
                0.2,
            );
            rosette(
                &mut m,
                top,
                5,
                0.7,
                0.35,
                0.10 * wide,
                (round(0.1), 0.6, 0.55),
                tones,
                1.0,
            );
        }
        "bamboo" => {
            mound(&mut m, l, base, 0.38);
            for i in 0..5u32 {
                let off = dir(i as f32 * 2.39996) * (0.08 + 0.12 * hash(i));
                let top = at(h * (0.75 + 0.25 * hash(i + 9))) + off * 1.6;
                let cane = saturate(c.fruit, l.sat * 0.9);
                m.tube(
                    &[(at(0.) + off, 0.04), (top, 0.032)],
                    8,
                    true,
                    move |t, a| {
                        let node = math::sin(t * 8. * PI).abs();
                        let band = if node < 0.12 { 0.72 } else { 1. };
                        scale(cane, band * (0.82 + 0.18 * math::cos(a - 2.)))
                    },
                );
                for j in 0..3u32 {
                    let p = at(0.).lerp(top, 0.55 + 0.2 * j as f32) + off * 0.2;
                    rosette(
                        &mut m,
                        p,
                        3,
                        0.26,
                        0.02,
                        0.028 * wide,
                        (round(0.15), 0.4, 0.2),
                        tones,
                        (i * 3 + j) as f32,
                    );
                }
            }
        }
        "cactus" => {
            mound(&mut m, l, base, 0.38);
            let tone = saturate(c.leaf, l.sat);
            let ribbed = move |_: f32, a: f32| {
                let rib = 0.8 + 0.2 * math::cos(a * 8.).abs();
                scale(tone, rib)
            };
            let column = |m: &mut Sculpt, from: Vec3, height: f32, r: f32| {
                let mut pts: Vec<(Vec3, f32)> = vec![(from, r)];
                pts.push((from + Vec3::Y * (height - r), r));
                for k in 1..=4 {
                    let t = k as f32 / 4. * PI / 2.;
                    pts.push((
                        from + Vec3::Y * (height - r + math::sin(t) * r),
                        math::cos(t) * r + 0.001,
                    ));
                }
                m.tube(&pts, 16, false, ribbed);
            };
            column(&mut m, at(0.), h * 0.92, 0.16);
            for (s, y, len) in [(-1f32, 0.35, 0.35), (1., 0.5, 0.28)] {
                let from = at(h * y);
                let elbow = from + Vec3::new(s * 0.3, 0.05, 0.);
                m.tube(&[(from, 0.09), (elbow, 0.09)], 12, false, ribbed);
                column(&mut m, elbow - Vec3::Y * 0.04, len, 0.09);
            }
            for i in 0..3u32 {
                m.ball(
                    at(h * 0.92) + dir(i as f32 * 2.1) * 0.07,
                    Vec3::splat(0.04),
                    [0.95, 0.55, 0.70],
                    [1.0, 0.75, 0.85],
                    0.,
                );
            }
        }
        "dragon" => {
            mound(&mut m, l, base, 0.38);
            m.slab(
                at(h * 0.5),
                Vec3::new(0.09, h, 0.09),
                Quat::IDENTITY,
                l.wood,
                0.4,
            );
            let tone = saturate(c.leaf, l.sat);
            for i in 0..7u32 {
                let a = i as f32 * TAU / 7.;
                let top = at(h * 0.95);
                let p = |r: f32, y: f32| top + dir(a) * r + Vec3::Y * y;
                m.tube(
                    &[
                        (p(0.02, 0.0), 0.05),
                        (p(0.28, 0.06), 0.05),
                        (p(0.45, -0.15), 0.045),
                        (p(0.52, -0.55 - 0.2 * hash(i)), 0.035),
                    ],
                    6,
                    true,
                    move |t, a| {
                        scale(
                            tone,
                            (0.75 + 0.25 * math::cos(a * 3.).abs()) * (1. - 0.2 * t),
                        )
                    },
                );
            }
        }
        "grape" => {
            mound(&mut m, l, base, 0.42);
            for x in [-0.42f32, 0.42] {
                m.slab(
                    at(h * 0.55) + Vec3::X * x,
                    Vec3::new(0.06, h * 1.1, 0.06),
                    Quat::IDENTITY,
                    l.wood,
                    0.4,
                );
            }
            m.slab(
                at(h * 1.08),
                Vec3::new(1.0, 0.05, 0.06),
                Quat::IDENTITY,
                l.wood,
                0.2,
            );
            let vine = scale(l.bark, 1.1);
            m.tube(
                &[
                    (at(0.), 0.03),
                    (at(h * 0.5) + Vec3::new(0.05, 0., 0.04), 0.025),
                    (at(h * 1.0), 0.02),
                ],
                6,
                false,
                |_, _| vine,
            );
            for i in 0..14u32 {
                let x = -0.45 + 0.9 * i as f32 / 13.;
                let p = at(h * (1.0 + 0.06 * hash(i))) + Vec3::X * x;
                let a = i as f32 * 2.39996;
                let tip = p + dir(a) * 0.2 - Vec3::Y * 0.06;
                m.leaf(
                    p,
                    tip,
                    0.1 * wide,
                    round(0.85),
                    0.35,
                    0.25,
                    tones.0,
                    tones.1,
                );
            }
            rosette(
                &mut m,
                at(h * 0.45),
                5,
                0.22,
                0.02,
                0.09 * wide,
                (round(0.85), 0.3, 0.2),
                tones,
                0.7,
            );
        }
        _ => {
            mound(&mut m, l, base, 0.35);
            rosette(
                &mut m,
                at(0.02),
                8,
                0.3,
                h * 0.7,
                0.07 * wide,
                (0.5, 0.3, 0.3),
                tones,
                0.,
            );
        }
    }
    m.finish()
}

/// Fruit is tinted by its entity's material (ripeness, mutations), so the
/// mesh carries only shading, in greys. The skin is its own part, as glossy as
/// its crop (glossier in the toy look); seeds are gilt, stems and leaves matte.
pub(crate) fn fruit(kind: usize, l: &Look) -> Model {
    let c = &CROPS[kind];
    let (mut skin, mut m, mut gilt) = (Sculpt::default(), Sculpt::default(), Sculpt::default());
    let r = c.fruit_size;
    let shine = |d: Vec3, base: f32| [base * (0.7 + 0.3 * (d.y * 0.5 + 0.5)); 3];
    let roughness = match c.id {
        "tomato" => 0.2,
        "apple" | "grape" => 0.25,
        "strawberry" | "watermelon" | "mango" | "dragon" => 0.3,
        "blueberry" | "corn" | "pumpkin" => 0.45,
        "bamboo" | "cactus" => 0.5,
        "carrot" => 0.6,
        "coconut" => 0.85,
        _ => 0.35,
    } * if l.toy { 0.75 } else { 1. };
    let dark = [0.42; 3];
    match c.id {
        "carrot" => {
            let profile: Vec<(f32, f32)> = (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.;
                    (-r * 1.25 + t * r * 2.4, r * 0.68 * math::powf(t, 0.6))
                })
                .chain([(r * 1.17, r * 0.4), (r * 1.18, 0.)])
                .collect();
            skin.lathe(Vec3::ZERO, &profile, 12, |y, a| {
                let groove = 0.86 + 0.14 * math::sin(y * 95.).abs();
                let v = groove * (0.85 + 0.15 * math::cos(a - 2.2));
                [v; 3]
            });
            m.tube(
                &[(Vec3::Y * r * 1.1, 0.03), (Vec3::Y * r * 1.5, 0.02)],
                6,
                true,
                |_, _| dark,
            );
        }
        "strawberry" => {
            skin.lathe(
                Vec3::ZERO,
                &[
                    (-r * 1.1, 0.),
                    (-r * 0.9, r * 0.35),
                    (-r * 0.4, r * 0.72),
                    (r * 0.1, r * 0.86),
                    (r * 0.45, r * 0.78),
                    (r * 0.6, r * 0.5),
                    (r * 0.62, 0.),
                ],
                14,
                |y, a| {
                    let (s, c) = math::sin_cos(a);
                    shine(Vec3::new(c, y / r, s).normalize(), 1.)
                },
            );
            for i in 0..18u32 {
                let a = i as f32 * 2.39996;
                let y = -r * 0.8 + r * 1.2 * hash(i + 5);
                let rad = r * (0.86 - 0.5 * ((y / r - 0.1) * 0.9).abs().min(1.)) * 0.98;
                gilt.ball(
                    Vec3::Y * y + dir(a) * rad,
                    Vec3::splat(r * 0.06),
                    [0.85, 0.68, 0.3],
                    [1.0, 0.85, 0.45],
                    0.,
                );
            }
            rosette(
                &mut m,
                Vec3::Y * r * 0.58,
                6,
                r * 0.6,
                -r * 0.1,
                r * 0.2,
                (0.4, 0.2, 0.1),
                (dark, [0.5; 3]),
                0.,
            );
        }
        "blueberry" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r, r * 0.9, r),
                Quat::IDENTITY,
                (10, 14),
                |d| shine(d, 0.95),
            );
            rosette(
                &mut m,
                Vec3::Y * r * 0.85,
                5,
                r * 0.3,
                r * 0.15,
                r * 0.12,
                (0.4, 0., 0.),
                (dark, [0.6; 3]),
                0.,
            );
        }
        "tomato" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r, r * 0.82, r),
                Quat::IDENTITY,
                (12, 18),
                |d| {
                    let a = math::atan2(d.z, d.x);
                    let rib = 1. - 0.1 * math::cos(a * 5.).abs() * (1. - d.y.abs());
                    shine(d, rib)
                },
            );
            rosette(
                &mut m,
                Vec3::Y * r * 0.8,
                5,
                r * 0.55,
                -r * 0.05,
                r * 0.14,
                (0.2, 0.3, 0.1),
                (dark, [0.45; 3]),
                0.,
            );
            m.tube(
                &[(Vec3::Y * r * 0.78, 0.022), (Vec3::Y * r * 1.05, 0.016)],
                6,
                true,
                |_, _| dark,
            );
        }
        "corn" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r * 0.55, r * 1.3, r * 0.55),
                Quat::IDENTITY,
                (16, 16),
                |d| {
                    let a = math::atan2(d.z, d.x);
                    let row = (math::sin(a * 8.) * math::sin(d.y * 14.)).abs();
                    shine(d, 0.86 + 0.18 * row)
                },
            );
            for i in 0..3u32 {
                let a = i as f32 * TAU / 3.;
                let root = Vec3::Y * -r * 1.25;
                let tip = Vec3::Y * r * 0.4 + dir(a) * r * 0.62;
                m.leaf(root, tip, r * 0.42, 0.4, -0.05, 0.6, [0.55; 3], [0.7; 3]);
            }
        }
        "watermelon" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r * 1.2, r * 0.85, r * 0.85),
                Quat::IDENTITY,
                (14, 26),
                |d| {
                    let a = math::atan2(d.z, d.y);
                    let wobble = math::sin(d.x * 7.) * 0.35;
                    let stripe = math::sin(a * 9. + wobble);
                    let band = if stripe > 0.25 { 0.5 } else { 1.0 };
                    shine(d, band)
                },
            );
        }
        "pumpkin" => {
            for i in 0..9u32 {
                let a = i as f32 * TAU / 9.;
                skin.ellipsoid(
                    dir(a) * r * 0.42,
                    Vec3::new(r * 0.6, r * 0.78, r * 0.48),
                    Quat::from_rotation_y(-a),
                    (10, 12),
                    |d| shine(d, 0.95),
                );
            }
            m.tube(
                &[
                    (Vec3::Y * r * 0.6, r * 0.12),
                    (Vec3::new(r * 0.05, r * 0.92, 0.), r * 0.08),
                    (Vec3::new(r * 0.18, r * 1.05, 0.), r * 0.06),
                ],
                8,
                true,
                |_, a| [0.38 + 0.08 * math::cos(a * 4.); 3],
            );
        }
        "apple" | "mango" => {
            if c.id == "apple" {
                skin.lathe(
                    Vec3::ZERO,
                    &[
                        (-r * 0.85, 0.),
                        (-r * 0.9, r * 0.2),
                        (-r * 0.8, r * 0.55),
                        (-r * 0.4, r * 0.92),
                        (r * 0.15, r * 1.0),
                        (r * 0.6, r * 0.88),
                        (r * 0.82, r * 0.5),
                        (r * 0.78, r * 0.15),
                        (r * 0.62, 0.),
                    ],
                    16,
                    |y, a| {
                        let (s, c) = math::sin_cos(a);
                        shine(Vec3::new(c, y / r, s).normalize(), 1.)
                    },
                );
            } else {
                skin.ellipsoid(
                    Vec3::ZERO,
                    Vec3::new(r * 0.8, r * 1.05, r * 0.68),
                    Quat::from_rotation_z(0.35),
                    (12, 16),
                    |d| shine(d, 0.85 + 0.15 * d.y),
                );
            }
            m.tube(
                &[
                    (Vec3::Y * r * 0.6, 0.016),
                    (Vec3::new(0.02, r * 1.1, 0.), 0.012),
                ],
                5,
                true,
                |_, _| dark,
            );
            m.leaf(
                Vec3::new(0.02, r * 1.0, 0.),
                Vec3::new(r * 0.7, r * 1.2, r * 0.1),
                r * 0.22,
                0.5,
                0.2,
                0.3,
                [0.45; 3],
                [0.6; 3],
            );
        }
        "bamboo" => {
            let profile: Vec<(f32, f32)> = (0..=8)
                .map(|i| {
                    let t = i as f32 / 8.;
                    (-r + t * r * 2.6, r * 0.62 * (1. - t * 0.9))
                })
                .chain([(r * 1.62, 0.)])
                .collect();
            skin.lathe(Vec3::ZERO, &profile, 10, |y, a| {
                let sheath = 0.78 + 0.22 * math::sin((y / r) * 9.).abs();
                [sheath * (0.85 + 0.15 * math::cos(a - 2.2)); 3]
            });
        }
        "coconut" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r, r * 1.08, r),
                Quat::IDENTITY,
                (12, 16),
                |d| {
                    let q = (d * 7.).round();
                    let fiber = 0.82 + 0.3 * hash((q.x * 31. + q.y * 17. + q.z * 7. + 400.) as u32);
                    shine(d, fiber * 0.9)
                },
            );
            for i in 0..3u32 {
                let a = i as f32 * TAU / 3.;
                m.ball(
                    -Vec3::Y * r * 0.95 + dir(a) * r * 0.22,
                    Vec3::splat(r * 0.09),
                    [0.3; 3],
                    [0.35; 3],
                    0.,
                );
            }
        }
        "cactus" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r * 0.72, r, r * 0.72),
                Quat::IDENTITY,
                (10, 14),
                |d| shine(d, 0.95),
            );
            for i in 0..12u32 {
                let a = i as f32 * 2.39996;
                let y = -0.7 + 1.4 * hash(i + 11);
                let ring = math::sqrt(1. - y * y);
                skin.ball(
                    Vec3::new(
                        math::cos(a) * ring * r * 0.72,
                        y * r,
                        math::sin(a) * ring * r * 0.72,
                    ),
                    Vec3::splat(r * 0.06),
                    [0.9; 3],
                    [1.0; 3],
                    0.,
                );
            }
        }
        "dragon" => {
            skin.ellipsoid(
                Vec3::ZERO,
                Vec3::new(r * 0.78, r, r * 0.78),
                Quat::IDENTITY,
                (10, 14),
                |d| shine(d, 1.0),
            );
            for i in 0..10u32 {
                let a = i as f32 * 2.39996;
                let y = -0.6 + 1.2 * (i as f32 / 9.);
                let ring = math::sqrt(1. - y * y);
                let root = Vec3::new(
                    math::cos(a) * ring * r * 0.7,
                    y * r,
                    math::sin(a) * ring * r * 0.7,
                );
                let tip = root * 1.45 + Vec3::Y * r * 0.35;
                m.leaf(root, tip, r * 0.2, 0.3, -0.2, 0.4, [0.9; 3], [0.6; 3]);
            }
        }
        "grape" => {
            for i in 0..14u32 {
                let t = i as f32 / 13.;
                let ring = (1. - t) * 0.75 + 0.1;
                let a = i as f32 * 2.39996;
                let p = Vec3::Y * (r * 0.9 - t * r * 2.0) + dir(a) * ring * r;
                skin.ellipsoid(
                    p,
                    Vec3::new(r * 0.36, r * 0.4, r * 0.36),
                    Quat::IDENTITY,
                    (6, 10),
                    |d| shine(d, 0.95),
                );
            }
            m.tube(
                &[
                    (Vec3::Y * r * 0.9, 0.018),
                    (Vec3::new(0.02, r * 1.4, 0.), 0.012),
                ],
                5,
                true,
                |_, _| dark,
            );
        }
        _ => skin.ellipsoid(Vec3::ZERO, Vec3::splat(r), Quat::IDENTITY, (10, 14), |d| {
            shine(d, 1.)
        }),
    }
    let parts = [
        (skin, MaterialData::surface(0., roughness)),
        (m, MaterialData::surface(0., 0.8)),
        (gilt, MaterialData::surface(1., 0.35)),
    ];
    Model::parts(
        parts
            .into_iter()
            .filter(|(part, _)| !part.is_empty())
            .map(|(part, material)| (part.finish(), material)),
    )
}
