use exact_game::{bin, hash, json, Component, Data, Entity, Quat, Vec2, Vec3, Vec4};
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
    assert_eq!(bin::from_slice::<T>(&bin::to_vec(&value)).unwrap(), value);
    assert_eq!(
        json::from_str::<T>(&json::to_string(&value).unwrap()).unwrap(),
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
fn skip_unknown_and_preserve_missing() {
    let mut r = Record {
        number: 42,
        text: "old".into(),
        cached: 99,
    };
    json::read_into(
        r#"{"text":"new","cached":456,"future":{"nested":[1,{"a":true}],"null":null}}"#,
        &mut r,
    )
    .unwrap();
    assert_eq!(
        r,
        Record {
            number: 42,
            text: "new".into(),
            cached: 0
        }
    );
    #[derive(Default, Data)]
    struct Extended {
        future: Vec<Choice>,
        number: i32,
        text: String,
    }
    let b = bin::to_vec(&Extended {
        future: vec![Choice::Tuple(123, "skip".into())],
        number: 6,
        text: "known".into(),
    });
    bin::read_into(&b, &mut r).unwrap();
    assert_eq!(r.number, 6);
    assert_eq!(r.text, "known");
    let with_cache = Record {
        number: 4,
        cached: 9,
        ..Record::default()
    };
    let without_cache = Record {
        number: 4,
        ..Record::default()
    };
    assert_eq!(hash::of(&with_cache), hash::of(&without_cache));
    assert_eq!(bin::to_vec(&with_cache), bin::to_vec(&without_cache));
    assert_eq!(
        bin::from_slice::<Record>(&bin::to_vec(&with_cache))
            .unwrap()
            .cached,
        0
    );
    let mut arm = Choice::Record {
        on: true,
        nested: vec![],
        cache: 12,
    };
    json::read_into(r#"{"Record":{}}"#, &mut arm).unwrap();
    assert_eq!(
        arm,
        Choice::Record {
            on: true,
            nested: vec![],
            cache: 0
        }
    );
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
        assert_eq!(bin::to_vec(&nan), bin::to_vec(&f32::NAN));
        assert_eq!(
            bin::from_slice::<f32>(&bin::to_vec(&nan))
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
            bin::from_slice::<f64>(&bin::to_vec(&nan))
                .unwrap()
                .to_bits(),
            0x7ff8_0000_0000_0000
        );
    }
    assert_ne!(hash::of(&0.0f32), hash::of(&-0.0f32));
    assert_eq!(
        bin::from_slice::<f32>(&bin::to_vec(&-0.0f32))
            .unwrap()
            .to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(
        json::from_str::<f32>(&json::to_string(&-0.0f32).unwrap())
            .unwrap()
            .to_bits(),
        (-0.0f32).to_bits()
    );
    round_trip((u64::MAX, i64::MIN, f32::MIN_POSITIVE, f64::MAX));
    round_trip((u8::MAX, u16::MAX, u32::MAX, i32::MIN));
    round_trip((i8::MIN, i16::MIN, -0.005f32, 1e-99f64));
    assert!(json::from_str::<u8>("256").is_err());
    assert!(json::from_str::<u64>("-1").is_err());
}
#[test]
fn builtins_and_unicode() {
    round_trip((true, vec![1u32, 2, 3], Some(Some(3i32)), Box::new(34i64)));
    round_trip(None::<Option<u32>>);
    round_trip(Some(None::<u32>));
    round_trip([1u16, 2, 3]);
    round_trip([0u8; 32]);
    round_trip(());
    round_trip(Vec2::new(1.0, 2.0));
    round_trip(Vec3::new(1.0, 2.0, 3.0));
    round_trip(Vec4::new(1.0, 2.0, 3.0, 4.0));
    round_trip(Quat::IDENTITY);
    round_trip(Entity::default());
    assert_eq!(json::from_str::<String>(r#""\ud83c\udf15""#).unwrap(), "🌕");
    assert!(json::from_str::<String>(r#""\ud83cX""#).is_err());
    assert!(json::from_str::<String>(r#""\udf15""#).is_err());
}
#[test]
fn errors_name_the_field_and_reject_malformed_input() {
    #[derive(Default, Component)]
    struct Lantern {
        glow: f32,
    }
    let e = json::from_str::<Lantern>(r#"{"glow":"bright"}"#)
        .err()
        .unwrap()
        .to_string();
    assert!(e.starts_with("Lantern.glow: expected a number"), "{e}");
    let e = json::to_string(&Lantern {
        glow: f32::INFINITY,
    })
    .unwrap_err()
    .to_string();
    assert!(e.starts_with("Lantern.glow:"), "{e}");
    for s in [
        "[1,]",
        "[1 2]",
        "[1] false",
        "[01]",
        "[+1]",
        "[1e]",
        "[1.]",
        "[1e999]",
    ] {
        assert!(json::from_str::<Vec<f64>>(s).is_err(), "{s}");
    }
    let mut bytes = bin::to_vec(&Record::default());
    bytes.push(0);
    assert!(bin::from_slice::<Record>(&bytes).is_err());
    for n in 0..bytes.len() - 1 {
        assert!(bin::from_slice::<Record>(&bytes[..n]).is_err());
    }
    let nested = format!("{}0{}", "[".repeat(300), "]".repeat(300));
    assert!(json::from_str::<Record>(&format!(r#"{{"unknown":{nested}}}"#)).is_err());
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
    });
    assert_eq!(bin::from_slice::<New>(&b).unwrap().kept.reused, 12);
}
