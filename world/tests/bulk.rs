use exact_world::{bin, hash, Data, Reader};

fn cards(empty: bool) -> [(Vec<u8>, u64); 4] {
    let a = if empty {
        vec![]
    } else {
        vec![0u8, 0, 0x80, 0x3f]
    };
    let b = if empty { vec![] } else { vec![0u16, 0x3f80] };
    let c = if empty { vec![] } else { vec![0x3f80_0000u32] };
    let d = if empty { vec![] } else { vec![1.0f32] };
    [
        (bin::to_vec(&a).unwrap(), hash::of(&a).unwrap()),
        (bin::to_vec(&b).unwrap(), hash::of(&b).unwrap()),
        (bin::to_vec(&c).unwrap(), hash::of(&c).unwrap()),
        (bin::to_vec(&d).unwrap(), hash::of(&d).unwrap()),
    ]
}

#[test]
fn bulk_kinds_have_distinct_wire_and_hash_including_empty() {
    for empty in [false, true] {
        let cards = cards(empty);
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(cards[i].0, cards[j].0, "wire {i}/{j}, empty={empty}");
                assert_ne!(cards[i].1, cards[j].1, "hash {i}/{j}, empty={empty}");
            }
        }
    }
}

#[test]
fn bulk_kinds_refuse_every_other_kind_by_name_including_empty() {
    for empty in [false, true] {
        for (source, (bytes, _)) in cards(empty).iter().enumerate() {
            let results = [
                bin::from_slice::<Vec<u8>>(bytes).map(|_| ()),
                bin::from_slice::<Vec<u16>>(bytes).map(|_| ()),
                bin::from_slice::<Vec<u32>>(bytes).map(|_| ()),
                bin::from_slice::<Vec<f32>>(bytes).map(|_| ()),
            ];
            for (target, result) in results.into_iter().enumerate() {
                if source == target {
                    result.unwrap();
                } else {
                    let error = result.unwrap_err().message;
                    let names = ["u8", "u16", "u32", "f32"];
                    assert!(
                        error.contains(&format!("expected bulk {}", names[target])),
                        "{error}"
                    );
                    assert!(
                        error.contains(&format!("found {}", names[source])),
                        "{error}"
                    );
                }
            }
        }
    }
}

#[test]
fn numeric_bulk_budget_counts_both_live_buffers() {
    fn check<T: Data + PartialEq + std::fmt::Debug>(values: Vec<T>) {
        let bytes = bin::to_vec(&values).unwrap();
        let size = std::mem::size_of::<T>()
            * values.len()
            * if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u8>() {
                1
            } else {
                2
            };
        for remaining in [size - 1, size] {
            let mut r = bin::Decoder::new(&bytes);
            r.claim(exact_world::data::MAX_LOAD_BYTES - remaining)
                .unwrap();
            let mut decoded = Vec::<T>::new();
            let result = decoded.read(&mut r);
            if remaining == size {
                result.unwrap();
                assert_eq!(decoded, values);
                r.finish().unwrap();
                assert!(r.claim(1).is_err());
            } else {
                assert!(result.unwrap_err().message.contains("budget"));
                assert_eq!(decoded.capacity(), 0);
            }
        }
    }
    check(vec![1u8, 2]);
    check(vec![1u16, 2]);
    check(vec![1u32, 2]);
    check(vec![1.0f32, 2.0]);
}

#[test]
fn every_bulk_kind_skips_without_losing_the_following_value() {
    use exact_world::{data::Number, Writer};
    for (bytes, _) in cards(false) {
        let mut tail = bin::Encoder::default();
        tail.number(Number::Unsigned(42));
        let bytes = [bytes, tail.finish().unwrap()].concat();
        let mut r = bin::Decoder::new(&bytes);
        r.skip().unwrap();
        let mut value = 0u32;
        value.read(&mut r).unwrap();
        assert_eq!(value, 42);
        r.finish().unwrap();
    }
}

#[test]
fn matching_numeric_kind_still_refuses_partial_elements() {
    for tag in [13, 14, 15] {
        let bytes = [tag, 1, 0];
        let error = match tag {
            13 => bin::from_slice::<Vec<u16>>(&bytes).unwrap_err(),
            14 => bin::from_slice::<Vec<u32>>(&bytes).unwrap_err(),
            _ => bin::from_slice::<Vec<f32>>(&bytes).unwrap_err(),
        };
        assert!(error.message.contains("invalid byte length"), "{error}");
    }
}

#[test]
fn encoder_refuses_small_wire_values_with_excessive_decoded_backing() {
    // 600k absent options occupy only 600k wire bytes but their Vec backing
    // exceeds 256 MiB at the decoder's geometric growth boundary.
    let values: Vec<Option<[u64; 32]>> = (0..600_000).map(|_| None).collect();
    assert!(exact_world::bin::to_vec(&values).is_err());
    let small = vec![Some([7u64; 32])];
    let bytes = exact_world::bin::to_vec(&small).unwrap();
    assert_eq!(
        exact_world::bin::from_slice::<Vec<Option<[u64; 32]>>>(&bytes).unwrap(),
        small
    );
}
