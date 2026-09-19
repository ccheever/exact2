use exact_game::*;
pub struct Probe;
impl Game for Probe {
    const ID: &'static str = "engine-probe";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd())
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Transform::default(), Mesh::cube(1.)));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        w.require_mut::<Transform>("player").position += input.stick_xz("move") / 60.;
    }
}
