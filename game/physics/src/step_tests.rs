use super::*;
use exact_game::{Entity, Vec3, PAGE};

fn force_full_scan(w: &World) {
    w.resource::<Physics>()
        .executor
        .0
        .borrow_mut()
        .live()
        .changes = Default::default();
}

#[test]
fn cached_sync_matches_full_scan_snapshot_bytes_through_edits_and_restore() {
    let mut a = World::new(60, 0);
    crate::register(&mut a);
    a.resource_mut::<Physics>().gravity = Vec3::ZERO;
    let grand = a.spawn(Transform::default());
    let parent = a.spawn(Parent(grand));
    let body = a.spawn((
        Transform::at(0., 20., 0.),
        Body {
            velocity: Vec3::X,
            ..Default::default()
        },
        Collider::default(),
    ));
    while a.len() < PAGE {
        a.spawn(());
    }
    let wall = a.spawn((
        Transform::at(3., 0., 0.),
        Collider::default(),
        Parent(parent),
    ));
    while a.len() < PAGE * 2 {
        a.spawn(());
    }
    let other = a.spawn((Transform::at(10., 0., 0.), Collider::default()));
    let initial = a.save();
    let mut b = World::new(60, 0);
    crate::register(&mut b);
    b.register::<Parent>();
    b.load(&initial).unwrap();
    let mut checkpoint = Vec::new();
    for tick in 0..24 {
        for w in [&mut a, &mut b] {
            match tick {
                2 => w.get_mut::<Transform>(other).unwrap().position.x = 9.,
                3 => w.get_mut::<Transform>(grand).unwrap().position.x = 1.,
                4 => {
                    w.insert(parent, Transform::at(1., 0., 0.));
                }
                5 => {
                    w.insert(parent, Parent(other));
                }
                6 => {
                    w.remove::<Transform>(other);
                }
                7 => {
                    w.remove::<Body>(body);
                }
                8 => {
                    w.insert(other, Body::default());
                }
                9 => {
                    w.get_mut::<Collider>(wall).unwrap().offset.x = 0.25;
                }
                10 => {
                    w.get_mut::<Transform>(wall).unwrap().scale = Vec3::splat(2.);
                }
                11 => {
                    w.remove::<Collider>(other);
                }
                12 => {
                    w.insert(other, Collider::default());
                }
                13 => {
                    w.despawn(wall);
                    w.spawn((Transform::at(7., 0., 0.), Collider::default()));
                }
                15 => {
                    w.load(&checkpoint).unwrap();
                }
                17 => {
                    let physics = w.resource::<Physics>().clone();
                    w.insert_resource(physics);
                }
                _ => {}
            }
        }
        force_full_scan(&b);
        step(&mut a);
        step(&mut b);
        assert_eq!(a.hash(), b.hash(), "tick {tick}");
        assert_eq!(a.save(), b.save(), "EXPHYS and entry order at tick {tick}");
        if tick == 1 {
            checkpoint = a.save();
        }
    }
}

#[test]
fn moved_static_and_edited_collider_reach_rapier_on_the_next_tick() {
    let mut w = World::new(60, 0);
    crate::register(&mut w);
    let root = w.spawn(Transform::default());
    let sensor = w.spawn((
        Transform::default(),
        Body {
            kind: BodyKind::Kinematic,
            ..Default::default()
        },
        Collider {
            sensor: true,
            ..Default::default()
        },
    ));
    while w.len() < PAGE {
        w.spawn(());
    }
    let wall = w.spawn((Transform::at(3., 0., 0.), Parent(root), Collider::default()));
    let touching = |w: &World, began: bool| {
        assert!(crate::events(w)
            .iter()
            .any(|e| e.a == sensor.min(wall) && e.b == sensor.max(wall) && e.began == began));
    };
    step(&mut w);
    assert!(crate::events(&w).is_empty());
    w.get_mut::<Transform>(wall).unwrap().position.x = 0.;
    step(&mut w);
    touching(&w, true);
    w.get_mut::<Transform>(root).unwrap().position.x = 3.;
    step(&mut w);
    touching(&w, false);
    w.get_mut::<Collider>(wall).unwrap().offset.x = -3.;
    step(&mut w);
    touching(&w, true);
    w.get_mut::<Collider>(wall).unwrap().mask = 0;
    step(&mut w);
    touching(&w, false);
}

#[test]
fn live_page_cache_does_not_follow_physics_into_another_world() {
    fn world(x: f32) -> (World, Entity) {
        let mut w = World::new(60, 0);
        crate::register(&mut w);
        let e = w.spawn((Transform::at(x, 0., 0.), Collider::default()));
        (w, e)
    }
    let (mut a, _) = world(3.);
    let (mut b, e) = world(9.);
    step(&mut a);
    b.insert_resource(std::mem::take(&mut *a.resource_mut::<Physics>()));
    step(&mut b);
    let p = b.resource::<Physics>();
    let mut saved = p.executor.0.borrow_mut();
    let live = saved.live();
    let entry = &live.entries[&e];
    assert_eq!(entry.pose.position.x, 9.);
    assert_eq!(
        live.rapier.colliders[entry.ch().unwrap()]
            .position()
            .translation
            .x,
        9.
    );
}
