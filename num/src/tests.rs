//! Differential tests: every answer is std's, bit for bit, error for error.

use super::{parse_f32, parse_f64};

/// Both widths agree with std on `s`: the same bits (NaN sign included), or
/// the same error with the same text.
fn check(s: &str) {
    match (parse_f64(s), s.parse::<f64>()) {
        (Ok(a), Ok(b)) => assert_eq!(a.to_bits(), b.to_bits(), "f64 {s:?}: {a:e} vs std {b:e}"),
        (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string(), "f64 {s:?}"),
        (a, b) => panic!("f64 {s:?}: {a:?} vs std {b:?}"),
    }
    match (parse_f32(s), s.parse::<f32>()) {
        (Ok(a), Ok(b)) => assert_eq!(a.to_bits(), b.to_bits(), "f32 {s:?}: {a:e} vs std {b:e}"),
        (Err(a), Err(b)) => {
            assert_eq!(a.to_string(), b.to_string(), "f32 {s:?}");
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "f32 {s:?}");
        }
        (a, b) => panic!("f32 {s:?}: {a:?} vs std {b:?}"),
    }
}

/// xorshift64*: deterministic, dependency-free.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn grammar_and_specials_match_std() {
    for s in [
        "",
        "+",
        "-",
        ".",
        "e",
        "E5",
        "1e",
        "1e+",
        "1e-",
        "1E+5",
        ".e1",
        "1.",
        ".1",
        "1.e1",
        "+.1",
        "-.1e-1",
        "0",
        "-0",
        "+0",
        "0.0",
        "-0.0e-5",
        "00000",
        "0e0",
        "0e999999",
        "-0e-999999",
        "1e999999",
        "1e-999999",
        "inf",
        "-inf",
        "+inf",
        "INF",
        "iNf",
        "Infinity",
        "infinity",
        "INFINITY",
        "-Infinity",
        "infinit",
        "infinityy",
        "nan",
        "NaN",
        "NAN",
        "-nan",
        "+NaN",
        "nana",
        "na",
        "1_0",
        " 1",
        "1 ",
        "0x10",
        "1e5000",
        "1e-5000",
        "1..2",
        "1.2.3",
        "--1",
        "+-1",
        "1e1.5",
        "1e--1",
        "١",
        "1\0",
        "e",
        "-e1",
        "12345678901234567890",
        "٣.٥",
        "0.000000000000000000000000000001",
        "100000000000000000000000000000000",
    ] {
        check(s);
    }
}

#[test]
fn boundaries_match_std() {
    for s in [
        // Subnormals, the smallest and half of it (a tie that rounds to even).
        "4.9406564584124654e-324",
        "2.4703282292062327e-324",
        "2.4703282292062328e-324",
        "2.4703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328125e-324",
        "2.4703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328125000000000000001e-324",
        "2.2250738585072011e-308",
        "2.2250738585072014e-308",
        "1e-320",
        "3e-324",
        "7e-324",
        // The largest finite values and just past them.
        "1.7976931348623157e308",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497791.9999999999",
        "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497792",
        "3.4028234e38",
        "3.4028235e38",
        "3.40282356e38",
        "3.4028236e38",
        "340282356779733661637539395458142568448",
        "340282356779733661637539395458142568447.999",
        "1.17549435e-38",
        "1.4e-45",
        "7e-46",
        "7.1e-46",
        "7.006492321624085354618647916449580656401309709382578858785341419448955413429303e-46",
        "1e-46",
        "1e-45",
        // Integers at and past the mantissa.
        "9007199254740992",
        "9007199254740993",
        "9007199254740993.0000000000000000001",
        "9007199254740995",
        "16777217",
        "16777217.00000000001",
        "123456789012345678901234567890",
        "0.1",
        "0.2",
        "0.3",
        "1e22",
        "1e23",
        "1e-22",
        "1e-23",
        "123456789e-30",
        "4503599627370496.5",
        "4503599627370497.5",
    ] {
        check(s);
        check(&format!("-{s}"));
    }
    // Saturating exponents: std's quirk, kept.
    let ones = format!("1{}e-70000", "0".repeat(70000));
    check(&ones);
    check(&format!("0.{}1e70000", "0".repeat(70000)));
    check(&format!("1{}", "0".repeat(400)));
    check(&format!("0.{}1", "0".repeat(400)));
}

#[test]
fn printed_floats_read_back_as_std_reads_them() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let x = f64::from_bits(rng.next());
        if x.is_nan() {
            continue;
        }
        for s in [
            format!("{x}"),
            format!("{x:e}"),
            format!("{x:?}"),
            format!("{x:.3e}"),
            format!("{x:.25e}"),
        ] {
            check(&s);
        }
        let y = f32::from_bits(rng.next() as u32);
        if y.is_nan() {
            continue;
        }
        for s in [
            format!("{y}"),
            format!("{y:e}"),
            format!("{y:.2e}"),
            format!("{y:.12e}"),
        ] {
            check(&s);
        }
    }
}

