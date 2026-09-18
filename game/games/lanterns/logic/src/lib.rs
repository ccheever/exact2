//! Lanterns: one complete three-minute island game.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use exact_game::audio::{self, AudioListener, AudioSource, Sounds, Synth};
use exact_game::scene::Animation;
use exact_game::{
    scene, Actions, Asset, Bloom, Camera, Component, DirectionalLight, Environment, Fog, Follow,
    Game, Id, Input, Kind, Material, Mesh, Parent, PointLight, Quat, Region, Resource, Spring,
    Stick, Transform, Vec2, Vec3, World,
};
use exact_game_physics::{self as physics, Body, Character, Collider, Physics};

#[derive(Kind)]
struct Hero {
    pub character: Character,
    #[read]
    pub transform: Transform,
}
#[derive(Kind)]
struct Stride {
    pub player: Player,
}
#[derive(Kind)]
struct Fox {
    pub animation: Animation,
}
#[derive(Kind)]
struct Pose {
    pub transform: Transform,
}
#[derive(Kind)]
struct Lamp {
    #[child("bulb", bulb)]
    pub lantern: Lantern,
}
#[derive(Kind)]
struct Lightable {
    #[read]
    pub transform: Transform,
    pub lantern: Lantern,
}
#[derive(Kind)]
struct Bulb {
    pub material: Material,
    pub light: PointLight,
}

#[derive(Default, Clone, Copy, exact_game::Data)]
struct Actors {
    pub hero: Id<Hero>,
    pub stride: Id<Stride>,
    pub fox: Id<Fox>,
    pub fox_pose: Id<Pose>,
    pub sun: Id<Pose>,
}

const FOX_BYTES: &[u8] = include_bytes!("../../assets/Fox.glb");
const DURATION_TICKS: u32 = 180 * 60;
const SPAWN: Vec3 = Vec3::new(0.0, 0.0, 12.0);
/// Canvas inputs. Action counters let Contract overlay buttons feed distinct
/// touch presses while the left thumb remains on the raw movement stick.
#[derive(Default, exact_game::Args)]
pub struct Options {
    /// Procedural scenery seed.
    pub seed: u64,
    /// Whether the title has started this round.
    #[live]
    pub started: bool,
    /// Pause simulation without destroying the world.
    #[live]
    pub paused: bool,
    /// Setup counter; changing it starts a fresh game.
    pub round: u32,
    /// Monotonic Contract Jump-button press counter.
    #[live]
    pub jump_press: u32,
    /// Monotonic Contract Light-button press counter.
    #[live]
    pub light_press: u32,
    /// Whether new synthesized sounds may start.
    #[live]
    pub sound: bool,
    /// Baked typed initial conditions; a construction argument, never a live edit.
    pub scene: String,
}

/// Player-only saved state.
#[derive(Default, Component)]
pub struct Player {
    /// Ground distance carried to the next footstep.
    pub stride: f32,
}

/// One saved lightable target.
#[derive(Default, Component)]
pub struct Lantern {
    /// Whether this target has been lit.
    pub lit: bool,
    /// Deterministic light-on envelope.
    pub glow: Spring,
    bulb: Id<Bulb>,
}

/// Saved round state independent of presentation.
#[derive(Default, Resource)]
pub struct Session {
    /// Active fixed ticks; pause and terminal states do not add to this.
    pub elapsed: u32,
    /// 0 title, 1 playing, 2 won, 3 lost.
    pub phase: u32,
    /// Last Contract Jump counter consumed.
    pub jump_press: u32,
    /// Last Contract Light counter consumed.
    pub light_press: u32,
    actors: Actors,
}

/// The complete Lanterns game behind `surface=world(...)`.
pub struct Lanterns;

impl Game for Lanterns {
    const CAPTURE_SUPPORTED: bool = true;
    const ID: &'static str = "lanterns";
    type Args = Options;

