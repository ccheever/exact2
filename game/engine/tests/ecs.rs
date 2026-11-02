use exact_game::{Component, Entity, Parent, Resource, Rng, Value, World};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Default, Component)]
struct A(u32);
#[derive(Default, Component)]
struct B(u32);
#[derive(Default, Component)]
struct C;
fn panic_text(f: impl FnOnce()) -> String {
    let e = catch_unwind(AssertUnwindSafe(f)).expect_err("must panic");
    if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        e.downcast_ref::<&str>().unwrap().to_string()
    }
}
#[test]
fn generations_names_and_order_under_churn() {
    let mut w = World::new(60, 19);
    let identity = w.id();
    assert_ne!(World::new(60, 19).id(), identity);
    assert_eq!(w.named("fox"), None);
    let first = w.spawn_named("fox", (A(1),));
    let second = w.spawn_named("fox", (A(2),));
    let third = w.spawn((A(3),));
    assert_eq!(w.named("fox"), Some(first));
    assert_eq!(w.resolve(&format!("fox#{}", second.index())), Some(second));
    assert_eq!(w.resolve(&format!("#{}", third.index())), Some(third));
    assert_eq!(w.resolve(&format!("fox#{}", third.index())), None);
    w.despawn(second);
    assert_eq!(w.named("fox"), Some(first));
    w.despawn(first);
    assert_eq!(w.named("fox"), None);
    let recycled = w.spawn_named("fox", (A(4),));
    assert_eq!(w.named("fox"), Some(recycled));
    assert_eq!(recycled.index(), first.index());
    assert_eq!(recycled.generation(), first.generation() + 1);
    assert!(!w.contains(first));
    assert!(!w.has::<A>(first));
    assert!(!w.despawn(first));
    let mut rng = Rng::new(13);
    let mut entities = vec![recycled, third];
    for _ in 0..2000 {
        if !entities.is_empty() && rng.chance(0.45) {
            let i = rng.range(0..entities.len() as u32) as usize;
            w.despawn(entities.remove(i));
        } else {
            let name = format!("group-{}", rng.range(0..5));
            entities.push(w.spawn_named(name, (A(rng.next_u32()),)));
        }
        for group in 0..5 {
            let name = format!("group-{group}");
            let expected = w.entities().find(|&e| w.name(e) == Some(name.as_str()));
            assert_eq!(w.named(&name), expected);
        }
        let got: Vec<_> = w.query::<&A>().iter().map(|(e, _)| e.index()).collect();
        assert!(got.windows(2).all(|p| p[0] < p[1]));
        let mut expected: Vec<_> = entities.iter().map(|e| e.index()).collect();
        expected.sort();
        assert_eq!(got, expected);
    }
    let mut restored = World::new(1, 0);
    let before_restore = restored.id();
    restored.register::<A>();
    restored.load(&w.save()).unwrap();
    assert_eq!(w.id(), identity, "mutation retains world identity");
    assert_ne!(
        restored.id(),
        before_restore,
        "load replaces world identity"
    );
    assert_ne!(
        restored.id(),
        identity,
        "equal worlds have distinct identities"
    );
    let loaded = restored.id();
    assert!(restored.load(b"invalid save").is_err());
    assert_eq!(restored.id(), loaded, "failed load retains world identity");
    let rows = |w: &World| {
        w.query::<&A>()
            .iter()
            .map(|(e, a)| (e, a.0))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&w), rows(&restored));
    assert_eq!(w.hash(), restored.hash());
    for group in 0..5 {
        let name = format!("group-{group}");
        assert_eq!(w.named(&name), restored.named(&name));
    }
    drop(w);
    for _ in 0..1000 {
        assert_ne!(
            World::new(60, 19).id(),
            identity,
            "a retained identity never recycles"
        );
    }
}
#[test]
fn joins_option_filters_and_nested_reads() {
    let mut w = World::new(60, 1);
    let both = w.spawn((A(10), B(20)));
    let just_a = w.spawn((A(30),));
    let empty = w.spawn(());
    assert_eq!(
        w.query::<(&A, &B)>()
            .iter()
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [both]
    );
    assert_eq!(
        w.query::<&A>()
            .without::<B>()
            .iter()
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [just_a]
    );
    assert_eq!(w.query::<&A>().with::<B>().iter().count(), 1);
    assert_eq!(
        w.query::<Option<&A>>()
            .iter()
            .map(|(e, a)| (e, a.is_some()))
            .collect::<Vec<_>>(),
        [(both, true), (just_a, true), (empty, false)]
    );
    for (_, (a, b)) in w.query::<(&mut A, Option<&B>)>().iter() {
        a.0 += b.map_or(0, |b| b.0);
        assert_eq!(w.query::<&B>().iter().count(), 1);
        assert_eq!(w.get::<B>(both).unwrap().0, 20);
        w.log("read inside query");
    }
    assert_eq!(w.get::<A>(both).unwrap().0, 30);
    for (_, b) in w.query::<Option<&mut B>>().iter() {
        if let Some(b) = b {
            b.0 += 1;
        }
    }
    assert_eq!(w.get::<B>(both).unwrap().0, 21);
    assert_eq!(w.query::<&C>().iter().count(), 0);
    assert_eq!(w.query::<(&A, Option<&C>)>().iter().count(), 2);
}
#[test]
fn borrows_name_conflicts_and_survive_iterator_drop() {
    let mut w = World::new(60, 0);
    let e = w.spawn((A(1), B(2)));
    let a = w.get_mut::<A>(e).unwrap();
    // A query leases at its first iteration, after its filters are known.
    let unleased = w.query::<&A>();
    assert!(panic_text(|| {
        w.query::<&A>().iter().count();
    })
    .contains("borrow conflict on A of #0"));
    assert!(panic_text(|| {
        w.get_mut::<A>(e);
    })
    .contains("held by:   exclusive borrow of one row"));
    drop((a, unleased));
    let a = w.get::<A>(e).unwrap();
    assert!(panic_text(|| {
        w.query::<&mut A>().iter().count();
    })
    .contains("held by:   shared borrow of one row"));
    drop(a);
    assert!(panic_text(|| {
        w.query::<(&A, &A)>();
    })
    .contains("A occurs twice"));
    assert!(panic_text(|| {
        w.query::<(&mut A, Option<&A>)>();
    })
    .contains("A occurs twice"));
    assert!(panic_text(|| {
        w.query::<(Option<&C>, &C)>();
    })
    .contains("C occurs twice"));
    let mut q = w.query::<&mut A>();
    let kept = {
        let mut iter = q.iter();
        iter.next().unwrap().1
    };
    assert!(panic_text(|| {
        w.get::<A>(e);
    })
    .contains("A"));
    kept.0 = 40;
    drop(q);
    assert_eq!(w.get::<A>(e).unwrap().0, 40);
}
#[test]
fn resource_and_non_state_outputs() {
    #[derive(Default, Resource)]
    struct Score(u32);
    let mut w = World::new(120, 37);
    assert!(panic_text(|| {
        w.resource::<Score>();
    })
    .contains("Score is absent"));
    w.insert_resource(Score(7));
    w.resource_mut::<Score>().0 += 1;
    assert_eq!(w.resource::<Score>().0, 8);
    let before = w.hash();
    w.log("an event");
    w.publish("score", Value::Number(4.0));
    let n = w.journal().len();
    w.publish("score", Value::Number(4.0));
    assert_eq!(w.journal().len(), n);
    assert_eq!(before, w.hash());
    assert_eq!(w.published("score"), Some(Value::Number(4.0)));
    for i in 0..5000 {
        w.log(i);
    }
    assert_eq!(w.journal().len(), 4096);
    assert_eq!(w.hz(), 120);
    assert_eq!(w.tick(), 0);
    assert_eq!(w.seconds(), 0.0);
}
#[test]
fn hierarchy_despawn_and_live_generations() {
    use exact_game::Transform;
    let mut w = World::new(60, 0);
    let child = w.spawn((A(1), Transform::at(2.0, 0.0, 0.0)));
    let root = w.spawn(Transform::at(1.0, 0.0, 0.0));
    let leaf = w.spawn((Transform::at(3.0, 0.0, 0.0), Parent(child)));
    w.insert(child, Parent(root));
    let survivor = w.spawn(Transform::at(10.0, 0.0, 0.0));
    let survivor_child = w.spawn((Transform::at(2.0, 0.0, 0.0), Parent(survivor)));
    w.propagate();
    assert_eq!(w.global(leaf).unwrap().translation.x, 6.0);
    assert_eq!(w.children(root), [child]);
    let cursor = w.journal().len();
    assert!(w.despawn(root));
    assert!(w.contains(leaf));
    w.reap_orphans();
    assert!(!w.contains(leaf));
    assert_eq!(w.entities().collect::<Vec<_>>(), [survivor, survivor_child]);
    assert_eq!(
        w.journal()[cursor..]
            .iter()
            .map(|e| e.line.clone())
            .collect::<Vec<_>>(),
        [root, child, leaf].map(|e| format!("t=0 tick=0 despawn #{}", e.index()))
    );
    assert_eq!(w.global(survivor_child).unwrap().translation.x, 12.0);
    w.propagate();
    assert_eq!(w.global(survivor_child).unwrap().translation.x, 12.0);
    let replacement = w.spawn(Transform::at(20.0, 0.0, 0.0));
    assert_eq!(replacement.index(), child.index());
    assert!(!w.contains(child));
    let next = w.spawn((Transform::at(2.0, 0.0, 0.0), Parent(replacement)));
    w.propagate();
    assert_eq!(w.global(next).unwrap().translation.x, 22.0);
    w.despawn(replacement);
    w.reap_orphans();
    assert!(!w.contains(next));
    assert_eq!(w.global(survivor_child).unwrap().translation.x, 12.0);
    w.despawn(survivor);
    w.reap_orphans();
    assert!(w.is_empty());
    assert!(!w.contains(Entity::default()));
}

