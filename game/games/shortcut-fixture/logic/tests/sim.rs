use exact_game::*;
use shortcut_fixture_logic::{Shop, ShortcutFixture};

#[test]
fn the_buy_control_and_the_digit1_key_are_separate_actions() {
    let mut sim = Sim::<ShortcutFixture>::new(()).unwrap();
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        sim.input(InputEvent::Control {
            name: "buy".into(),
            id: 4_294_967_292,
            phase,
            x: 0.,
            y: 0.,
            at_ms: 0.,
            player: 0,
        });
    }
    sim.run(100.);
    assert_eq!((sim.world().resource::<Shop>().buys, sim.world().resource::<Shop>().ones), (1, 0));
    for down in [true, false] {
        sim.input(InputEvent::Key {
            code: "Digit1".into(),
            down,
            at_ms: 100.,
            player: 0,
        });
    }
    sim.run(100.);
    assert_eq!((sim.world().resource::<Shop>().buys, sim.world().resource::<Shop>().ones), (1, 1));
}