    fn assets() -> &'static [Asset] {
        static ASSETS: [Asset; 1] = [Asset::glb("Fox.glb", FOX_BYTES)];
        &ASSETS
    }

    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows().or_touch(Region::Left))
            .button("jump", &["Space"])
            .button("light", &["KeyE"])
    }

    fn validate(args: &Options) -> Result<(), String> {
        scene_types()
            .prepare(&args.scene, Self::assets())
            .map(|_| ())
    }

    fn setup(world: &mut World, args: &Options) {
        world.reseed(args.seed);
        physics::register(world);
        world.register_audio().register_resource::<Session>();
        world.insert_resource(Session {
            phase: u32::from(args.started),
            jump_press: args.jump_press,
            light_press: args.light_press,
            ..Session::default()
        });
        world.resource_mut::<Physics>().gravity = Vec3::new(0.0, -12.0, 0.0);
        define_sounds(world);
        world.insert_resource(Environment {
            background: None,
            zenith: [0.08, 0.12, 0.28],
            horizon: [0.52, 0.22, 0.16],
            ground: [0.035, 0.025, 0.06],
            ambient: 0.42,
            sun_disc: 0.015,
            exposure: 1.0,
            fog: Some(Fog {
                color: Some([0.12, 0.14, 0.24]),
                density: 0.008,
                height_falloff: 0.04,
            }),
            bloom: Some(Bloom {
                threshold: 1.0,
                intensity: 0.8,
                radius: 1.0,
            }),
        });
        let types = scene_types();
        types.register(world);
        types
            .prepare(&args.scene, Self::assets())
            .expect("validated scene")
            .instantiate(world)
            .expect("fresh scene identities");
        let character = Character {
            radius: 0.35,
            height: 1.3,
            step: 0.32,
            mass: 80.0,
            ..Character::default()
        };
        let at = Transform::at(SPAWN.x, SPAWN.y + 0.65, SPAWN.z);
        let player = world.spawn_named("player", (at, character, Player::default()));
        world.spawn_named(
            "fox",
            (
                Transform::at(0.0, -0.65, 0.0).with_scale(0.018),
                Parent(player),
                Mesh::asset("Fox.glb"),
                Material::default().rough(0.58),
                Animation::looping("Survey"),
            ),
        );
        let camera = Camera {
            fov_y_degrees: 45.0,
            near: 0.08,
            far: 100.0,
            active: true,
        };
        let wind = AudioSource {
            sound: "wind".into(),
            gain: 0.22,
            playing: true,
        };
        let follow = Follow::new(player).offset(10.0, 8.5, 14.0).lag(0.12);
        world.spawn_named(
            "camera",
            (Transform::default(), camera, AudioListener, wind, follow),
        );
        let sunlight = DirectionalLight {
            color: [1.0, 0.52, 0.24],
            illuminance: 3.4,
            shadows: true,
        };
        let sun = Transform::at(16.0, 25.0, -18.0).looking_at(Vec3::ZERO, Vec3::Y);
        world.spawn_named("sun", (sun, sunlight));
        spawn_sign(world);
        spawn_decor(world, args.seed);
        bind_scene(world);
        publish(world);
    }

    fn paused(args: &Options) -> bool {
        args.paused || !args.started
    }

    fn tick(world: &mut World, input: &Input, args: &Options) {
        let ids = world.resource::<Session>().actors;
        let (terminal, jump_button, light_button) = world.edit_resource::<Session, _>(|session| {
            if session.phase == 0 && args.started {
                session.phase = 1;
            }
            let jump = session.jump_press != args.jump_press;
            let light = session.light_press != args.light_press;
            session.jump_press = args.jump_press;
            session.light_press = args.light_press;
            (session.phase >= 2, jump, light)
        });
        if terminal {
            publish(world);
            audio::step(world);
            return;
        }
        world.resource_mut::<Session>().elapsed += 1;
        let dt = world.dt();
        let jump = jump_button || input.pressed("jump");
        let direction = input.stick_xz("move");
        let jumped = world.edit(ids.hero, |hero| {
            let character = &mut hero.character;
            if jump && character.grounded {
                character.velocity.y = 6.4;
                character.grounded = false;
                true
            } else {
                character.velocity.y = (character.velocity.y - 12.0 * dt).max(-30.0);
                false
            }
        });
        physics::move_character(world, ids.hero.entity(), direction * 4.5);
        physics::step(world);
        if jumped {
            world.log("jump");
            if args.sound {
                world.play("jump").at(ids.hero.entity()).start();
            }
        }
        if let Err(error) = update_player(world, ids, direction, args.sound) {
            world.log(error);
            return;
        }
        if light_button || input.pressed("light") {
            if let Err(error) = light_nearest(world, args.sound) {
                world.log(error);
                return;
            }
        }
        update_lanterns(world);
        let elapsed = world.resource::<Session>().elapsed;
        if elapsed >= DURATION_TICKS {
            world.resource_mut::<Session>().phase = 3;
            world.log("night fell");
            if args.sound {
                world.play("night").start();
            }
        }
        update_sun(world, ids.sun);
        scene::follow(world);
        audio::step(world);
        publish(world);
    }
}

