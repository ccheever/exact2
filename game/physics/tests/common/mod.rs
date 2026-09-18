#![allow(dead_code)]
use exact_game::{Entity, Game, Input, Quat, Sim, Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Shape};

pub fn rotated(degrees: f32) -> Quat {
    let a = degrees * std::f32::consts::PI / 360.0;
    Quat::from_xyzw(0.0, 0.0, exact_game::math::sin(a), exact_game::math::cos(a))
}
pub fn box_at(w: &mut World, name: &str, p: Vec3, half: Vec3, dynamic: bool) -> Entity {
    let e = w.spawn_named(
        name,
        (
            Transform {
                position: p,
                ..Transform::default()
            },
            Collider {
                shape: Shape::Box { half },
                ..Collider::default()
            },
        ),
    );
    if dynamic {
        w.insert(e, Body::default());
    }
    e
}
pub fn ground(w: &mut World) {
    box_at(
        w,
        "ground",
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(50.0, 0.5, 50.0),
        false,
    );
}
pub fn pile(w: &mut World, side: usize, layers: usize, drop: f32) {
    ground(w);
    for y in 0..layers {
        for z in 0..side {
            for x in 0..side {
                box_at(
                    w,
                    &format!("box-{x}-{y}-{z}"),
                    Vec3::new(
                        x as f32 * 1.01,
                        0.5 + drop + y as f32 * 1.01,
                        z as f32 * 1.01,
                    ),
                    Vec3::splat(0.5),
                    true,
                );
            }
        }
    }
}
pub struct Scene;
/// Typed canvas arguments in positional order.
#[derive(Default, exact_game::Args)]
pub struct SceneArgs {
    /// Canvas setup argument.
    pub scene: String,
}
impl Game for Scene {
    const ID: &'static str = "exact-physics-scenes";
    type Args = SceneArgs;
    fn setup(w: &mut World, args: &Self::Args) {
        physics::register(w);
        match args.scene.as_str() {
            "pile" => pile(w, 5, 5, 2.0),
            "stack" => pile(w, 1, 10, 0.0),
            "drop" => pile(w, 1, 1, 2.0),
            "bounce" => {
                ground(w);
                w.spawn_named(
                    "ball",
                    (
                        Transform::at(0.0, 2.5, 0.0),
                        Body::default(),
                        Collider {
                            shape: Shape::Sphere { radius: 0.5 },
                            bounce: 0.5,
                            ..Collider::default()
                        },
                    ),
                );
            }
            _ => panic!("unknown scene"),
        }
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        physics::step(w);
    }
}
pub fn scene(name: &str) -> Sim<Scene> {
    {
        let mut s = Sim::from_values(&[exact_game::Value::str(name)]).unwrap();
        s.advance(0.0, exact_game::Clock::Seekable);
        s
    }
}
pub fn tick(sim: &mut Sim<Scene>, tick: u32) {
    sim.advance(
        tick as f64 * 1000.0 / 60.0 + 0.001,
        exact_game::Clock::Seekable,
    );
}
pub fn positions(w: &World) -> Vec<(Vec3, Quat)> {
    w.query::<(&Body, &Transform)>()
        .iter()
        .map(|(_, (_, p))| (p.position, p.rotation))
        .collect()
}
