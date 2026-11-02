//! Differential tests: every piece's text is `{}`'s, byte for byte.

use crate::{Piece, Shortest, Shortest32};
use std::rc::Rc;

/// xorshift64*, deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

fn piece(p: impl Piece) -> String {
    let mut out = String::new();
    p.push_to(&mut out);
    out
}

#[test]
fn integers_print_as_display_prints_them() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut values: Vec<u64> = vec![0, 1, 9, 10, 99, 100, 999, 1000];
    values.extend(
        (0..20)
            .map(|k| 10u64.pow(k))
            .flat_map(|p| [p - 1, p, p + 1]),
    );
    values.extend([
        u64::from(u8::MAX),
        u64::from(u16::MAX),
        u64::from(u32::MAX),
        u64::MAX,
    ]);
    values.extend((0..20_000).map(|_| rng.next() >> (rng.next() % 64)));
    for v in values {
        assert_eq!(piece(v), format!("{v}"));
        assert_eq!(piece(v as u8), format!("{}", v as u8));
        assert_eq!(piece(v as u16), format!("{}", v as u16));
        assert_eq!(piece(v as u32), format!("{}", v as u32));
        assert_eq!(piece(v as usize), format!("{}", v as usize));
        assert_eq!(piece(v as i64), format!("{}", v as i64));
        assert_eq!(piece(v as i32), format!("{}", v as i32));
        assert_eq!(piece(v as isize), format!("{}", v as isize));
    }
    for v in [i64::MIN, i64::MIN + 1, -1, i64::MAX] {
        assert_eq!(piece(v), format!("{v}"));
    }
    for v in [i32::MIN, -1, i32::MAX] {
        assert_eq!(piece(v), format!("{v}"));
    }
}

#[test]
fn floats_print_as_display_prints_them() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let special = [
        0.0,
        -0.0,
        1.0,
        -1.5,
        0.1,
        1e21,
        1e-7,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        5e-324,
    ];
    for v in special
        .into_iter()
        .chain((0..20_000).map(|_| f64::from_bits(rng.next())))
    {
        assert_eq!(piece(Shortest(v)), format!("{v}"));
        assert_eq!(piece(Shortest32(v as f32)), format!("{}", v as f32));
    }
    for _ in 0..20_000 {
        let v = (rng.next() % 1_000_000) as f64 / 10f64.powi((rng.next() % 6) as i32);
        assert_eq!(piece(Shortest(v)), format!("{v}"));
    }
}

#[test]
fn a_template_fills_as_format_does() {
    let rc: Rc<str> = Rc::from("rc");
    let owned = String::from("owned");
    let what = "act";
    assert_eq!(
        crate::text!("{} → epoch {} (+{} −{})", what, 7u64, 3usize, 0usize),
        format!("{what} → epoch {} (+{} −{})", 7u64, 3usize, 0usize)
    );
    assert_eq!(
        crate::text!(
            "{}|{} {},{}{}{}",
            rc,
            owned,
            true,
            false,
            -12i64,
            Shortest(0.25)
        ),
        format!("{rc}|{owned} {},{}{}{}", true, false, -12i64, 0.25)
    );
    assert_eq!(
        crate::text!("{{\"op\":\"destroy\",\"id\":{}}}", 7u32),
        format!("{{\"op\":\"destroy\",\"id\":{}}}", 7u32)
    );
    assert_eq!(crate::text!(""), "");
    assert_eq!(crate::text!("{{}}"), format!("{{}}"));
    assert_eq!(crate::text!("{{{}}}", 1u8), format!("{{{}}}", 1u8));
    assert_eq!(crate::text!("{}{}", 'a', "b"), "ab");
    assert_eq!(crate::text!("é{}💬 {{ }} {}", 'x', -0i32), "éx💬 { } 0");
    let mut out = String::from("{\"id\":");
    crate::push_text!(&mut out, "{},\"ms\":{}}}", 42u32, Shortest(16.5));
    assert_eq!(out, "{\"id\":42,\"ms\":16.5}");
    // A `&mut String` binding stays usable after the macro borrows it.
    let target = &mut out;
    crate::push_text!(target, "!",);
    target.push('.');
    assert_eq!(out, "{\"id\":42,\"ms\":16.5}!.");
}
