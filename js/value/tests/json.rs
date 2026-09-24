//! The lean codec against serde_json and std: the same trees, the same
//! errors at the same positions, and floats read as std reads them
//! (correctly rounded), which is where serde_json's reader can differ.

use exact_js_value::json::{self, Json, Number};

/// xorshift64*, deterministic.
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

/// A number token and the value std reads it as, classified as serde_json
/// classifies integers.
fn number(rng: &mut Rng) -> (String, Number) {
    let text = match rng.below(9) {
        0 => rng.below(1000).to_string(),
        1 => format!("-{}", rng.below(1 << 40)),
        2 => rng.next().to_string(),
        3 => format!("{}{}", rng.next(), rng.below(1000)), // past u64
        4 => format!("{}", f64::from_bits(rng.next() >> 2) * 1e-3),
        5 => format!("{:e}", f64::from_bits(rng.next() >> 1)),
        6 => format!("{}.{}", rng.below(100_000), rng.next()),
        7 => format!(
            "-0.{:017}e{}",
            rng.next() % 100_000_000_000_000_000,
            rng.below(40) as i64 - 20
        ),
        _ => format!("{}e-{}", rng.below(1 << 53), rng.below(330)),
    };
    let integer = !text.contains(['.', 'e', 'E']);
    let value = match (integer, text.parse::<u64>(), text.parse::<i64>()) {
        (true, Ok(n), _) => Number::PosInt(n),
        (true, _, Ok(n)) if n < 0 => Number::NegInt(n),
        (true, _, _) if text == "-0" => Number::Float(-0.0),
        _ => Number::Float(text.parse::<f64>().unwrap()),
    };
    (text, value)
}

fn string(rng: &mut Rng) -> (String, String) {
    let pieces = [
        'a', 'é', '😀', '"', '\\', '/', '\n', '\t', '\u{0}', '\u{1f}', '\u{7f}', '\u{2028}', ' ',
        'z',
    ];
    let mut value = String::new();
    let mut text = String::from("\"");
    for _ in 0..rng.below(8) {
        let c = pieces[rng.below(pieces.len() as u64) as usize];
        value.push(c);
        let escape = rng.below(2) == 0;
        match c {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            c if (c as u32) < 0x20 => text.push_str(&format!("\\u{:04x}", c as u32)),
            '/' if escape => text.push_str("\\/"),
            c if escape && (c as u32) >= 0x10000 => {
                for unit in c.encode_utf16(&mut [0u16; 2]) {
                    text.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c if escape => text.push_str(&format!("\\u{:04x}", c as u32)),
            c => text.push(c),
        }
    }
    text.push('"');
    (text, value)
}

/// A document and the tree it denotes.
fn document(rng: &mut Rng, depth: u32) -> (String, Json) {
    let space = |rng: &mut Rng| [" ", "", "\n", "\t", "\r\n "][rng.below(5) as usize].to_string();
    match rng.below(if depth > 4 { 6 } else { 8 }) {
        0 => ("null".into(), Json::Null),
        1 => ("true".into(), Json::Bool(true)),
        2 => ("false".into(), Json::Bool(false)),
        3 | 4 => {
            let (text, value) = number(rng);
            (text, Json::Number(value))
        }
        5 => {
            let (text, value) = string(rng);
            (text, Json::String(value))
        }
        6 => {
            let mut text = String::from("[");
            let mut items = Vec::new();
            for i in 0..rng.below(5) {
                if i > 0 {
                    text.push(',');
                }
                text.push_str(&space(rng));
                let (t, v) = document(rng, depth + 1);
                text.push_str(&t);
                text.push_str(&space(rng));
                items.push(v);
            }
            text.push(']');
            (text, Json::Array(items))
        }
        _ => {
            let mut text = String::from("{");
            let mut object = json::Object::new();
            for i in 0..rng.below(5) {
                if i > 0 {
                    text.push(',');
                }
                let (key_text, key) = string(rng);
                let (t, v) = document(rng, depth + 1);
                text.push_str(&format!(
                    "{}{key_text}{}:{}{t}",
                    space(rng),
                    space(rng),
                    space(rng)
                ));
                object.insert(key, v);
            }
            text.push('}');
            (text, Json::Object(object))
        }
    }
}

/// Every float in `serde`'s tree that differs from `expected`'s, bit for bit.
fn float_differences(expected: &Json, serde: &serde_json::Value, out: &mut Vec<(f64, f64)>) {
    match (expected, serde) {
        (Json::Number(Number::Float(a)), serde_json::Value::Number(n)) => {
            let b = n.as_f64().unwrap();
            if a.to_bits() != b.to_bits() {
                out.push((*a, b));
            }
        }
        (Json::Array(a), serde_json::Value::Array(b)) => {
            for (a, b) in a.iter().zip(b) {
                float_differences(a, b, out);
            }
        }
        (Json::Object(a), serde_json::Value::Object(b)) => {
            for (key, a) in a.iter() {
                float_differences(a, &b[key.as_str()], out);
            }
        }
        _ => {}
    }
}

/// `a` equals `b` with every float compared bit for bit (-0 is not 0).
fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(Number::Float(x)), Json::Number(Number::Float(y))) => {
            x.to_bits() == y.to_bits()
        }
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((kx, vx), (ky, vy))| kx == ky && same(vx, vy))
        }
        _ => a == b,
    }
}

