use exact_game::character::Character;
use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
}
#[derive(Default, Data)]
pub struct Movement {
    pub speed: asset::Curve,
    pub jump: f32,
}
/// A held sprint runs at this many times the movement table's speed.
const SPRINT: f32 = 1.75;
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    const HZ: u32 = 60;
    const TABLES: &'static [asset::Table] = &[asset::Table::of::<Movement>("movement.json")];
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick(
                "move",
                Stick::wasd().or_arrows().or_pad(Pad::Left).or_tilt(Tilt {
                    max_degrees: 20.0,
                    dead_zone: 2.0,
                }),
            )
            .button("jump", &["Space", "GamepadSouth"])
            // Held 200 ms, Shift runs; a tap of it does nothing.
            .hold("sprint", &["ShiftLeft", "GamepadL1"], 0.2)
            // E lights a beacon only within reach of one.
            .context("beacon", |a| a.button("light", &["KeyE", "GamepadWest"]))
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(Environment {
            background: Some(Color([0.49, 0.67, 0.64])),
            ..Environment::default()
        });
        w.spawn((
            Transform::default(),
            Mesh::plane(40.0, 40.0),
            Material::rgb(0.16, 0.23, 0.24),
        ));
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.4, 0.1),
                Character::new().ground(0.9).bounds_xz(-19.6..=19.6),
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
                    Material::rgb(0.55, 0.38, 0.22),
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
                    Material {
                        color: [0.12, 0.55, 0.6, 1.],
                        ..Material::glow([3.; 3])
                    },
                    Glow::default(),
                    Beacon::default(),
                ),
            );
            w.spawn_named(
                format!("ring-{}", i + 1),
                (
                    Transform::at(x, 0.205, z),
                    Mesh::ring(0.72, 0.9),
                    Material::glow([0.12, 0.55, 0.6]),
                ),
            );
            w.spawn_named(
                format!("halo-{}", i + 1),
                (
                    Transform::at(x, 1.0, z),
                    Mesh::sphere(0.7),
                    Material::glow([0.12, 0.55, 0.6]).alpha(0.12),
                ),
            );
        }
        w.publish("lit", 0);
        w.publish("near", "");
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        // Set level: the app posts the gravity it read (`tilt x y z`), the
        // level of tilt to move from the next tick.
        for m in input.messages() {
            let level: Vec<f32> = (m.strip_prefix("tilt ").into_iter())
                .flat_map(str::split_whitespace)
                .filter_map(|n| n.parse().ok())
                .collect();
            if let [x, y, z] = level[..] {
                w.set_tilt_neutral("move", Vec3::new(x, y, z));
            }
        }
        // Set before the step, every tick: the table tunes walking, a held
        // sprint scales it, and a sprint released while paused (no edge) walks
        // again from the first tick on resume.
        {
            let tuning = w.table::<Movement>("movement.json").unwrap();
            let walk = tuning.speed.at(input.stick_xz("move").length());
            let mut player = w.require_mut::<Character>("player");
            player.speed = if input.held("sprint") {
                walk * SPRINT
            } else {
                walk
            };
            player.jump = tuning.jump;
        }
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let mut near = "";
        let mut reach = false;
        let mut lit = false;
        if let Some((_, mut beacon)) = w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit) {
            reach = true;
            if input.pressed("light") {
                beacon.lit = true;
                beacon.glow_to(1.0, 0.5);
                lit = true;
            } else {
                near = beacon.name().unwrap_or_default();
            }
        }
        let count = w.count::<Beacon>(|b| b.lit);
        // The page answers a beacon lit in the hand, and the last one with
        // the light bar (LLP 1133.022 §10): the presentation path, outside the hash.
        if lit {
            w.emit(if count == 3 { "all lit" } else { "lit" });
        }
        w.publish("lit", count);
        w.publish("near", near);
        w.input_context("beacon", reach);
    }
}