#[test]
fn random_decimal_text_matches_std() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let alphabet = b"0123456789000009.eE+-";
    for _ in 0..30_000 {
        let mut s = String::new();
        let len = 1 + rng.below(30) as usize;
        for _ in 0..len {
            s.push(alphabet[rng.below(alphabet.len() as u64) as usize] as char);
        }
        check(&s);
        // Well-formed: digits, a point, digits, an exponent.
        let mut t = String::new();
        for _ in 0..rng.below(25) {
            t.push((b'0' + rng.below(10) as u8) as char);
        }
        if rng.below(2) == 0 {
            t.push('.');
            for _ in 0..rng.below(25) {
                t.push((b'0' + rng.below(10) as u8) as char);
            }
        }
        if rng.below(2) == 0 {
            t.push_str(["e", "E", "e-", "e+"][rng.below(4) as usize]);
            t.push_str(&rng.below(700).to_string());
        }
        check(&t);
    }
}

#[test]
fn halfway_points_round_to_even_as_std_does() {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    for _ in 0..20_000 {
        // An f64 mantissa m (53 bits) and the midpoint (2m + 1) * 2^(e - 1),
        // written exactly: an integer, or (2m + 1) * 5^j / 10^j.
        let m = (1u64 << 52) | (rng.next() >> 12);
        let odd = 2 * u128::from(m) + 1;
        let e = rng.below(60) as i32 - 30;
        let text = if e >= 1 {
            (odd << (e - 1)).to_string()
        } else {
            let j = (1 - e) as u32;
            let digits = odd * 5u128.pow(j);
            format!("{digits}e-{j}")
        };
        check(&text);
        for tail in ["0000000000000000000001", "9999999999999999999999"] {
            let (mantissa, exp) = text.split_once('e').unwrap_or((&text, ""));
            let bumped = format!(
                "{mantissa}.{tail}{}",
                if exp.is_empty() {
                    String::new()
                } else {
                    format!("e{exp}")
                }
            );
            check(&bumped);
        }
        // An f32 midpoint the same way.
        let m = (1u32 << 23) | (rng.next() as u32 >> 9);
        let odd = 2 * u128::from(m) + 1;
        let e = rng.below(40) as i32 - 20;
        let text = if e >= 1 {
            (odd << (e - 1)).to_string()
        } else {
            let j = (1 - e) as u32;
            format!("{}e-{j}", odd * 5u128.pow(j))
        };
        check(&text);
    }
}

#[test]
fn long_mantissas_match_std() {
    let mut rng = Rng(42);
    for _ in 0..300 {
        let mut s = String::new();
        for _ in 0..1 + rng.below(1200) {
            s.push((b'0' + rng.below(10) as u8) as char);
        }
        let point = rng.below(s.len() as u64 + 1) as usize;
        s.insert(point, '.');
        let exp = rng.below(800) as i64 - 400;
        check(&s);
        check(&format!("{s}e{exp}"));
    }
}

#[test]
fn errors_display_and_debug_as_std() {
    let empty = parse_f64("").unwrap_err();
    assert_eq!(format!("{empty}"), "cannot parse float from empty string");
    assert_eq!(
        format!("{empty:?}"),
        format!("{:?}", "".parse::<f64>().unwrap_err())
    );
    let invalid = parse_f32("x").unwrap_err();
    assert_eq!(
        format!("{invalid:>30}"),
        format!("{:>30}", "x".parse::<f32>().unwrap_err())
    );
    assert_eq!(
        format!("{invalid:?}"),
        format!("{:?}", "x".parse::<f32>().unwrap_err())
    );
}

#[test]
fn the_u128_path_matches_std_at_its_edges() {
    let mut rng = Rng(0x0bad_cafe_1234_5678);
    for _ in 0..40_000 {
        // Up to 19 significant digits, exponents across and just past ±27.
        let digits = 1 + rng.below(19) as u32;
        let d = rng.next() % 10u64.pow(digits).max(1);
        let e = rng.below(60) as i64 - 30;
        check(&format!("{d}e{e}"));
        check(&format!("{}e{e}", u64::MAX / (1 + rng.below(9))));
        // Exact f64 ties with at most 19 digits: (2m + 1) × 5^j × 10^-j.
        let m = (1u64 << 52) | (rng.next() >> 12);
        for j in 0..=2u32 {
            let odd = u128::from(2 * m + 1) * 5u128.pow(j);
            check(&format!("{odd}e-{j}"));
            check(&format!("{odd}e-{}", j + rng.below(8) as u32));
        }
        // Exact f32 ties: (2m + 1) × 5^j, up to 19 digits.
        let m = (1u64 << 23) | (rng.next() >> 41);
        let j = rng.below(12) as u32;
        let odd = u128::from(2 * m + 1) * 5u128.pow(j);
        check(&format!("{odd}e-{j}"));
        check(&format!("{odd}e{}", rng.below(10)));
    }
    for e in [-28, -27, -26, 26, 27, 28] {
        for d in [
            1u64,
            9,
            12_345_678_901_234_567,
            9_999_999_999_999_999_999,
            u64::MAX,
        ] {
            check(&format!("{d}e{e}"));
        }
    }
}