fn bind_scene(world: &World) {
    let ids = Actors {
        hero: world.bind("player").expect("player Hero"),
        stride: world.bind("player").expect("player Stride"),
        fox: world.bind("fox").expect("fox animation"),
        fox_pose: world.bind("fox").expect("fox pose"),
        sun: world.bind("sun").expect("sun pose"),
    };
    let lamps: Vec<_> = world
        .entities()
        .filter(|&e| world.has::<Lantern>(e))
        .collect();
    for lamp in lamps {
        world.bind::<Lamp>(lamp).expect("lamp child binding");
    }
    world.resource_mut::<Session>().actors = ids;
}

fn define_sounds(world: &mut World) {
    let mut sounds = world.resource_mut::<Sounds>();
    let foot = Synth::noise().seconds(0.08).attack(0.002).release(0.07);
    sounds.add("footstep", foot.lowpass_hz(720.0).gain(0.24));
    let jump = Synth::sine(280.0).seconds(0.12).attack(0.005).release(0.1);
    sounds.add("jump", jump.gain(0.18));
    let chime = Synth::sine(660.0).seconds(0.7).attack(0.005).release(0.6);
    let overtone = Synth::sine(990.0).seconds(0.5).release(0.42).gain(0.14);
    sounds.add("chime", chime.gain(0.28).layer(overtone));
    let night = Synth::sine(160.0).seconds(1.2).attack(0.04).release(1.0);
    sounds.add("night", night.gain(0.24));
    let wind = Synth::noise().seconds(2.0).sustain(1.0).lowpass_hz(420.0);
    sounds.add("wind", wind.gain(0.1).looped());
}

fn spawn_static_box(world: &mut World, name: &str, at: Vec3, size: Vec3, material: Material) {
    let mesh = Mesh::cuboid(size);
    let pose = Transform::at(at.x, at.y, at.z);
    let collider = Collider::of(&mesh);
    world.spawn_named(name, (pose, mesh, material, collider));
}

fn spawn_sign(world: &mut World) {
    for (name, y, size, color) in [
        ("sign-post", 0.85, [0.18, 1.7, 0.18], [0.32, 0.18, 0.08]),
        ("sign-board", 1.55, [2.5, 0.9, 0.14], [0.36, 0.21, 0.1]),
    ] {
        let at = Vec3::new(3.0, y, 10.0);
        let material = Material::rgb(color[0], color[1], color[2]);
        spawn_static_box(world, name, at, size.into(), material);
    }
}

