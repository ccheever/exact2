use super::*;

fn pointer_event(id: u64, phase: PointerPhase, x: f32, buttons: u32) -> InputEvent {
    InputEvent::Pointer {
        id,
        phase,
        x,
        y: 10.,
        dx: 0.,
        dy: 0.,
        buttons,
        at_ms: 0.,
        player: 0,
    }
}
#[test]
fn primary_origin_survives_both_button_chords_and_secondary_release() {
    for right_first in [false, true] {
        let mut input = Input::new(Actions::new().button("fire", &["MouseLeft"]));
        input.apply(pointer_event(1, PointerPhase::Move, 90., 0), 0);
        assert!(input.pointer().unwrap().press_origin.is_none());
        if right_first {
            input.apply(pointer_event(1, PointerPhase::Down, 80., 2), 0);
        }
        input.apply(
            pointer_event(1, PointerPhase::Down, 20., if right_first { 3 } else { 1 }),
            0,
        );
        input.clear_edges();
        if !right_first {
            input.apply(pointer_event(1, PointerPhase::Down, 50., 3), 0);
        }
        assert_eq!(
            input.pointer().unwrap().press_origin,
            Some(Vec2::new(20., 10.))
        );
        assert_eq!(input.contacts[0].origin, Vec2::new(20., 10.));
        input.apply(pointer_event(1, PointerPhase::Up, 60., 1), 0);
        assert!(input.pointer().unwrap().down && input.held("fire"));
        assert!(!input.released("fire"));
        assert_eq!(input.contacts.len(), 1);
        assert_eq!(input.contacts[0].origin, Vec2::new(20., 10.));
        input.apply(pointer_event(1, PointerPhase::Up, 70., 0), 0);
        assert!(!input.pointer().unwrap().down && input.released("fire"));
        assert_eq!(
            input.pointer().unwrap().press_origin,
            Some(Vec2::new(20., 10.))
        );
        assert!(input.contacts.is_empty());
        input.apply(pointer_event(1, PointerPhase::Down, 30., 1), 0);
        assert_eq!(
            input.pointer().unwrap().press_origin,
            Some(Vec2::new(30., 10.))
        );
        input.clear_edges();
        input.apply(pointer_event(1, PointerPhase::Down, 50., 3), 0);
        input.apply(pointer_event(1, PointerPhase::Up, 60., 2), 0);
        assert!(input.pointer().unwrap().down && input.released("fire"));
        assert!(!input.held("fire"));
        assert_eq!(input.contacts.len(), 1);
        assert_eq!(
            input.pointer().unwrap().press_origin,
            Some(Vec2::new(30., 10.))
        );
        input.apply(pointer_event(1, PointerPhase::Up, 70., 0), 0);
        assert!(!input.pointer().unwrap().down && input.contacts.is_empty());
    }
}
#[test]
fn touch_origin_survives_up_but_cancel_blur_and_replacement_clear_it() {
    let mut input = Input::default();
    input.apply(pointer_event(7, PointerPhase::Down, 20., 0), 0);
    input.apply(pointer_event(7, PointerPhase::Move, 60., 0), 0);
    input.apply(pointer_event(7, PointerPhase::Up, 70., 0), 0);
    assert_eq!(
        input.pointer().unwrap().press_origin,
        Some(Vec2::new(20., 10.))
    );
    input.clear_edges();
    assert_eq!(
        input.pointer().unwrap().press_origin,
        Some(Vec2::new(20., 10.))
    );
    input.apply(pointer_event(9, PointerPhase::Down, 80., 2), 0);
    assert_eq!(input.pointer().unwrap().id, 9);
    assert!(input.pointer().unwrap().press_origin.is_none());
    input.apply(pointer_event(9, PointerPhase::Cancel, 80., 0), 0);
    input.apply(pointer_event(12, PointerPhase::Down, 30., 0), 0);
    input.apply(pointer_event(12, PointerPhase::Cancel, 80., 0), 0);
    assert!(input.pointer().unwrap().press_origin.is_none());
    input.apply(pointer_event(12, PointerPhase::Down, 40., 0), 0);
    input.apply(
        InputEvent::Blur {
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert!(input.pointer().is_none());
}
#[test]
fn malformed_saved_press_origins_are_refused_before_live_state_changes() {
    use crate::{Game, Sim, World};
    struct Empty;
    impl Game for Empty {
        const ID: &'static str = "origin-preflight";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut live = Sim::<Empty>::new(()).unwrap();
    let before = live.save().unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut source = Sim::<Empty>::new(()).unwrap();
        source
            .input
            .apply(pointer_event(1, PointerPhase::Down, 20., 1), 0);
        source.input.pointer.as_mut().unwrap().press_origin = Some(Vec2::new(value, 10.));
        let malformed = source.save().unwrap();
        assert!(live
            .restore(&malformed)
            .unwrap_err()
            .to_string()
            .contains("finite"));
        assert_eq!(live.save().unwrap(), before);
    }
}

#[test]
fn restore_rejects_unknown_saved_control_atomically() {
    use crate::{Clock, Game, Sim, World};
    struct Old;
    struct New;
    impl Game for Old {
        const ID: &'static str = "renamed";
        type Args = ();
        fn actions() -> Actions {
            Actions::new().button("old", &[])
        }
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    impl Game for New {
        const ID: &'static str = "renamed";
        type Args = ();
        fn actions() -> Actions {
            Actions::new().button("new", &[])
        }
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    for queued in [true, false] {
        let mut old = Sim::<Old>::new(()).unwrap();
        old.advance(0., Clock::Seekable);
        old.input(control("old", 7, PointerPhase::Down, 10., 20.));
        if !queued {
            old.advance(100., Clock::Seekable);
        }
        let mut new = Sim::<New>::new(()).unwrap();
        let before = new.save().unwrap();
        let error = new.restore(&old.save().unwrap()).unwrap_err().to_string();
        assert!(
            error.contains("old") && error.contains("control"),
            "{error}"
        );
        assert_eq!(new.save().unwrap(), before);
    }
}
#[test]
fn saved_keys_require_sorted_unique_order_before_restore_commits() {
    use crate::{Clock, Game, Sim, World};
    struct Keys;
    impl Game for Keys {
        const ID: &'static str = "saved-key-order";
        type Args = ();
        fn actions() -> Actions {
            Actions::new().button("z", &["KeyZ"])
        }
        fn setup(_: &mut World, _: &()) {}
        fn tick(w: &mut World, input: &Input, _: &()) {
            w.publish("held", input.held("z"));
        }
    }
    let mut source = Sim::<Keys>::new(()).unwrap();
    let mut live = Sim::<Keys>::new(()).unwrap();
    live.advance(0., Clock::Seekable);
    live.key_down("KeyB");
    live.advance(17., Clock::Seekable);
    live.key_down("KeyZ"); // Preserve a pending press across refusal too.
    let before = live.save().unwrap();
    let state = live.agent(r#"{"op":"state"}"#);
    let hash = live.world().hash();
    for keys in [
        vec!["KeyZ", "KeyA"],
        vec!["KeyA", "KeyA"],
        vec!["KeyZ", "KeyA", "KeyZ"],
    ] {
        source.input.keys = keys.iter().map(|s| (*s).into()).collect();
        let malformed = source.save().unwrap();
        for bound in [false, true] {
            let error = if bound {
                live.restore_bound(&malformed)
            } else {
                live.restore(&malformed)
            }
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("saved keys must be sorted and unique"),
                "{error}"
            );
            assert_eq!(live.save().unwrap(), before);
            assert_eq!(live.agent(r#"{"op":"state"}"#), state);
            assert_eq!(live.world().hash(), hash);
        }
    }
    for keys in [vec![], vec!["KeyZ"], vec!["KeyA", "KeyZ"]] {
        source.input.keys = keys.iter().map(|s| (*s).into()).collect();
        let saved = source.save().unwrap();
        let mut restored = Sim::<Keys>::new(()).unwrap();
        restored.restore(&saved).unwrap();
        assert_eq!(restored.save().unwrap(), saved);
        restored.advance(1000., Clock::Seekable);
        restored.key_up("KeyZ");
        restored.advance(1017., Clock::Seekable);
        assert!(!restored.input.keys.iter().any(|key| key == "KeyZ"));
        assert_eq!(
            restored.take_published().as_deref(),
            Some(r#"{"held":false}"#)
        );
    }
}

#[test]
fn saved_contacts_validate_origins_positions_and_keyboard_identity() {
    let mut original = input();
    original.apply(control("jump", 4294967294, PointerPhase::Down, 10., 20.), 0);
    let record = crate::json::to_string(&original.control_contacts()).unwrap();
    assert!(
        record.contains("\"code\":[\"Space\"]") && record.contains("\"id\":4294967294"),
        "{record}"
    );
    for origin in [true, false] {
        let mut bad = original.clone();
        if origin {
            bad.contacts[0].origin.x = f32::NAN;
        } else {
            bad.contacts[0].position.y = f32::INFINITY;
        }
        assert!(input().validate_saved(&bad).unwrap_err().contains("finite"));
    }
    for event in [
        control("jump", 1, PointerPhase::Move, f32::NAN, 0.),
        InputEvent::Pointer {
            id: 1,
            phase: PointerPhase::Down,
            x: f32::INFINITY,
            y: 0.,
            dx: 0.,
            dy: 0.,
            buttons: 1,
            at_ms: 0.,
            player: 0,
        },
        InputEvent::Pointer {
            id: 1,
            phase: PointerPhase::Move,
            x: 0.,
            y: 0.,
            dx: f32::NAN,
            dy: 0.,
            buttons: 0,
            at_ms: 0.,
            player: 0,
        },
    ] {
        assert!(input().validate(&event).unwrap_err().contains("finite"));
    }
}
#[test]
fn invalid_control_application_never_panics() {
    let mut input = input();
    input.apply(control("missing", 1, PointerPhase::Down, 0., 0.), 0);
    assert!(input.held_controls().is_empty());
}
#[test]
fn wheel_preserves_action_edges_and_expires_after_the_tick() {
    let mut input = input();
    input.apply(control("jump", 1, PointerPhase::Down, 0., 0.), 0);
    input.apply(control("jump", 1, PointerPhase::Up, 0., 0.), 0);
    input.apply(control("move", 2, PointerPhase::Down, 0., 0.), 0);
    input.apply(control("move", 2, PointerPhase::Move, 60., 0.), 0);
    for (dx, dy) in [(2.5, -3.), (-1.5, 7.), (f32::NAN, 1.)] {
        input.apply(
            InputEvent::Wheel {
                dx,
                dy,
                at_ms: 0.,
                player: 0,
            },
            0,
        );
    }
    assert_eq!(input.wheel(), Vec2::new(1., 4.));
    assert!(input.pressed("jump") && input.released("jump") && !input.held("jump"));
    assert!(input.pressed("move") && input.held("move") && !input.released("move"));
    assert_eq!(input.stick_xz("move"), crate::Vec3::X);
    input.clear_edges();
    assert_eq!(input.wheel(), Vec2::ZERO);
    assert!(!input.pressed("jump") && !input.released("jump"));
    assert!(input.held("move") && !input.pressed("move"));
    input.apply_paused(
        InputEvent::Wheel {
            dx: 2.,
            dy: 3.,
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert_eq!(input.wheel(), Vec2::ZERO);
    assert!(input.held("move"));
}
#[test]
fn hover_keeps_pointer_motion_and_key_edges_without_suppressing_drag_edges() {
    let mut input = Input::new(
        Actions::new()
            .stick("move", Stick::wasd().or_touch(Region::Left))
            .button("jump", &["Space"]),
    );
    input.viewport = Vec2::new(640., 480.);
    for (code, down) in [("Space", true), ("Space", false), ("KeyD", true)] {
        input.apply(
            InputEvent::Key {
                code: code.into(),
                down,
                at_ms: 0.,
                player: 0,
            },
            0,
        );
    }
    let last = std::cell::Cell::new(Vec2::new(20., 40.));
    let pointer = |phase, x, y| {
        let to = Vec2::new(x, y);
        let by = to - last.get();
        if to.is_finite() {
            last.set(to);
        }
        InputEvent::Pointer {
            id: 1,
            phase,
            x,
            y,
            dx: by.x,
            dy: by.y,
            buttons: u32::from(phase == PointerPhase::Down),
            at_ms: 0.,
            player: 0,
        }
    };
    for (x, y) in [(20., 40.), (50., 70.), (f32::NAN, 70.), (80., 90.)] {
        input.apply(pointer(PointerPhase::Move, x, y), 0);
    }
    let p = input.pointer().unwrap();
    assert_eq!(p.position, Vec2::new(80., 90.));
    assert_eq!(p.delta, Vec2::new(60., 50.));
    assert!(!p.down);
    assert!(input.pressed("jump") && input.released("jump") && !input.held("jump"));
    assert!(input.pressed("move") && input.held("move") && !input.released("move"));
    input.clear_edges();
    assert_eq!(input.pointer().unwrap().delta, Vec2::ZERO);
    assert!(!input.pressed("jump") && !input.released("jump"));
    input.apply(pointer(PointerPhase::Down, 80., 90.), 0);
    input.apply(pointer(PointerPhase::Move, 20., 90.), 0);
    assert!(input.pointer().unwrap().down);
    assert!(input.released("move") && !input.held("move"));
    assert_eq!(input.stick_xz("move"), crate::Vec3::ZERO);
    input.apply(pointer(PointerPhase::Move, 80., 90.), 0);
    assert!(input.pressed("move") && input.released("move") && input.held("move"));
    assert_eq!(input.stick_xz("move"), crate::Vec3::X);
    input.apply(pointer(PointerPhase::Up, 80., 90.), 0);
    input.apply(pointer(PointerPhase::Move, 90., 100.), 0);
    assert!(!input.pointer().unwrap().down);
    assert_eq!(input.pointer().unwrap().delta, Vec2::new(10., 10.));
    assert!(input.pressed("move") && input.released("move") && input.held("move"));
}
#[test]
fn mouse_buttons_bind_as_keys_and_motion_is_the_devices() {
    let mut input = Input::new(
        Actions::new()
            .button("fire", &["MouseLeft"])
            .button("aim", &["MouseRight"])
            .button("ping", &["MouseMiddle"]),
    );
    let mouse = |phase, dx, buttons| InputEvent::Pointer {
        id: 1,
        phase,
        x: 10.,
        y: 10.,
        dx,
        dy: -1.,
        buttons,
        at_ms: 0.,
        player: 0,
    };
    // A locked pointer: the position stays, the motion is unbounded.
    for _ in 0..3 {
        input.apply(mouse(PointerPhase::Move, 500., 0), 0);
    }
    let p = input.pointer().unwrap();
    assert_eq!(
        (p.position, p.delta),
        (Vec2::new(10., 10.), Vec2::new(1500., -3.))
    );
    input.clear_edges();
    // The secondary button alone begins the contact, as the web's pointerdown.
    input.apply(mouse(PointerPhase::Down, 0., 2), 0);
    assert!(input.pressed("aim") && input.held("aim") && !input.held("fire"));
    input.clear_edges();
    // Chorded buttons change on moves.
    input.apply(mouse(PointerPhase::Move, 2., 3), 0);
    input.apply(mouse(PointerPhase::Move, 2., 7), 0);
    assert!(input.pressed("fire") && input.pressed("ping") && !input.pressed("aim"));
    assert!(input.held("aim") && input.held("fire") && input.held("ping"));
    input.clear_edges();
    input.apply(mouse(PointerPhase::Move, 2., 6), 0);
    assert!(input.released("fire") && input.held("aim"));
    input.apply(mouse(PointerPhase::Up, 0., 0), 0);
    assert!(input.released("aim") && input.released("ping"));
    assert!(!input.held("aim") && !input.held("fire") && !input.held("ping"));
    input.clear_edges();
    // A hover move with a button held (pressed off the canvas) holds it;
    // blur releases every device.
    input.apply(mouse(PointerPhase::Move, 1., 2), 0);
    assert!(input.pressed("aim") && input.held("aim"));
    input.apply(
        InputEvent::Blur {
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert!(!input.held("aim"));
}
#[test]
fn repeated_keys_preserve_edges_and_independent_control_ownership() {
    let mut input = Input::new(Actions::new().button("jump", &["Space", "KeyJ"]));
    let key = |code: &str, down, at_ms| InputEvent::Key {
        code: code.into(),
        down,
        at_ms,
        player: 0,
    };
    input.apply(key("Space", true, 0.), 0);
    assert!(input.held("jump") && input.pressed("jump") && !input.released("jump"));
    let saved = crate::bin::to_vec(&input);
    for at_ms in [1., f64::NAN] {
        input.apply(key("Space", true, at_ms), 0);
        assert_eq!(crate::bin::to_vec(&input), saved);
    }
    input.clear_edges();
    input.apply(key("Space", true, 2.), 0);
    assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
    input.apply(key("KeyJ", true, 3.), 0);
    input.apply(key("Space", false, 4.), 0);
    assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
    let saved = crate::bin::to_vec(&input);
    for code in ["Space", "KeyZ"] {
        input.apply(key(code, false, 5.), 0);
        assert_eq!(crate::bin::to_vec(&input), saved);
    }
    input.apply(control("jump", 1, PointerPhase::Down, 0., 0.), 0);
    input.apply(key("KeyJ", false, 6.), 0);
    input.apply(key("KeyJ", false, 7.), 0);
    assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
    input.apply(control("jump", 1, PointerPhase::Up, 0., 0.), 0);
    assert!(!input.held("jump") && !input.pressed("jump") && input.released("jump"));
    let saved = crate::bin::to_vec(&input);
    input.apply(key("KeyJ", false, 8.), 0);
    assert_eq!(crate::bin::to_vec(&input), saved);
    input.clear_edges();
    input.apply(key("Space", true, 9.), 0);
    assert!(input.held("jump") && input.pressed("jump") && !input.released("jump"));
}
fn control(name: &str, id: u64, phase: PointerPhase, x: f32, y: f32) -> InputEvent {
    InputEvent::Control {
        name: name.into(),
        id,
        phase,
        x,
        y,
        at_ms: 0.0,
        player: 0,
    }
}
fn input() -> Input {
    Input::new(
        Actions::new()
            .stick("move", Stick::wasd())
            .button("jump", &["Space"])
            .button("light", &["KeyE"]),
    )
}
#[test]
fn cloned_inputs_keep_keys_contacts_and_edges_independent() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Input>();
    let mut original = input();
    original.apply(
        InputEvent::Key {
            code: "Space".into(),
            down: true,
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    original.apply(control("move", 1, PointerPhase::Down, 10., 10.), 0);
    original.apply(control("move", 1, PointerPhase::Move, 70., 10.), 0);
    let saved = crate::bin::to_vec(&original);
    let mut copy = original.clone();
    copy.apply(
        InputEvent::Blur {
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert!(!copy.held("jump"));
    assert!(copy.released("jump"));
    assert_eq!(copy.stick_xz("move"), crate::Vec3::ZERO);
    assert!(original.held("jump") && original.pressed("jump"));
    assert_eq!(original.stick_xz("move"), crate::Vec3::X);
    assert_eq!(crate::bin::to_vec(&original), saved);
    original.clear_edges();
    assert!(!original.pressed("jump"));
    assert!(copy.released("jump"));
    copy.restore_dynamic(crate::bin::from_slice(&saved).unwrap());
    assert!(copy.held("jump") && !copy.pressed("jump"));
    assert_eq!(copy.stick_xz("move"), crate::Vec3::X);
}
#[test]
fn app_controls_share_actions_with_keys_and_release_independently() {
    let mut input = input();
    input.apply(control("jump", 1, PointerPhase::Down, 0., 0.), 0);
    assert!(input.pressed("jump") && input.held("jump"));
    input.clear_edges();
    input.apply(
        InputEvent::Key {
            code: "Space".into(),
            down: true,
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert!(!input.pressed("jump"));
    input.apply(control("jump", 1, PointerPhase::Cancel, 0., 0.), 0);
    assert!(input.held("jump") && !input.released("jump"));
    input.apply(
        InputEvent::Key {
            code: "Space".into(),
            down: false,
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert!(input.released("jump") && !input.held("jump"));
    input.apply(control("light", 2, PointerPhase::Down, 0., 0.), 0);
    input.apply(control("light", 2, PointerPhase::Up, 0., 0.), 0);
    assert!(input.pressed("light") && input.released("light"));
    input.apply(control("light", 3, PointerPhase::Down, 0., 0.), 0);
    input.apply(control("jump", 4, PointerPhase::Down, 0., 0.), 0);
    input.apply(control("jump", 5, PointerPhase::Down, 0., 0.), 0);
    assert_eq!(input.held_controls(), ["jump", "light"]);
    input.apply(control("jump", 4, PointerPhase::Up, 0., 0.), 0);
    assert_eq!(input.held_controls(), ["jump", "light"]);
    input.apply(control("jump", 5, PointerPhase::Cancel, 0., 0.), 0);
    assert_eq!(input.held_controls(), ["light"]);
    input.apply(control("light", 3, PointerPhase::Up, 0., 0.), 0);
    assert!(input.held_controls().is_empty());
}
#[test]
fn app_stick_is_local_unit_clamped_saved_and_cancelled_on_blur() {
    let mut input = input();
    input.apply(control("move", 1, PointerPhase::Down, 40., 40.), 0);
    input.apply(control("move", 1, PointerPhase::Move, 100., 40.), 0);
    input.apply(control("jump", 2, PointerPhase::Down, 0., 0.), 0);
    assert_eq!(input.stick_xz("move"), crate::Vec3::X);
    assert!(input.held("jump"));
    let saved = crate::bin::to_vec(&input);
    let mut restored = self::input();
    restored.restore_dynamic(crate::bin::from_slice(&saved).unwrap());
    assert_eq!(restored.stick_xz("move"), crate::Vec3::X);
    assert!(restored.held("jump") && !restored.pressed("jump"));
    restored.apply(control("move", 1, PointerPhase::Move, 100., -20.), 0);
    let direction = restored.stick_xz("move");
    assert!((direction.length() - 1.).abs() < 1e-6 && direction.z < 0.);
    restored.apply(
        InputEvent::Blur {
            at_ms: 0.,
            player: 0,
        },
        0,
    );
    assert_eq!(restored.stick_xz("move"), crate::Vec3::ZERO);
    assert!(!restored.held("jump"));
}

#[test]
fn queued_ui_contact_continues_identically_in_a_fresh_simulation() {
    use crate::{Clock, Game, Sim, Transform, World};
    struct Controlled;
    impl Game for Controlled {
        const ID: &'static str = "ui-control-test";
        const HZ: u32 = 60;
        type Args = ();
        fn actions() -> Actions {
            input().actions().clone()
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("player", (Transform::default(),));
        }
        fn tick(w: &mut World, input: &Input, _: &()) {
            let mut t = w.get_mut::<Transform>("player").unwrap();
            t.position += input.stick_xz("move");
            if input.pressed("jump") {
                t.position.y += 1.;
            }
        }
    }
    for mode in [
        crate::Paranoid::Off,
        crate::Paranoid::Save,
        crate::Paranoid::FreshGame,
    ] {
        let mut sim = Sim::<Controlled>::new(()).unwrap().paranoid(mode);
        sim.advance(1000., Clock::Seekable);
        let mut down = control("move", 1, PointerPhase::Down, 0., 0.);
        down.set_at_ms(800.); // Past stamps, like keys, become due now.
        sim.input(down);
        let mut motion = control("move", 1, PointerPhase::Move, 60., 0.);
        motion.set_at_ms(900.);
        sim.input(motion);
        sim.advance(1100., Clock::Seekable);
        assert_eq!(sim.get::<Transform>("player").unwrap().position.x, 6.);
        assert_eq!(sim.held_controls(), ["move"]);
        let saved = sim.save().unwrap(); // Contact is held inside Input, not just queued.
        let mut restored = Sim::<Controlled>::new(()).unwrap().paranoid(mode);
        restored.restore(&saved).unwrap();
        restored.advance(1100., Clock::Seekable);
        for game in [&mut sim, &mut restored] {
            let mut jump = control("jump", 2, PointerPhase::Down, 0., 0.);
            jump.set_at_ms(1200.);
            game.input(jump);
            game.advance(1200., Clock::Seekable);
            assert_eq!(game.get::<Transform>("player").unwrap().position.y, 0.);
            game.advance(1217., Clock::Seekable);
            assert_eq!(game.get::<Transform>("player").unwrap().position.y, 1.);
            assert!(game
                .agent(r#"{"op":"state"}"#)
                .contains(r#""controls":["jump","move"]"#));
            let bad = control("typo", 3, PointerPhase::Down, 0., 0.);
            assert!(game.validate_input(&bad).unwrap_err().contains("typo"));
            game.input(bad); // Never panics even for direct callers.
            let mut invalid = control("jump", 4, PointerPhase::Down, 0., 0.);
            invalid.set_at_ms(f64::NAN);
            assert!(game.validate_input(&invalid).is_err());
            game.input(invalid);
            game.input(InputEvent::Blur {
                at_ms: 1217.,
                player: 0,
            });
            game.advance(1300., Clock::Seekable);
            assert!(game.held_controls().is_empty());
        }
        assert_eq!(sim.save().unwrap(), restored.save().unwrap());
    }
}
