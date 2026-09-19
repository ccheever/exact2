//! The first game: a capsule, landmarks, and one beacon. No host and no GPU.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::audio::{self, AudioListener, AudioSource, Synth};
use exact_game::character::Character;
use exact_game::Vec3Swizzles;
use exact_game::{
    scene, Actions, Camera, Component, DirectionalLight, Follow, Game, Input, Material, Mesh,
    Spring, Stick, Transform, Vec3, World,
};

#[derive(Default, exact_game::Data)]
struct Hud {
    beacons: u32,
}
/// Horizontal acceleration and a ballistic hop, in meters and seconds.
#[derive(Default, Component)]
pub struct Player {
    /// Ground distance carried toward the next footstep.
    pub stride: f32,
}
/// A one-shot beacon whose glow can be sampled at any tick.
#[derive(Default, Component)]
pub struct Beacon {
    /// Whether act has lit this beacon.
    pub lit: bool,
    /// Emission envelope; the engine discovers its settling state automatically.
    pub glow: Spring,
}
/// Logic behind `canvas surface=world(seed: seed, paused: paused)`.
pub struct Greybox;
/// Typed canvas arguments bound by field name.
#[derive(Default, exact_game::Args)]
pub struct GreyboxArgs {
    /// Canvas setup argument.
    pub seed: u64,
    /// Canvas live argument.
    #[live]
    pub paused: bool,
    /// Toggle to reconstruct through setup.
    #[restart]
    pub restart: bool,
}
impl Game for Greybox {
    const ID: &'static str = "greybox";
    const HZ: u32 = 60;
    type Args = GreyboxArgs;

    fn actions() -> Actions {
        Actions::new()
            .button("act", &["KeyE", "Enter"])
            .button("jump", &["Space"])
            .stick("move", Stick::wasd().or_arrows())
    }
    fn setup(world: &mut World, args: &Self::Args) {
        world.reseed(args.seed);
        world.sounds([
            (
                "footstep",
                Synth::noise()
                    .seconds(0.09)
                    .attack(0.002)
                    .release(0.08)
                    .lowpass_hz(650.0)
                    .gain(0.4),
            ),
            (
                "chime",
                Synth::sine(880.0)
                    .seconds(0.8)
                    .attack(0.005)
                    .release(0.7)
                    .gain(0.3)
                    .layer(Synth::sine(1320.0).seconds(0.5).release(0.45).gain(0.12)),
            ),
            (
                "wind",
                Synth::noise()
                    .seconds(2.0)
                    .attack(0.0)
                    .release(0.0)
                    .sustain(1.0)
                    .lowpass_hz(380.0)
                    .gain(0.12)
                    .looped(),
            ),
        ]);
        world.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(40.0, 40.0),
                Material::grid([0.25, 0.27, 0.3], 1.0),
            ),
        );
        let player = world.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.45, 0.15),
                Player::default(),
                // Character defaults, explicit so the movement is auditable here.
                Character::new()
                    .speed(4.0)
                    .accel(12.0)
                    .brake(20.0)
                    .jump(1.2)
                    .gravity(9.81)
                    .ground(0.9)
                    .bounds_xz(-19.6..=19.6),
            ),
        );
        world.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                AudioListener,
                AudioSource::new("wind").gain(0.3),
                Follow::new(player).offset(0.0, 5.0, 8.0).lag(0.0),
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
        world.publish_record(&Hud::default());
    }
    fn paused(args: &Self::Args) -> bool {
        args.paused
    }
    fn tick(world: &mut World, input: &Input, _: &Self::Args) {
        let now = world.tick_end();
        let mut footsteps = 0;
        {
            let motion = world
                .character("player")
                .step(input.stick_xz("move"), input.pressed("jump"));
            if motion.grounded {
                let mut player = world.require_mut::<Player>("player");
                player.stride += motion.displacement.xz().length();
                while player.stride >= 0.45 {
                    player.stride -= 0.45;
                    footsteps += 1;
                }
            }
        }
        for _ in 0..footsteps {
            let pitch = world.rand(0.94..1.06);
            world.play("footstep").at("player").pitch(pitch).start();
        }
        if input.pressed("act") {
            if let Some((entity, mut beacon)) =
                world.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit)
            {
                beacon.lit = true;
                beacon.glow.set_target(now, 1.0);
                world.publish_record(&Hud { beacons: 1 });
                world.log("beacon-1 lit");
                world.play("chime").at(entity).start();
            }
        }
        for (beacon, mut material) in world.query::<(&Beacon, &mut Material)>() {
            material.emissive = [beacon.glow.value(now) * 3.0; 3];
        }
        scene::follow(world);
        audio::step(world);
    }
}
