use exact_game::*;
use placement_fixture_logic::{Options, SmallGame};
#[test]
fn sign_name_button_and_lamp_save_together() {
    let mut sim = Sim::<SmallGame>::new(Options::default()).unwrap();
    sim.run(1000.);
    let w = sim.world();
    assert_eq!(w.query::<&Placed>().iter().count(), 3);
    assert_eq!(w.get::<Placed>("sign").unwrap().facing, Facing::Fixed);
    assert_eq!(w.get::<Placed>("name").unwrap().child, 2);
}
