//! Saved gestures and bounded effects for successful garden actions. Cosmetic
//! motion uses the simulation clock and never draws from the crop/weather RNG.
use crate::{crops, garden};
use exact_game::{audio::Synth, character::Contact, *};

#[derive(Clone, Copy, Default, PartialEq, Data)]
pub enum Cue {
    #[default]
    None,
    Plant,
    Water,
    Harvest,
    Feed,
    Refill,
}

#[derive(Clone, Default, PartialEq, Resource)]
pub struct Feedback {
    pub heading: f32,
    pub stride: f32,
    pub cue: Cue,
    pub began: u64,
    pub target: Vec3,
    pub fruit_from: Vec3,
    pub fruit_scale: f32,
}

pub fn setup(w: &mut World) {
    w.insert_resource(Feedback::default());
    w.sounds([
        (
            "plant",
            Synth::noise()
                .seconds(0.16)
                .decay(0.10)
                .lowpass_hz(650.)
                .gain(0.14)
                .layer(Synth::sine(130.).slide(-12.).seconds(0.13).gain(0.16)),
        ),
        (
            "water",
            Synth::noise()
                .seconds(0.50)
                .attack(0.03)
                .decay(0.38)
                .lowpass_hz(2600.)
                .highpass_hz(550.)
                .gain(0.09)
                .layer(Synth::sine(620.).slide(-14.).seconds(0.25).gain(0.045)),
        ),
        (
            "harvest",
            Synth::triangle(660.)
                .slide(7.)
                .seconds(0.32)
                .decay(0.22)
                .gain(0.12)
                .layer(Synth::sine(990.).seconds(0.42).decay(0.31).gain(0.075)),
        ),
    ]);
    for (name, color, speed, lifetime, spread, gravity, size, additive) in [
        (
            "dirt",
            [0.30, 0.15, 0.055],
            1.6,
            0.45,
            1.1,
            -8.,
            0.11,
            false,
        ),
        (
            "water",
            [0.18, 0.65, 1.0],
            0.0,
            0.60,
            1.6,
            -4.5,
            0.10,
            false,
        ),
        ("reward", [1.0, 0.73, 0.18], 1.8, 0.65, 1.2, -3., 0.10, true),
    ] {
        w.spawn_named(
            format!("feedback-{name}"),
            (
                Transform::default(),
                Emitter {
                    shape: emitter::Shape::Sphere(0.18),
                    rate: 0.,
                    speed,
                    lifetime,
                    spread,
                    gravity: Vec3::Y * gravity,
                    size: [size, 0.015],
                    color: [
                        [color[0], color[1], color[2], 0.95],
                        [color[0], color[1], color[2], 0.],
                    ],
                    bound: [-2., -2., -2., 2., 3., 2.],
                    additive,
                    seed: 47,
                    ..Emitter::default()
                },
            ),
        );
    }
    w.spawn_named(
        "picked-fruit",
        (
            Transform::default(),
            Mesh::sphere(0.2),
            Material::default(),
            Visible(false),
        ),
    );
}

fn burst(w: &World, name: &str, at: Vec3, count: u32) {
    let name = format!("feedback-{name}");
    w.require_mut::<Transform>(&name).position = at;
    // The next action replaces this short effect; no entity or RNG stream churn.
    let mut emitter = w.require_mut::<Emitter>(&name);
    emitter.state = emitter::EmitterState {
        burst: count,
        ..Default::default()
    };
}

pub fn cue(w: &World, cue: Cue, at: Vec3) {
    {
        let mut feedback = w.resource_mut::<Feedback>();
        feedback.cue = cue;
        feedback.began = w.tick();
        feedback.target = at;
    }
    w.require_mut::<Visible>("picked-fruit").0 = false;
    match cue {
        Cue::Plant | Cue::Feed => {
            burst(w, "dirt", at + Vec3::Y * 0.08, 14);
            w.play("plant").start();
        }
        Cue::Water | Cue::Refill => {
            burst(w, "water", at + Vec3::Y * 0.85, 32);
            w.play("water").start();
        }
        Cue::Harvest => {
            burst(w, "reward", at + Vec3::Y * 0.25, 20);
            w.play("harvest").start();
        }
        Cue::None => {}
    }
}

pub fn harvest(w: &World, at: Vec3, item: &garden::Item) {
    cue(w, Cue::Harvest, at);
    *w.require_mut::<Mesh>("picked-fruit") = Mesh::asset(format!("fruit-{}.model", item.kind));
    *w.require_mut::<Material>("picked-fruit") =
        garden::paint(crops::fruit_color(item.kind, item.muts));
    let mut feedback = w.resource_mut::<Feedback>();
    feedback.fruit_from = at;
    feedback.fruit_scale = (item.weight / crops::crop(item.kind).weight)
        .sqrt()
        .min(2.0);
}

