use super::*;
use exact_game::Quat;

fn fixture(dynamic_first: bool, count: usize) -> (World, Vec<Entity>, Entity, Entity, Entity) {
    let mut w = World::new(60, 0);
    crate::register(&mut w);
    w.resource_mut::<crate::Physics>().gravity = Vec3::ZERO;
    let platform = w.spawn((
        Transform::at(0., -0.2, 0.),
        Collider {
            shape: Shape::Box {
                half: Vec3::new(6., 0.2, 4.),
            },
            ..Default::default()
        },
        Body {
            kind: BodyKind::Kinematic,
            ..Default::default()
        },
    ));
    let a = w.spawn((Transform::at(1., 0.9, 0.), Collider::default()));
    let b = w.spawn((Transform::at(1., 0.9, 0.), Collider::default()));
    let pushed = if dynamic_first { a } else { b };
    w.insert(
        pushed,
        Body {
            mass: 10.,
            ..Default::default()
        },
    );
    let heavy = w.spawn((
        Transform::at(1., 0.9, -0.4),
        Collider::default(),
        Body {
            mass: 1000.,
            ..Default::default()
        },
    ));
    w.spawn((
        Transform::at(0.2, 0.9, 0.),
        Collider {
            sensor: true,
            ..Default::default()
        },
    ));
    w.spawn((
        Transform::at(0.2, 0.9, 0.),
        Collider {
            layer: 2,
            ..Default::default()
        },
    ));
    // Spatially overlapping AABBs and distant scenery both influence binned
    // partitions, even when the controller filters a collider out.
    for i in 0..2_000 {
        w.spawn((
            Transform::at((i % 100) as f32 * 3. - 150., -5., (i / 100) as f32 * 3.),
            Collider::default(),
        ));
    }
    let players = (0..count)
        .map(|i| {
            w.spawn((
                Transform::at(0., 0.91, i as f32 * 2.),
                CapsuleController {
                    grounded: true,
                    support: Some(platform),
                    support_pose: Transform::at(0., -0.2, 0.),
                    velocity: -Vec3::Y,
                    ..Default::default()
                },
                Collider {
                    mask: 1,
                    ..Default::default()
                },
            ))
        })
        .collect();
    (w, players, platform, pushed, heavy)
}

#[test]
fn character_carry_support_ties_and_pushes_match_canonical_single_scene() {
    for dynamic_first in [false, true] {
        for count in [1, 8] {
            let (mut actual, players, platform, pushed, heavy) = fixture(dynamic_first, count);
            let (mut canonical, _, _, _, _) = fixture(dynamic_first, count);
            let mut carried = false;
            let mut hits = 0;
            let mut supported = false;
            let mut impulse = false;
            for tick in 0..40 {
                for w in [&mut actual, &mut canonical] {
                    let t = tick as f32;
                    w.insert(
                        platform,
                        Transform {
                            position: Vec3::new(t * 0.004, -0.2, 0.),
                            rotation: Quat::from_rotation_y(t * 0.001),
                            ..Default::default()
                        },
                    );
                    for &p in &players {
                        w.get_mut::<CapsuleController>(p).unwrap().velocity.y = -1.;
                    }
                    // Include a same-tick edit and an explicit restoration of a
                    // pushed body's old controls; cached impulses must disappear.
                    if tick == 5 || tick == 20 {
                        w.get_mut::<Body>(pushed).unwrap().velocity = Vec3::ZERO;
                        w.get_mut::<Body>(pushed).unwrap().spin = Vec3::ZERO;
                        w.get_mut::<Collider>(pushed).unwrap().offset.z = 0.03;
                    }
                }
                for &p in &players {
                    let v = if tick == 0 {
                        Vec3::X * 120.
                    } else {
                        Vec3::X * 3.
                    };
                    let got = move_capsule_inner(&mut actual, p, v, false);
                    let expected = move_capsule_inner(&mut canonical, p, v, true);
                    assert_eq!(
                        got, expected,
                        "tick={tick} player={p:?} dynamic_first={dynamic_first}"
                    );
                    carried |= got.carried != Vec3::ZERO;
                    hits += got.movement.len();
                    supported |= got.support.is_some();
                    impulse |= actual.get::<Body>(pushed).unwrap().velocity != Vec3::ZERO;
                    assert_eq!(
                        actual.get::<Body>(heavy).unwrap().velocity,
                        canonical.get::<Body>(heavy).unwrap().velocity
                    );
                    assert_eq!(
                        actual.save(),
                        canonical.save(),
                        "complete state after character at tick {tick}"
                    );
                }
                // Deliberately coincident static/dynamic obstacles exercise exact
                // TOI ties. This oracle drives the controller directly; solver
                // contacts/snapshots are covered by step_tests and scenes.rs.
            }
            assert!(
                carried && hits > 0 && supported && impulse,
                "carry={carried} hits={hits} support={supported} push={impulse}"
            );
        }
    }
}
