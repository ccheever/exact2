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
    #[restart]
    pub restart: bool,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
    pub glow: Tween,
}
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    const HZ: u32 = 60; // Preserve the r4 movement timings.
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
                // These are Character's defaults, kept visible for auditing the game.
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
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let nearest = w.nearest_xz_where::<Beacon>("player", 1.5, |b| !b.lit);
        if input.pressed("light") {
            if let Some(e) = nearest {
                let mut beacon = w.get_mut::<Beacon>(e).unwrap();
                beacon.lit = true;
                beacon.glow.to(w.now(), 1.0, 0.5);
            }
        }
        let near = nearest
            .filter(|&e| !w.get::<Beacon>(e).unwrap().lit)
            .and_then(|e| w.name(e))
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
