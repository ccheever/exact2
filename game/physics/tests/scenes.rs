mod common;
use common::*;
use exact_game::*;
use exact_game_physics::{self as physics, Body, BodyKind, Collider, Physics, Shape};

#[test]
fn drop_stack_and_block_sleep() {
    for (name, ticks) in [("drop", 600), ("stack", 1200), ("pile", 1200)] {
        let mut s = scene(name);
        let initial = positions(s.world());
        let mut slept = 0;
        for t in 1..=ticks {
            tick(&mut s, t);
            if slept == 0 && physics::quiescent(s.world()) {
                slept = t;
            }
        }
        let final_pos = positions(s.world());
        let drift = initial
            .iter()
            .zip(&final_pos)
            .map(|(a, b)| Vec2::new(a.0.x - b.0.x, a.0.z - b.0.z).length())
            .fold(0.0, f32::max);
        let min_y = final_pos
            .iter()
            .map(|p| p.0.y)
            .fold(f32::INFINITY, f32::min);
        eprintln!("scene {name}: sleep={slept} drift={drift:.6} bottom={min_y:.6}");
        assert!(min_y > 0.49, "{name}: fell through ground");
        assert!(slept > 0, "{name}: never slept");
        if name == "stack" {
            assert!(drift < 0.01, "stack drift {drift}");
        }
        if name == "drop" {
            assert!((min_y - 0.5).abs() < 0.0004, "drop rest height {min_y}");
        }
    }
}
#[test]
fn bounce_height() {
    let mut s = scene("bounce");
    let ball = s.world().named("ball").unwrap();
    let mut touched = false;
    let mut apex = 0.0f32;
    for t in 1..180 {
        tick(&mut s, t);
        let y = s.world().get::<Transform>(ball).unwrap().position.y;
        if y < 0.51 {
            touched = true;
        }
        if touched {
            apex = apex.max(y - 0.5);
        }
    }
    eprintln!("bounce apex={apex:.6}, expected 0.5");
    assert!((apex - 0.5).abs() < 0.075);
}
#[test]
fn resume_and_replay_every_tick() {
    let mut a = scene("pile").paranoid(Paranoid::Off);
    let mut b = scene("pile").paranoid(Paranoid::Save);
    let mut fresh = scene("pile").paranoid(Paranoid::FreshGame);
    for t in 1..=90 {
        tick(&mut a, t);
        tick(&mut b, t);
        tick(&mut fresh, t);
        assert_eq!(
            a.world().hash(),
            fresh.world().hash(),
            "fresh-game tick {t}"
        );
        assert_eq!(a.world().hash(), b.world().hash(), "replay tick {t}");
    }
    let save = a.save().unwrap();
    let mut restored = scene("pile");
    restored.restore(&save).unwrap();
    tick(&mut restored, 90);
    assert_eq!(a.world().hash(), restored.world().hash());
    assert!(
        a.world().resource::<Physics>().refresh_snapshot() > 0,
        "resume fixture must save Rapier state"
    );
    for t in 91..=600 {
        tick(&mut a, t);
        tick(&mut b, t);
        tick(&mut restored, t);
        tick(&mut fresh, t);
        assert_eq!(
            a.world().hash(),
            fresh.world().hash(),
            "fresh-game tick {t}"
        );
        assert_eq!(a.world().hash(), b.world().hash(), "replay tick {t}");
        assert_eq!(a.world().hash(), restored.world().hash(), "resume tick {t}");
    }
    let bytes = a.save().unwrap();
    assert!(bytes == b.save().unwrap() && bytes == fresh.save().unwrap());
    eprintln!(
        "PILE_HASH_600 continuous=save=fresh-game=0x{:016x}",
        a.world().hash()
    );
    let pins: std::collections::BTreeMap<String, String> =
        exact_game::json::from_str(include_str!("pins.json")).unwrap();
    assert_eq!(format!("0x{:016x}", a.world().hash()), pins["pile-600"]);
}
#[test]
fn slopes_box_static_and_sliding() {
    for angle in [20.0, 40.0] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        let rotation = rotated(angle);
        w.spawn((
            Transform {
                rotation,
                position: rotation * Vec3::new(0.0, -0.5, 0.0),
                ..Transform::default()
            },
            Collider {
                shape: Shape::Box {
                    half: Vec3::new(20.0, 0.5, 20.0),
                },
                ..Collider::default()
            },
        ));
        let e = w.spawn((
            Transform {
                rotation,
                position: rotation * Vec3::new(0.0, 0.5, 0.0),
                ..Transform::default()
            },
            Body::default(),
            Collider::default(),
        ));
        let start = w.get::<Transform>(e).unwrap().position;
        for _ in 0..600 {
            physics::step(&mut w);
        }
        let distance = w.get::<Transform>(e).unwrap().position.distance(start);
        eprintln!("box slope {angle}: displacement={distance}");
        if angle == 20.0 {
            assert!(distance < 0.01);
        } else {
            assert!(distance > 1.0);
        }
    }
}
#[test]
fn wake_supporter_edits_and_despawn() {
    let mut s = scene("stack");
    for t in 1..=600 {
        tick(&mut s, t);
    }
    assert!(physics::quiescent(s.world()));
    let support = s.world().named("ground").unwrap();
    s.world_mut().despawn(support);
    tick(&mut s, 601);
    assert!(!physics::quiescent(s.world()));
    assert!(s.world().query::<&Body>().iter().all(|(_, b)| !b.asleep));
    let mut s = scene("drop");
    for t in 1..=300 {
        tick(&mut s, t);
    }
    let e = s.world().named("box-0-0-0").unwrap();
    s.world().get_mut::<Transform>(e).unwrap().position.y += 1.0;
    tick(&mut s, 301);
    assert!(!s.world().get::<Body>(e).unwrap().asleep);
    for t in 302..=600 {
        tick(&mut s, t);
    }
    s.world().get_mut::<Body>(e).unwrap().velocity = Vec3::X;
    tick(&mut s, 601);
    assert!(!s.world().get::<Body>(e).unwrap().asleep);
}
#[test]
fn sensors_filter_events_and_journal() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let a = w.spawn_named(
        "sensor",
        (
            Transform::default(),
            Collider {
                sensor: true,
                ..Collider::default()
            },
            physics::Announce,
        ),
    );
    let b = w.spawn_named(
        "crate",
        (
            Transform::at(0.0, 0.9, 0.0),
            Body {
                kind: BodyKind::Kinematic,
                ..Body::default()
            },
            Collider::default(),
        ),
    );
    physics::step(&mut w);
    assert_eq!(physics::events(&w).len(), 1);
    assert!(physics::events(&w)[0].began);
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.ends_with("touch sensor × crate"))
            .count(),
        1
    );
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty());
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("touch sensor × crate"))
            .count(),
        1
    );
    w.get_mut::<Transform>(b).unwrap().position.y = 5.0;
    physics::step(&mut w);
    assert_eq!(physics::events(&w).len(), 1);
    assert!(!physics::events(&w)[0].began);
    w.get_mut::<Collider>(a).unwrap().mask = 0;
    w.get_mut::<Transform>(b).unwrap().position.y = 0.0;
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty());
}

