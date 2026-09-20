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
    report("construction.100", counts, (28, 70848));
    assert_eq!(sim.world().len(), 100);
    let (ticks, counts) = counting::measure(|| sim.run(17.).unwrap());
    report("activation.first_tick", counts, (3, 640));
    assert_eq!(ticks, 1);
    assert_eq!(sim.world().get::<CellValue>("#99").unwrap().number, 100);
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
    let (restored, counts) = counting::measure(|| Sim::<Board<32>>::from_save(&bytes).unwrap());
    report("restore.10KiB", counts, (1038, 106671));
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