fn spawn_decor(world: &mut World, seed: u64) {
    world.reseed(seed);
    let before: Vec<_> = world.entities().collect();
    for i in 0..18 {
        let angle = world.rand(0.0..std::f32::consts::TAU);
        let radius = world.rand(13.0..16.5);
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let trunk = world.spawn_named(
            format!("tree-{i}"),
            (
                Transform::at(x, 1.05, z),
                Mesh::cylinder(0.17, 2.1),
                Material::rgb(0.19, 0.12, 0.07),
            ),
        );
        world.spawn((
            Transform::at(0.0, 2.15, 0.0).with_scale(Vec3::new(1.0, 1.6, 1.0)),
            Parent(trunk),
            Mesh::cylinder(1.15, 2.4),
            Material::rgb(0.04, 0.17, 0.1).rough(0.9),
        ));
    }
    for i in 0..22 {
        let angle = world.rand(0.0..std::f32::consts::TAU);
        let radius = world.rand(14.0..17.0);
        world.spawn_named(
            format!("rock-{i}"),
            (
                Transform::at(angle.cos() * radius, 0.2, angle.sin() * radius).with_scale(
                    Vec3::new(
                        world.rand(0.4..1.0),
                        world.rand(0.2..0.6),
                        world.rand(0.4..1.0),
                    ),
                ),
                Mesh::sphere(0.5),
                Material::rgb(0.24, 0.25, 0.3).rough(0.95),
            ),
        );
    }
    let generated: Vec<_> = world.entities().filter(|e| !before.contains(e)).collect();
    for entity in generated {
        world.insert(
            entity,
            exact_game_scene::GeneratedBy {
                generator: "lanterns::spawn_decor".into(),
                parameters: [
                    ("seed".into(), seed.to_string()),
                    ("trees".into(), "18".into()),
                    ("rocks".into(), "22".into()),
                ]
                .into(),
            },
        );
    }
}

fn update_player(
    world: &mut World,
    ids: Actors,
    direction: Vec3,
    sound: bool,
) -> Result<(), exact_game::KindError> {
    let h = world.row(ids.hero)?;
    let (grounded, velocity, mut pose) = (h.character.grounded, h.character.velocity, *h.transform);
    drop(h);
    let speed = Vec2::new(velocity.x, velocity.z).length();
    let clip = if !grounded || speed >= 2.8 {
        "Run"
    } else if speed > 0.08 {
        "Walk"
    } else {
        "Survey"
    };
    world.edit(ids.fox, |fox| {
        fox.animation.play(clip);
        fox.animation.advance(world.dt());
    });
    if direction.length_squared() > 0.0025 {
        let yaw = direction.x.atan2(-direction.z);
        world.edit(ids.fox_pose, |fox| {
            fox.transform.rotation = Quat::from_rotation_y(yaw)
        });
    }
    if grounded && speed > 0.08 {
        let footsteps = world.edit(ids.stride, |row| {
            let mut footsteps = 0;
            row.player.stride += speed * world.dt();
            while row.player.stride >= 0.7 {
                row.player.stride -= 0.7;
                footsteps += 1;
            }
            footsteps
        });
        if sound {
            for _ in 0..footsteps {
                world.play("footstep").at(ids.hero.entity()).start();
            }
        }
    }
    if pose.position.y - 0.65 < -6.0 {
        pose.position = SPAWN + Vec3::Y * 0.65;
        world.teleport(ids.hero.entity(), pose);
        world.edit(ids.hero, |h| {
            h.character.velocity = Vec3::ZERO;
            h.character.grounded = false;
        });
        world.log("fall reset");
    }
    Ok(())
}

fn light_nearest(world: &mut World, sound: bool) -> Result<(), exact_game::KindError> {
    let hero = world.resource::<Session>().actors.hero;
    let center = world.row(hero)?.transform.position - Vec3::Y * 0.65;
    let mut nearest = None;
    let mut distance = 1.5f32;
    for row in world.rows::<Lightable>() {
        if row.lantern.lit {
            continue;
        }
        let d = row.transform.position.distance(center);
        if d <= distance {
            distance = d;
            nearest = Some(row.id);
        }
    }
    let Some(entity) = nearest else { return Ok(()) };
    world.edit(entity, |row| {
        row.lantern.lit = true;
        row.lantern.glow.set_target(world.now(), 1.0);
    });
    let name = world
        .name(entity.entity())
        .ok_or_else(|| exact_game::KindError::unnamed::<Lamp>(world, entity.entity()))?;
    world.log(format_args!("{name} lit"));
    if sound {
        world.play("chime").at(entity.entity()).start();
    }
    if world.rows::<Lamp>().all(|row| row.lantern.lit) {
        world.resource_mut::<Session>().phase = 2;
        world.log("all lanterns lit");
    }
    Ok(())
}

