use super::*;
use crate::{Follow, Parent, Transform};

// Compile the production Body schema against this unit-test engine, without
// making the engine depend on the physics executor (which depends on the engine).
#[path = "../../../physics/src/body.rs"]
mod physics_body;
use physics_body::Body;

#[derive(Default, crate::Component)]
struct Skipped {
    saved: u32,
    #[data(skip)]
    transient: u32,
}

fn check(w: &mut World, before: &Observation, changed: bool) {
    for full in [false, true] {
        let mut cached = Observation::default();
        let mut oracle = Observation::default();
        w.observe_with_hash(&mut cached, full);
        w.observe_uncached(&mut oracle, full);
        assert_eq!(cached.entries, oracle.entries);
        assert_eq!(before.entries != cached.entries, changed);
        w.compare(before, &oracle);
        let state = w.observation;
        let reasons = w.changing();
        w.compare(before, &cached);
        assert!(w.observation == state);
        assert_eq!(w.changing(), reasons);
        assert_eq!(w.observation == ObservationState::Changing, changed);
        let mut canonical = crate::hash::Hasher::default();
        w.write(&mut canonical, false);
        assert_eq!(w.hash(), canonical.finish());
    }
}

// One independently initialized table row per API. All fixtures warm first;
// each action asserts its postcondition before the common oracle comparison.
#[test]
fn mutation_api_table() {
    type Action = fn(&mut World, Entity);
    let cases: &[(&str, bool, Action)] = &[
        ("insert replacement", true, |w, e| {
            assert!(w.insert(e, Transform::at(8.0, 0.0, 0.0)));
            assert_eq!(w.get::<Transform>(e).unwrap().position.x, 8.0);
        }),
        ("remove", true, |w, e| {
            assert!(w.remove::<Transform>(e).is_some());
            assert!(!w.has::<Transform>(e));
        }),
        ("insert_resource replacement", true, |w, _| {
            w.insert_resource(Count(12));
            assert_eq!(w.resource::<Count>().0, 12);
        }),
        ("reseed", true, |w, _| {
            let old = crate::hash::of(&*w.rng.get().unwrap());
            w.reseed(987);
            assert_ne!(crate::hash::of(&*w.rng.get().unwrap()), old);
        }),
        ("retained resource guard", true, |w, _| {
            let mut guard = w.resource_mut::<Count>();
            guard.0 = 19;
            for full in [false, true] {
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    w.observe_with_hash(&mut Observation::default(), full);
                }))
                .is_err());
            }
            drop(guard);
            assert_eq!(w.resource::<Count>().0, 19);
        }),
        ("retained RNG guard", true, |w, _| {
            let old = crate::hash::of(&*w.rng.get().unwrap());
            let mut guard = w.rng();
            guard.next_u32();
            for full in [false, true] {
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    w.observe_with_hash(&mut Observation::default(), full);
                }))
                .is_err());
            }
            drop(guard);
            assert_ne!(crate::hash::of(&*w.rng.get().unwrap()), old);
        }),
        ("teleport with follower", true, |w, e| {
            let follower = w.named("follower").unwrap();
            let old = crate::hash::of(&*w.get::<Follow>(follower).unwrap());
            w.teleport(e, Transform::at(20.0, 0.0, 0.0));
            assert_eq!(w.get::<Transform>(e).unwrap().position.x, 20.0);
            assert_ne!(crate::hash::of(&*w.get::<Follow>(follower).unwrap()), old);
            crate::scene::follow(w);
            assert_eq!(w.get::<Transform>(follower).unwrap().position.x, 20.0);
        }),
        (
            "physics consecutive Transform and Body write-back",
            true,
            |w, e| {
                // The same two insert calls as physics/step.rs, using its Body schema.
                let mut pose = *w.get::<Transform>(e).unwrap();
                let mut body = w.get::<Body>(e).unwrap().clone();
                pose.position.y -= 0.25;
                body.velocity.y = -3.0;
                w.insert(e, pose);
                w.insert(e, body);
                assert_eq!(w.get::<Transform>(e).unwrap().position.y, -0.25);
                assert_eq!(w.get::<Body>(e).unwrap().velocity.y, -3.0);
            },
        ),
        ("skip-only change", false, |w, e| {
            w.get_mut::<Skipped>(e).unwrap().transient = 42;
            assert_eq!(w.get::<Skipped>(e).unwrap().transient, 42);
            assert_eq!(w.get::<Skipped>(e).unwrap().saved, 0);
        }),
        ("empty mutable query", false, |w, _| {
            let epoch = w.mutation_epoch();
            assert_eq!(w.query::<(&mut Transform, &Payload)>().iter().count(), 0);
            assert_ne!(w.mutation_epoch(), epoch);
        }),
        ("rejected load", false, |w, _| {
            let before = w.save();
            // A valid header gets deep into candidate decoding before refusal.
            assert!(w.load(&before[..before.len() - 1]).is_err());
            assert_eq!(w.save(), before);
        }),
        ("high-index parent", true, |w, e| {
            let parent = w.named("high-parent").unwrap();
            assert!(parent.index() > e.index());
            w.insert(e, Parent(parent));
            w.propagate();
            assert_eq!(w.get::<Parent>(e).unwrap().0, parent);
            assert_eq!(w.global(e).unwrap().translation.x, 100.0);
        }),
    ];
    for &(name, changed, action) in cases {
        let mut w = World::new(60, 7);
        let e = w.spawn((Transform::default(), Body::default(), Skipped::default()));
        w.insert_resource(Count(0));
        w.spawn_named(
            "follower",
            (Transform::default(), Follow::new(e).offset(0.0, 2.0, 5.0)),
        );
        for _ in 0..storage::PAGE {
            w.spawn(());
        }
        w.spawn_named("high-parent", Transform::at(100.0, 0.0, 0.0));
        crate::scene::place_followers(&w);
        w.propagate();
        let mut before = Observation::default();
        w.observe_with_hash(&mut before, true);
        action(&mut w, e);
        eprintln!("invalidation case: {name}");
        check(&mut w, &before, changed);
    }
}