#[test]
fn documents_parse_to_their_trees_and_serde_json_agrees_but_for_rounding() {
    let mut rng = Rng(0x1234_5678_9abc_def0);
    let mut differences = Vec::new();
    for _ in 0..20_000 {
        let (text, expected) = document(&mut rng, 0);
        let parsed = json::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert!(same(&parsed, &expected), "{text}\n{parsed:?}\n{expected:?}");
        // serde_json reads the same structure; its floats may miss by an ulp.
        let serde: serde_json::Value = serde_json::from_str(&text).unwrap();
        let before = differences.len();
        float_differences(&expected, &serde, &mut differences);
        if differences.len() == before {
            assert_eq!(serde_json::Value::from(parsed.clone()), serde, "{text}");
        }
        // Written back, it reads as the same tree, to us; serde_json reads
        // it as JSON (its floats, again, up to its rounding).
        let written = parsed.text();
        let again = json::parse(written.as_bytes()).unwrap();
        assert!(same(&again, &parsed), "{written}");
        let serde_again: serde_json::Value = serde_json::from_str(&written).unwrap();
        let mut misses = Vec::new();
        float_differences(&parsed, &serde_again, &mut misses);
        if misses.is_empty() {
            assert_eq!(
                serde_again,
                serde_json::Value::from(parsed.clone()),
                "{written}"
            );
        }
    }
    // Where serde_json's reader lands off the correctly rounded value (what
    // std, and JavaScript's Number(), read): the reason for the lean reader.
    eprintln!(
        "{} floats serde_json reads off by rounding; e.g. {:?}",
        differences.len(),
        &differences[..differences.len().min(3)]
    );
    for (correct, serde) in &differences {
        // Same sign and a few units in the last place apart.
        assert!(
            correct.to_bits().abs_diff(serde.to_bits()) <= 4,
            "{correct:e} vs {serde:e}"
        );
    }
}

#[test]
fn damaged_documents_fail_as_serde_json_fails_them() {
    let mut rng = Rng(0x0fed_cba9_8765_4321);
    let damage = b" \n\t\"\\{}[],:.-+0123456789eEnultrfas\x00\x1f\x7f\xc3\xa9\xff";
    for _ in 0..50_000 {
        let (text, _) = document(&mut rng, 0);
        let mut bytes = text.into_bytes();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(bytes.len() as u64 + 1) as usize;
            let byte = damage[rng.below(damage.len() as u64) as usize];
            match rng.below(3) {
                0 if at < bytes.len() => {
                    bytes.remove(at);
                }
                1 if at < bytes.len() => bytes[at] = byte,
                _ => bytes.insert(at, byte),
            }
        }
        let ours = json::parse(&bytes);
        let theirs = serde_json::from_slice::<serde_json::Value>(&bytes);
        match (&ours, &theirs) {
            (Err(a), Err(b)) => assert_eq!(
                a.to_string(),
                b.to_string(),
                "{:?}",
                String::from_utf8_lossy(&bytes)
            ),
            (Ok(a), Ok(b)) => {
                let mut differences = Vec::new();
                float_differences(a, b, &mut differences);
                if differences.is_empty() {
                    assert_eq!(
                        &serde_json::Value::from(a.clone()),
                        b,
                        "{:?}",
                        String::from_utf8_lossy(&bytes)
                    );
                }
            }
            _ => {
                // Only a number at the edge of f64's range may parse on one
                // side and overflow on the other.
                let message = ours
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .or(theirs.as_ref().err().map(ToString::to_string))
                    .unwrap();
                assert!(
                    message.starts_with("number out of range"),
                    "{:?}: {ours:?} vs {theirs:?}",
                    String::from_utf8_lossy(&bytes)
                );
            }
        }
    }
}