#[test]
fn removed_colliders_keep_their_event_identity_across_restore() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let sensor = w.spawn_named(
        "sensor",
        (
            Transform::default(),
            Collider {
                sensor: true,
                ..Collider::default()
            },
            physics::Announce,
        ),
    );
    let mover = |w: &mut World| {
        w.spawn((
            Transform::at(0.0, 0.9, 0.0),
            Body {
                kind: BodyKind::Kinematic,
                ..Body::default()
            },
            Collider::default(),
        ))
    };
    let events = |w: &World| {
        physics::events(w)
            .iter()
            .map(|event| (event.a, event.b, event.began))
            .collect::<Vec<_>>()
    };
    let old = mover(&mut w);
    physics::step(&mut w);
    assert_eq!(events(&w), [(sensor, old, true)]);
    w.remove::<Collider>(old);
    let replacement = mover(&mut w);
    physics::step(&mut w);
    assert_eq!(
        events(&w),
        [(sensor, old, false), (sensor, replacement, true)]
    );

    let mut restored = World::new(60, 0);
    physics::register(&mut restored);
    restored.load(&w.save()).unwrap();
    for world in [&mut w, &mut restored] {
        world.despawn(replacement);
        physics::step(world);
        assert_eq!(events(world), [(sensor, replacement, false)]);
        assert!(world
            .journal()
            .last()
            .unwrap()
            .line
            .ends_with(&format!("untouch sensor × #{}", replacement.index())));
    }
    assert_eq!(w.save(), restored.save());
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty());
}
#[test]
fn observed_bodies_participate_in_settle() {
    let mut s = scene("drop");
    tick(&mut s, 1);
    assert!(!s.quiescent());
    for t in 2..=600 {
        if s.quiescent() {
            break;
        }
        tick(&mut s, t);
    }
    assert!(s.quiescent());
    assert!(physics::quiescent(s.world()));
}
#[test]
#[should_panic(expected = "Body `child` must be a root")]
#[cfg(debug_assertions)]
fn refuses_parented_body_by_name() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let p = w.spawn(Transform::default());
    w.spawn_named(
        "child",
        (
            Parent(p),
            Transform::default(),
            Body::default(),
            Collider::default(),
        ),
    );
    physics::step(&mut w);
}

