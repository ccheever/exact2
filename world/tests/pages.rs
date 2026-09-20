use exact_world::{Component, World, PAGE};
#[derive(Default, Component)]
struct Item(f32);
#[test]
fn write_generations_cover_mutable_rows_without_marking_other_pages() {
    use exact_world::Component;
    #[derive(Default, Component)]
    struct Selected;
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..PAGE * 3).map(|_| w.spawn(Item::default())).collect();
    let generations = |w: &World| {
        w.pages::<Item>()
            .iter()
            .map(|p| p.generation)
            .collect::<Vec<_>>()
    };
    let initial = generations(&w);
    let epoch = w.mutation_epoch();
    let lease = w.get_mut::<Item>(entities[PAGE + 7]).unwrap();
    drop(lease); // Handing out a row marks even a same-value/no assignment lease.
    assert_ne!(w.mutation_epoch(), epoch);
    let after = generations(&w);
    assert_eq!(after[0], initial[0]);
    assert_ne!(after[1], initial[1]);
    assert_eq!(after[2], initial[2]);
    w.insert(entities[PAGE * 2 + 9], Selected);
    for (_, t) in w.query::<&mut Item>().with::<Selected>().iter() {
        t.0 = 1.;
    }
    let filtered = generations(&w);
    assert_eq!(&filtered[..2], &after[..2]);
    assert_ne!(filtered[2], after[2]);
    for (_, mut t) in w.query::<(&Selected, Option<&mut Item>)>() {
        t.as_mut().unwrap().0 = 2.;
    }
    let owned = generations(&w);
    assert_eq!(&owned[..2], &filtered[..2]);
    assert_ne!(owned[2], filtered[2]);
    let mut query = w.query::<&mut Item>();
    query.iter().next().unwrap().1 .0 = 3.;
    drop(query);
    let partial = generations(&w);
    assert_ne!(partial[0], owned[0]);
    assert_eq!(&partial[1..], &owned[1..]);
    w.remove::<Item>(entities[PAGE + 7]);
    assert_ne!(generations(&w)[1], partial[1]);
    let saved = w.save();
    let hash = w.hash();
    let presentation = w.replacement();
    w.load(&saved).unwrap();
    assert_ne!(w.replacement(), presentation);
    assert_eq!(w.hash(), hash);
    assert!(generations(&w).iter().all(|g| *g != 0));
}

#[test]
fn membership_union_is_ordered_and_does_not_require_both_columns() {
    use exact_world::Component;
    #[derive(Default, Component)]
    struct A;
    #[derive(Default, Component)]
    struct B;
    let mut w = World::new(60, 0);
    let es: Vec<_> = (0..PAGE * 3).map(|_| w.spawn(())).collect();
    for i in [0, PAGE + 9, PAGE * 2] {
        w.insert(es[i], A);
    }
    for i in [17, PAGE + 9] {
        w.insert(es[i], B);
    }
    let found: Vec<_> = w
        .query::<(Option<&A>, Option<&B>)>()
        .with_any::<A, B>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    assert_eq!(found, [es[0], es[17], es[PAGE + 9], es[PAGE * 2]]);
}
