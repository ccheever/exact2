//! Rolo: a tiny, entirely engine-rendered, animated brand companion.
use exact_game::math::{cos, sin, smoothstep};
use exact_game::*;
use std::f32::consts::{PI, TAU};

mod model;

#[derive(Default, Args)]
pub struct Options {
    #[live]
    pub paused: bool,
    pub round: u32,
}

/// Rest pose plus a small saved rig channel; all animation uses the world clock.
#[derive(Default, Component)]
pub struct Joint {
    pub kind: u32,
    pub side: f32,
    pub rest: Transform,
}

pub struct Rolo;
impl Game for Rolo {
    const ID: &'static str = "rolo";
    type Args = Options;

    fn setup(w: &mut World, _: &Options) {
        w.insert_resource(Environment {
            // Inverse of the renderer's ACES/sRGB for the warm paper UI.
            background: Some([2.123_387, 1.619_517, 1.054_719]),
            zenith: [0.8, 0.86, 1.0],
            horizon: [0.9, 0.83, 0.72],
            ground: [0.4, 0.36, 0.3],
            ambient: 0.85,
            sun_disc: 0.0,
            exposure: 1.0,
            fog: None,
            bloom: None,
        });
        w.spawn_named(
            "camera",
            (
                Transform::at(0.0, 2.8, 6.9).looking_at(Vec3::new(0.0, 0.94, 0.0), Vec3::Y),
                Camera {
                    fov_y_degrees: 30.0,
                    ..Camera::default()
                },
            ),
        );
        w.spawn_named(
            "softbox",
            (
                Transform::at(-3.0, 6.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight {
                    illuminance: 7500.0,
                    color: [1.0, 0.91, 0.79],
                    shadows: false,
                },
            ),
        );
        w.spawn_named(
            "rim",
            (
                Transform::at(2.5, 3.0, -1.8),
                PointLight {
                    color: [0.65, 0.78, 1.0],
                    intensity: 28.0,
                    range: 9.0,
                },
            ),
        );
        model::build(w);
        animate(w, 0.0);
    }

    fn paused(args: &Options) -> bool {
        args.paused
    }

    fn tick(w: &mut World, _: &Input, _: &Options) {
        animate(w, (w.seconds() % 10.0) as f32);
    }
}

// Quaternion construction also uses the engine's portable transcendental math.
fn turn(axis: Vec3, angle: f32) -> Quat {
    let s = sin(angle * 0.5);
    Quat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, cos(angle * 0.5))
}

fn animate(w: &World, t: f32) {
    let belly = smoothstep(1.9, 3.25, t) * (1.0 - smoothstep(6.7, 8.1, t));
    let wiggle = smoothstep(3.2, 3.7, t) * (1.0 - smoothstep(6.25, 6.9, t));
    let angle = if t < 3.3 {
        PI * smoothstep(1.9, 3.3, t)
    } else if t < 6.7 {
        PI + 0.36 * sin((t - 3.3) * 5.5) * wiggle
    } else {
        PI + PI * smoothstep(6.7, 8.15, t)
    };
    let blink = (1.0 - (t - 1.15).abs() / 0.12).max(0.0) + (1.0 - (t - 8.85).abs() / 0.14).max(0.0);
    for (joint, mut pose) in w.query::<(&Joint, &mut Transform)>() {
        *pose = joint.rest;
        match joint.kind {
            0 => {
                pose.rotation = turn(Vec3::Z, angle) * turn(Vec3::Y, 0.08 * sin(t * TAU / 10.0));
                pose.position.x = -0.19 * sin(angle);
                pose.position.y += 0.035 * sin(t * TAU / 5.0) - belly * 0.34;
            }
            1 => {
                pose.position.y -= 0.60 * belly;
                pose.rotation =
                    turn(Vec3::Z, 0.085 * sin(t * TAU / 5.0)) * turn(Vec3::X, 0.10 * belly);
            }
            2 => {
                pose.rotation = joint.rest.rotation
                    * turn(
                        Vec3::Z,
                        joint.side * (0.10 * sin(t * 9.0) * (0.25 + belly) - 0.90 * belly),
                    );
            }
            3 => {
                pose.rotation =
                    turn(
                        Vec3::Z,
                        joint.side * (0.14 + belly * 0.25)
                            + wiggle * 0.20 * sin(t * 9.0 + joint.side),
                    ) * turn(Vec3::X, -belly * (0.28 + 0.18 * sin(t * 8.0 + joint.side)));
            }
            4 => {
                pose.rotation =
                    turn(Vec3::Y, sin(t * 15.0) * 0.55) * turn(Vec3::X, 0.18 * sin(t * 10.0));
            }
            5 => pose.scale.y = 1.0 - 0.95 * blink,
            6 => pose.scale.y = 1.0 + 0.10 * sin(t * 8.0),
            _ => {}
        }
    }
}
