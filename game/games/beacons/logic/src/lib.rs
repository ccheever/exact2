//! Beacons: deterministic movement, three lights, and a following camera.
use exact_game::motion::{Gravity, Jump, Move};
use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    pub round: u32,
}
#[derive(Default, Component)]
pub struct Player {
    pub velocity: Vec3,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
    pub glow: Tween,
}
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(40.0, 40.0),
                Material::grid([0.16, 0.23, 0.24], 1.0),
            ),
        );
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.95, 0.64, 0.25),
                Player::default(),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 12.0, 17.0).lag(0.15),
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(8.0, 14.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for i in 1..=6 {
            w.spawn_named(
                format!("crate-{i}"),
                (
                    Transform::at(w.rand(-16.0..16.0), 0.5, w.rand(-16.0..16.0)),
                    Mesh::cube(1.0),
                    Material::rgb(0.46, 0.34, 0.23),
                ),
            );
        }
        for (i, (x, z)) in [(8.0, 0.0), (-6.0, 7.0), (3.0, -9.0)]
            .into_iter()
            .enumerate()
        {
            w.spawn_named(
                format!("beacon-{}", i + 1),
                (
                    Transform::at(x, 1.0, z),
                    Mesh::sphere(0.5),
                    Material::rgb(0.15, 0.65, 0.72),
                    Beacon::default(),
                ),
            );
            w.spawn_named(
                format!("plinth-{}", i + 1),
                (
                    Transform::at(x, 0.1, z),
                    Mesh::cube(1.0),
                    Material::rgb(0.29, 0.40, 0.41),
                ),
            );
        }
        w.publish("beacons", 0);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        let now = w.now();
        let mut position = Vec3::ZERO;
        if let Some((player, pose)) = w.query::<(&mut Player, &mut Transform)>().one() {
            Move {
                speed: 4.0,
                accel: 12.0,
                brake: 20.0,
            }
            .step(&mut player.velocity, input.stick_xz("move"), dt);
            if input.pressed("jump") && pose.position.y <= 0.9 {
                Jump {
                    height: 1.2,
                    gravity: 9.81,
                }
                .start(&mut player.velocity);
            }
            pose.position.x = (pose.position.x + player.velocity.x * dt).clamp(-19.6, 19.6);
            pose.position.z = (pose.position.z + player.velocity.z * dt).clamp(-19.6, 19.6);
            if pose.position.y > 0.9 || player.velocity.y > 0.0 {
                pose.position.y += player.velocity.y * dt - 0.5 * 9.81 * dt * dt;
                Gravity(9.81).step(&mut player.velocity, dt);
                if pose.position.y <= 0.9 {
                    pose.position.y = 0.9;
                    player.velocity.y = 0.0;
                }
            }
            position = pose.position;
        }
        let mut count = 0;
        for (mut beacon, pose, mut material) in
            w.query::<(&mut Beacon, &Transform, &mut Material)>()
        {
            if !beacon.lit
                && input.pressed("light")
                && position.distance_squared(pose.position) <= 2.25
            {
                beacon.lit = true;
                beacon.glow.to(now, 1.0, 0.5);
                w.log("beacon lit");
            }
            let glow = beacon.glow.value(now);
            material.emissive = [glow * 2.0, glow * 2.5, glow * 2.0];
            count += u32::from(beacon.lit);
        }
        w.publish("beacons", count);
        scene::follow(w);
    }
}
