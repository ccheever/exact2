use super::{turn, Joint};
use exact_game::math::{cos, sin, sqrt};
use exact_game::*;

fn fur() -> Material {
    Material::rgb(0.016, 0.019, 0.024).rough(0.92)
}
fn velvet() -> Material {
    Material::rgb(0.027, 0.029, 0.032).rough(0.92)
}

fn orb(w: &mut World, parent: Entity, p: [f32; 3], size: [f32; 3], material: Material) -> Entity {
    w.spawn((
        Transform::at(p[0], p[1], p[2]).with_scale(size),
        Parent(parent),
        Mesh::sphere(1.0),
        material,
    ))
}

fn joint(w: &mut World, name: &str, parent: Entity, p: [f32; 3], kind: u32, side: f32) -> Entity {
    let rest = Transform::at(p[0], p[1], p[2]);
    w.spawn_named(name, (rest, Parent(parent), Joint { kind, side, rest }))
}

/// Overlapping sculpted curls distributed on an ellipsoid, with an open face.
fn fuzzy(
    w: &mut World,
    parent: Entity,
    p: [f32; 3],
    size: [f32; 3],
    count: u32,
    curl: f32,
    face: bool,
) {
    orb(w, parent, p, size, fur());
    for i in 0..count {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let ring = sqrt((1.0 - y * y).max(0.0));
        let phi = i as f32 * 2.399_963_1;
        let x = cos(phi) * ring;
        let z = sin(phi) * ring;
        if face && z > 0.38 && y < 0.58 && y > -0.55 {
            continue;
        }
        let radius = curl * (0.87 + 0.18 * sin(i as f32 * 5.31));
        let tone = 0.016 + 0.007 * (0.5 + 0.5 * sin(i as f32 * 7.3));
        orb(
            w,
            parent,
            [
                p[0] + x * size[0] * 0.95,
                p[1] + y * size[1] * 0.95,
                p[2] + z * size[2] * 0.95,
            ],
            [radius, radius * 0.9, radius],
            Material::rgb(tone, tone * 1.15, tone * 1.34).rough(0.94),
        );
    }
}

