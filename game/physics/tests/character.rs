mod common;
use common::*;
use exact_game::{Entity, Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, BodyKind, CapsuleController, Collider};
fn world() -> (World, Entity) {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    ground(&mut w);
    let e = w.spawn_named(
        "fox",
        (Transform::at(-1.0, 0.91, 0.0), CapsuleController::default()),
    );
    physics::capsule(&mut w, e).step(Vec3::ZERO);
    physics::step(&mut w);
    (w, e)
}
fn drive(w: &mut World, e: Entity, v: Vec3, ticks: usize) {
    for _ in 0..ticks {
        {
            let mut c = w.get_mut::<CapsuleController>(e).unwrap();
            c.velocity.y -= 9.81 / 60.0;
        }
        physics::capsule(w, e).step(v);
        physics::step(w);
    }
}
#[test]
fn push_respects_character_mask() {
    for layer in [1, 2] {
        for wall in [false, true] {
            let mut w = World::new(60, 0);
            physics::register(&mut w);
            let e = w.spawn((
                Transform::at(0.0, 1.0, 0.0),
                CapsuleController::default(),
                Collider {
                    mask: 1,
                    ..Collider::default()
                },
            ));
            let b = box_at(
                &mut w,
                "crate",
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::splat(0.5),
                true,
            );
            w.get_mut::<Body>(b).unwrap().mass = 10.0;
            w.get_mut::<Collider>(b).unwrap().layer = layer;
            if wall {
                // An included wall triggers the impulse routine's proximity query
                // even when the overlapping crate is excluded from movement.
                box_at(
                    &mut w,
                    "wall",
                    Vec3::new(1.0, 1.0, 0.0),
                    Vec3::splat(0.5),
                    false,
                );
            }
            physics::capsule(&mut w, e).step(Vec3::X * 120.0);
            let body = w.get::<Body>(b).unwrap();
            let x = w.get::<Transform>(e).unwrap().position.x;
            if layer == 1 {
                assert!(x < 0.21, "included box must obstruct: {x}");
                assert!(body.velocity.x > 0.0, "included box must be pushed");
            } else {
                assert_eq!(
                    body.velocity,
                    Vec3::ZERO,
                    "excluded box was pushed (wall={wall})"
                );
                assert_eq!(body.spin, Vec3::ZERO);
                assert_eq!(
                    w.get::<Transform>(b).unwrap().position,
                    Vec3::new(1.0, 1.0, 0.0)
                );
                if !wall {
                    assert!((x - 2.0).abs() < 1e-6, "excluded box obstructed: {x}");
                }
            }
        }
    }
}
#[test]
fn steps_below_and_above_limit() {
    for height in [0.25, 0.5] {
        let (mut w, e) = world();
        box_at(
            &mut w,
            "step",
            Vec3::new(2.0, height * 0.5, 0.0),
            Vec3::new(1.0, height * 0.5, 2.0),
            false,
        );
        drive(&mut w, e, Vec3::X * 2.0, 100);
        let p = w.get::<Transform>(e).unwrap().position;
        eprintln!("step {height}: {p:?}");
        if height < 0.3 {
            assert!(p.x > 1.5 && p.y > 1.1);
        } else {
            assert!(p.x < 0.8);
        }
    }
}
#[test]
fn slopes_walkable_and_steep() {
    for angle in [30.0, 60.0] {
        let (mut w, e) = world();
        let rotation = rotated(angle);
        w.spawn_named(
            "ramp",
            (
                Transform {
                    position: rotation * Vec3::new(3.0, -0.1, 0.0),
                    rotation,
                    ..Transform::default()
                },
                Collider {
                    shape: physics::Shape::Box {
                        half: Vec3::new(3.0, 0.1, 2.0),
                    },
                    ..Collider::default()
                },
            ),
        );
        drive(&mut w, e, Vec3::X * 2.0, 100);
        let p = w.get::<Transform>(e).unwrap().position;
        eprintln!("ramp {angle}: {p:?}");
        if angle == 30.0 {
            assert!(p.x > 1.0 && p.y > 1.5);
        } else {
            assert!(p.x < 0.3);
        }
    }
}
#[test]
fn wall_slide_and_thin_wall_at_twenty_metres_per_second() {
    let (mut w, e) = world();
    box_at(
        &mut w,
        "wall",
        Vec3::new(1.0, 2.0, 0.0),
        Vec3::new(0.05, 2.0, 10.0),
        false,
    );
    drive(&mut w, e, Vec3::new(20.0, 0.0, 2.0), 60);
    let p = w.get::<Transform>(e).unwrap().position;
    assert!(p.x < 0.66, "tunneled: {p:?}");
    assert!(p.z > 1.8, "stuck: {p:?}");
}
#[test]
fn platform_transport_and_finite_push_budget() {
    let (mut w, e) = world();
    let p = box_at(
        &mut w,
        "platform",
        Vec3::new(0.0, 0.2, 0.0),
        Vec3::new(2.0, 0.2, 2.0),
        false,
    );
    w.insert(
        p,
        Body {
            kind: BodyKind::Kinematic,
            ..Body::default()
        },
    );
    w.get_mut::<Transform>(e).unwrap().position = Vec3::new(0.0, 1.31, 0.0);
    drive(&mut w, e, Vec3::ZERO, 3);
    for _ in 0..60 {
        w.get_mut::<Transform>(p).unwrap().position.x += 0.01;
        drive(&mut w, e, Vec3::ZERO, 1);
    }
    let pos = w.get::<Transform>(e).unwrap().position;
    assert!((pos.x - 0.6).abs() < 0.02, "platform carry {pos:?}");
    assert!(w.get::<CapsuleController>(e).unwrap().grounded);
    for mass in [10.0, 1000.0] {
        let (mut w, e) = world();
        let b = box_at(
            &mut w,
            "crate",
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::splat(0.5),
            true,
        );
        w.get_mut::<Body>(b).unwrap().mass = mass;
        drive(&mut w, e, Vec3::X * 2.0, 120);
        let x = w.get::<Transform>(b).unwrap().position.x;
        eprintln!("push {mass}kg: crate x={x}");
        if mass < 80.0 {
            assert!(x > 2.0);
        } else {
            assert!((x - 1.0).abs() < 0.01);
        }
    }
}

