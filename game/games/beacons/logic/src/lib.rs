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
pub struct Beacons;
impl Game for Beacons {
    const ID: &'static str = "beacons";
    const HZ: u32 = 60;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(Environment {
            background: Some([0.49, 0.67, 0.64]),
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
        }
        w.publish("lit", 0);
        w.publish("near", "");
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let mut near = "";
        if let Some((entity, mut beacon)) = w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit) {
            if input.pressed("light") {
                beacon.lit = true;
                w.require_mut::<Glow>(entity).0.to(w.tick_end(), 1.0, 0.5);
            } else {
                near = w.name(entity).unwrap_or_default();
            }
        }
        w.publish("lit", w.count::<Beacon>(|b| b.lit));
        w.publish("near", near);
    }
}
