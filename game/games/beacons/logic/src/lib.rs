//! A capsule, landmarks, and three beacons. No host and no GPU.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::{
    math, scene, Actions, Camera, Component, DirectionalLight, Follow, Game, Input, Material, Mesh,
    Stick, Transform, Tween, Vec3, World,
};

/// Horizontal acceleration and a ballistic hop, in meters and seconds.
#[derive(Default, Component)]
pub struct Player {
    /// Current velocity, carried by saves along with the pose.
    pub velocity: Vec3,
}
/// A one-shot beacon whose glow can be sampled at any tick.
#[derive(Default, Component)]
pub struct Beacon {
    /// Emission envelope; the engine discovers its settling state automatically.
    pub glow: Tween,
}
/// Logic behind `canvas surface=world(seed, run, paused)`.
pub struct Beacons;
/// Typed canvas arguments in positional order.
#[derive(Default, exact_game::Args)]
pub struct BeaconsArgs {
    /// Canvas setup argument.
    pub seed: u64,
    /// Canvas setup argument.
    pub run: u32,
    /// Canvas live argument.
    #[live]
    pub paused: bool,
}
impl Game for Beacons {
    const ID: &'static str = "beacons";
    type Args = BeaconsArgs;

    fn actions() -> Actions {
        Actions::new()
            .button("act", &["KeyE", "Enter"])
            .button("jump", &["Space"])
            .stick("move", Stick::wasd().or_arrows())
    }
    fn setup(world: &mut World, args: &Self::Args) {
        world.reseed(args.seed);
        world.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(40.0, 40.0),
                Material::rgb(0.25, 0.27, 0.3),
            ),
        );
        let player = world.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.45, 0.15),
                Player::default(),
            ),
        );
        world.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15),
            ),
        );
        world.spawn_named(
            "sun",
            (
                Transform::at(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for i in 1..=6 {
            world.spawn_named(
                format!("crate-{i}"),
                (
                    Transform::at(world.rand(-16.0..16.0), 0.5, world.rand(-16.0..16.0)),
                    Mesh::cube(1.0),
                    Material::default(),
                ),
            );
        }
        for (i, (x, z)) in [(8.0, 0.0), (-6.0, 7.0), (3.0, -9.0)]
            .into_iter()
            .enumerate()
        {
            world.spawn_named(
                format!("beacon-{}", i + 1),
                (
                    Transform::at(x, 1.0, z),
                    Mesh::sphere(0.5),
                    Material::rgb(0.1, 0.55, 0.65),
                    Beacon::default(),
                ),
            );
        }
        world.publish("beacons", 0);
        scene::follow(world);
    }
    fn paused(args: &Self::Args) -> bool {
        args.paused
    }
    fn tick(world: &mut World, input: &Input, _: &Self::Args) {
        let dt = world.dt();
        let desired = input.stick_xz("move") * 4.0;
        let mut position = Vec3::ZERO;
        if let Some((player, pose)) = world.query::<(&mut Player, &mut Transform)>().one() {
            player.velocity.x = math::ease(player.velocity.x, desired.x, 0.074690334, dt);
            player.velocity.z = math::ease(player.velocity.z, desired.z, 0.074690334, dt);
            if input.pressed("jump") && pose.position.y <= 0.9 {
                player.velocity.y = 4.852216; // sqrt(2 * 9.81 * 1.2)
            }
            pose.position.x += player.velocity.x * dt;
            pose.position.z += player.velocity.z * dt;
            if pose.position.y > 0.9 || player.velocity.y > 0.0 {
                pose.position.y += player.velocity.y * dt - 0.5 * 9.81 * dt * dt;
                player.velocity.y -= 9.81 * dt;
                if pose.position.y <= 0.9 {
                    pose.position.y = 0.9;
                    player.velocity.y = 0.0;
                }
            }
            pose.position.x = pose.position.x.clamp(-19.6, 19.6);
            pose.position.z = pose.position.z.clamp(-19.6, 19.6);
            position = pose.position;
        }
        let mut count = 0;
        for (mut beacon, pose, mut material) in
            world.query::<(&mut Beacon, &Transform, &mut Material)>()
        {
            let delta = position - pose.position;
            if beacon.glow.target == 0.0
                && input.pressed("act")
                && delta.x * delta.x + delta.z * delta.z <= 1.5 * 1.5
                && position.y <= 0.9001
            {
                beacon.glow.to(world.now(), 1.0, 0.5);
                world.log("beacon lit");
            }
            count += u32::from(beacon.glow.target == 1.0);
            material.emissive = [beacon.glow.value(world.now()) * 3.0; 3];
        }
        world.publish("beacons", count);
        scene::follow(world);
    }
}
