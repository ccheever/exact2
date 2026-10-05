//! The looks' orchard, flowers, meadow, backdrop, border and barrel.
use crate::{
    looks::{dir, Look},
    sculpt::*,
};
use exact_game::{asset::MeshData, *};
use std::f32::consts::{PI, TAU};

fn tree(m: &mut Sculpt, l: &Look, p: Vec3, size: f32, seed: u32) {
    let bark = l.bark;
    let lean = Vec3::new(hash(seed) - 0.5, 0., hash(seed + 1) - 0.5) * 0.4;
    let trunk_top = p + (Vec3::Y * 2.5 + lean) * size;
    m.tube(
        &[
            (p - Vec3::Y * 0.1, 0.3 * size),
            (p + Vec3::Y * 0.4 * size, 0.22 * size),
            (p + (Vec3::Y * 1.5 + lean * 0.5) * size, 0.17 * size),
            (trunk_top, 0.11 * size),
        ],
        12,
        true,
        move |t, a| {
            let streak = 0.86 + 0.14 * math::sin(a * 7. + t * 3.);
            scale(bark, streak * (0.65 + 0.35 * t))
        },
    );
    let crown = trunk_top + Vec3::Y * 0.9 * size;
    let clumps: Vec<(Vec3, f32)> = if l.toy {
        vec![
            (Vec3::new(0., 0.35, 0.), 1.45),
            (Vec3::new(-0.95, -0.25, 0.25), 1.0),
            (Vec3::new(0.9, -0.15, -0.2), 1.05),
            (Vec3::new(0.15, -0.35, 0.9), 0.9),
        ]
    } else {
        (0..9u32)
            .map(|i| {
                let a = i as f32 * 2.39996 + hash(seed + i) * 0.5;
                let up = -0.5 + 1.3 * hash(seed + i + 20);
                let out = 0.6 + 0.6 * hash(seed + i + 40);
                (dir(a) * out + Vec3::Y * up, 0.8 + 0.4 * hash(seed + i + 60))
            })
            .collect()
    };
    for (i, (off, r)) in clumps.iter().enumerate() {
        let centre = crown + *off * size;
        let tone = l.canopy[(i + seed as usize) % 3];
        let tone = mix(tone, l.canopy[1], 0.4);
        if !l.toy {
            m.tube(
                &[
                    (trunk_top - Vec3::Y * 0.3 * size, 0.08 * size),
                    (centre, 0.04 * size),
                ],
                6,
                false,
                move |_, _| bark,
            );
        }
        m.ellipsoid(
            centre,
            Vec3::new(1., 0.84, 1.) * r * size,
            Quat::from_rotation_y(i as f32),
            if l.toy { (12, 18) } else { (9, 14) },
            |d| l.foliage(tone, d),
        );
    }
    let fruit = l.orchard_fruit;
    for i in 0..7u32 {
        let a = i as f32 * 2.39996 + seed as f32;
        let off = clumps[(i as usize) % clumps.len()];
        let d = (dir(a) * 0.8 + Vec3::Y * (hash(i + seed) - 0.6)).normalize();
        let at = crown + (off.0 + d * off.1 * 0.97) * size;
        m.ball(at, Vec3::splat(0.17 * size), scale(fruit, 0.7), fruit, 0.6);
    }
}

fn mushroom(m: &mut Sculpt, at: Vec3, s: f32) {
    m.lathe(
        at,
        &[
            (0., 0.),
            (0., 0.06 * s),
            (0.18 * s, 0.05 * s),
            (0.2 * s, 0.),
        ],
        10,
        |y, _| [0.92 + y; 3],
    );
    m.ellipsoid(
        at + Vec3::Y * 0.2 * s,
        Vec3::new(0.16, 0.1, 0.16) * s,
        Quat::IDENTITY,
        (6, 14),
        |d| scale([0.92, 0.22, 0.2], 0.8 + 0.25 * d.y),
    );
    for i in 0..5u32 {
        let a = i as f32 * 2.39996;
        let d = (dir(a) * 0.7 + Vec3::Y * 0.75).normalize();
        m.ball(
            at + Vec3::Y * 0.2 * s + d * Vec3::new(0.16, 0.1, 0.16) * s,
            Vec3::new(0.03, 0.015, 0.03) * s,
            [0.95; 3],
            [1.0; 3],
            0.,
        );
    }
}

