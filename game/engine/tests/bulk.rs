use exact_game::{bin, hash, json, Data, Reader};

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
        (bin::to_vec(&a), hash::of(&a)),
        (bin::to_vec(&b), hash::of(&b)),
        (bin::to_vec(&c), hash::of(&c)),
        (bin::to_vec(&d), hash::of(&d)),
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
    assert!(bin::from_slice::<Vec<u32>>(&bin::to_vec(&[1u32, 2]))
        .unwrap_err()
        .message
        .contains("non-bulk"));
}

#[test]
fn json_bulk_summary_refusal_consumes_the_value_before_the_next_one() {
    for value in [
        json::to_string(&vec![1u8, 2]).unwrap(),
        r#"{"bytes":2,"hash":"0x0","values":[1,2]}"#.into(),
    ] {
        let text = format!("[{value},42]");
        let mut r = json::Decoder::new(&text);
        r.begin_seq().unwrap();
        assert!(r.item().unwrap());
        let error = Vec::<u8>::new().read(&mut r).unwrap_err().to_string();
        assert!(r.item().unwrap());
        let mut tail = 0u32;
        tail.read(&mut r).unwrap();
        assert_eq!(tail, 42);
        assert!(!r.item().unwrap());
        r.finish().unwrap();
        assert!(error.contains("inspection-only"), "{error}");
    }
}

#[test]
fn numeric_json_arrays_use_the_same_values_and_binary_framing() {
    fn check<T: Data>(text: &str, values: Vec<T>) {
        let decoded = json::from_str::<Vec<T>>(text).unwrap();
        assert_eq!(bin::to_vec(&decoded), bin::to_vec(&values));
        assert_eq!(hash::of(&decoded), hash::of(&values));
        assert!(json::from_str::<Vec<T>>("[]").unwrap().is_empty());
        let summary = json::to_string(&decoded).unwrap();
        assert!(summary.starts_with("{\"bytes\":"));
        assert!(json::from_str::<Vec<T>>(&summary).is_err());
    }
    check("[0,255]", vec![0u8, 255]);
    check("[0,65535]", vec![0u16, 65535]);
    check("[0,4294967295]", vec![0u32, u32::MAX]);
    check(
        "[-0.0,1.25,1.0000000596046447753906251]",
        vec![-0.0f32, 1.25, f32::from_bits(1.0f32.to_bits() + 1)],
    );
    let mut values = vec![99u32; 3];
    json::read_into("[1,2]", &mut values).unwrap();
    assert_eq!(values, [1, 2]);
    let mut r = json::Decoder::new("[[1,2],42]");
    r.begin_seq().unwrap();
    assert!(r.item().unwrap());
    values.read(&mut r).unwrap();
    assert_eq!(values, [1, 2]);
    assert!(r.item().unwrap());
    let mut tail = 0u32;
    tail.read(&mut r).unwrap();
    assert_eq!(tail, 42);
    assert!(!r.item().unwrap());
    r.finish().unwrap();
}

#[test]
fn numeric_json_arrays_keep_scalar_ranges_paths_and_allocation_limits() {
    fn refuses<T: Data>(text: &str) {
        let error = json::from_str::<Vec<T>>(text).err().unwrap();
        assert!(error.path.ends_with(".1"), "{error}");
    }
    refuses::<u8>("[1,256]");
    refuses::<u16>("[1,65536]");
    refuses::<u32>("[1,4294967296]");
    refuses::<u32>("[1,-1]");
    refuses::<u32>("[1,1.5]");
    refuses::<f32>("[1,1e100]");
    refuses::<f32>("[1,\"bad\"]");
    assert!(json::from_str::<Vec<f32>>("[1,2,]").is_err());
    assert!(json::from_str::<Vec<f32>>("[1,2] true").is_err());
    let text = format!("[{}]", vec!["1"; 200].join(","));
    let mut r = json::Decoder::new(&text);
    r.claim(exact_game::data::MAX_LOAD_BYTES - 512).unwrap();
    let mut values = Vec::<f32>::new();
    assert!(values.read(&mut r).unwrap_err().message.contains("budget"));
    assert!(values.capacity() * std::mem::size_of::<f32>() <= 512);
    assert!(values.len() < 200);
}

#[test]
fn numeric_bulk_budget_counts_destination_once() {
    fn check<T: Data + PartialEq + std::fmt::Debug>(values: Vec<T>) {
        let bytes = bin::to_vec(&values);
        let size = std::mem::size_of::<T>() * values.len();
        for remaining in [size - 1, size] {
            let mut r = bin::Decoder::new(&bytes);
            r.claim(exact_game::data::MAX_LOAD_BYTES - remaining)
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
    use exact_game::{data::Number, Writer};
    for (bytes, _) in cards(false) {
        let mut tail = bin::Encoder::default();
        tail.number(Number::Unsigned(42));
        let bytes = [bytes, tail.finish()].concat();
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
