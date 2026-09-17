//! The first game: a capsule, landmarks, and one beacon. No host and no GPU.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::{
    Actions, Arg, Args, Camera, Component, DirectionalLight, Game, Input, Material, Mesh, Stick,
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
    pub glow: f32,
    /// Ticks since ignition.
    pub age: u32,
}
/// Logic behind `canvas surface=world(seed, paused)`.
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    const ARGS: &'static [Arg] = &[Arg::setup("seed"), Arg::live("paused"), Arg::setup("run")];
    fn check(args: &Args) -> Result<(), String> {
        args.integer("seed")?;
        args.integer("run")?;
        args.flag("paused")?;
        Ok(())
    }
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
        world.reseed(args.integer("seed")?);
        world.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::Plane { size: 40.0 },
                Material::rgb(0.25, 0.27, 0.3),
            ),
        );
        world.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::Capsule {
                    radius: 0.4,
                    height: 1.8,
                },
                Material::rgb(0.8, 0.45, 0.15),
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
        for i in 1..=6 {
            world.spawn_named(
                format!("crate-{i}"),
                (
                    Transform::at(world.rand(-16.0..16.0), 0.5, world.rand(-16.0..16.0)),
                    Mesh::Cube,
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
                    Mesh::Sphere,
                    Material::rgb(0.1, 0.55, 0.65),
                    Beacon::default(),
                ),
            );
        }
        world.publish("beacons", 0);
        Ok(())
    }
    fn paused(args: &Args) -> bool {
        args.flag("paused").unwrap()
    }
    fn tick(world: &mut World, input: &Input) {
        let dt = world.dt();
        let direction = input.stick("move");
        let desired = Vec3::new(direction.x, 0.0, -direction.y) * 4.0;
        let mut position = Vec3::ZERO;
        let mut moving = false;
        if let Some((_, (player, pose))) = world.query::<(&mut Player, &mut Transform)>().one() {
            // A fixed first-order response makes release and acceleration gradual;
            // snapping below this threshold makes clock settle have a finite end.
            player.velocity.x += (desired.x - player.velocity.x) * 0.2;
            player.velocity.z += (desired.z - player.velocity.z) * 0.2;
            if desired == Vec3::ZERO && player.velocity.x.abs() + player.velocity.z.abs() < 0.0001 {
                player.velocity.x = 0.0;
                player.velocity.z = 0.0;
            }
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
            moving = player.velocity != Vec3::ZERO;
        }
        if moving {
            world.busy("player moving");
        }
        let mut count = 0;
        let mut glowing = false;
        for (_, (beacon, pose, material)) in world
            .query::<(&mut Beacon, &Transform, &mut Material)>()
            .iter()
        {
            let delta = position - pose.position;
            if !beacon.lit
                && input.pressed("act")
                && delta.x * delta.x + delta.z * delta.z <= 1.5 * 1.5
                && position.y <= 0.9001
            {
                beacon.lit = true;
                world.log("beacon lit");
            }
            if beacon.lit {
                count += 1;
                beacon.age = (beacon.age + 1).min(30);
                let t = beacon.age as f32 / 30.0;
                beacon.glow = t * t * (3.0 - 2.0 * t);
                glowing |= beacon.age < 30;
            }
            material.emissive = [beacon.glow * 3.0; 3];
        }
        world.publish("beacons", count);
        if glowing {
            world.busy("beacon easing");
        }
        let e = world.named("camera").unwrap();
        let mut pose = world.get_mut::<Transform>(e).unwrap();
        let desired = position + Vec3::new(0.0, 9.0, 13.0);
        let delta = desired - pose.position;
        let moving = delta.length_squared() > 0.000001;
        let next = if moving {
            pose.position + delta * 0.1
        } else {
            desired
        };
        // Looking along a fixed offset gives a smoothly lagging target as well.
        *pose = Transform::at(next.x, next.y, next.z)
            .looking_at(next - Vec3::new(0.0, 9.0, 13.0), Vec3::Y);
        drop(pose);
        if moving {
            world.busy("camera following");
        }
    }
}
fn camera(player: Vec3) -> Transform {
    Transform::at(player.x, player.y + 9.0, player.z + 13.0).looking_at(player, Vec3::Y)
}