pub(crate) fn orchard(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    for (i, (x, z, size)) in [
        (-5., -5., 1.4),
        (2., -4., 1.1),
        (8., -6., 1.7),
        (15., -3., 1.3),
        (23., -7., 1.8),
    ]
    .into_iter()
    .enumerate()
    {
        let p = Vec3::new(x, 0., z);
        tree(&mut m, l, p, size, i as u32 * 97);
        if l.toy {
            for j in 0..3u32 {
                let a = (i as u32 * 5 + j) as f32 * 2.1;
                mushroom(
                    &mut m,
                    p + dir(a) * (0.7 + 0.4 * hash(j + i as u32)),
                    1.2 + 0.6 * hash(j + 9),
                );
            }
        }
    }
    let fence = l.fence;
    if l.toy {
        // White pickets with rounded rails.
        for i in 0..49u32 {
            let x = -5.0 + i as f32 * 0.55;
            m.slab(
                Vec3::new(x, 0.5, 0.),
                Vec3::new(0.13, 1.0, 0.05),
                Quat::IDENTITY,
                fence,
                0.25,
            );
            m.slab(
                Vec3::new(x, 1.0, 0.),
                Vec3::new(0.092, 0.092, 0.05),
                Quat::from_rotation_z(PI / 4.),
                fence,
                0.,
            );
        }
        for y in [0.32f32, 0.74] {
            m.tube(
                &[
                    (Vec3::new(-5.2, y, -0.05), 0.045),
                    (Vec3::new(21.6, y, -0.05), 0.045),
                ],
                8,
                true,
                move |_, a| scale(fence, 0.85 + 0.15 * math::cos(a - 1.5)),
            );
        }
    } else {
        // Weathered split rails.
        let mut x = -5.0f32;
        let mut i = 0u32;
        while x < 21.5 {
            let tilt = Quat::from_rotation_z((hash(i) - 0.5) * 0.08)
                * Quat::from_rotation_x((hash(i + 50) - 0.5) * 0.06);
            m.slab(
                Vec3::new(x, 0.62, 0.),
                Vec3::new(0.15, 1.25 + 0.1 * hash(i + 3), 0.15),
                tilt,
                scale(fence, 0.9 + 0.15 * hash(i + 7)),
                0.45,
            );
            if x + 2.2 < 21.6 {
                for (k, y) in [0.45f32, 0.92].into_iter().enumerate() {
                    let sag = Quat::from_rotation_z((hash(i * 3 + k as u32) - 0.5) * 0.05);
                    m.slab(
                        Vec3::new(x + 1.1, y, 0.03),
                        Vec3::new(2.3, 0.095, 0.075),
                        sag,
                        scale(fence, 0.85 + 0.2 * hash(i * 5 + k as u32)),
                        0.3,
                    );
                }
            }
            x += 2.2;
            i += 1;
        }
    }
    m.finish()
}

fn flower(m: &mut Sculpt, l: &Look, root: Vec3, kind: u32, k: f32) {
    let s = if l.toy { 2.3 } else { 1.8 } * k;
    let h = (0.22 + 0.12 * hash(kind * 13 + (root.x * 10.) as u32)) * s;
    let stem: Rgb = scale(l.grass[0], 1.2);
    let head = root + Vec3::Y * h + Vec3::new(0.02, 0., 0.01) * s;
    m.tube(
        &[(root, 0.012 * s), (head, 0.009 * s)],
        5,
        false,
        move |_, _| stem,
    );
    for i in 0..2u32 {
        let a = i as f32 * PI + hash(kind + i) * 1.5;
        let p = root + Vec3::Y * 0.03 * s;
        m.leaf(
            p,
            p + dir(a) * 0.09 * s + Vec3::Y * 0.06 * s,
            0.022 * s,
            0.3,
            0.2,
            0.3,
            l.grass[0],
            l.grass[1],
        );
    }
    let blooms = &l.blooms;
    let centre: Rgb = [0.98, 0.74, 0.22];
    match kind % 4 {
        0 => {
            // Daisy-like: a ring of petals around a gold centre.
            let petal = blooms[(kind / 4) as usize % 2 * 3];
            for i in 0..9u32 {
                let a = i as f32 * TAU / 9.;
                m.leaf(
                    head,
                    head + dir(a) * 0.075 * s + Vec3::Y * 0.01 * s,
                    0.018 * s,
                    0.6,
                    0.15,
                    0.1,
                    scale(petal, 0.9),
                    petal,
                );
            }
            m.ball(
                head + Vec3::Y * 0.01 * s,
                Vec3::new(0.026, 0.018, 0.026) * s,
                scale(centre, 0.8),
                centre,
                0.,
            );
        }
        1 => {
            // Cup: tulip or poppy.
            let petal = blooms[if l.toy { 4 } else { 1 }];
            for i in 0..5u32 {
                let a = i as f32 * TAU / 5.;
                let tip = head + dir(a) * 0.045 * s + Vec3::Y * 0.075 * s;
                m.leaf(
                    head,
                    tip,
                    0.035 * s,
                    0.9,
                    -0.1,
                    0.5,
                    scale(petal, 0.75),
                    petal,
                );
            }
            m.ball(
                head + Vec3::Y * 0.02 * s,
                Vec3::splat(0.018 * s),
                [0.2; 3],
                [0.25; 3],
                0.,
            );
        }
        2 => {
            // A spike of small buds (lavender).
            for i in 0..6u32 {
                let t = i as f32 / 5.;
                m.ball(
                    head + Vec3::Y * (t * 0.11 * s),
                    Vec3::splat((0.022 - 0.01 * t) * s),
                    scale(blooms[2], 0.75),
                    blooms[2],
                    0.,
                );
            }
        }
        _ => {
            if l.toy {
                // A pom-pom.
                m.ball(
                    head + Vec3::Y * 0.03 * s,
                    Vec3::splat(0.05 * s),
                    scale(blooms[1], 0.75),
                    blooms[1],
                    0.4,
                );
            } else {
                let petal = blooms[3];
                for i in 0..5u32 {
                    let a = i as f32 * TAU / 5.;
                    m.leaf(
                        head,
                        head + dir(a) * 0.05 * s + Vec3::Y * 0.02 * s,
                        0.028 * s,
                        0.9,
                        0.,
                        0.4,
                        scale(petal, 0.85),
                        petal,
                    );
                }
            }
        }
    }
}

