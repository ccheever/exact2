use exact_game::*;
#[derive(Default, Args)]
struct Original {
    left: u32,
    right: u32,
    removed: String,
}
#[derive(Default, Args)]
struct Reordered {
    right: u32,
    left: u32,
    added: u32,
}
struct Old;
struct New;
impl Game for Old {
    type Args = Original;
    const ID: &'static str = "named-args";
    fn setup(_: &mut World, _: &Original) {}
    fn tick(_: &mut World, _: &Input, _: &Original) {}
}
impl Game for New {
    type Args = Reordered;
    const ID: &'static str = "named-args";
    fn setup(_: &mut World, _: &Reordered) {}
    fn tick(_: &mut World, _: &Input, _: &Reordered) {}
}
#[test]
fn saved_arguments_follow_names_default_new_fields_and_ignore_removed_fields() {
    let s = Sim::<Old>::new(Original {
        left: 17,
        right: 42,
        removed: "former".into(),
    })
    .unwrap();
    let mut next = Sim::<New>::new(Reordered::default()).unwrap();
    next.restore(&s.save().unwrap()).unwrap();
    assert_eq!(
        (next.args().left, next.args().right, next.args().added),
        (17, 42, 0)
    );
    let mut bound = Sim::<New>::new(Reordered {
        left: 2,
        right: 3,
        added: 4,
    })
    .unwrap();
    bound.restore_bound(&s.save().unwrap()).unwrap();
    assert_eq!(
        (bound.args().left, bound.args().right, bound.args().added),
        (2, 3, 4)
    );
    let state = bound.agent(r#"{"op":"state"}"#);
    assert!(
        state.contains(r#""restoredFrom":{"left":17,"right":42,"removed":"former"}"#),
        "{state}"
    );
    bound.run(17.0);
    assert!(!bound.agent(r#"{"op":"state"}"#).contains("restoredFrom"));
}
#[derive(Default, Args)]
struct Floats {
    setup: f32,
    precise: f64,
    #[live]
    volume: f32,
}
struct Validated;
impl Game for Validated {
    type Args = Floats;
    const ID: &'static str = "validated";
    fn validate(args: &Floats) -> Result<(), String> {
        if args.volume > 1.0 {
            Err("volume exceeds one".into())
        } else {
            Ok(())
        }
    }
    fn setup(w: &mut World, args: &Floats) {
        w.spawn_named("setup", Transform::at(args.setup, 0.0, 0.0));
    }
    fn tick(_: &mut World, _: &Input, _: &Floats) {}
}
#[test]
fn decoded_float_state_is_exact_signed_zero_rebuilds_and_validation_is_atomic() {
    let mut s = Sim::<Validated>::new(Floats::default()).unwrap();
    let before = s.save().unwrap();
    let bad = Floats {
        volume: 2.0,
        ..Default::default()
    };
    assert!(Sim::<Validated>::new(bad).is_err());
    assert!(s
        .bind(
            &Floats {
                volume: 2.0,
                ..Default::default()
            }
            .values(),
            Some(2000.0)
        )
        .is_err());
    assert_eq!(before, s.save().unwrap());
    s.run(100.0);
    s.bind(
        &Floats {
            setup: -0.0,
            precise: 1.234567890123,
            volume: 0.12345678,
        }
        .values(),
        None,
    )
    .unwrap();
    assert_eq!(s.world().tick(), 0);
    assert_eq!(
        s.world()
            .get::<Transform>("setup")
            .unwrap()
            .position
            .x
            .to_bits(),
        (-0.0f32).to_bits()
    );
    let state = s.agent(r#"{"op":"state"}"#);
    let canonical = json::to_string(s.args()).unwrap();
    assert!(state.contains(&format!("\"args\":{canonical}")), "{state}");
    assert!(state.contains("1.234567890123"));
    let bytes = s.save().unwrap();
    let old = json::to_string(s.args()).unwrap();
    let mut replaced = bytes.clone();
    let start = replaced
        .windows(old.len())
        .position(|w| w == old.as_bytes())
        .unwrap();
    // Same-length invalid volume keeps the complete envelope intact.
    let invalid = old.replace("\"volume\":0.", "\"volume\":2.");
    replaced.splice(start..start + old.len(), invalid.bytes());
    assert!(s
        .restore(&replaced)
        .unwrap_err()
        .to_string()
        .contains("volume"));
    assert_eq!(s.save().unwrap(), bytes);
}
#[test]
fn setup_comparison_is_bitwise_for_both_float_widths() {
    let a = Floats::default();
    assert!(a.setup_changed(&Floats {
        setup: -0.0,
        ..Default::default()
    }));
    assert!(a.setup_changed(&Floats {
        precise: -0.0,
        ..Default::default()
    }));
}
#[derive(Default, Component)]
struct Projectile;
struct Late;
impl Game for Late {
    type Args = ();
    const ID: &'static str = "late-component";
    fn setup(_: &mut World, _: &()) {}
    fn tick(w: &mut World, _: &Input, _: &()) {
        if w.is_empty() {
            w.spawn(Projectile);
        }
    }
}
#[test]
fn unregistered_saved_type_names_the_setup_fix() {
    let mut s = Sim::<Late>::new(()).unwrap();
    s.run(17.0);
    let mut fresh = Sim::<Late>::new(()).unwrap();
    let error = fresh.restore(&s.save().unwrap()).unwrap_err().to_string();
    assert!(
        error.contains("Projectile") && error.contains("world.register::<Projectile>() in setup"),
        "{error}"
    );
    struct Registered;
    impl Game for Registered {
        type Args = ();
        const ID: &'static str = Late::ID;
        fn setup(w: &mut World, _: &()) {
            w.register::<Projectile>();
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut registered = Sim::<Registered>::new(()).unwrap();
    registered.restore(&s.save().unwrap()).unwrap();
    assert_eq!(registered.world().query::<&Projectile>().iter().count(), 1);
}
