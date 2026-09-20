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
    let mut destination = T::default();
    let before = bin::to_vec(&destination).unwrap();
    let allowance = LoadBudget::new(budget);
    let result = measured(label, budget, || {
        bin::from_slice_in::<T>(bytes, Some(&allowance))
    });
    match result {
        Ok(next) => {
            destination = next;
            Ok(destination)
        }
        Err(e) => {
            assert_eq!(bin::to_vec(&destination).unwrap(), before);
            Err(e)
        }
    }
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
#[derive(Default, Data)]
struct Padded(bool);
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
            Padded(false).write(&mut w);
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
                    Padded(false).write(&mut w);
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