#[test]
fn edges_read_as_serde_json_reads_them() {
    let mut rounding = Vec::new();
    for text in [
        "0",
        "-0",
        "-0.0",
        "0.0e0",
        "1e-400",
        "-1e-400",
        "1e308",
        "1e309",
        "-1e309",
        "1.7976931348623157e308",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "18446744073709551615",
        "18446744073709551616",
        "-9223372036854775808",
        "-9223372036854775809",
        "1e2147483647",
        "1e2147483648",
        "1e-2147483648",
        "0e2147483648",
        "-0e99999999999",
        "1.5e99999999999",
        "12.5e-99999999999",
        "\"\\ud83d\\ude00\"",
        "\"\\ud83d\"",
        "\"\\ud83d\\u0041\"",
        "\"\\ud83d\\\"",
        "\"\\ud83d x\"",
        "\"\\udc00\"",
        "\"\\u12\"",
        "\"\\uGGGG\"",
        "\"\\",
        "\"abc",
        "[1,2",
        "[1 2]",
        "{\"a\" 1}",
        "{\"a\":1 \"b\":2}",
        "{1:2}",
        "{\"a\":1,}",
        "[1,]",
        "[,1]",
        "tru",
        "nul",
        "fals",
        "truex",
        "\n\n  [\n 1,\n x]",
        "\u{feff}1",
        "1 2",
        "{\"a\":{\"b\":[1,{\"c\":}]}}",
    ] {
        let ours = json::parse(text.as_bytes())
            .map(serde_json::Value::from)
            .map_err(|e| e.to_string());
        let theirs = serde_json::from_str::<serde_json::Value>(text).map_err(|e| e.to_string());
        if ours != theirs {
            // Only where serde_json's reader misses the correctly rounded
            // value: there ours is std's, and JavaScript's Number(text).
            let std = text.parse::<f64>().unwrap();
            assert_eq!(ours, Ok(serde_json::Value::from(std)), "{text:?}");
            rounding.push(text);
        }
    }
    assert_eq!(
        rounding,
        ["1.7976931348623158e308"],
        "serde_json's reader overflows below f64::MAX + half an ulp"
    );
    // Nesting: serde_json's limit of 128.
    for depth in [126, 127, 128, 129] {
        let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        let ours = json::parse(text.as_bytes())
            .map_err(|e| e.to_string())
            .err();
        let theirs = serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|e| e.to_string())
            .err();
        assert_eq!(ours, theirs, "depth {depth}");
    }
}

#[test]
fn accessors_and_comparisons_answer_as_serde_json_does() {
    let text = br#"{"tag":2,"float":2.0,"neg":-3,"big":18446744073709551615,"s":"x","t":true,"n":null,"a":[1,"two"]}"#;
    let ours = json::parse(text).unwrap();
    let theirs: serde_json::Value = serde_json::from_slice(text).unwrap();
    for key in ["tag", "float", "neg", "big", "s", "t", "n", "a", "missing"] {
        let (a, b) = (&ours[key], &theirs[key]);
        assert_eq!(a.as_u64(), b.as_u64(), "{key}");
        assert_eq!(a.as_i64(), b.as_i64(), "{key}");
        assert_eq!(a.as_f64(), b.as_f64(), "{key}");
        assert_eq!(a.as_str(), b.as_str(), "{key}");
        assert_eq!(a.as_bool(), b.as_bool(), "{key}");
        assert_eq!(a.is_null(), b.is_null(), "{key}");
        assert_eq!(*a == 2, *b == 2, "{key}");
        assert_eq!(*a == true, *b == true, "{key}");
        assert_eq!(*a == "x", *b == "x", "{key}");
    }
    assert_eq!(ours["a"][1], "two");
    assert!(ours["a"][9].is_null());
    let mut built = json::object([("op", "answer".into()), ("id", 7u64.into())]);
    built["op"] = "resume".into();
    built["extra"] = Json::Array(vec![Json::Null]);
    assert_eq!(built.text(), r#"{"extra":[null],"id":7,"op":"resume"}"#);
}
