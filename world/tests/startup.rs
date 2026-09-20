#[path = "../src/storage/counting.rs"]
mod counting;
use exact_world::*;
#[derive(Default, Component)]
struct CellValue {
    number: u64,
    payload: [u64; 3],
}
#[derive(Default, Resource)]
struct Blob(Vec<u8>);
struct Board<const N: u64>;
impl<const N: u64> Game for Board<N> {
    const ID: &'static str = "startup-board";
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<CellValue>().unwrap();
        w.register_resource::<Blob>().unwrap();
        Ok(())
    }
    fn setup(w: &mut World, _: &()) {
        for number in 0..N {
            w.spawn(CellValue {
                number,
                payload: [number; 3],
            })
            .unwrap();
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        for (_, value) in w.query::<&mut CellValue>().iter() {
            value.number += 1;
        }
        let n = w.rng().next_u32();
        w.publish("tick", n);
    }
}
fn report(label: &str, actual: (usize, usize), ceiling: (usize, usize)) {
    println!(
        "{label}: {} allocations, {} requested bytes; ceilings {} / {}",
        actual.0, actual.1, ceiling.0, ceiling.1
    );
    assert!(
        actual.0 <= ceiling.0 && actual.1 <= ceiling.1,
        "{label}: {actual:?} exceeds {ceiling:?}"
    );
}
#[test]
fn construction_activation_restore_counts() {
    let (empty, counts) = counting::measure(|| World::new(60, 0));
    report("construction.empty", counts, (1, 24));
    assert!(empty.is_empty());
    let (mut sim, counts) = counting::measure(|| Sim::<Board<100>>::new(()).unwrap());
    report("construction.100", counts, (21, 42400));
    assert_eq!(sim.world().len(), 100);
    let (ticks, counts) = counting::measure(|| sim.run(17.).unwrap());
    report("activation.first_tick", counts, (3, 640));
    assert_eq!(ticks, 1);
    assert_eq!(sim.world().get::<CellValue>("#99").unwrap().number, 100);
    let (ticks, counts) = counting::measure(|| sim.run(1_000_000. / 60.).unwrap());
    report("live.1000_publishing_ticks", counts, (1003, 204704));
    assert_eq!(ticks, 1000);
    let mut sim = Sim::<Board<32>>::new(()).unwrap();
    sim.run(17.).unwrap();
    sim.world_mut().insert_resource(Blob::default()).unwrap();
    let mut size = 0usize;
    for _ in 0..4 {
        let len = sim.save().unwrap().len();
        if len == 10240 {
            break;
        }
        size = (size as isize + 10240 - len as isize) as usize;
        sim.world_mut()
            .insert_resource(Blob(vec![7; size]))
            .unwrap();
    }
    let bytes = sim.save().unwrap();
    assert_eq!(bytes.len(), 10240);
    let (restored, counts, histogram) =
        counting::histogram(|| Sim::<Board<32>>::from_save(&bytes).unwrap());
    println!("restore allocation sizes (bytes, calls): {histogram:?}");
    report("restore.10KiB", counts, (80, 47297));
    assert_eq!(restored.world().hash(), sim.world().hash());
    assert_eq!(restored.save().unwrap(), bytes);
    let (_, again) = counting::measure(|| Sim::<Board<32>>::from_save(&bytes).unwrap());
    assert_eq!(again, counts, "counts are deterministic, not time samples");
}

#[test]
fn first_component_at_high_slot_allocates_one_page_not_world_high_water() {
    let mut w = World::new(60, 0);
    w.register::<CellValue>().unwrap();
    for _ in 0..MAX_ENTITIES {
        w.spawn(()).unwrap();
    }
    let e = w.resolve("#199999").unwrap();
    let (_, counts) = counting::measure(|| w.insert(e, CellValue::default()).unwrap());
    report("insert.slot199999", counts, (8, 8192));
    assert_eq!(
        w.query::<&CellValue>()
            .iter()
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [e]
    );
    assert_eq!(w.pages::<CellValue>().iter().count(), 1);
    w.remove::<CellValue>(e);
    assert_eq!(w.pages::<CellValue>().iter().count(), 0);
}