#[test]
fn mutable_optional_and_owned_queries_span_pages() {
    for owned in [false, true] {
        let mut w = World::new(60, 0);
        let entities: Vec<_> = (0..storage::PAGE * 3 + 3)
            .map(|i| {
                let e = w.spawn(Transform::default());
                if i % 3 == 0 {
                    w.insert(e, CountComponent(0));
                }
                e
            })
            .collect();
        let mut before = Observation::default();
        w.observe(&mut before);
        let mut rows = 0;
        let mut present = 0;
        if owned {
            let mut guards: Vec<_> = w
                .query::<(&mut Transform, Option<&mut CountComponent>)>()
                .into_iter()
                .collect();
            // Drop the iterator while retaining guards, including the last page.
            for (t, optional) in &mut guards {
                t.position.x = 4.0;
                if let Some(c) = optional {
                    c.0 = 9;
                    present += 1;
                }
                rows += 1;
            }
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                w.observe(&mut Observation::default());
            }))
            .is_err());
        } else {
            for (_, (t, optional)) in w
                .query::<(&mut Transform, Option<&mut CountComponent>)>()
                .iter()
            {
                t.position.x = 4.0;
                if let Some(c) = optional {
                    c.0 = 9;
                    present += 1;
                }
                rows += 1;
            }
        }
        assert_eq!(rows, entities.len());
        assert_eq!(present, entities.len().div_ceil(3));
        for (i, &e) in entities.iter().enumerate() {
            assert_eq!(w.get::<Transform>(e).unwrap().position.x, 4.0);
            assert_eq!(
                w.get::<CountComponent>(e).map(|c| c.0),
                (i % 3 == 0).then_some(9)
            );
        }
        check(&mut w, &before, true);
    }
}
#[derive(Default, crate::Component)]
struct CountComponent(u32);

struct Moving;
impl crate::Game for Moving {
    type Args = ();
    const ID: &'static str = "observation-invalidation";
    fn setup(w: &mut World, _: &()) {
        let e = w.spawn_named("target", Transform::default());
        w.spawn_named(
            "camera",
            (Transform::default(), Follow::new(e).offset(0.0, 1.0, 4.0)),
        );
    }
    fn tick(w: &mut World, _: &crate::Input, _: &()) {
        w.get_mut::<Transform>("target").unwrap().position.x += 1.0;
        crate::scene::follow(w);
    }
}

#[test]
fn sim_restore_carry_and_paranoid_table() {
    for mode in [
        crate::Paranoid::Off,
        crate::Paranoid::Save,
        crate::Paranoid::FreshGame,
    ] {
        for carry in [false, true] {
            let mut sim = crate::Sim::<Moving>::new(()).unwrap().paranoid(mode);
            let mut before = Observation::default();
            sim.world().observe(&mut before);
            let id = sim.world().id();
            sim.run(100.0);
            assert_eq!(sim.world().tick(), 6);
            assert_eq!(
                sim.world().get::<Transform>("target").unwrap().position.x,
                6.0
            );
            if mode != crate::Paranoid::Off {
                assert_ne!(sim.world().id(), id);
            }
            check(sim.world_mut(), &before, true);
            let save = sim.save();
            sim.run(100.0);
            assert_eq!(
                sim.world().get::<Transform>("target").unwrap().position.x,
                12.0
            );
            sim.world().observe(&mut before);
            let id = sim.world().id();
            if carry {
                sim.restore_bound(&save).unwrap();
            } else {
                sim.restore(&save).unwrap();
            }
            assert_ne!(sim.world().id(), id);
            assert_eq!(sim.world().tick(), 6);
            assert_eq!(
                sim.world().get::<Transform>("target").unwrap().position.x,
                6.0
            );
            assert_eq!(sim.save(), save);
            check(sim.world_mut(), &before, true);
        }
    }
}
