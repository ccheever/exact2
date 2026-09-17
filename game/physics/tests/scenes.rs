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
    let mut a = scene("pile");
    let mut b = scene("pile");
    for t in 1..=90 {
        tick(&mut a, t);
        tick(&mut b, t);
        assert_eq!(a.world().hash(), b.world().hash(), "replay tick {t}");
    }
    let save = a.save();
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
        assert_eq!(a.world().hash(), b.world().hash(), "replay tick {t}");
        assert_eq!(a.world().hash(), restored.world().hash(), "resume tick {t}");
    }
    eprintln!("PILE_HASH_600=0x{:016x}", a.world().hash());
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
    restored.restore(&original.save()).unwrap();
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
