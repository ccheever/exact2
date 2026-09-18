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
    fn setup(w: &mut World, _: &Options) {
        w.spawn_named(
            "fox",
            (
                Transform::at(1.8, 0., 0.).with_scale(0.025),
                Mesh::asset("fox.model"),
                Animator::new([
                    State::new("survey", Play::Clip("Survey".into())).to(
                        "travel",
                        Condition::Arg("speed".into(), Cmp::Gt, 0.5.into()),
                    ),
                    State::new(
                        "travel",
                        Play::Blend(Blend::across([(0., "Survey"), (1., "Walk"), (3., "Run")])),
                    )
                    .fade(0.5),
                ]),
                Socket("b_Head_05".into()),
            ),
        );
        w.spawn_named(
            "charm",
            (
                Transform::default(),
                Mesh::sphere(0.12),
                Material::rgb(1., 0.65, 0.1),
                SocketFollow {
                    offset: Transform::at(0., 8., 0.).with_scale(40.),
                    ..SocketFollow::new("fox")
                },
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
    fn tick(w: &mut World, _: &Input, args: &Options) {
        let time = (w.tick() + 1) as f32 * w.dt();
        let speed = if w.tick() < 30 {
            0.
        } else {
            1.6 + args.blend_bias
        };
        {
            let mut animator = w.get_mut::<Animator>("fox").unwrap();
            animator.set("speed", speed);
            if let Play::Blend(blend) = &mut animator.states[1].play {
                blend.axis = speed;
            }
        }
        // This script is the fox Transform's sole writer; animation owns only its pose.
        let angle = time * 0.45;
        let mut fox = w.get_mut::<Transform>("fox").unwrap();
        fox.position = Vec3::new(1.8 * math::cos(angle), 0., 1.8 * math::sin(angle));
        fox.rotation = Quat::from_rotation_y(-angle);
        drop(fox);
        animation::step(w);
        w.publish_record(&Hud {
            motion: w.get::<Animator>("fox").unwrap().state().into(),
            tick: (w.tick() + 1) as u32,
        });
    }
}