fn update_lanterns(world: &mut World) {
    let now = world.now();
    for row in world.rows::<Lamp>() {
        let bulb = row.lantern.bulb;
        let glow = row.lantern.glow.value(now);
        world.edit(bulb, |b| {
            b.material.emissive = [glow * 4.0, glow * 1.65, glow * 0.32];
            b.light.intensity = glow * 5.0;
        });
    }
}

fn update_sun(world: &mut World, sun: Id<Pose>) {
    let progress = world.resource::<Session>().elapsed as f32 / DURATION_TICKS as f32;
    let angle = 0.22 - progress * 0.52;
    let position = Vec3::new(24.0 * angle.cos(), 24.0 * angle.sin() + 4.0, -18.0);
    world.edit(sun, |row| {
        *row.transform =
            Transform::at(position.x, position.y, position.z).looking_at(Vec3::ZERO, Vec3::Y)
    });
}

fn publish(world: &mut World) {
    let count = world.rows::<Lamp>().filter(|row| row.lantern.lit).count() as u32;
    let remaining = DURATION_TICKS
        .saturating_sub(world.resource::<Session>().elapsed)
        .div_ceil(60);
    let phase = match world.resource::<Session>().phase {
        0 => "title",
        1 => "playing",
        2 => "won",
        _ => "lost",
    };
    world.publish("count", count);
    world.publish("total", world.rows::<Lamp>().count() as u32);
    world.publish("remaining", remaining);
    world.publish("phase", phase);
    world.publish("assetReady", true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::Sim;

    fn game() -> Sim<Lanterns> {
        Sim::new(Options {
            scene: bake_scene().content,
            seed: 1_041_003,
            started: true,
            sound: true,
            ..Options::default()
        })
        .unwrap()
    }

    #[test]
    fn level_has_matching_player_crate_obstacles_and_twelve_targets() {
        let sim = game();
        let world = sim.world();
        assert_eq!(world.rows::<Lamp>().count(), 12);
        assert_eq!(
            world.get::<Transform>("crate").unwrap().position,
            Vec3::new(6.0, 0.6, 8.0)
        );
        assert_eq!(
            world.get::<Transform>("lantern-12").unwrap().position,
            Vec3::new(10.0, 2.4, 8.0)
        );
        assert!(world.has::<Animation>(world.named("fox").unwrap()));
    }

    #[test]
    fn one_second_moves_at_four_point_five_and_selects_run() {
        let mut sim = game();
        sim.key_down("KeyW");
        sim.run(1000.0);
        sim.key_up("KeyW");
        let pose = sim.world().get::<Transform>("player").unwrap();
        assert!(
            (pose.position.z - 7.5).abs() < 0.08,
            "z={}",
            pose.position.z
        );
        assert_eq!(sim.world().get::<Animation>("fox").unwrap().clip, "Run");
    }

    #[test]
    fn save_restores_dynamic_body_timer_animation_and_lights() {
        let mut sim = game();
        sim.run(100.0);
        let bytes = sim.save().unwrap();
        let hash = sim.world().hash();
        let mut restored = game();
        restored.restore(&bytes).unwrap();
        assert_eq!(restored.world().hash(), hash);
        assert_eq!(restored.world().resource::<Session>().elapsed, 6);
        assert_eq!(
            restored.world().get::<Animation>("fox").unwrap().clip,
            "Survey"
        );
    }

    #[test]
    fn no_input_loses_at_exactly_three_minutes() {
        let mut sim = game();
        sim.run(180_000.0);
        assert_eq!(sim.world().resource::<Session>().phase, 3);
        assert_eq!(sim.world().resource::<Session>().elapsed, DURATION_TICKS);
    }
}

/// Scene type selection; every field/default/variant comes from these Rust declarations.
pub fn scene_types() -> exact_game_scene::Types {
    let mut types = exact_game_scene::Types::standard();
    types
        .component::<Animation>()
        .component::<Lantern>()
        .component::<Collider>()
        .component::<Body>();
    types
}

#[cfg(test)]
fn bake_scene() -> exact_game_scene::bake::Baked {
    exact_game_scene::bake::compile(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
        &scene_types(),
        Lanterns::assets(),
    )
    .unwrap()
}
#[cfg(test)]
mod scene_tests;

#[cfg(test)]
mod kind_tests;
