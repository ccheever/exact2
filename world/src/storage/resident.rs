#![cfg(test)]
//! Untrusted input is prepared outside the measurement. The live destination is
//! retained until success, so the allocator's net high-water mark measures the candidate.
use crate::data::{LoadBudget, MAX_LOAD_BYTES};
use crate::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
fn measured<T>(
    label: &str,
    budget: usize,
    f: impl FnOnce() -> Result<T, DataError>,
) -> Result<T, DataError> {
    let (result, peak, (_, total)) = crate::counting::peak(|| catch_unwind(AssertUnwindSafe(f)));
    assert!(result.is_ok(), "{label}: panic");
    println!("{label}: budget={budget} peak={peak} cumulative={total}");
    assert!(
        peak <= budget + 8192,
        "{label}: peak {peak} > {budget} + 8192"
    );
    result.unwrap()
}
fn decode<T: Data>(label: &str, bytes: &[u8], budget: usize) -> Result<T, DataError> {
    let allowance = LoadBudget::new(budget);
    measured(label, budget, || {
        bin::from_slice_in::<T>(bytes, Some(&allowance))
    })
}
#[test]
fn huge_count_tiny_input_and_huge_string_length() {
    // u64::MAX varint: no payload can justify reservation.
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let mut count = vec![7];
        count.extend([255; 9]);
        count.push(1);
        assert!(decode::<Vec<String>>("huge count", &count, budget).is_err());
        count[0] = 6;
        assert!(decode::<String>("huge string", &count, budget).is_err());
        let positive = bin::to_vec(&"kept".to_owned()).unwrap();
        assert_eq!(
            decode::<String>("string positive", &positive, budget).unwrap(),
            "kept"
        );
    }
}
#[allow(clippy::large_enum_variant)]
#[derive(Default, Data)]
enum Wide {
    #[default]
    Empty,
    Full([[u8; 32]; 32]),
}
#[repr(align(4096))]
#[derive(Default)]
struct Padded(bool);
impl Data for Padded {
    fn default_size() -> usize {
        17 // Preserve the original derived portable declaration and wire shape.
    }
    fn write(&self, w: &mut dyn Writer) {
        w.claim_decoded(crate::data::admit::<Self>());
        write_padded(self.0, w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        r.required_item("missing padded value")?;
        self.0.read(r)?;
        if r.item()? {
            return Err(DataError::new("extra padded value"));
        }
        Ok(())
    }
}
// Forge portable bytes independently of the real writer's native admission.
fn write_padded(value: bool, w: &mut dyn Writer) {
    w.begin_seq(1);
    w.item();
    value.write(w);
    w.end_seq();
}
#[test]
fn manual_native_floors_precede_default_construction() {
    fn check<T: Data>(value: T, budget: usize) {
        let bytes = bin::to_vec(&value).unwrap();
        assert!(decode::<T>("manual native floor", &bytes, budget).is_err());
        let loaded = decode::<T>("manual native positive", &bytes, 1 << 20).unwrap();
        assert_eq!(bin::to_vec(&loaded).unwrap(), bytes);
    }
    // Debug builds copy the 128 KiB array through several generic Result frames.
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            check(Padded(true), 256);
            check(std::array::from_fn::<_, 32, _>(|_| Padded(true)), 8192);
            check((Padded(true), Padded(false)), 1024);
            fn patch<T: Data>(value: &mut T, budget: usize) {
                let bytes = bin::to_vec(value).unwrap();
                let allowance = LoadBudget::new(budget);
                let mut reader = bin::Decoder::for_load(&bytes, Some(&allowance));
                assert!(value.read(&mut reader).is_err());
            }
            patch(&mut std::array::from_fn::<_, 32, _>(|_| Padded(true)), 8192);
            patch(&mut (Padded(true), Padded(false)), 1024);
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn wide_enum_and_padded_struct_vectors() {
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let mut w = bin::Encoder::default();
        w.begin_seq(300_000);
        for _ in 0..300_000 {
            w.variant("Empty", 0);
            w.begin_struct();
            w.end_struct();
            w.end_variant();
        }
        w.end_seq();
        let bytes = w.finish().unwrap();
        assert!(decode::<Vec<Wide>>("wide enum", &bytes, budget).is_err());
        let mut w = bin::Encoder::default();
        w.begin_seq(70_000);
        for _ in 0..70_000 {
            write_padded(false, &mut w);
        }
        w.end_seq();
        let bytes = w.finish().unwrap();
        assert!(decode::<Vec<Padded>>("padded struct", &bytes, budget).is_err());
        let bytes = bin::to_vec(&vec![Wide::Empty, Wide::Empty]).unwrap();
        assert_eq!(
            decode::<Vec<Wide>>("wide positive", &bytes, budget)
                .unwrap()
                .len(),
            2
        );
    }
}
#[test]
fn many_map_keys() {
    let values: std::collections::BTreeMap<_, _> = (0..20_000)
        .map(|i| (format!("key{i:06}"), i as u64))
        .collect();
    let bytes = bin::to_vec(&values).unwrap();
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let result = decode::<std::collections::BTreeMap<String, u64>>("map keys", &bytes, budget);
        if budget == MAX_LOAD_BYTES {
            assert_eq!(result.unwrap(), values);
        } else {
            assert!(result.is_err());
        }
    }
}
fn load(
    world: &mut World,
    label: &str,
    bytes: &[u8],
    budget: usize,
    adapt: bool,
) -> Result<bool, DataError> {
    let before = world.save().unwrap();
    let allowance = LoadBudget::new(budget);
    let result = measured(label, budget, || {
        world.load_in(bytes, Some(&allowance), adapt)
    });
    if result.is_err() {
        assert_eq!(world.save().unwrap(), before);
    }
    result
}
#[test]
fn near_cap_entity_table() {
    let mut source = World::new(60, 0);
    for _ in 0..MAX_ENTITIES {
        source.spawn(()).unwrap();
    }
    let bytes = source.save().unwrap();
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let mut destination = World::new(60, 17);
        destination.spawn_named("retain", ()).unwrap();
        let result = load(&mut destination, "200k slots", &bytes, budget, false);
        if budget == MAX_LOAD_BYTES {
            assert!(result.is_ok());
            assert_eq!(destination.len(), MAX_ENTITIES);
        } else {
            assert!(result.is_err());
        }
    }
}
// Only the first byte is semantic; the rest is native inline padding.
struct Large<const I: usize>([u8; 1024]);
impl<const I: usize> Default for Large<I> {
    fn default() -> Self {
        Self([0; 1024])
    }
}
impl<const I: usize> Data for Large<I> {
    fn write(&self, w: &mut dyn Writer) {
        self.0[0].write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        self.0[0].read(r)
    }
}
impl<const I: usize> Component for Large<I> {
    const NAME: &'static str = ["A", "B", "C", "D", "E", "F", "G", "H"][I];
}
impl Component for Padded {
    const NAME: &'static str = "Padded";
}
#[test]
fn sparse_chunks_across_types_charge_real_layouts() {
    sparse_chunks(false);
}
#[test]
fn aligned_chunks_charge_real_layouts() {
    sparse_chunks(true);
}
fn sparse_chunks(padded: bool) {
    let mut destination = World::new(60, 0);
    destination
        .register::<Large<0>>()
        .unwrap()
        .register::<Large<1>>()
        .unwrap()
        .register::<Large<2>>()
        .unwrap()
        .register::<Large<3>>()
        .unwrap()
        .register::<Large<4>>()
        .unwrap()
        .register::<Large<5>>()
        .unwrap()
        .register::<Large<6>>()
        .unwrap()
        .register::<Large<7>>()
        .unwrap()
        .register::<Padded>()
        .unwrap();
    // Forge sparse presence without ever allocating its multi-gigabyte decoded shape.
    {
        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("state");
        w.begin_struct();
        w.field("slots");
        w.begin_seq(MAX_ENTITIES);
        for _ in 0..MAX_ENTITIES {
            w.begin_struct();
            w.field("alive");
            w.boolean(true);
            w.end_struct();
        }
        w.end_seq();
        w.end_struct();
        w.field("rng");
        destination.rng().write(&mut w);
        w.field("components");
        w.begin_struct();
        for name in if padded {
            &["Padded"][..]
        } else {
            &["A", "B", "C", "D", "E", "F", "G", "H"][..]
        } {
            w.key(name);
            w.begin_seq(MAX_ENTITIES.div_ceil(64));
            for index in (0..MAX_ENTITIES).step_by(64) {
                w.begin_seq(2);
                w.entity(index as u32, 0);
                if padded {
                    write_padded(false, &mut w);
                } else {
                    0u8.write(&mut w);
                }
                w.end_seq();
            }
            w.end_seq();
        }
        w.end_struct();
        w.field("resources");
        w.begin_struct();
        w.end_struct();
        w.end_struct();
        let mut bytes = b"EXGAME\0\x04".to_vec();
        bytes.extend(w.finish().unwrap());
        for budget in [1 << 20, MAX_LOAD_BYTES] {
            assert!(load(
                &mut destination,
                if padded {
                    "aligned chunks"
                } else {
                    "sparse types"
                },
                &bytes,
                budget,
                true,
            )
            .is_err());
        }
    }
    let mut source = World::new(60, 0);
    source.register::<Large<0>>().unwrap();
    let mut value = Large::<0>::default();
    value.0[0] = 73;
    source.spawn(value).unwrap();
    let bytes = source.save().unwrap();
    assert!(load(&mut destination, "chunk positive", &bytes, 1 << 20, false).is_ok());
    assert_eq!(destination.get::<Large<0>>("#0").unwrap().0[0], 73);
    assert_eq!(destination.save().unwrap(), bytes);
}
#[test]
fn nested_publication_peak_is_bounded() {
    for depth in [1, 8, 40, 81] {
        let mut value = Published::Unit;
        for _ in 0..depth {
            let mut children = vec![Published::Unit; if depth == 1 { 2 } else { 2000 }];
            children[0] = value;
            value = Published::List(children);
        }
        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.key("x");
        value.write(&mut w);
        w.end_struct();
        let bytes = w.finish().unwrap();
        for budget in [1 << 20, MAX_LOAD_BYTES] {
            let mut destination = World::new(60, 0);
            destination.publish("old", true).unwrap();
            let before = json::to_string(&*destination.publications()).unwrap();
            let allowance = LoadBudget::new(budget);
            let result = measured(&format!("nested publication depth {depth}"), budget, || {
                let mut next = World::new(60, 0);
                next.read_publications(&mut bin::Decoder::for_load(&bytes, Some(&allowance)))?;
                Ok(next)
            });
            assert_eq!(result.is_err(), depth > 1);
            if let Ok(next) = result {
                destination = next;
                assert_eq!(destination.publications().get("x"), Some(&value));
            } else {
                assert_eq!(
                    json::to_string(&*destination.publications()).unwrap(),
                    before
                );
            }
        }
    }
}