#[test]
fn eight_way_query_and_retained_mutable_rows() {
    #[derive(Default, Component)]
    struct D;
    #[derive(Default, Component)]
    struct E;
    #[derive(Default, Component)]
    struct F;
    #[derive(Default, Component)]
    struct G;
    #[derive(Default, Component)]
    struct H;
    let mut w = World::new(60, 0);
    for n in 0..10 {
        w.spawn((A(n), B(n), C, D, E, F, G, H));
    }
    let mut query = w.query::<(&mut A, &B, &C, &D, &E, &F, &G, &H)>();
    let mut rows: Vec<_> = query.iter().collect();
    for (_, (a, b, ..)) in &mut rows {
        a.0 += b.0;
    }
    assert_eq!(rows[9].1 .0 .0, 18);
}

#[test]
fn drops_exactly_present_slots_including_load_over_existing() {
    use std::cell::RefCell;
    thread_local! { static DROPS: RefCell<[u32; 8]> = const { RefCell::new([0; 8]) }; }
    #[derive(Default, Component)]
    struct Counted {
        id: u32,
        payload: String,
    }
    impl Drop for Counted {
        fn drop(&mut self) {
            if self.id != 0 {
                DROPS.with_borrow_mut(|counts| counts[self.id as usize] += 1);
            }
        }
    }
    let value = |id| Counted {
        id,
        payload: format!("owned {id}"),
    };
    let count = |id: usize| DROPS.with_borrow(|counts| counts[id]);
    let mut w = World::new(60, 0);
    let e = w.spawn((value(1),));
    assert_eq!(count(1), 0);
    w.insert(e, value(2));
    assert_eq!(count(1), 1);
    let removed = w.remove::<Counted>(e).unwrap();
    assert_eq!(count(2), 0);
    assert_eq!(w.pages::<Counted>().iter().count(), 0);
    drop(removed);
    assert_eq!(count(2), 1);
    w.insert(e, value(3));
    w.despawn(e);
    assert_eq!(count(3), 1);
    let first = w.spawn((value(4),));
    for _ in 0..exact_game::PAGE {
        w.spawn(());
    }
    let last = w.spawn((value(5),));
    assert_eq!(w.pages::<Counted>().iter().count(), 2);
    assert!(w.remove::<Counted>(e).is_none()); // stale incarnation cannot remove #4
    assert!(w.get::<Counted>(e).is_none());
    assert!(w.get_mut::<Counted>(e).is_none());
    assert!(w.has::<Counted>(first));
    let bytes = w.save();
    let mut loaded = World::new(60, 0);
    loaded.spawn((value(6),));
    loaded.load(&bytes).unwrap();
    assert_eq!(count(6), 1);
    assert_eq!(loaded.get::<Counted>(last).unwrap().payload, "owned 5");
    assert_eq!(loaded.save(), bytes);
    drop(loaded);
    assert_eq!(count(4), 1);
    assert_eq!(count(5), 1);
    drop(w);
    assert_eq!(
        DROPS.with_borrow(|counts| *counts),
        [0, 1, 1, 1, 2, 2, 1, 0]
    );
}