fn pose(w: &World, name: &str, value: Transform) {
    if *w.require::<Transform>(name) != value {
        *w.require_mut::<Transform>(name) = value;
    }
}

fn visible(w: &World, name: &str, value: bool) {
    if w.require::<Visible>(name).0 != value {
        w.require_mut::<Visible>(name).0 = value;
    }
}

pub fn step(w: &World, contact: Contact) {
    let mut f = w.resource::<Feedback>().clone();
    let horizontal = Vec3::new(contact.displacement.x, 0., contact.displacement.z);
    let distance = horizontal.length();
    let pace = (distance / (w.dt() * 4.0)).min(1.0);
    let player = w.require::<Transform>("player").position;
    let seconds = w.tick().saturating_sub(f.began) as f32 / w.hz() as f32;
    let duration = if f.cue == Cue::Harvest { 0.8 } else { 0.65 };
    if seconds >= duration {
        f.cue = Cue::None;
    }
    let working = f.cue != Cue::None;
    let direction = if working {
        f.target - player
    } else {
        horizontal
    };
    if direction.x * direction.x + direction.z * direction.z > 0.00001 {
        let heading = math::atan2(direction.x, direction.z);
        let turn = math::atan2(
            math::sin(heading - f.heading),
            math::cos(heading - f.heading),
        );
        f.heading += turn.clamp(-w.dt() * 14., w.dt() * 14.);
    }
    if distance > 0. {
        f.stride = (f.stride + distance * 3.5) % std::f32::consts::TAU;
    }
    let swing = math::sin(f.stride) * pace * if contact.grounded { 0.70 } else { 0.18 };
    let gesture = if working {
        math::sin((seconds / duration).min(1.) * std::f32::consts::PI)
    } else {
        0.
    };
    let bend = match f.cue {
        Cue::Plant | Cue::Feed => 0.38 * gesture,
        Cue::Water | Cue::Refill => 0.12 * gesture,
        _ => 0.,
    };
    let bob = if contact.grounded {
        math::sin(f.stride).abs() * pace * 0.055
    } else {
        0.
    };
    let heading = Quat::from_rotation_y(f.heading);
    pose(
        w,
        "gardener",
        Transform {
            position: Vec3::Y * (bob - bend * 0.20),
            rotation: heading * Quat::from_rotation_x(bend),
            scale: Vec3::ONE,
        },
    );
    let reach = match f.cue {
        Cue::Harvest => 1.5,
        Cue::Water | Cue::Refill => 1.15,
        _ => 0.8,
    } * gesture;
    for (name, x, y, angle) in [
        ("arm-left", -0.38, 0.18, swing * 0.65 - reach),
        ("arm-right", 0.38, 0.18, -swing * 0.65 - reach * 0.7),
        ("leg-left", -0.18, -0.28, -swing),
        ("leg-right", 0.18, -0.28, swing),
    ] {
        pose(
            w,
            name,
            Transform {
                position: Vec3::new(x, y, 0.),
                rotation: Quat::from_rotation_x(angle),
                scale: Vec3::ONE,
            },
        );
    }
    let watering = matches!(f.cue, Cue::Water | Cue::Refill);
    visible(w, "watering-can", watering);
    if watering {
        pose(
            w,
            "watering-can",
            Transform {
                position: Vec3::new(0., -0.47, 0.16),
                rotation: Quat::from_rotation_x(reach - swing * 0.65 - bend + 0.4),
                scale: Vec3::splat(1.25),
            },
        );
    }
    let collecting = f.cue == Cue::Harvest;
    visible(w, "picked-fruit", collecting);
    if collecting {
        let t = (seconds / duration).clamp(0., 1.);
        let end = player + heading * Vec3::new(0.32, -0.10, -0.21);
        let position = f.fruit_from.lerp(end, t * t * (3. - 2. * t))
            + Vec3::Y * (math::sin(t * std::f32::consts::PI) * 1.45);
        pose(
            w,
            "picked-fruit",
            Transform {
                position,
                rotation: Quat::from_rotation_y(t * 4.),
                scale: Vec3::splat(f.fruit_scale * 1.8 * (1. - t * 0.90)),
            },
        );
    }
    if *w.resource::<Feedback>() != f {
        *w.resource_mut::<Feedback>() = f;
    }
    emitter::step(w);
}