#[test]
fn zero_budget_refuses_before_constructing_boxed_defaults() {
    thread_local! { static DEFAULTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    struct Large([u64; 1024]);
    impl Default for Large {
        fn default() -> Self {
            DEFAULTS.set(DEFAULTS.get() + 1);
            Self([0; 1024])
        }
    }
    impl Data for Large {
        fn write(&self, w: &mut dyn Writer) {
            self.0[0].write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            self.0[0].read(r)
        }
    }
    DEFAULTS.set(0);
    let (_, counts) = counting::measure(|| {
        assert!(bin::from_slice_in::<Box<Large>>(&[], Some(&data::LoadBudget::new(0))).is_err());
    });
    assert_eq!(
        DEFAULTS.get(),
        0,
        "no allocation-producing Default before admission"
    );
    assert!(
        counts.1 < 8192,
        "box allocation happened before refusal: {counts:?}"
    );
}

#[test]
fn bounded_encoder_stops_before_payload_allocation_and_element_traversal() {
    let values = vec![0u32; 1_000_000];
    let (_, counts) = counting::measure(|| {
        let mut encoder = bin::Encoder::bounded(64);
        values.write(&mut encoder);
        assert!(encoder.finish().is_err());
    });
    assert!(
        counts.1 < 1024,
        "bulk preflight allocated payload: {counts:?}"
    );
    let mut encoder = bin::Encoder::bounded(64);
    vec![42u32].write(&mut encoder);
    assert_eq!(
        bin::from_slice::<Vec<u32>>(&encoder.finish().unwrap()).unwrap(),
        [42]
    );
}

#[test]
fn nested_box_default_is_preflighted_before_allocating() {
    thread_local! { static DEFAULTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    struct Large([u64; 1024]);
    impl Default for Large {
        fn default() -> Self {
            DEFAULTS.set(DEFAULTS.get() + 1);
            Self([0; 1024])
        }
    }
    impl Data for Large {
        fn write(&self, w: &mut dyn Writer) {
            self.0[0].write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            self.0[0].read(r)
        }
    }
    #[derive(Default, Data)]
    struct Outer {
        inner: Box<Large>,
    }
    DEFAULTS.set(0);
    assert!(bin::from_slice_in::<Outer>(&[], Some(&data::LoadBudget::new(1024))).is_err());
    assert_eq!(DEFAULTS.get(), 0);
}

#[test]
fn omitted_box_fields_and_container_resets_claim_defaults_before_allocation() {
    thread_local! { static DEFAULTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    struct Large([u64; 1024]);
    impl Default for Large {
        fn default() -> Self {
            DEFAULTS.set(DEFAULTS.get() + 1);
            Self([0; 1024])
        }
    }
    impl Data for Large {
        fn write(&self, w: &mut dyn Writer) {
            self.0[0].write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            self.0[0].read(r)
        }
    }
    #[derive(Default, Data)]
    struct Record {
        boxed: Box<Large>,
    }
    // Hostile patch records omit the allocating field. Charging only Box::read
    // never sees it, so the outer construction must charge the recursive default.
    let empty_records = |n| {
        let mut w = bin::Encoder::default();
        w.begin_seq(n);
        for _ in 0..n {
            w.item();
            w.begin_struct();
            w.end_struct();
        }
        w.end_seq();
        w.finish().unwrap()
    };
    DEFAULTS.set(0);
    assert!(bin::from_slice_in::<Vec<Record>>(
        &empty_records(10),
        Some(&data::LoadBudget::new(10000))
    )
    .is_err());
    assert!(
        DEFAULTS.get() <= 1,
        "constructed {} large defaults",
        DEFAULTS.get()
    );
    DEFAULTS.set(0);
    let decoded =
        bin::from_slice_in::<Vec<Record>>(&empty_records(2), Some(&data::LoadBudget::new(30000)))
            .unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(DEFAULTS.get(), 2); // negative control: admitted defaults are constructed
    let mut array = [Box::<Large>::default()];
    let bytes = bin::to_vec(&Vec::<()>::new()).unwrap();
    DEFAULTS.set(0);
    let mut r = bin::Decoder::for_load(&bytes, Some(&data::LoadBudget::new(1024)));
    assert!(array.read(&mut r).is_err());
    assert_eq!(DEFAULTS.get(), 0, "array reset allocated before admission");
}

#[test]
fn live_ticks_never_visit_components_and_settle_samples_each_boundary_once() {
    thread_local! { static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    #[derive(Default)]
    struct Counted(u32);
    impl Component for Counted {
        const NAME: &'static str = "Counted";
    }
    impl Data for Counted {
        fn write(&self, w: &mut dyn Writer) {
            VISITS.set(VISITS.get() + 1);
            self.0.write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            self.0.read(r)
        }
    }
    struct Live;
    impl Game for Live {
        type Args = ();
        const ID: &'static str = "live-counted";
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<Counted>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            for i in 0..200_000 {
                w.spawn(Counted(i)).unwrap();
            }
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            // Sparse touching in a maximal world, with a controller still active.
            if w.tick() < 1002 {
                w.get_mut::<Counted>("#199999").unwrap().0 += 1;
            }
        }
    }
    let mut sim = Sim::<Live>::new(()).unwrap();
    VISITS.set(0);
    let (ticks, allocations) = counting::measure(|| sim.run(1_000_000. / 60.).unwrap());
    assert_eq!(ticks, 1000);
    report(
        "live.1000_sparse_touches_200k_entities",
        allocations,
        (0, 0),
    );
    assert_eq!(VISITS.get(), 0);
    assert_eq!(sim.world().observation(), None);
    assert!(!sim.world().quiescent());
    assert_eq!(sim.settle(4).unwrap(), 3);
    assert_eq!(
        VISITS.get(),
        4 * 200_000,
        "initial boundary plus three ticks"
    );
    assert_eq!(sim.world().observation(), Some(true));
    sim.run(17.).unwrap();
    assert_eq!(sim.world().observation(), None);
    assert_eq!(VISITS.get(), 800_000);
}

#[test]
fn unchanged_ownership_reuses_scratch_and_churn_reaps_reverse_chains() {
    let mut w = World::new(60, 0);
    let child = w.spawn(()).unwrap();
    let middle = w.spawn(()).unwrap();
    for _ in 2..200_000 {
        w.spawn(()).unwrap();
    }
    let root = w.resolve("#199999").unwrap();
    w.set_parent(child, Some(middle)).unwrap();
    w.set_parent(middle, Some(root)).unwrap();
    w.reap_orphans().unwrap();
    let (_, counts) = counting::measure(|| {
        for _ in 0..1000 {
            w.reap_orphans().unwrap();
        }
    });
    assert_eq!(counts, (0, 0));
    w.despawn(root);
    w.spawn(()).unwrap(); // Recycled slot must not rescue descendants.
    w.reap_orphans().unwrap();
    assert!(!w.contains(child));
    assert!(!w.contains(middle));
    let fresh = w.spawn(()).unwrap();
    w.set_parent(fresh, Some(w.resolve("#199999").unwrap()))
        .unwrap();
    let (_, counts) = counting::measure(|| w.reap_orphans().unwrap());
    assert_eq!(
        counts,
        (0, 0),
        "changed ownership also reuses its high-water scratch"
    );
}

#[test]
fn typed_bulk_hash_and_inspection_never_allocate_conversion_payloads() {
    let large = vec![u32::MAX; 1_000_000];
    let (hash, counts) = counting::measure(|| exact_world::hash::of(&large));
    assert_eq!(counts, (0, 0));
    assert_ne!(hash, exact_world::hash::of(&vec![0u32; large.len()]));
    let small = vec![0x1234u16; 32_768];
    let (json, counts) = counting::measure(|| json::to_string(&small).unwrap());
    assert!(json.contains("65536"));
    assert!(
        counts.1 < 1024,
        "inspection allocated a conversion payload: {counts:?}"
    );
    let (_, counts) = counting::measure(|| assert!(json::to_string(&large).is_err()));
    assert!(counts.1 < 1024);
}

#[test]
fn inspection_refusal_stops_nested_traversal_and_later_growth() {
    thread_local! { static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    #[derive(Default)]
    struct Item;
    impl Data for Item {
        fn write(&self, w: &mut dyn Writer) {
            VISITS.set(VISITS.get() + 1);
            w.string("0123456789");
        }
        fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
            Ok(())
        }
    }
    let values: Vec<Vec<Item>> = (0..1000)
        .map(|_| (0..1000).map(|_| Item).collect())
        .collect();
    VISITS.set(0);
    assert!(json::to_string(&values).is_err());
    assert!(
        VISITS.get() > 0 && VISITS.get() < 6000,
        "visits {}",
        VISITS.get()
    );
    let mut w = json::Encoder::default();
    w.begin_seq(json::LIMIT + 1);
    assert!(w.stopped());
    let (_, counts) = counting::measure(|| values.write(&mut w));
    assert_eq!(counts, (0, 0));
    assert!(w.finish().is_err());
    assert_eq!(json::to_string(&vec![Item]).unwrap(), "[\"0123456789\"]");
    #[derive(Default, Data)]
    struct Record {
        values: Vec<u32>,
    }
    let record = Record {
        values: vec![0; 1_000_000],
    };
    let world = World::new(60, 0);
    let (_, counts) = counting::measure(|| {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || world.publish_record(&record)
        ))
        .is_err());
    });
    assert!(
        counts.1 < 4096,
        "publication allocated before refusal: {counts:?}"
    );
}

#[test]
fn unchanged_work_and_publications_do_not_allocate_or_invalidate() {
    let w = World::new(60, 0);
    w.work("assets", Work::Pending).unwrap();
    w.publish("score", 7u32);
    let before = (w.mutation_epoch(), w.journal_next());
    let (_, counts) = counting::measure(|| {
        for _ in 0..1000 {
            w.work("assets", Work::Pending).unwrap();
            w.publish("score", 7u32);
        }
    });
    assert_eq!(counts, (0, 0));
    assert_eq!(before, (w.mutation_epoch(), w.journal_next()));
    w.work("assets", Work::Ready).unwrap();
    w.publish("score", 8u32);
    assert_ne!(before, (w.mutation_epoch(), w.journal_next()));
}