#[test]
fn mask_joins_across_words_pages_and_optional_only_queries() {
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..3 * exact_game::PAGE + 19)
        .map(|_| w.spawn(()))
        .collect();
    for &e in &entities {
        let i = e.index();
        if i % 3 == 0 {
            w.insert(e, A(i));
        }
        if i % 5 == 0 {
            w.insert(e, B(i));
        }
        if i % 7 == 0 {
            w.insert(e, C);
        }
    }
    for &e in entities.iter().step_by(11) {
        w.despawn(e);
    }
    let expected: Vec<_> = entities
        .iter()
        .copied()
        .filter(|&e| {
            w.contains(e) && e.index() % 3 == 0 && e.index() % 5 == 0 && e.index() % 7 != 0
        })
        .collect();
    let got: Vec<_> = w
        .query::<(&mut A, Option<&B>)>()
        .with::<B>()
        .without::<C>()
        .iter()
        .map(|(e, (a, b))| {
            a.0 += b.unwrap().0;
            e
        })
        .collect();
    assert_eq!(got, expected);
    assert_eq!(
        w.query::<(&A, &B)>()
            .without::<C>()
            .iter()
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        expected
    );
    for (e, (a, b)) in &mut w.query::<(Option<&A>, Option<&B>)>() {
        assert!(w.contains(e));
        assert_eq!(a.is_some(), e.index() % 3 == 0);
        assert_eq!(b.is_some(), e.index() % 5 == 0);
    }
    assert_eq!(w.query::<Option<&A>>().iter().count(), w.len());
    assert_eq!(w.query::<&A>().with::<A>().without::<A>().iter().count(), 0);
    let mut query = w.query::<&mut C>();
    let rows: Vec<_> = query.iter().collect(); // simultaneous ZST mutable references
    assert_eq!(
        rows.len(),
        entities
            .iter()
            .filter(|e| e.index() % 7 == 0 && e.index() % 11 != 0)
            .count()
    );
    drop(rows);
    assert_eq!(
        query.iter().count(),
        w.query::<Option<&A>>().with::<C>().iter().count()
    );
}

