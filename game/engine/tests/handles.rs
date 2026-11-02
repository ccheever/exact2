use exact_game::*;

#[derive(Default, Component)]
struct Beacon;

fn world() -> World {
    let mut w = World::new(120, 7);
    w.spawn_named("player", Transform::default());
    w.spawn_named(
        "beacon",
        (
            Beacon,
            Transform::at(1., 0., 0.),
            Material::default(),
            Glow::default(),
        ),
    );
    w
}

#[test]
fn handle_verbs_match_explicit_writes_and_keep_row_leases() {
    let mut a = world();
    let b = world();
    {
        let (_, mut beacon) = a.nearest_xz_mut::<Beacon>("player", 2., |_| true).unwrap();
        assert_eq!(beacon.name(), Some("beacon"));
        beacon.glow_to(2., 0.5);
        beacon.tint([0.2, 0.3, 0.4]);
        beacon.offset(1., 2., 3.);
    }
    b.require_mut::<Glow>("beacon").0.to(b.tick_end(), 2., 0.5);
    b.require_mut::<Material>("beacon").color[..3].copy_from_slice(&[0.2, 0.3, 0.4]);
    b.require_mut::<Transform>("beacon").position += Vec3::new(1., 2., 3.);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.save(), b.save());
    let saved = a.save();
    a.load(&saved).unwrap();
    // Retargeting the leased component itself must not borrow it again.
    a.require_mut::<Glow>("beacon").glow_to(0.5, 0.25);
    a.require_mut::<Material>("beacon").tint([1.; 3]);
    a.get_mut::<Transform>("beacon")
        .unwrap()
        .offset(-1., 0., 0.);
    let held = a.require_mut::<Glow>("beacon");
    let conflict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        a.require_mut::<Beacon>("beacon").glow_to(1., 1.);
    }));
    assert!(conflict.is_err());
    drop(held);
    assert_eq!(a.require::<Glow>("beacon").0.start_tick, 1);
    assert_eq!(a.require::<Glow>("beacon").0.target, 0.5);
}

#[test]
fn query_rows_report_the_required_entity_handle_constructors() {
    let w = world();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for mut glow in w.query::<&mut Glow>() {
            glow.glow_to(1., 1.);
        }
    }))
    .unwrap_err();
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap();
    assert!(message.contains("entity-handle-required"));
    for constructor in [
        "World::get_mut",
        "World::require_mut",
        "World::nearest_xz_mut",
    ] {
        assert!(message.contains(constructor));
    }
}
