use exact_game::*;
use exact_game::motion::{Move, Jump, Gravity};

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
}
#[derive(Default, Component)]
struct Player { velocity: Vec3 }
#[derive(Default, Component)]
struct Beacon { glow: Spring }
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"]).button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn((Transform::default(), Mesh::plane(40.0, 40.0), Material::grid([0.16, 0.23, 0.24], 1.0)));
        let player = w.spawn_named("player", (Transform::at(0.0, 0.9, 0.0), Mesh::capsule(0.4, 1.8), Material::rgb(0.8, 0.4, 0.1), Player::default()));
        w.spawn_named("camera", (Transform::default(), Camera::default(),
            Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15)));
        w.spawn((Transform::at(2.0, 0.5, 0.0), Mesh::sphere(0.5), Material::default(), Beacon::default()));
        w.publish("lit", 0);
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        if let Some((player, pose)) = w.query::<(&mut Player, &mut Transform)>().one() {
            Move { speed: 4.0, accel: 12.0, brake: 20.0 }
                .step(&mut player.velocity, input.stick_xz("move"), dt);
            if input.pressed("jump") && pose.position.y <= 0.9 {
                Jump { height: 1.2, gravity: 9.81 }.start(&mut player.velocity);
            }
            pose.position.x += player.velocity.x * dt;
            pose.position.z += player.velocity.z * dt;
            if pose.position.y > 0.9 || player.velocity.y > 0.0 {
                pose.position.y += player.velocity.y * dt - 0.5 * 9.81 * dt * dt;
                Gravity(9.81).step(&mut player.velocity, dt);
                if pose.position.y <= 0.9 {
                    pose.position.y = 0.9;
                    player.velocity.y = 0.0;
                }
            }
        }
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
