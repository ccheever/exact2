//! Beacons: a deterministic, one-screen search for three lights.
use exact_game::character::Character;
use exact_game::*;
#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    pub round: u32,
    pub scene: String,
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
#[derive(Kind)]
struct Hero {
    player: Player,
    transform: Transform,
}
#[derive(Kind)]
struct Lightable {
    #[read]
    transform: Transform,
    beacon: Beacon,
}
#[derive(Kind)]
struct Glow {
    #[read]
    beacon: Beacon,
    material: Material,
}
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    const SAVE_VERSION: u32 = 2;
    const CAPTURE_SUPPORTED: bool = true;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn validate(args: &Options) -> Result<(), String> {
        scene_types()
            .prepare(&args.scene, Self::assets())
            .map(|_| ())
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        let types = scene_types();
        types.register(w);
        types
            .prepare(&args.scene, Self::assets())
            .expect("validated scene")
            .instantiate(w)
            .expect("fresh scene identities");
        let player = w.spawn_kind(
            "player",
            Hero {
                transform: Transform::at(0.0, 0.9, 0.0),
                player: Player {
                    character: Character::new()
                        .speed(4.0)
                        .accel(12.0)
                        .brake(20.0)
                        .jump(1.2)
                        .gravity(9.81)
                        .ground(0.9)
                        .bounds_xz(-19.6..=19.6),
                },
            },
        );
        w.insert(player.entity(), Mesh::capsule(0.4, 1.8));
        w.insert(player.entity(), Material::rgb(0.96, 0.65, 0.22));
        let player = player.entity();
        let follow = Follow::new(player).offset(0.0, 12.0, 17.0).lag(0.15);
        w.spawn_named("camera", (Transform::default(), Camera::default(), follow));
        let pose = Transform::at(8.0, 16.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y);
        w.spawn_named("sun", (pose, DirectionalLight::default()));
        spawn_crates(w, args.seed, 6);
        w.publish("beacons", 0);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let direction = input.stick_xz("move");
        let jump = input.pressed("jump");
        let now = w.now();
        let position = w.rows_mut::<Hero>().one().map(|mut row| {
            row.player
                .character
                .step(&mut row.transform, direction, jump, w.dt());
            row.transform.position
        });
        if input.pressed("light") {
            let selected: Vec<_> = w
                .rows::<Lightable>()
                .filter(|row| {
                    !row.beacon.lit
                        && position.is_some_and(|p| {
                            let d = p - row.transform.position;
                            Vec2::new(d.x, d.z).length_squared() <= 2.25
                        })
                })
                .map(|row| row.id)
                .collect();
            for id in selected {
                w.edit(id, |row| {
                    row.beacon.lit = true;
                    row.beacon.glow.to(now, 1.0, 0.5);
                });
                w.log("beacon lit");
            }
        }
        let mut count = 0;
        for mut row in w.rows_mut::<Glow>() {
            let glow = row.beacon.glow.value(now);
            row.material.emissive = [glow * 0.7, glow * 2.5, glow * 3.0];
            count += u32::from(row.beacon.lit);
        }
        w.publish("beacons", count);
        scene::follow(w);
    }
}
pub fn scene_types() -> exact_game_scene::Types {
    let mut types = exact_game_scene::Types::standard();
    types.component::<Beacon>();
    types
}

fn spawn_crates(w: &mut World, seed: u64, count: u32) {
    w.reseed(seed);
    for i in 1..=count {
        w.spawn_named(
            format!("crate-{i}"),
            (
                Transform::at(w.rand(-16.0..16.0), 0.5, w.rand(-16.0..16.0)),
                Mesh::cube(1.0),
                Material::rgb(0.44, 0.34, 0.25),
                exact_game_scene::GeneratedBy {
                    generator: "beacons::spawn_crates".into(),
                    parameters: [
                        ("seed".into(), seed.to_string()),
                        ("count".into(), count.to_string()),
                    ]
                    .into(),
                },
            ),
        );
    }
}
