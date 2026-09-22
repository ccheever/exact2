use exact_game::*;

pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "particles-fixture";
    const HZ: u32 = 60;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.015, 0.025, 0.05]),
            fog: None,
            bloom: None,
            ..Environment::default()
        });
        for i in 0..20 {
            w.spawn_named(
                format!("sparks-{i}"),
                (
                    Transform::at((i % 5) as f32 * 4. - 8., (i / 5) as f32 * 4. - 6., 0.),
                    Emitter::sparks().rate(200.).lifetime(5.).seed(i).burst(0),
                ),
            );
        }
        w.spawn_named(
            "camera",
            (Transform::at(0., 0., 30.), Camera::orthographic(24.)),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        emitter::step(w);
    }
}
