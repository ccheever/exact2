//! The first game: a capsule, landmarks, and one beacon. No host and no GPU.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::{
    math, scene, Actions, Camera, Component, DirectionalLight, Follow, Game, Input, Material, Mesh,
    Spring, Stick, Transform, Vec3, World,
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
    /// Whether act has lit this beacon.
    pub lit: bool,
    /// Emission envelope; the engine discovers its settling state automatically.
    pub glow: Spring,
}
/// Logic behind `canvas surface=world(seed, paused)`.
pub struct Greybox;
/// Typed canvas arguments in positional order.
#[derive(Default, exact_game::Args)]
pub struct GreyboxArgs {
    /// Canvas setup argument.
    pub seed: u64,
    /// Canvas live argument.
    #[live]
    pub paused: bool,
}
impl Game for Greybox {
    const ID: &'static str = "greybox";
    type Args = GreyboxArgs;

    fn actions() -> Actions {
        Actions::new()
            .button("act", &["KeyE", "Enter"])
            .button("jump", &["Space"])
            .stick(
                "move",
                Stick::keys("KeyW", "KeyS", "KeyA", "KeyD").or_keys(
                    "ArrowUp",
                    "ArrowDown",
                    "ArrowLeft",
                    "ArrowRight",
                ),
            )
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
        world.spawn_named(
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
                Transform::at(0.0, 5.9, 8.0).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
                Camera::default(),
                Follow::new("player").offset(0.0, 5.0, 8.0).lag(0.0),
            ),
        );
        world.spawn_named(
            "sun",
            (
                Transform::at(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for i in 1..=3 {
            world.spawn_named(
                format!("crate-{i}"),
                (
                    Transform::at(world.rand(3.0..12.0), 0.5, world.rand(-12.0..8.0)),
                    Mesh::cube(1.0),
                    Material::default(),
                ),
            );
        }
        world.spawn_named(
            "beacon-1",
            (
                Transform::at(0.0, 0.75, -6.5),
                Mesh::sphere(0.5),
                Material::rgb(0.15, 0.6, 0.8),
                Beacon::default(),
            ),
        );
        world.publish("beacons", 0);
    }
    fn paused(args: &Self::Args) -> bool {
        args.paused
    }
    fn tick(world: &mut World, input: &Input, _: &Self::Args) {
        let dt = world.dt();
        let now = world.now();
        let direction = input.stick("move");
        let desired = Vec3::new(direction.x, 0.0, -direction.y) * 4.0;
        let mut position = Vec3::ZERO;
        if let Some((_, (player, pose))) = world.query::<(&mut Player, &mut Transform)>().one() {
            player.velocity.x = math::ease(player.velocity.x, desired.x, 0.074690334, dt);
            player.velocity.z = math::ease(player.velocity.z, desired.z, 0.074690334, dt);
            if input.pressed("jump") && pose.position.y <= 0.9 {
                player.velocity.y = 5.0;
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
            position = pose.position;
        }
        for (_, (beacon, pose, material)) in world
            .query::<(&mut Beacon, &Transform, &mut Material)>()
            .iter()
        {
            if !beacon.lit
                && input.pressed("act")
                && position.distance_squared(pose.position) <= 1.5 * 1.5
            {
                beacon.lit = true;
                beacon.glow.set_target(now, 1.0);
                world.publish("beacons", 1);
                world.log("beacon-1 lit");
            }
            material.emissive = [beacon.glow.value(now) * 3.0; 3];
        }
        scene::follow(world);
    }
}