#[test]
fn free_spheres_roll_without_slipping_on_both_slopes() {
    // Coulomb friction does not hold a free sphere stationary. Rolling requires
    // mu >= (2/7) tan(theta), which 0.6 satisfies at both 20 and 40 degrees.
    for angle in [20.0, 40.0] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        let rotation = rotated(angle);
        w.spawn((
            Transform {
                rotation,
                position: rotation * Vec3::new(0.0, -0.5, 0.0),
                ..Transform::default()
            },
            Collider {
                shape: Shape::Box {
                    half: Vec3::new(30.0, 0.5, 30.0),
                },
                ..Collider::default()
            },
        ));
        let e = w.spawn((
            Transform {
                position: rotation * Vec3::new(0.0, 0.5, 0.0),
                ..Transform::default()
            },
            Body::default(),
            Collider {
                shape: Shape::Sphere { radius: 0.5 },
                ..Collider::default()
            },
        ));
        for _ in 0..60 {
            physics::step(&mut w);
        }
        let b = w.get::<Body>(e).unwrap();
        let n = rotation * Vec3::Y;
        let contact_velocity = b.velocity + b.spin.cross(-n * 0.5);
        let tangent = rotation * Vec3::X;
        // 0.1 m/s bounds residual contact drift from the discrete friction solve.
        assert!(
            contact_velocity.length() < 0.1,
            "rolling slip at {angle}: {} m/s ({contact_velocity:?})",
            contact_velocity.length()
        );
        let expected =
            5.0 / 7.0 * 9.81 * exact_game::math::sin(angle * std::f32::consts::PI / 180.0);
        eprintln!(
            "rolling {angle}: contact speed={} acceleration={} expected={expected}",
            contact_velocity.dot(tangent),
            b.velocity.dot(-tangent)
        );
        // 3% preserves the solid sphere's rolling acceleration at a 60 Hz tick.
        let relative_error = (b.velocity.dot(-tangent) - expected).abs() / expected;
        assert!(
            relative_error <= 0.03,
            "rolling {angle}: acceleration error={} %",
            relative_error * 100.0
        );
    }
}

#[test]
fn resume_mid_bounce_every_tick() {
    let mut original = scene("bounce");
    for t in 1..=45 {
        tick(&mut original, t);
    }
    let ball = original.world().named("ball").unwrap();
    assert!(original.world().get::<Body>(ball).unwrap().velocity.y > 0.0);
    let mut restored = scene("bounce");
    restored.restore(&original.save().unwrap()).unwrap();
    tick(&mut restored, 45);
    for t in 46..=240 {
        tick(&mut original, t);
        tick(&mut restored, t);
        assert_eq!(
            original.world().hash(),
            restored.world().hash(),
            "mid-bounce resume tick {t}"
        );
    }
}
#[test]
fn fixed_solids_make_no_pairs_but_fixed_sensors_still_touch() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let snapshot = |w: &World| w.resource::<Physics>().refresh_snapshot();
    w.spawn((Transform::default(), Collider::default()));
    physics::step(&mut w);
    let one = snapshot(&w);
    // A hundred static solids overlapping the first and each other.
    for i in 0..100 {
        w.spawn((Transform::at(i as f32 * 0.01, 0., 0.), Collider::default()));
    }
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty(), "fixed solids touched");
    // No pair state, and Rapier's copy of each collider is rebuilt from its entry.
    let per = (snapshot(&w) - one) / 100;
    assert!(per < 120, "a static collider costs {per} snapshot bytes");
    let sensor = w.spawn((
        Transform::default(),
        Collider {
            sensor: true,
            ..Collider::default()
        },
    ));
    physics::step(&mut w);
    assert_eq!(physics::events(&w).len(), 101);
    assert!(physics::events(&w)
        .iter()
        .all(|t| t.began && (t.a == sensor || t.b == sensor)));
}

