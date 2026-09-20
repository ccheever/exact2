use exact_world::{bin, hash, json, Data};
use std::collections::BTreeMap;

#[derive(Default, Debug, PartialEq, Data)]
pub(crate) struct Record {
    /// Visibility and documentation are intentionally part of this fixture.
    pub number: i32,
    #[allow(dead_code)]
    pub(crate) text: String,
    #[data(skip)]
    cached: u32,
}
#[derive(Default, Debug, PartialEq, Data)]
struct Newtype(pub(crate) u64);
#[derive(Default, Debug, PartialEq, Data)]
struct Unit;
#[derive(Default, Debug, PartialEq, Data)]
struct Empty {}
#[derive(Default, Debug, PartialEq, Data)]
struct Pair(i32, #[data(skip)] u8, String);
#[derive(Default, Debug, PartialEq, Data)]
enum Choice {
    #[default]
    Idle,
    Tuple(i32, String),
    Record {
        /// A documented arm field.
        on: bool,
        nested: Vec<BTreeMap<String, (u32, u32)>>,
        #[data(skip)]
        cache: u32,
    },
    Empty {},
    TupleEmpty(),
}
fn round_trip<T: Data + PartialEq + std::fmt::Debug>(value: T) {
    assert_eq!(
        bin::from_slice::<T>(&bin::to_vec(&value).unwrap()).unwrap(),
        value
    );
}
#[test]
fn derive_shapes() {
    round_trip(Record {
        number: -34,
        text: "quote \" slash \\ line\n 🌕".into(),
        cached: 0,
    });
    round_trip(Newtype(u64::MAX));
    round_trip(Unit);
    round_trip(Empty {});
    round_trip(Pair(-1, 0, "tail".into()));
    round_trip(Choice::Idle);
    round_trip(Choice::Tuple(i32::MIN, "value".into()));
    round_trip(Choice::Record {
        on: true,
        nested: vec![BTreeMap::from([("key".into(), (3, 4))])],
        cache: 0,
    });
    round_trip(Choice::Empty {});
    round_trip(Choice::TupleEmpty());
}
#[test]
fn field_names_are_not_values() {
    #[derive(Default, Data)]
    struct A {
        first: u32,
        second: String,
    }
    #[derive(Default, Data)]
    struct B {
        renamed: u32,
        other: String,
    }
    let a = A {
        first: 123,
        second: "value".into(),
    };
    let b = B {
        renamed: 123,
        other: "value".into(),
    };
    assert_eq!(hash::of(&a), hash::of(&b));
    assert_ne!(hash::of(&a), hash::of(&B { renamed: 124, ..b }));
    assert_ne!(hash::of(&vec![1u8]), hash::of(&vec![1u8, 0]));
    assert_ne!(hash::of(&Choice::Idle), hash::of(&Choice::Empty {}));
    assert_ne!(
        hash::of(&BTreeMap::from([("a".into(), 1u32)])),
        hash::of(&BTreeMap::from([("b".into(), 1u32)]))
    );
}
#[test]
fn numbers_nan_and_negative_zero() {
    for nan in [
        f32::NAN,
        f32::from_bits(0xffff_ffff),
        f32::from_bits(0x7f80_0001),
    ] {
        assert_eq!(hash::of(&nan), hash::of(&f32::NAN));
        assert_eq!(bin::to_vec(&nan).unwrap(), bin::to_vec(&f32::NAN).unwrap());
        assert_eq!(
            bin::from_slice::<f32>(&bin::to_vec(&nan).unwrap())
                .unwrap()
                .to_bits(),
            0x7fc0_0000
        );
        assert!(json::to_string(&nan).is_err());
    }
    for nan in [
        f64::NAN,
        f64::from_bits(u64::MAX),
        f64::from_bits(0x7ff0_0000_0000_0001),
    ] {
        assert_eq!(hash::of(&nan), hash::of(&f64::NAN));
        assert_eq!(
            bin::from_slice::<f64>(&bin::to_vec(&nan).unwrap())
                .unwrap()
                .to_bits(),
            0x7ff8_0000_0000_0000
        );
    }
    assert_ne!(hash::of(&0.0f32), hash::of(&-0.0f32));
    assert_eq!(
        bin::from_slice::<f32>(&bin::to_vec(&-0.0f32).unwrap())
            .unwrap()
            .to_bits(),
        (-0.0f32).to_bits()
    );
    round_trip((u64::MAX, i64::MIN, f32::MIN_POSITIVE, f64::MAX));
    round_trip((u8::MAX, u16::MAX, u32::MAX, i32::MIN));
    round_trip((i8::MIN, i16::MIN, -0.005f32, 1e-99f64));
    assert!(bin::from_slice::<u8>(&bin::to_vec(&256u32).unwrap()).is_err());
    assert!(bin::from_slice::<u64>(&bin::to_vec(&-1i64).unwrap()).is_err());
}

#[test]
fn name_table_walks_unknown_fields() {
    #[derive(Default, Data)]
    struct Inner {
        reused: u32,
    }
    #[derive(Default, Data)]
    struct Old {
        unknown: Inner,
        kept: Inner,
    }
    #[derive(Default, Data)]
    struct New {
        kept: Inner,
    }
    let b = bin::to_vec(&Old {
        unknown: Inner { reused: 7 },
        kept: Inner { reused: 12 },
    })
    .unwrap();
    assert_eq!(bin::from_slice::<New>(&b).unwrap().kept.reused, 12);
}

#[test]
fn inspection_floats_round_trip_signed_zero_subnormals_and_random_bits() {
    let mut bits = 1u64;
    for _ in 0..100_000 {
        bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = f32::from_bits(bits as u32);
        let b = f64::from_bits(bits);
        if a.is_finite() {
            assert_eq!(
                json::to_string(&a)
                    .unwrap()
                    .parse::<f32>()
                    .unwrap()
                    .to_bits(),
                a.to_bits()
            );
        }
        if b.is_finite() {
            assert_eq!(
                json::to_string(&b)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
                    .to_bits(),
                b.to_bits()
            );
        }
    }
    for n in [0., -0., f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)] {
        assert_eq!(
            json::to_string(&n)
                .unwrap()
                .parse::<f64>()
                .unwrap()
                .to_bits(),
            n.to_bits()
        );
    }
    assert!(json::to_string(&f64::INFINITY).is_err());
}
