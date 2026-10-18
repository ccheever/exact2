//! Actions on world time: a button's edges.
use exact_game::*;

#[derive(Default, Resource)]
struct Fired {
    shots: u32,
    held_ticks: u32,
}
struct Fire;
impl Game for Fire {
    const ID: &'static str = "input-fire";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().button("fire", &["KeyF"])
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Fired::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut fired = w.resource_mut::<Fired>();
        // The recipe: a fire button is a `button` read with `pressed`.
        if input.pressed("fire") {
            fired.shots += 1;
        }
        if input.held("fire") {
            fired.held_ticks += 1;
        }
    }
}

/// The Rivals case: both edges of a tap land in one tick. `held` never sees it;
/// `pressed` does, with no trigger declared.
#[test]
fn a_same_tick_tap_is_pressed_but_never_held() {
    let mut sim = Sim::<Fire>::new(()).unwrap();
    sim.run(100.);
    sim.tap("KeyF");
    sim.run(100.);
    let fired = sim.world().resource::<Fired>();
    assert_eq!((fired.shots, fired.held_ticks), (1, 0));
}