pub fn build(w: &mut World) {
    let rest = Transform::at(0.0, 0.94, 0.0);
    let root = w.spawn_named(
        "rolo",
        (
            rest,
            Joint {
                kind: 0,
                side: 0.0,
                rest,
            },
        ),
    );

    // Simple grounding oval: one quiet graphic mark, no surrounding scene.
    w.spawn_named(
        "shadow",
        (
            Transform::at(0.0, 0.08, 0.0).with_scale([0.95, 0.015, 0.85]),
            Mesh::sphere(1.0),
            Material::rgb(0.43, 0.46, 0.34).rough(1.0),
        ),
    );

    fuzzy(
        w,
        root,
        [0.0, -0.08, -0.28],
        [0.49, 0.50, 0.72],
        180,
        0.086,
        false,
    );
    // A subtly softer tummy, visible while he asks for belly rubs.
    orb(w, root, [0.0, -0.48, -0.20], [0.33, 0.105, 0.48], velvet());

    let head = joint(w, "head", root, [0.0, 0.37, 0.65], 1, 0.0);
    fuzzy(
        w,
        head,
        [0.0, 0.0, 0.0],
        [0.59, 0.56, 0.46],
        150,
        0.076,
        true,
    );

    // One Chihuahua ear stands up; the other has a floppy terrier tip.
    for side in [-1.0_f32, 1.0] {
        let name = if side < 0.0 {
            "floppy-ear"
        } else {
            "upright-ear"
        };
        let ear = joint(w, name, head, [side * 0.46, 0.31, -0.03], 2, side);
        let rotation = turn(Vec3::Z, -side * 0.32);
        w.get_mut::<Joint>(ear).unwrap().rest.rotation = rotation;
        fuzzy(
            w,
            ear,
            [side * 0.06, 0.17, 0.0],
            [0.19, 0.36, 0.115],
            46,
            0.053,
            false,
        );
        orb(
            w,
            ear,
            [side * 0.055, 0.19, 0.102],
            [0.105, 0.225, 0.024],
            Material::rgb(0.10, 0.060, 0.060).rough(1.0),
        );
        if side < 0.0 {
            fuzzy(
                w,
                ear,
                [-0.12, 0.36, 0.13],
                [0.19, 0.15, 0.13],
                26,
                0.047,
                false,
            );
        }
    }

    for side in [-1.0_f32, 1.0] {
        let eye = joint(
            w,
            if side < 0.0 { "left-eye" } else { "right-eye" },
            head,
            [side * 0.236, 0.085, 0.397],
            5,
            side,
        );
        // Warm sclera, hazel irises, dark pupils and two glossy catchlights.
        orb(
            w,
            eye,
            [0.0, 0.0, 0.0],
            [0.185, 0.203, 0.083],
            Material::rgb(0.69, 0.64, 0.54).rough(0.35),
        );
        orb(
            w,
            eye,
            [-side * 0.018, -0.001, 0.07],
            [0.130, 0.147, 0.040],
            Material::rgb(0.20, 0.092, 0.028).rough(0.25),
        );
        orb(
            w,
            eye,
            [-side * 0.018, 0.0, 0.103],
            [0.080, 0.105, 0.021],
            Material::rgb(0.004, 0.005, 0.007).rough(0.12),
        );
        orb(
            w,
            eye,
            [-0.038, 0.054, 0.126],
            [0.031, 0.037, 0.012],
            Material::rgb(0.96, 0.95, 0.90).emissive(0.3, 0.3, 0.3),
        );
        orb(
            w,
            eye,
            [0.043, -0.046, 0.124],
            [0.013, 0.014, 0.009],
            Material::rgb(0.70, 0.71, 0.67),
        );
        // A slightly worried, very expressive human-like brow.
        let brow = orb(
            w,
            head,
            [side * 0.23, 0.296, 0.401],
            [0.195, 0.055, 0.065],
            velvet(),
        );
        w.get_mut::<Transform>(brow).unwrap().rotation = turn(Vec3::Z, side * -0.16);
        fuzzy(
            w,
            head,
            [side * 0.16, -0.19, 0.427],
            [0.225, 0.155, 0.125],
            35,
            0.045,
            false,
        );
        // Three little scruffy cheek hairs, sculpted rather than painted.
        for i in 0..3 {
            let tuft = orb(
                w,
                head,
                [
                    side * (0.38 + i as f32 * 0.037),
                    -0.17 - i as f32 * 0.048,
                    0.29,
                ],
                [0.125, 0.027, 0.040],
                velvet(),
            );
            w.get_mut::<Transform>(tuft).unwrap().rotation =
                turn(Vec3::Z, side * (0.2 + i as f32 * 0.25));
        }
    }

    let mouth = Material::rgb(0.003, 0.003, 0.005).rough(0.6);
    orb(w, head, [0.0, -0.284, 0.459], [0.131, 0.088, 0.048], mouth);
    let tongue = joint(w, "tongue", head, [0.025, -0.325, 0.506], 6, 0.0);
    orb(
        w,
        tongue,
        [0.0, 0.0, 0.0],
        [0.071, 0.087, 0.027],
        Material::rgb(0.60, 0.19, 0.22).rough(0.65),
    );
    orb(
        w,
        tongue,
        [0.0, 0.018, 0.026],
        [0.003, 0.035, 0.002],
        Material::rgb(0.35, 0.08, 0.10),
    );
    let nose = Material::rgb(0.008, 0.009, 0.012).rough(0.21);
    orb(w, head, [0.0, -0.10, 0.593], [0.136, 0.082, 0.068], nose);
    orb(w, head, [0.0, -0.15, 0.578], [0.074, 0.057, 0.057], nose);
    orb(
        w,
        head,
        [-0.038, -0.073, 0.654],
        [0.035, 0.013, 0.009],
        Material::rgb(0.15, 0.16, 0.17).rough(0.25),
    );

    // Mint collar and a tiny gold tag peeking through the beard.
    orb(
        w,
        root,
        [0.0, 0.04, 0.57],
        [0.395, 0.28, 0.205],
        Material::rgb(0.31, 0.49, 0.30).rough(0.8),
    );
    orb(
        w,
        root,
        [0.0, -0.18, 0.78],
        [0.085, 0.101, 0.028],
        Material::rgb(0.67, 0.40, 0.10).metallic(0.5).rough(0.35),
    );

    for (index, z) in [0.35, -0.68].into_iter().enumerate() {
        for side in [-1.0_f32, 1.0] {
            let leg = joint(
                w,
                &format!("paw-{index}-{side}"),
                root,
                [side * 0.34, -0.26, z],
                3,
                side * if index == 0 { 1.0 } else { -1.0 },
            );
            fuzzy(
                w,
                leg,
                [0.0, -0.17, 0.0],
                [0.155, 0.25, 0.16],
                34,
                0.051,
                false,
            );
            fuzzy(
                w,
                leg,
                [0.0, -0.40, 0.08],
                [0.21, 0.145, 0.235],
                35,
                0.044,
                false,
            );
            let pads = Material::rgb(0.105, 0.070, 0.080).rough(0.95);
            orb(w, leg, [0.0, -0.532, 0.048], [0.095, 0.018, 0.105], pads);
            for toe in -1..=1 {
                orb(
                    w,
                    leg,
                    [toe as f32 * 0.094, -0.51, 0.196],
                    [0.036, 0.023, 0.046],
                    pads,
                );
            }
        }
    }

    let tail = joint(w, "tail", root, [0.0, 0.02, -0.89], 4, 0.0);
    for i in 0..7 {
        let a = i as f32 * 0.26;
        fuzzy(
            w,
            tail,
            [0.0, sin(a) * 0.44, -cos(a) * 0.30],
            [0.10, 0.10, 0.11],
            10,
            0.038,
            false,
        );
    }
}
