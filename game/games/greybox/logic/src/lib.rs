//! The first game: a capsule, landmarks, and one beacon. No host and no GPU.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::{
    Actions, Args, Camera, Component, DirectionalLight, Game, Input, Material, Mesh, Spring, Stick,
    Transform, Vec3, World,
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
impl Game for Greybox {
    const ARGS: &'static [&'static str] = &["seed", "paused"];
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
    fn setup(world: &mut World, args: &Args) -> Result<(), String> {
        let seed = args.number(0, "seed")?;
        if seed < 0.0 || seed.fract() != 0.0 || seed > 9_007_199_254_740_991.0 {
            return Err("argument `seed` must be a nonnegative safe integer".into());
        }
        args.flag(1, "paused")?;
        world.reseed(seed as u64);
        world.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::Plane { size: 40.0 },
                Material {
                    color: [0.25, 0.27, 0.3, 1.0],
                    ..Material::default()
                },
            ),
        );
        world.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::Capsule {
                    radius: 0.4,
                    height: 1.0,
                },
                Material {
                    color: [0.8, 0.45, 0.15, 1.0],
                    ..Material::default()
                },
                Player::default(),
            ),
        );
        world.spawn_named(
            "camera",
            (camera(Vec3::new(0.0, 0.9, 0.0)), Camera::default()),
        );
        world.spawn_named(
            "sun",
            (
                Transform::at(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for i in 1..=3 {
            // Draw positions before spawning: the RNG lease must end before a
            // structural edit. Ordinary query borrows never escape a tick either.
            let x = world.rng().range(3.0..12.0);
            let z = world.rng().range(-12.0..8.0);
            world.spawn_named(
                &format!("crate-{i}"),
                (Transform::at(x, 0.5, z), Mesh::Cube, Material::default()),
            );
        }
        world.spawn_named(
            "beacon-1",
            (
                Transform::at(0.0, 0.75, -6.5).with_scale(0.5),
                Mesh::Sphere,
                Material {
                    color: [0.15, 0.6, 0.8, 1.0],
                    ..Material::default()
                },
                Beacon::default(),
            ),
        );
        world.publish("beacons", 0);
        Ok(())
    }
    fn bind(_world: &mut World, args: &Args) -> Result<(), String> {
        args.number(0, "seed")?;
        args.flag(1, "paused")?;
        Ok(())
    }
    fn paused(args: &Args) -> bool {
        args.flag(1, "paused").unwrap_or(false)
    }
    fn tick(world: &mut World, input: &Input) {
        let dt = world.dt();
        let now = world.now();
        let direction = input.stick("move");
        let desired = Vec3::new(direction.x, 0.0, -direction.y) * 4.0;
        let mut position = Vec3::ZERO;
        let mut moving = false;
        for (_, (player, pose)) in world.query::<(&mut Player, &mut Transform)>().iter() {
            // A fixed first-order response makes release and acceleration gradual;
            // snapping below this threshold makes clock settle have a finite end.
            player.velocity.x += (desired.x - player.velocity.x) * 0.2;
            player.velocity.z += (desired.z - player.velocity.z) * 0.2;
            if desired == Vec3::ZERO && player.velocity.x.abs() + player.velocity.z.abs() < 0.0001 {
                player.velocity.x = 0.0;
                player.velocity.z = 0.0;
            }
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
            moving |= player.velocity != Vec3::ZERO;
        }
        if moving {
            world.busy("player moving");
        }
        for (_, (beacon, pose, material)) in world
            .query::<(&mut Beacon, &Transform, &mut Material)>()
            .iter()
        {
            if !beacon.lit && input.pressed("act") && position.distance(pose.position) <= 1.5 {
                beacon.lit = true;
                beacon.glow.set_target(now, 1.0);
                world.publish("beacons", 1);
                world.log("beacon-1 lit");
            }
            material.emissive = [beacon.glow.value(now) * 3.0; 3];
        }
        let e = world.named("camera").unwrap();
        *world.get_mut::<Transform>(e).unwrap() = camera(position);
    }
}
fn camera(player: Vec3) -> Transform {
    Transform {
        position: player + Vec3::new(0.0, 5.0, 8.0),
        ..Transform::default()
    }
    .looking_at(player, Vec3::Y)
}