#[test]
fn large_churn_has_history_independent_order_hash_and_save() {
    // Miri still crosses multiple pages; native runs the requested 100k world.
    let n: usize = if cfg!(miri) { 2_100 } else { 100_000 };
    let batch: usize = if cfg!(miri) { 32 } else { 2_000 };
    let mut left = World::new(120, 42);
    let mut right = World::new(120, 42);
    let mut entities = Vec::new();
    for i in 0..n {
        entities.push(left.spawn((A(i as u32), B(i as u32))));
        assert_eq!(right.spawn(()), entities[i]);
    }
    // Same state, opposite component insertion history.
    for &e in entities.iter().rev() {
        right.insert(e, B(e.index()));
        right.insert(e, A(e.index()));
    }
    for tick in 0..4 {
        let mut selected: Vec<_> = (0..batch).map(|i| (i * 47 + tick * 131) % n).collect();
        selected.sort_unstable();
        selected.dedup();
        for &i in &selected {
            left.despawn(entities[i]);
        }
        for &i in selected.iter().rev() {
            right.despawn(entities[i]);
        }
        for &i in &selected {
            let a = left.spawn(());
            let b = right.spawn(());
            assert_eq!(a, b);
            assert_eq!(a.index() as usize, i); // lowest free index first
            left.insert(a, A(a.index()));
            left.insert(a, B(a.index()));
            right.insert(b, B(b.index()));
            right.insert(b, A(b.index()));
            entities[i] = a;
        }
        let order: Vec<_> = left
            .query::<(&A, &B)>()
            .iter()
            .map(|(e, _)| e.index())
            .collect();
        assert_eq!(order.len(), n);
        assert!(order.windows(2).all(|w| w[0] < w[1]));
    }
    assert_eq!(left.hash(), right.hash());
    assert_eq!(left.save(), right.save());
}
