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
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) {
        w.register::<CellValue>();
        w.register_resource::<Blob>();
    }
    fn setup(w: &mut World, _: &()) {
        for number in 0..N {
            w.spawn(CellValue {
                number,
                payload: [number; 3],
            });
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
    assert_eq!(sim.world().require::<CellValue>("#99").number, 100);
    let mut sim = Sim::<Board<32>>::new(()).unwrap();
    sim.run(17.).unwrap();
    sim.world_mut().insert_resource(Blob::default());
    let mut size = 0usize;
    for _ in 0..4 {
        let len = sim.save().unwrap().len();
        if len == 10240 {
            break;
        }
        size = (size as isize + 10240 - len as isize) as usize;
        sim.world_mut().insert_resource(Blob(vec![7; size]));
    }
    let bytes = sim.save().unwrap();
    assert_eq!(bytes.len(), 10240);
    let (restored, counts) = counting::measure(|| Sim::<Board<32>>::from_save(&bytes).unwrap());
    report("restore.10KiB", counts, (997, 78262));
    assert_eq!(restored.world().hash(), sim.world().hash());
    assert_eq!(restored.save().unwrap(), bytes);
    let (_, again) = counting::measure(|| Sim::<Board<32>>::from_save(&bytes).unwrap());
    assert_eq!(again, counts, "counts are deterministic, not time samples");
}

#[test]
fn first_component_at_high_slot_allocates_one_page_not_world_high_water() {
    let mut w = World::new(60, 0);
    w.register::<CellValue>();
    for _ in 0..MAX_ENTITIES {
        w.spawn(());
    }
    let e = w.resolve("#199999").unwrap();
    let (_, counts) = counting::measure(|| w.insert(e, CellValue::default()));
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