#[test]
fn flat_motor_and_named_capsule_coexist_and_report_actual_displacement() {
    let (mut w, e) = world();
    w.spawn_named(
        "flat",
        (
            Transform::default(),
            exact_game::character::Character::new(),
        ),
    );
    let flat = w.character("flat").step(Vec3::X, false);
    assert!(flat.displacement.x > 0.);
    let before = w.global_position(e).unwrap();
    let moved = physics::capsule(&mut w, "fox").step(Vec3::X);
    assert_eq!(moved.displacement, w.global_position(e).unwrap() - before);
    let saved = w.save();
    w.load(&saved).unwrap();
    assert!(w.get::<CapsuleController>("fox").is_some());
    assert!(w.get::<exact_game::character::Character>("flat").is_some());
}

#[test]
fn same_entity_movement_controllers_are_refused_by_both_handles() {
    let mut w = exact_game::World::new(60, 7);
    w.spawn_named(
        "ranger",
        (
            exact_game::Transform::default(),
            exact_game::character::Character::new(),
            exact_game_physics::CapsuleController::default(),
        ),
    );
    for core in [false, true] {
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if core {
                let _ = w.character("ranger");
            } else {
                let _ = exact_game_physics::capsule(&mut w, "ranger");
            }
        }))
        .expect_err("ambiguous controller accepted");
        let text = failure.downcast_ref::<String>().unwrap();
        assert!(
            text.contains("ranger")
                && text.contains("Character")
                && text.contains("CapsuleController"),
            "{text}"
        );
    }
}
