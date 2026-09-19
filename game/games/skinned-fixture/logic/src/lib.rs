use exact_game::*;
#[derive(Default, Args)]
pub struct Options {
    #[live]
    pub blend_bias: f32,
}
#[derive(Default, Data)]
struct Hud {
    motion: String,
    tick: u32,
}
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "skinned-fixture";
    const ASSETS: &'static [&'static str] = &["fox.model"];
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .button("mirror-charm", &["KeyM"])
            .button("skip-animation", &["KeyK"])
    }
    fn setup(w: &mut World, _: &Options) {
        w.spawn_named(
            "fox",
            (
                Transform::at(0., 0., -1.8),
                Mesh::asset("fox.model"),
                Animator::new([
                    State::clip("survey", "Survey").to("travel", Condition::gt("speed", 0.5)),
                    State::blend(
                        "travel",
                        Blend::across([(0., "Survey"), (1., "Walk"), (3., "Run")])
                            .parameter("speed"),
                    )
                    .fade(0.5),
                ])
                .motion_root("b_Root_00"),
            ),
        );
        w.spawn_named(
            "charm",
            (
                Transform::default(),
                Mesh::sphere(0.12),
                Material::rgb(1., 0.65, 0.1),
                SocketFollow::new("fox", "b_Head_05").offset(Transform::at(0., 0.2, 0.)),
            ),
        );
        w.spawn((
            Transform::default(),
            Mesh::plane(18., 18.),
            Material::grid([0.13, 0.21, 0.22], 1.),
        ));
        w.spawn_named(
            "camera",
            (
                Transform::at(6., 3.4, 7.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn((
            Transform::at(4., 7., 3.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.insert_resource(Environment {
            fog: None,
            ..Environment::default()
        });
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        if input.pressed("mirror-charm") {
            w.require_mut::<SocketFollow>("charm").offset.scale.x *= -1.;
        }
        if input.held("skip-animation") {
            return;
        }
        let end = w.tick_end();
        let speed = if w.tick() < 30 {
            0.
        } else {
            1.6 + args.blend_bias
        };
        w.require_mut::<Animator>("fox").set("speed", speed);
        let motion = animation::step(w);
        if motion.crossed("fox", "step") {
            w.log("fox footstep");
        }
        w.require_mut::<Transform>("fox").rotation = Quat::from_rotation_y(-end.seconds() * 0.45);
        motion.apply_local(w, "fox");
        w.publish_record(&Hud {
            motion: w.require::<Animator>("fox").state().into(),
            tick: end.tick as u32,
        });
    }
}
