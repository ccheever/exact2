use exact_world::*;

struct Keyboard;
impl Game for Keyboard {
    const ID: &'static str = "rollover";
    const ACTIONS: &'static [Action] = &[
        Action::button("first", &["key00"]),
        Action::button("overflow", &["key16"]),
    ];
    type Args = ();
    fn setup(_: &mut World, _: &()) -> Result<(), DataError> {
        Ok(())
    }
    fn tick(_: &mut World, _: &Input, _: &()) -> Result<(), DataError> {
        Ok(())
    }
}

#[test]
fn full_input_queue_drops_rollover_across_ticks_restore_and_paranoid_modes() {
    let mut expected = Vec::new();
    for (mode, restore) in [
        (Paranoid::Off, false),
        (Paranoid::Off, true),
        (Paranoid::Save, true),
        (Paranoid::FreshGame, true),
    ] {
        let mut sim = Sim::<Keyboard>::new(()).unwrap().paranoid(mode);
        for i in 0..1020 {
            sim.input(InputEvent::Key {
                code: format!("key{i:02}"),
                down: true,
                at_ms: if i < 17 { 1. } else { 20. },
            })
            .unwrap();
        }
        // Free a slot, release a dropped key, then admit its next down exactly once.
        for (i, down) in [(0, false), (16, false), (16, true), (16, true)] {
            sim.input(InputEvent::Key {
                code: format!("key{i:02}"),
                down,
                at_ms: 40.,
            })
            .unwrap();
        }
        assert!(sim.input(InputEvent::Blur { at_ms: 100. }).is_err());
        for (step, at) in [0., 17., 34., 51.].into_iter().enumerate() {
            sim.advance_to(at).unwrap();
            let input = sim.input_state();
            if step > 0 {
                for i in 1..16 {
                    assert!(input.key(&format!("key{i:02}")));
                }
                assert_eq!(input.key("key00"), step < 3);
                assert_eq!(input.held("overflow"), step == 3);
                assert_eq!(input.pressed("overflow"), step == 3);
                assert!(!input.released("overflow"));
                assert_eq!(input.pressed("first"), step == 1);
                assert_eq!(input.released("first"), step == 3);
                assert!(!input.key("key1019"));
            }
            let bytes = sim.save().unwrap();
            if !restore {
                expected.push(bytes);
            } else {
                assert_eq!(bytes, expected[step], "{mode:?}, step {step}");
                sim = Sim::from_save(&bytes).unwrap().paranoid(mode);
                assert_eq!(sim.save().unwrap(), bytes);
            }
        }
    }
}