pub(crate) fn flower_bank(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    for i in 0..46u32 {
        let x = hash(i) * 8.2;
        let z = hash(i + 100) * 1.4;
        flower(
            &mut m,
            l,
            Vec3::new(x, 0., z),
            i,
            0.85 + 0.35 * hash(i + 200),
        );
    }
    m.finish()
}

fn rock(m: &mut Sculpt, l: &Look, at: Vec3, size: Vec3, seed: u32) {
    let stone = l.stone;
    let moss = l.grass[0];
    let toy = l.toy;
    m.ellipsoid(
        at,
        size,
        Quat::from_rotation_y(hash(seed) * TAU)
            * Quat::from_rotation_z((hash(seed + 1) - 0.5) * 0.4),
        (7, 11),
        move |d| {
            let q = (d * 3.).round();
            let grain = 0.88 + 0.14 * hash((q.x * 13. + q.y * 7. + q.z * 3. + 50.) as u32 + seed);
            let c = scale(stone, grain * (0.65 + 0.35 * (d.y * 0.5 + 0.5)));
            if toy {
                c
            } else {
                mix(
                    c,
                    scale(moss, 1.1),
                    ((d.y - 0.55) * 2.5).clamp(0., 1.) * 0.8,
                )
            }
        },
    );
}

/// The meadow south and west of the garden: those edges never move as the
/// garden expands toward +x and −z.
pub(crate) fn meadow(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let [low, high] = l.grass;
    let tufts = if l.toy { 1400 } else { 2600 };
    let spot = |i: u32| -> Vec3 {
        let (u, v) = (hash(i * 2 + 1), hash(i * 2 + 2));
        // Denser toward the garden's south and west edges.
        let near = |t: f32| t * t;
        if i.is_multiple_of(3) {
            Vec3::new(-3.5 - near(u) * 18.5, 0., -38. + v * 41.4)
        } else {
            Vec3::new(-22. + u * 58., 0., 3.5 + near(v) * 13.)
        }
    };
    for i in 0..tufts {
        let p = spot(i) - Vec3::Y * 0.1;
        let dry = hash(i + 7000);
        let tip = if l.toy {
            high
        } else {
            mix(high, [0.86, 0.78, 0.48], dry * 0.6)
        };
        let blades = if l.toy { 3 } else { 5 };
        for b in 0..blades {
            let a = (i * 5 + b) as f32 * 2.39996;
            let lean = dir(a) * (0.05 + 0.08 * hash(i * 9 + b));
            let height = (0.16 + 0.22 * hash(i * 11 + b + 3)) * if l.toy { 1.3 } else { 1. };
            let root = p + dir(a + 1.3) * 0.04;
            if l.toy {
                m.leaf(
                    root,
                    root + lean * 1.5 + Vec3::Y * height,
                    0.045,
                    0.35,
                    0.25,
                    0.3,
                    low,
                    tip,
                );
            } else {
                m.blade(root, root + lean + Vec3::Y * height, 0.016, low, tip);
            }
        }
    }
    for i in 0..10u32 {
        let p = spot(i * 37 + 5) - Vec3::Y * 0.12;
        let s = 0.25 + 0.45 * hash(i + 300);
        rock(&mut m, l, p, Vec3::new(s, s * 0.6, s * 0.8), i);
    }
    if l.toy {
        // Round bushes.
        for i in 0..9u32 {
            let p = spot(i * 53 + 11);
            let s = 0.5 + 0.4 * hash(i + 500);
            for j in 0..3u32 {
                let off = dir(j as f32 * 2.2 + i as f32) * s * 0.45
                    + Vec3::Y * s * (0.45 + 0.1 * j as f32);
                let tone = l.canopy[(i + j) as usize % 3];
                m.ellipsoid(
                    p + off,
                    Vec3::splat(s * (0.6 - 0.08 * j as f32)),
                    Quat::IDENTITY,
                    (8, 12),
                    |d| l.foliage(tone, d),
                );
            }
        }
    } else {
        for i in 0..30u32 {
            let p = spot(i * 29 + 3);
            flower(&mut m, l, p, i * 4 + 1 + (i % 2) * 2, 0.9);
        }
    }
    m.finish()
}

