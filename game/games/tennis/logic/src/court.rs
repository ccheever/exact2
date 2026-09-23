//! The court, the players' bodies and the synthesized sounds: scenery the
//! rules never read.
use crate::ball::{HALF_LENGTH, HALF_WIDTH, NET_CENTER, NET_POST, POST_X, SERVICE_LINE};
use crate::players::Player;
use crate::rules::Side;
use exact_game::audio::Synth;
use exact_game::*;

/// Racket hit, bounce, net, swish, point chime and a crowd swell.
pub fn sounds(w: &mut World) {
    w.sounds([
        (
            "hit",
            Synth::noise()
                .seconds(0.07)
                .attack(0.001)
                .decay(0.05)
                .release(0.02)
                .highpass_hz(900.0)
                .lowpass_hz(5200.0)
                .gain(0.8)
                .layer(
                    Synth::sine(430.0)
                        .slide(-24.0)
                        .seconds(0.07)
                        .attack(0.001)
                        .decay(0.05)
                        .release(0.02)
                        .gain(0.55),
                ),
        ),
        (
            "bounce",
            Synth::sine(165.0)
                .slide(-14.0)
                .seconds(0.09)
                .attack(0.001)
                .decay(0.07)
                .release(0.02)
                .gain(0.6)
                .layer(
                    Synth::noise()
                        .seconds(0.04)
                        .attack(0.001)
                        .release(0.03)
                        .lowpass_hz(900.0)
                        .gain(0.2),
                ),
        ),
        (
            "net",
            Synth::noise()
                .seconds(0.25)
                .attack(0.002)
                .decay(0.2)
                .release(0.05)
                .lowpass_hz(480.0)
                .gain(0.8),
        ),
        (
            "swish",
            Synth::noise()
                .seconds(0.2)
                .attack(0.06)
                .decay(0.1)
                .release(0.08)
                .highpass_hz(1400.0)
                .lowpass_hz(4200.0)
                .gain(0.12),
        ),
        (
            "point",
            Synth::sine(660.0)
                .seconds(0.5)
                .attack(0.005)
                .release(0.45)
                .gain(0.22)
                .layer(Synth::sine(990.0).seconds(0.35).release(0.3).gain(0.1)),
        ),
        (
            "crowd",
            Synth::noise()
                .seconds(1.6)
                .attack(0.2)
                .decay(0.4)
                .sustain(0.5)
                .release(0.9)
                .highpass_hz(350.0)
                .lowpass_hz(2400.0)
                .gain(0.18),
        ),
    ]);
}

/// A hard court at ITF singles proportions, US-Open colours, under a sky.
pub fn court(w: &mut World) {
    w.insert_resource(Environment {
        zenith: [0.22, 0.42, 0.78],
        horizon: [0.68, 0.78, 0.88],
        ground: [0.05, 0.08, 0.05],
        ambient: 0.65,
        fog: Some(Fog {
            color: Some([0.68, 0.78, 0.88]),
            ..Fog::new(0.0025, 0.08)
        }),
        ..Environment::default()
    });
    w.spawn_named(
        "sun",
        (
            Transform::at(-9.0, 22.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                illuminance: 9000.0,
                ..DirectionalLight::default()
            },
        ),
    );
    let flat = |w: &mut World, name: &str, size: Vec3, at: Vec3, material: Material| {
        w.spawn_named(
            name,
            (
                Transform::at(at.x, at.y, at.z),
                Mesh::cuboid(size),
                material,
            ),
        );
    };
    w.spawn_named(
        "surround",
        (
            Transform::default(),
            Mesh::plane(80.0, 80.0),
            Material::rgb(0.05, 0.19, 0.1).rough(0.95),
        ),
    );
    w.spawn_named(
        "court",
        (
            Transform::at(0.0, 0.002, 0.0),
            Mesh::plane(2.0 * HALF_WIDTH, 2.0 * HALF_LENGTH),
            Material::rgb(0.04, 0.13, 0.33).rough(0.9),
        ),
    );
    let line = Material::rgb(0.92, 0.92, 0.9).rough(0.9);
    let (lw, y) = (0.05, 0.004);
    for s in [-1.0, 1.0] {
        flat(
            w,
            "baseline",
            Vec3::new(2.0 * HALF_WIDTH, y, lw),
            Vec3::new(0.0, y / 2.0, s * (HALF_LENGTH - lw / 2.0)),
            line,
        );
        flat(
            w,
            "sideline",
            Vec3::new(lw, y, 2.0 * HALF_LENGTH),
            Vec3::new(s * (HALF_WIDTH - lw / 2.0), y / 2.0, 0.0),
            line,
        );
        flat(
            w,
            "service-line",
            Vec3::new(2.0 * HALF_WIDTH, y, lw),
            Vec3::new(0.0, y / 2.0, s * (SERVICE_LINE - lw / 2.0)),
            line,
        );
        flat(
            w,
            "center-mark",
            Vec3::new(lw, y, 0.1),
            Vec3::new(0.0, y / 2.0, s * (HALF_LENGTH - 0.05)),
            line,
        );
    }
    flat(
        w,
        "center-line",
        Vec3::new(lw, y, 2.0 * SERVICE_LINE),
        Vec3::new(0.0, y / 2.0, 0.0),
        line,
    );
    // The net: two tilted halves so the tape rises from 0.914 m to 1.07 m at the sticks.
    let tilt = math::atan2(NET_POST - NET_CENTER, POST_X);
    let span = math::sqrt(POST_X * POST_X + (NET_POST - NET_CENTER) * (NET_POST - NET_CENTER));
    let mid = 0.5 * (NET_CENTER + NET_POST);
    for s in [-1.0f32, 1.0] {
        let turn = Quat::from_rotation_z(s * tilt);
        w.spawn_named(
            "net",
            (
                Transform {
                    position: Vec3::new(s * POST_X / 2.0, mid - 0.55, 0.0),
                    rotation: turn,
                    scale: Vec3::ONE,
                },
                Mesh::cuboid(Vec3::new(span, 1.1, 0.012)),
                Material::grid([0.01, 0.01, 0.012], 0.06).rough(1.0),
            ),
        );
        w.spawn_named(
            "net-tape",
            (
                Transform {
                    position: Vec3::new(s * POST_X / 2.0, mid - 0.025, 0.0),
                    rotation: turn,
                    scale: Vec3::ONE,
                },
                Mesh::cuboid(Vec3::new(span, 0.05, 0.02)),
                Material::rgb(0.95, 0.95, 0.95),
            ),
        );
        w.spawn_named(
            "net-post",
            (
                Transform::at(s * POST_X, NET_POST / 2.0, 0.0),
                Mesh::cylinder(0.035, NET_POST),
                Material::rgb(0.5, 0.52, 0.55).metallic(0.8).rough(0.35),
            ),
        );
        // Backstops and side walls frame the court in dark green.
        flat(
            w,
            "backstop",
            Vec3::new(22.0, 3.0, 0.3),
            Vec3::new(0.0, 1.5, s * 19.5),
            Material::rgb(0.01, 0.05, 0.035).rough(0.9),
        );
        flat(
            w,
            "sidewall",
            Vec3::new(0.3, 1.2, 39.0),
            Vec3::new(s * 10.8, 0.6, 0.0),
            Material::rgb(0.01, 0.05, 0.035).rough(0.9),
        );
    }
    flat(
        w,
        "net-strap",
        Vec3::new(0.05, NET_CENTER, 0.022),
        Vec3::new(0.0, NET_CENTER / 2.0, 0.0),
        Material::rgb(0.95, 0.95, 0.95),
    );
    flat(
        w,
        "umpire-chair",
        Vec3::new(0.7, 1.9, 0.7),
        Vec3::new(-6.4, 0.95, 0.0),
        Material::rgb(0.01, 0.05, 0.035),
    );
    flat(
        w,
        "umpire",
        Vec3::new(0.45, 0.8, 0.45),
        Vec3::new(-6.4, 2.3, 0.0),
        Material::rgb(0.03, 0.06, 0.2),
    );
}

