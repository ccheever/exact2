use exact_game::asset::AlphaMode;
use exact_game::*;

pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "sprites-fixture";
    const HZ: u32 = 60;
    const ASSETS: &'static [&'static str] = &["strip.tex"];
    type Args = ();
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows())
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.025, 0.04, 0.08]),
            fog: None,
            bloom: None,
            ..Environment::default()
        });
        for (i, depth) in [-240., -160., -80.].into_iter().enumerate() {
            w.spawn_named(
                format!("parallax-{i}"),
                (
                    Transform::at(0., 45., depth),
                    Sprite {
                        size: Vec2::new(640., 140.),
                        frame: [32, 0, 16, 16],
                        layer: i as i32,
                        color: [
                            [0.18, 0.23, 0.38, 1.],
                            [0.18, 0.34, 0.40, 0.8],
                            [0.25, 0.45, 0.37, 0.7],
                        ][i],
                        ..Sprite::new("strip.tex", [640., 140.])
                    },
                ),
            );
        }
        for i in -20..40 {
            w.spawn((
                Transform::at(i as f32 * 16., -4., 0.),
                Sprite {
                    frame: [48, 0, 16, 16],
                    alpha: AlphaMode::Opaque,
                    ..Sprite::new("strip.tex", [16., 16.])
                },
            ));
        }
        w.spawn_named(
            "player",
            (
                Transform::at(0., 20., 0.),
                Sprite {
                    frame: [0, 0, 16, 16],
                    ..Sprite::new("strip.tex", [24., 32.])
                },
                SpriteAnimation::new([[0, 0, 16, 16], [16, 0, 16, 16]], 6.),
            ),
        );
        w.spawn_named(
            "leaves",
            (
                Transform::at(100., 80., 0.),
                Emitter {
                    shape: emitter::Shape::Sphere(60.),
                    speed: -12.,
                    spread: 0.,
                    gravity: Vec3::ZERO,
                    size: [5., 5.],
                    color: [[1., 0.3, 0.025, 0.8]; 2],
                    additive: false,
                    bound: [-65., -245., -65., 65., 65., 65.],
                    ..Emitter::sparks().rate(0.).lifetime(20.).seed(7).burst(200)
                },
                Ambient,
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::orthographic(180.).integer_scale(),
                Follow::new("player").offset(0., 0., 500.),
            ),
        );
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let direction = input.stick_xz("move").x;
        w.get_mut::<Transform>("player").unwrap().position.x += direction * 40. * w.dt();
        w.get_mut::<Sprite>("player").unwrap().flip[0] = direction < 0.;
        for i in 0..3 {
            let x = w.global_position("player").unwrap().x * (0.15 + i as f32 * 0.2);
            w.get_mut::<Transform>(format!("parallax-{i}").as_str())
                .unwrap()
                .position
                .x = x;
        }
        sprite::step(w);
        emitter::step(w);
        scene::follow(w);
    }
}
