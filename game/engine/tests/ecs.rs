use exact_game::{Component, Entity, Parent, Rng, Value, World};
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
    let first = w.spawn_named("fox", (A(1),));
    let second = w.spawn_named("fox", (A(2),));
    let third = w.spawn((A(3),));
    assert_eq!(w.named("fox"), Some(first));
    assert_eq!(w.resolve(&format!("fox#{}", second.index())), Some(second));
    assert_eq!(w.resolve(&format!("#{}", third.index())), Some(third));
    assert_eq!(w.resolve(&format!("fox#{}", third.index())), None);
    w.despawn(second);
    w.despawn(first);
    let recycled = w.spawn_named("fox", (A(4),));
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
            entities.push(w.spawn((A(rng.next_u32()),)));
        }
        let got: Vec<_> = w.query::<&A>().map(|(e, _)| e.index()).collect();
        assert!(got.windows(2).all(|p| p[0] < p[1]));
        let mut expected: Vec<_> = entities.iter().map(|e| e.index()).collect();
        expected.sort();
        assert_eq!(got, expected);
    }
    let mut restored = World::new(1, 0);
    restored.register::<A>();
    restored.load(&w.save()).unwrap();
    let rows = |w: &World| w.query::<&A>().map(|(e, a)| (e, a.0)).collect::<Vec<_>>();
    assert_eq!(rows(&w), rows(&restored));
    assert_eq!(w.hash(), restored.hash());
}
#[test]
fn joins_option_filters_and_nested_reads() {
    let mut w = World::new(60, 1);
    let both = w.spawn((A(10), B(20)));
    let just_a = w.spawn((A(30),));
    let empty = w.spawn(());
    assert_eq!(
        w.query::<(&A, &B)>().map(|(e, _)| e).collect::<Vec<_>>(),
        [both]
    );
    assert_eq!(
        w.query::<&A>()
            .without::<B>()
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [just_a]
    );
    assert_eq!(w.query::<&A>().with::<B>().count(), 1);
    assert_eq!(
        w.query::<Option<&A>>()
            .map(|(e, a)| (e, a.is_some()))
            .collect::<Vec<_>>(),
        [(both, true), (just_a, true), (empty, false)]
    );
    for (_, (mut a, b)) in w.query::<(&mut A, Option<&B>)>() {
        a.0 += b.map_or(0, |b| b.0);
        assert_eq!(w.query::<&B>().count(), 1);
        assert_eq!(w.get::<B>(both).unwrap().0, 20);
        w.log("read inside query");
    }
    assert_eq!(w.get::<A>(both).unwrap().0, 30);
    for (_, b) in w.query::<Option<&mut B>>() {
        if let Some(mut b) = b {
            b.0 += 1;
        }
    }
    assert_eq!(w.get::<B>(both).unwrap().0, 21);
    assert_eq!(w.query::<&C>().count(), 0);
    assert_eq!(w.query::<(&A, Option<&C>)>().count(), 2);
}
#[test]
fn borrows_name_conflicts_and_survive_iterator_drop() {
    let mut w = World::new(60, 0);
    let e = w.spawn((A(1), B(2)));
    let a = w.get_mut::<A>(e).unwrap();
    assert!(panic_text(|| {
        w.query::<&A>();
    })
    .contains("A is already borrowed mutably"));
    assert!(panic_text(|| {
        w.get_mut::<A>(e);
    })
    .contains("A"));
    drop(a);
    let a = w.get::<A>(e).unwrap();
    assert!(panic_text(|| {
        w.query::<&mut A>();
    })
    .contains("A is already borrowed immutably"));
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
    let (_, mut kept) = q.next().unwrap();
    drop(q);
    assert!(panic_text(|| {
        w.get::<A>(e);
    })
    .contains("A"));
    kept.0 = 40;
    drop(kept);
    assert_eq!(w.get::<A>(e).unwrap().0, 40);
}
#[test]
fn resource_and_non_state_outputs() {
    let mut w = World::new(120, 37);
    assert!(panic_text(|| {
        w.resource::<A>();
    })
    .contains("A is absent"));
    w.insert_resource(A(7));
    w.resource_mut::<A>().0 += 1;
    assert_eq!(w.resource::<A>().0, 8);
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
    let mut w = World::new(60, 0);
    let child = w.spawn((A(1),));
    let root = w.spawn(());
    let leaf = w.spawn((Parent(child),));
    w.insert(child, Parent(root));
    assert_eq!(w.children(root), [child]);
    assert!(w.despawn(root));
    assert!(!w.contains(leaf));
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
    let mut rows: Vec<_> = w.query::<(&mut A, &B, &C, &D, &E, &F, &G, &H)>().collect();
    for (_, (a, b, ..)) in &mut rows {
        a.0 += b.0;
    }
    assert_eq!(rows[9].1 .0 .0, 18);
}