// Static colliders are rebuilt from their entries on restore. Edits through every
// sync path (move, reshape, add, remove, and an edit still pending at the save)
// continue identically to the run that never saved.
#[test]
fn rebuilt_static_colliders_continue_like_the_original() {
    let edit = |w: &mut World, t: u32| match t {
        20 => w.get_mut::<Transform>("wall-3").unwrap().position.y += 0.25,
        30 => {
            w.get_mut::<Collider>("wall-5").unwrap().shape = Shape::Box {
                half: Vec3::new(0.5, 2.0, 0.5),
            }
        }
        40 => {
            box_at(w, "late", Vec3::new(4.0, 0.5, 4.0), Vec3::splat(0.5), false);
        }
        50 => {
            let e = w.named("wall-7").unwrap();
            w.despawn(e);
        }
        _ => {}
    };
    let run = |save_at: Option<u32>| {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        ground(&mut w);
        for i in 0..10 {
            let p = Vec3::new(i as f32 * 1.2 - 6.0, 0.5, 2.0);
            box_at(&mut w, &format!("wall-{i}"), p, Vec3::splat(0.5), false);
        }
        box_at(
            &mut w,
            "crate",
            Vec3::new(0.0, 3.0, 2.0),
            Vec3::splat(0.4),
            true,
        );
        for t in 1..=90 {
            physics::step(&mut w);
            edit(&mut w, t);
            if save_at == Some(t) {
                let bytes = w.save();
                w = World::new(60, 0);
                physics::register(&mut w);
                w.load(&bytes).unwrap();
            }
        }
        (w.hash(), w.save())
    };
    let expected = run(None);
    for at in [1, 20, 30, 35, 40, 50, 60] {
        assert!(run(Some(at)) == expected, "restored at tick {at} diverged");
    }
}

// A static collider switched between solid and sensor in place gains and loses its
// fixed-fixed pairing: sensor touches the static ground, solid drops the pair.
#[test]
fn switching_a_static_collider_to_sensor_and_back_updates_its_pairing() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    ground(&mut w);
    let wall = box_at(
        &mut w,
        "wall",
        Vec3::new(0.0, 0.2, 0.0),
        Vec3::splat(0.5),
        false,
    );
    let snapshot = |w: &World| w.resource::<Physics>().refresh_snapshot();
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty());
    let solid = snapshot(&w);
    w.get_mut::<Collider>(wall).unwrap().sensor = true;
    physics::step(&mut w);
    assert_eq!(
        physics::events(&w).len(),
        1,
        "a static sensor must touch the ground"
    );
    assert!(physics::events(&w)[0].began);
    w.get_mut::<Collider>(wall).unwrap().sensor = false;
    physics::step(&mut w);
    assert_eq!(
        physics::events(&w).len(),
        1,
        "back to solid must end the touch"
    );
    assert!(!physics::events(&w)[0].began);
    physics::step(&mut w);
    assert!(physics::events(&w).is_empty());
    // The pair is gone; only the re-inserted broad-phase leaf may differ. Repeated
    // switching must not accumulate state.
    let back = snapshot(&w);
    for _ in 0..10 {
        for sensor in [true, false] {
            w.get_mut::<Collider>(wall).unwrap().sensor = sensor;
            physics::step(&mut w);
            let events = physics::events(&w);
            assert!(
                events.len() == 1 && events[0].began == sensor,
                "{:?}",
                &*events
            );
        }
    }
    physics::step(&mut w);
    assert!(back < solid + 128 && snapshot(&w) == back, "{solid} {back}");
}
