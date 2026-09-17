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
        for (_, (t, v)) in w.query::<(&mut Transform, &Velocity)>().iter() {
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
fn hierarchy_and_fresh_tick() {
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
    assert_eq!(w.global(child).unwrap().translation.x, 11.0);
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 11.0);
    w.begin_tick();
    assert!(w.fresh().is_empty());
    w.teleport(root, Transform::at(100.0, 0.0, 0.0));
    assert_eq!(w.fresh(), [root]);
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 101.0);
    assert!(w.despawn(root));
    assert_eq!(w.len(), 2);
    w.reap_orphans();
    assert!(w.is_empty());
}

#[test]
fn changed_tracks_leases_structure_and_load_but_is_not_saved() {
    let mut w = World::new(60, 0);
    assert_eq!(w.changed::<Transform>(), 0);
    w.step_clock();
    let e = w.spawn((Transform::default(),));
    assert_eq!(w.changed::<Transform>(), 1);
    w.step_clock();
    let bytes = w.save();
    let hash = w.hash();
    drop(w.get::<Transform>(e));
    drop(w.query::<&Transform>());
    drop(w.pages::<Transform>());
    assert_eq!(w.changed::<Transform>(), 1);
    drop(w.query::<Option<&mut Transform>>());
    assert_eq!(w.changed::<Transform>(), 2);
    assert_eq!(w.save(), bytes);
    assert_eq!(w.hash(), hash);
    w.step_clock();
    drop(w.get_mut::<Transform>(e));
    assert_eq!(w.changed::<Transform>(), 3);
    w.step_clock();
    w.insert(e, Transform::default());
    assert_eq!(w.changed::<Transform>(), 4);
    w.step_clock();
    w.remove::<Transform>(e);
    assert_eq!(w.changed::<Transform>(), 5);
    w.step_clock();
    w.insert(e, Transform::default());
    w.step_clock();
    w.despawn(e);
    assert_eq!(w.changed::<Transform>(), 7);
    w.load(&bytes).unwrap();
    assert_eq!(w.tick(), 2);
    assert_eq!(w.changed::<Transform>(), 2);
    assert_eq!(w.save(), bytes);
}

#[test]
fn save_entity_limit_is_checked_before_reserving_slots() {
    let mut out = bin::Encoder::default();
    out.begin_struct();
    out.field("state");
    out.begin_struct();
    out.field("slots");
    out.begin_seq(crate::data::MAX_LOAD_ENTITIES + 1);
    let mut bytes = MAGIC.to_vec();
    bytes.extend(out.finish());
    // Enough input to satisfy the codec's minimum byte count, without constructing entities.
    bytes.resize(bytes.len() + crate::data::MAX_LOAD_ENTITIES + 1, 0);
    let mut w = World::new(60, 0);
    let before = w.save();
    let err = w.load(&bytes).unwrap_err().to_string();
    assert!(err.contains("slots") && err.contains("limit"), "{err}");
    assert_eq!(w.save(), before);
}
