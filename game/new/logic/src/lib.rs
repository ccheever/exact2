use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
}
#[derive(Default, Component)]
struct Beacon { glow: Spring }
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn((Transform::default(), Mesh::plane(40.0, 40.0), Material::default()));
        let player = w.spawn_named("player", (Transform::at(0.0, 0.9, 0.0), Mesh::capsule(0.4, 1.8), Material::rgb(0.8, 0.4, 0.1)));
        w.spawn_named("camera", (Transform::default(), Camera::default(),
            Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15)));
        w.spawn((Transform::at(2.0, 0.5, 0.0), Mesh::sphere(0.5), Material::default(), Beacon::default()));
        w.publish("lit", 0);
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.get_mut::<Transform>("player").unwrap().position +=
            input.stick_xz("move") * 4.0 * w.dt();
        let mut count = 0;
        for (mut beacon, mut material) in w.query::<(&mut Beacon, &mut Material)>() {
            if input.pressed("light") { beacon.glow.set_target(w.now(), 1.0); }
            material.emissive = [beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(beacon.glow.target == 1.0);
        }
        w.publish("lit", count);
        scene::follow(w);
    }
}