#[test]
fn allocating_derived_defaults_are_admitted() {
    #[derive(Default, Data)]
    struct Defaults {
        #[data(skip)]
        bytes: Box<[u8; 32]>,
    }
    let mut w = bin::Encoder::default();
    w.begin_seq(100_000);
    for _ in 0..100_000 {
        w.begin_struct();
        w.end_struct();
    }
    w.end_seq();
    let bytes = w.finish().unwrap();
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let loaded = decode::<Vec<Defaults>>("allocating defaults", &bytes, budget);
        if budget == MAX_LOAD_BYTES {
            let loaded = loaded.unwrap();
            assert_eq!(loaded.len(), 100_000);
            assert!(loaded.iter().all(|v| *v.bytes == [0; 32]));
        } else {
            assert!(loaded.is_err());
        }
    }
}

fn padded_container_bytes(n: usize, map: bool, width: usize) -> Vec<u8> {
    let mut out = bin::Encoder::default();
    if map {
        out.begin_struct();
    } else {
        out.begin_seq(n);
    }
    for i in 0..n {
        if map {
            out.key(&format!("key{i:0width$}"));
        } else {
            out.item();
        }
        write_padded(true, &mut out);
    }
    if map {
        out.end_struct();
    } else {
        out.end_seq();
    }
    out.finish().unwrap()
}
fn padded_container(map: bool) {
    let mut bounded = true;
    for (n, width) in [(1024, 5), (1024, 6), (70_000, 6)] {
        let bytes = padded_container_bytes(n, map, width);
        for budget in [1 << 20, MAX_LOAD_BYTES] {
            let allowance = LoadBudget::new(budget);
            let (result, peak, _) = crate::counting::peak(|| {
                if map {
                    bin::from_slice_in::<std::collections::BTreeMap<String, Padded>>(
                        &bytes,
                        Some(&allowance),
                    )
                    .map(|values| {
                        assert_eq!(values.len(), n);
                        assert!(values.values().all(|v| v.0));
                    })
                } else {
                    bin::from_slice_in::<Vec<Box<Padded>>>(&bytes, Some(&allowance)).map(|values| {
                        assert_eq!(values.len(), n);
                        assert!(values.iter().all(|v| v.0));
                    })
                }
            });
            println!(
                "padded {} n={n} key_width={width}: budget={budget} peak={peak} accepted={}",
                if map { "map" } else { "boxes" },
                result.is_ok()
            );
            bounded &= peak <= budget + 8192;
            if n == 1024 && budget == MAX_LOAD_BYTES {
                result.unwrap();
            }
        }
    }
    assert!(bounded, "padded container exceeded a measured budget");
}
#[test]
fn boxed_padded_values_charge_native_payloads() {
    padded_container(false);
}
#[test]
fn mapped_padded_values_charge_half_empty_nodes() {
    padded_container(true);
}