/// Hills and a treeline north of the orchard, placed relative to it.
pub(crate) fn backdrop(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    for i in 0..16u32 {
        let x = -70. + 150. * hash(i + 900);
        let z = -22. - 26. * hash(i + 950);
        let rx = 12. + 14. * hash(i + 990);
        let ry = 4. + 7. * hash(i + 1030);
        let tone = mix(l.hills[0], l.hills[1], hash(i + 1070));
        m.ellipsoid(
            Vec3::new(x, -ry * 0.25, z),
            Vec3::new(rx, ry, rx * 0.6),
            Quat::IDENTITY,
            (8, 16),
            |d| l.foliage(tone, d),
        );
    }
    for i in 0..44u32 {
        let x = -40. + 100. * hash(i + 1200);
        let z = -10. - 9. * hash(i + 1300);
        let p = Vec3::new(x, 0., z);
        let tone = l.canopy[(i % 3) as usize];
        let s = 0.8 + 0.6 * hash(i + 1400);
        if l.toy {
            m.tube(
                &[(p, 0.2 * s), (p + Vec3::Y * 1.6 * s, 0.14 * s)],
                6,
                false,
                |_, _| l.bark,
            );
            m.ellipsoid(
                p + Vec3::Y * 2.6 * s,
                Vec3::splat(1.4 * s),
                Quat::IDENTITY,
                (8, 12),
                |d| l.foliage(tone, d),
            );
        } else {
            m.ellipsoid(
                p + Vec3::Y * 3.2 * s,
                Vec3::new(0.95, 3.4, 0.95) * s,
                Quat::IDENTITY,
                (8, 10),
                |d| l.foliage(scale(tone, 0.85), d),
            );
        }
    }
    m.finish()
}

/// A unit-length border rail along X, stretched to each edge's length.
pub(crate) fn rail(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let wood = l.wood;
    if l.toy {
        m.tube(
            &[
                (Vec3::new(-0.5, 0.13, 0.), 0.14),
                (Vec3::new(0.5, 0.13, 0.), 0.14),
            ],
            12,
            false,
            move |_, a| scale(wood, 0.8 + 0.2 * math::cos(a - 1.4)),
        );
    } else {
        for (y, h, tone) in [(0.07f32, 0.13, 0.92), (0.2, 0.12, 1.05)] {
            m.slab(
                Vec3::new(0., y, 0.),
                Vec3::new(1., h, 0.22),
                Quat::IDENTITY,
                scale(wood, tone),
                0.35,
            );
        }
    }
    m.finish()
}

pub(crate) fn barrel(l: &Look) -> MeshData {
    let mut m = Sculpt::default();
    let (wood, metal) = if l.toy {
        (l.accent, l.metal)
    } else {
        (l.wood, l.metal)
    };
    let mut profile: Vec<(f32, f32)> = vec![(0., 0.)];
    for i in 0..=40 {
        let y = i as f32 / 40.;
        let bulge = 1. - (2. * y - 1.) * (2. * y - 1.);
        profile.push((y, 0.58 + 0.08 * bulge));
    }
    profile.extend([(1., 0.53), (0.9, 0.53), (0.9, 0.)]);
    m.lathe(Vec3::Y * -0.5, &profile, 36, move |y, a| {
        let hoop = [0.1f32, 0.3, 0.7, 0.9]
            .iter()
            .any(|h| (y - h).abs() < 0.035);
        if hoop {
            return scale(metal, 0.9 + 0.2 * math::cos(a - 2.2));
        }
        let stave = (a / (TAU / 18.)) as u32 % 2;
        scale(
            wood,
            (if stave == 0 { 0.92 } else { 1.04 })
                * (0.8 + 0.2 * math::cos(a - 2.2))
                * (0.85 + 0.15 * y),
        )
    });
    m.finish()
}
