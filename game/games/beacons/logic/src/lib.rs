use exact_game::character::Character;
use exact_game::*;

#[derive(Default, exact_game::Data)]
struct Hud {
    lit: u32,
    near: String,
}
#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    // Restart idiom: changing restart_generation reconstructs setup; Play again increments it.
    pub restart_generation: u32,
}
#[derive(Default, Component)]
pub struct Player {
    pub character: Character,
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
                Material::rgb(0.16, 0.23, 0.24),
            ),
        );
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.4, 0.1),
                Player {
                    character: Character::new().ground(0.9).bounds_xz(-19.6..=19.6),
                },
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15),
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for i in 1..=6 {
            w.spawn_named(
                format!("crate-{i}"),
                (
                    Transform::at(w.rand(-16.0..16.0), 0.5, w.rand(-16.0..16.0)),
                    Mesh::cube(1.0),
                    Material::rgb(0.48, 0.34, 0.22),
                ),
            );
        }
        for (i, (x, z)) in [(8.0, 0.0), (-6.0, 7.0), (3.0, -9.0)]
            .into_iter()
            .enumerate()
        {
            w.spawn_named(
                format!("plinth-{}", i + 1),
                (
                    Transform::at(x, 0.1, z),
                    Mesh::cylinder(0.9, 0.2),
                    Material::rgb(0.3, 0.4, 0.42),
                ),
            );
            w.spawn_named(
                format!("beacon-{}", i + 1),
                (
                    Transform::at(x, 1.0, z),
                    Mesh::sphere(0.5),
                    Material::rgb(0.22, 0.65, 0.72),
                    Beacon::default(),
                ),
            );
        }
        w.publish_record(&Hud::default());
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        {
            let mut query = w.query::<(&mut Player, &mut Transform)>();
            let (player, pose) = query.one().expect("one player");
            player
                .character
                .step(pose, input.stick_xz("move"), input.pressed("jump"), dt);
        }
        let position = Vec3::from(
            w.current_global(w.named("player").unwrap())
                .unwrap()
                .translation,
        );
        let mut nearest = None;
        for (entity, pose) in w.near_xz::<Beacon>("player", 1.5) {
            let mut beacon = w.get_mut::<Beacon>(entity).unwrap();
            if !beacon.lit && input.pressed("light") {
                beacon.lit = true;
                beacon.glow.to(w.now(), 1.0, 0.5);
            }
            let distance = Vec2::new(pose.position.x - position.x, pose.position.z - position.z)
                .length_squared();
            if !beacon.lit && nearest.is_none_or(|(_, old)| distance < old) {
                nearest = Some((entity, distance));
            }
        }
        let near = nearest
            .and_then(|(e, _)| w.name(e))
            .unwrap_or("")
            .to_owned();
        let mut count = 0;
        for (beacon, mut material) in w.query::<(&Beacon, &mut Material)>() {
            material.emissive = [beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(beacon.lit);
        }
        w.publish_record(&Hud { lit: count, near });
        scene::follow(w);
    }
}
