//! Renderer-owned scene: moving capsule, emissive sphere, ground and camera.
use exact_game::*;
#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
pub struct Fixture;
impl Game for Fixture {
    const ID: &'static str = "renderer-fixture";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .button("act", &["KeyE"])
            .stick("move", Stick::wasd())
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(40., 40.),
                Material::grid([0.25, 0.27, 0.3], 1.),
            ),
        );
        w.spawn_named(
            "player",
            (
                Transform::at(0., 0.9, 0.),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.45, 0.15),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 5.9, 8.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn((
            Transform::at(5., 10., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.spawn_named(
            "beacon-1",
            (
                Transform::at(1.5, 0.75, -6.5),
                Mesh::sphere(0.5),
                Material {
                    emissive: [3.; 3],
                    ..Material::rgb(0.03, 0.12, 0.16)
                },
                Glow::default(),
            ),
        );
        w.publish("beacons", 0u32);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let delta = input.stick_xz("move") * (4. / 60.);
        if delta != Vec3::ZERO {
            w.require_mut::<Transform>("player").position += delta;
        }
        let now = w.tick_end();
        if input.pressed("act") {
            w.require_mut::<Glow>("beacon-1").0.to(now, 1., 0.5);
            w.publish("beacons", 1u32);
        }
    }
}