#[derive(Default, Resource)]
struct Boxes(Vec<Box<Padded>>);
#[test]
fn refused_boxed_resource_replacement_preserves_the_destination() {
    // Forge the expensive candidate without allocating it in the source world.
    let mut out = bin::Encoder::default();
    out.begin_struct();
    out.field("state");
    out.begin_struct();
    out.end_struct();
    out.field("rng");
    World::new(60, 0).rng().write(&mut out);
    out.field("components");
    out.begin_struct();
    out.end_struct();
    out.field("resources");
    out.begin_struct();
    out.key("Boxes");
    out.begin_seq(1);
    out.item();
    out.begin_seq(2);
    out.item();
    out.entity(0, 0);
    out.item();
    out.begin_seq(1);
    out.item();
    out.begin_seq(70_000);
    for _ in 0..70_000 {
        out.item();
        write_padded(true, &mut out);
    }
    out.end_seq();
    out.end_seq();
    out.end_seq();
    out.end_seq();
    out.end_struct();
    out.end_struct();
    let mut bytes = b"EXGAME\0\x04".to_vec();
    bytes.extend(out.finish().unwrap());
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let mut destination = World::new(60, 17);
        destination.register_resource::<Boxes>().unwrap();
        destination
            .insert_resource(Boxes(vec![Box::new(Padded(false))]))
            .unwrap();
        destination.spawn_named("keep", ()).unwrap();
        let error = load(&mut destination, "boxed replacement", &bytes, budget, true).unwrap_err();
        assert!(error.message.contains("budget"), "{error}");
        assert!(!destination.resource::<Boxes>().0[0].0);
        assert!(destination.named("keep").is_some());
    }
}
#[test]
fn padded_world_save_cannot_succeed_when_its_default_budget_load_refuses() {
    let mut source = World::new(60, 0);
    source.register::<Padded>().unwrap();
    for _ in 0..70_000 {
        source.spawn(Padded(true)).unwrap();
    }
    let saved = source.save();
    println!("70000 padded components: save accepted={}", saved.is_ok());
    if let Ok(bytes) = saved {
        let mut destination = World::new(60, 0);
        destination.register::<Padded>().unwrap();
        load(
            &mut destination,
            "padded save/load",
            &bytes,
            MAX_LOAD_BYTES,
            false,
        )
        .unwrap();
    }
    // A positive control must still save and load at both budgets.
    let mut small = World::new(60, 0);
    small.register::<Padded>().unwrap();
    for _ in 0..64 {
        small.spawn(Padded(true)).unwrap();
    }
    let bytes = small.save().unwrap();
    for budget in [1 << 20, MAX_LOAD_BYTES] {
        let mut destination = World::new(60, 0);
        destination.register::<Padded>().unwrap();
        load(
            &mut destination,
            "padded world positive",
            &bytes,
            budget,
            false,
        )
        .unwrap();
        assert_eq!(destination.save().unwrap(), bytes);
    }
}
#[test]
fn registration_refuses_oversized_or_overflowing_native_pages_without_construction() {
    struct Huge<const N: usize>([u8; N]);
    impl<const N: usize> Default for Huge<N> {
        fn default() -> Self {
            panic!("registration must not construct Huge")
        }
    }
    impl<const N: usize> Data for Huge<N> {
        fn default_size() -> usize {
            1
        }
        fn write(&self, w: &mut dyn Writer) {
            w.boolean(false);
        }
        fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
            Ok(())
        }
    }
    impl<const N: usize> Component for Huge<N> {
        const NAME: &'static str = "Huge";
    }
    let mut destination = World::new(60, 0);
    let before = destination.save().unwrap();
    assert!(destination
        .register::<Huge<{ MAX_LOAD_BYTES / PAGE + 1 }>>()
        .is_err());
    assert!(destination.register::<Huge<{ 1 << 25 }>>().is_err()); // Layout overflow on i686/wasm32.
    assert_eq!(destination.save().unwrap(), before);
    destination.register::<Padded>().unwrap();
    destination.spawn(Padded(true)).unwrap();
}
