use exact_world::{Component, World, PAGE};
#[derive(Default, Component)]
struct Item(f32);
#[test]
fn write_generations_cover_mutable_rows_without_marking_other_pages() {
    use exact_world::Component;
    #[derive(Default, Component)]
    struct Selected;
    let mut w = World::new(60, 0);
    w.register::<Item>().unwrap();
    w.register::<Selected>().unwrap();
    let entities: Vec<_> = (0..PAGE * 3)
        .map(|_| w.spawn(Item::default()).unwrap())
        .collect();
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
    w.insert(entities[PAGE * 2 + 9], Selected).unwrap();
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
    let saved = w.save().unwrap();
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
    w.register::<Item>().unwrap();
    w.register::<A>().unwrap();
    w.register::<B>().unwrap();
    let es: Vec<_> = (0..PAGE * 3).map(|_| w.spawn(()).unwrap()).collect();
    for i in [0, PAGE + 9, PAGE * 2] {
        w.insert(es[i], A).unwrap();
    }
    for i in [17, PAGE + 9] {
        w.insert(es[i], B).unwrap();
    }
    let found: Vec<_> = w
        .query::<(Option<&A>, Option<&B>)>()
        .with_any::<A, B>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    assert_eq!(found, [es[0], es[17], es[PAGE + 9], es[PAGE * 2]]);
}

// Safety audit (Miri-style reasoning; Miri is unavailable on this builder):
// Page's only public fields, first/generation, are reporting metadata. slots,
// mask, PhantomData, leases, descriptors and every constructor used to obtain
// references are private or crate-private. Query is sealed; its public raw fetch
// entry points are unsafe. The compile-refusal harness locks mask and lifetimes.
// ZST references cover zero bytes, so equal aligned nonnull addresses do not
// imply overlapping memory. Safe std slice::IterMut has the same property.
// Presence still owns one Drop per logical slot; leases still forbid reborrows.
#[test]
fn zst_rows_retain_leases_after_iterator_drop_and_unwind() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    thread_local! { static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    #[repr(align(128))]
    #[derive(Default, Component)]
    struct Flag;
    impl Drop for Flag {
        fn drop(&mut self) {
            DROPS.set(DROPS.get() + 1);
        }
    }
    let mut plain = [Flag, Flag];
    let refs: Vec<_> = plain.iter_mut().collect();
    assert!(std::ptr::eq(refs[0], refs[1]), "safe std ZST control");
    let mut w = World::new(60, 0);
    w.register::<Item>().unwrap();
    w.register::<Flag>().unwrap();
    let es: Vec<_> = (0..PAGE * 3 + 1).map(|_| w.spawn(Flag).unwrap()).collect();
    let mut rows = w.query::<Option<&mut Flag>>().into_iter();
    let mut first = rows.next().unwrap().unwrap();
    let last = rows.last().unwrap().unwrap(); // iterator has now dropped
    assert_eq!((&*first as *const Flag as usize) % 128, 0);
    assert!(w.try_query::<&Flag>().is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _borrow = &mut *first;
        panic!("unwind with a retained row");
    }))
    .is_err());
    drop(first);
    assert!(w.try_query::<&mut Flag>().is_err());
    drop(last);
    assert_eq!(w.query::<&mut Flag>().iter().count(), es.len());
    DROPS.set(0);
    for &e in &es {
        assert!(w.despawn(e));
    }
    assert_eq!(DROPS.get(), es.len());
    let reused = w.spawn(Flag).unwrap();
    assert_eq!(reused.index(), es[0].index());
    assert_ne!(reused, es[0]);
    drop(w);
    assert_eq!(DROPS.get(), es.len() + 1);
}

#[test]
fn panicking_writers_and_row_bodies_release_leases_before_reuse() {
    use exact_world::{Data, DataError, Reader, Writer};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    thread_local! { static PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(true) }; }
    #[derive(Default)]
    struct Owned(String);
    impl Data for Owned {
        fn write(&self, w: &mut dyn Writer) {
            assert!(!PANIC.get(), "writer panic");
            self.0.write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            self.0.read(r)
        }
    }
    impl Component for Owned {
        const NAME: &'static str = "Owned";
    }
    let mut w = World::new(60, 0);
    w.register::<Owned>().unwrap();
    let e = w.spawn(Owned("first".into())).unwrap();
    // A partial writer never owns the slot: the shared lease unwinds while the
    // initialized String remains owned by its presence bit (Miri-style audit).
    assert!(catch_unwind(AssertUnwindSafe(|| w.save())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut rows = w.query::<&mut Owned>().into_iter();
        rows.next().unwrap().0.push_str(" changed");
        panic!("row body");
    }))
    .is_err());
    PANIC.set(false);
    assert_eq!(w.get::<Owned>(e).unwrap().0, "first changed");
    let bytes = w.save().unwrap();
    w.load(&bytes).unwrap();
    w.despawn(e);
    let reused = w.spawn(Owned("second".into())).unwrap();
    assert_eq!(reused.index(), e.index());
    assert_ne!(reused, e);
    assert_eq!(w.get::<Owned>(reused).unwrap().0, "second");
}