/// Capsules for torso and legs, a head, and an arm that carries the racket.
pub fn player(w: &mut World, side: Side) {
    let name = if side == Side::Near { "near" } else { "far" };
    let (shirt, skin) = match side {
        Side::Near => ([0.85, 0.3, 0.06], [0.75, 0.5, 0.36]),
        Side::Far => ([0.04, 0.42, 0.46], [0.55, 0.36, 0.25]),
    };
    let facing = if side == Side::Near {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_y(std::f32::consts::PI)
    };
    let root = w.spawn_named(
        name,
        (
            Transform {
                position: Vec3::new(0.0, 0.0, side.half() * (HALF_LENGTH + 0.6)),
                rotation: facing,
                scale: Vec3::ONE,
            },
            Player {
                side,
                ..Player::default()
            },
        ),
    );
    let part =
        |w: &mut World, part: &str, parent: Entity, at: Vec3, mesh: Mesh, color: [f32; 3]| {
            w.spawn_named(
                format!("{name}-{part}"),
                (
                    Transform::at(at.x, at.y, at.z),
                    mesh,
                    Material::rgb(color[0], color[1], color[2]),
                    Parent(parent),
                ),
            )
        };
    part(
        w,
        "body",
        root,
        Vec3::new(0.0, 1.15, 0.0),
        Mesh::capsule(0.25, 0.8),
        shirt,
    );
    part(
        w,
        "legs",
        root,
        Vec3::new(0.0, 0.45, 0.0),
        Mesh::capsule(0.19, 0.9),
        [0.9, 0.9, 0.88],
    );
    part(
        w,
        "head",
        root,
        Vec3::new(0.0, 1.7, 0.0),
        Mesh::sphere(0.13),
        skin,
    );
    let arm = w.spawn_named(
        format!("{name}-arm"),
        (Transform::at(0.0, 1.3, 0.0), Parent(root)),
    );
    part(
        w,
        "forearm",
        arm,
        Vec3::new(0.2, 0.0, 0.0),
        Mesh::cuboid(Vec3::new(0.4, 0.07, 0.07)),
        skin,
    );
    part(
        w,
        "grip",
        arm,
        Vec3::new(0.52, 0.0, 0.0),
        Mesh::cuboid(Vec3::new(0.26, 0.035, 0.035)),
        [0.05, 0.05, 0.06],
    );
    let head = part(
        w,
        "racket",
        arm,
        Vec3::new(0.8, 0.0, 0.0),
        Mesh::cylinder(0.13, 0.018),
        [0.9, 0.9, 0.86],
    );
    let mut t = w.require_mut::<Transform>(head);
    t.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    t.scale = Vec3::new(1.25, 1.0, 1.0);
}
