use super::*;
use crate::{Component, Transform, Vec3};

#[derive(Default, Component)]
struct Velocity(Vec3);
fn churn(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        if w.rng().chance(0.6) {
            let x = w.rng().range(-10.0..10.0);
            w.spawn_named(
                "particle",
                (Transform::at(x, 0.0, 0.0), Velocity(Vec3::new(1.0, x, 0.0))),
            );
        }
        let dt = w.dt();
        for (_, (mut t, v)) in w.query::<(&mut Transform, &Velocity)>() {
            t.position += v.0 * dt;
        }
        if w.rng().chance(0.3) {
            let entities: Vec<_> = w.entities().collect();
            let selected = w.rng().pick(&entities).copied();
            if let Some(e) = selected {
                w.despawn(e);
            }
        }
        w.step_clock();
    }
}
#[test]
fn replay_equivalence() {
    let mut straight = World::new(60, 42);
    churn(&mut straight, 600);
    let mut first = World::new(60, 42);
    churn(&mut first, 300);
    let mut restored = World::new(60, 0);
    restored.register_scene().register::<Velocity>();
    restored.load(&first.save()).unwrap();
    assert_eq!(first.hash(), restored.hash());
    churn(&mut restored, 300);
    assert_eq!(straight.tick(), 600);
    assert_eq!(straight.hash(), restored.hash());
    assert_eq!(straight.save(), restored.save());
}
#[test]
fn hierarchy_and_previous_tick() {
    let mut w = World::new(60, 0);
    let child = w.spawn((Transform::at(1.0, 0.0, 0.0),));
    let middle = w.spawn((Transform::at(0.0, 2.0, 0.0),));
    let root = w.spawn((Transform::at(0.0, 0.0, 3.0),));
    w.insert(child, Parent(middle));
    w.insert(middle, Parent(root));
    w.propagate();
    let old = w.global(child).unwrap();
    assert_eq!(old.translation, Vec3::new(1.0, 2.0, 3.0).into());
    let hash = w.hash();
    w.propagate();
    assert_eq!(hash, w.hash());
    w.step_clock();
    w.get_mut::<Transform>(root).unwrap().position.x = 10.0;
    w.propagate();
    let new = w.global(child).unwrap();
    assert_eq!(w.global_lerp(child, 0.0), Some(old));
    assert_eq!(w.global_lerp(child, 1.0), Some(new));
    w.propagate();
    assert_eq!(w.global_lerp(child, 0.0), Some(old));
    w.teleport(root, Transform::at(100.0, 0.0, 0.0));
    assert_eq!(w.global_lerp(child, 0.0), w.global_lerp(child, 1.0));
    assert!(w.despawn(root));
    assert!(w.is_empty());
}
