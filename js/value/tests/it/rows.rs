//! A reply that repeats its last reply's list elements reuses their values
//! (`reply_reusing`, rt1's crossing plan): the answer is exactly a
//! full decode's, element by element, whatever changed between replies.
use exact_js_value::{reply_from_json_text, reply_reusing, Rows, Shape};
use exact_plan::{Items, Value};
use std::cell::RefCell;

/// A transcript window: rows of a record shape with a nested list, an option
/// and numbers, beside fields of the window's own.
fn window() -> Shape {
    let row = Shape::Record(vec![
        ("id".into(), Shape::String),
        ("text".into(), Shape::String),
        ("at".into(), Shape::Number),
        ("edited".into(), Shape::Option(Box::new(Shape::Number))),
        (
            "reactions".into(),
            Shape::List(Box::new(Shape::Record(vec![
                ("emoji".into(), Shape::String),
                ("count".into(), Shape::Number),
            ]))),
        ),
        ("isNew".into(), Shape::Bool),
    ]);
    Shape::Record(vec![
        ("rows".into(), Shape::List(Box::new(row))),
        ("firstId".into(), Shape::String),
        ("hasBefore".into(), Shape::Bool),
    ])
}

/// A deterministic generator (no dependency): xorshift64*.
struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

#[derive(Clone)]
struct Row {
    id: u64,
    text: String,
    at: f64,
    edited: Option<f64>,
    reactions: Vec<(String, u32)>,
}

fn row_json(row: &Row, new: bool) -> String {
    let reactions: Vec<String> = row
        .reactions
        .iter()
        .map(|(e, c)| {
            format!(
                r#"{{"emoji":{},"count":{c}}}"#,
                serde_json::to_string(e).unwrap()
            )
        })
        .collect();
    format!(
        r#"{{"id":"{}","text":{},"at":{},"edited":{},"reactions":[{}],"isNew":{new}}}"#,
        row.id,
        serde_json::to_string(&row.text).unwrap(),
        row.at,
        row.edited.map_or("null".to_string(), |e| e.to_string()),
        reactions.join(",")
    )
}

fn reply(rows: &[Row], since: Option<u64>) -> String {
    let body: Vec<String> = rows
        .iter()
        .map(|r| row_json(r, since == Some(r.id)))
        .collect();
    format!(
        r#"{{"tag":0,"value":{{"rows":[{}],"firstId":"{}","hasBefore":true}}}}"#,
        body.join(","),
        rows.first().map_or(0, |r| r.id)
    )
}

fn equal(a: &exact_js_value::Reply, b: &exact_js_value::Reply, context: &str) {
    assert_eq!(a.fields, b.fields, "{context}");
    assert_eq!(
        a.value.as_ref().map(Value::to_bytes),
        b.value.as_ref().map(Value::to_bytes),
        "{context}"
    );
}

fn records(reply: &exact_js_value::Reply) -> Vec<Items> {
    let Ok(Value::Record(window)) = &reply.value else {
        panic!("a window")
    };
    let Value::List(rows) = &window.as_slice()[0] else {
        panic!("rows")
    };
    rows.as_slice()
        .iter()
        .map(|r| match r {
            Value::Record(items) => items.clone(),
            _ => panic!("a row"),
        })
        .collect()
}

#[test]
fn a_random_sequence_of_windows_decodes_as_a_full_decode_does() {
    let shape = window();
    let kept = RefCell::new(Rows::default());
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let mut next_id = 1000u64;
    let fresh = |random: &mut Random, id: u64| Row {
        id,
        text: [
            "hi",
            "a \"quoted\" word",
            "line\nbreak",
            "émoji 🎉",
            "back\\slash",
            "{[not]}",
        ][random.below(6) as usize]
            .to_string(),
        at: (random.below(1_000_000) as f64) / 7.0,
        edited: (random.below(3) == 0).then(|| random.below(100) as f64),
        reactions: (0..random.below(3))
            .map(|i| (format!("e{i}"), random.below(9) as u32))
            .collect(),
    };
    let mut rows: Vec<Row> = (0..60)
        .map(|_| {
            next_id += 1;
            fresh(&mut random, next_id)
        })
        .collect();
    let mut reused = 0usize;
    for step in 0..400 {
        match random.below(7) {
            // A page lands before the first row (shedding past 300).
            0 => {
                for _ in 0..50 {
                    next_id += 1;
                    rows.insert(0, fresh(&mut random, next_id));
                }
                rows.truncate(300);
            }
            // A message arrives at the end.
            1 => {
                next_id += 1;
                rows.push(fresh(&mut random, next_id));
            }
            // An edit, a reaction, a deletion.
            2 if !rows.is_empty() => {
                let i = random.below(rows.len() as u64) as usize;
                rows[i].text.push_str(" (edited)");
                rows[i].edited = Some(step as f64);
            }
            3 if !rows.is_empty() => {
                let i = random.below(rows.len() as u64) as usize;
                rows[i].reactions.push(("👍".into(), 1));
            }
            4 if rows.len() > 1 => {
                let i = random.below(rows.len() as u64) as usize;
                rows.remove(i);
            }
            // The same window again (a commit that re-reads it).
            _ => {}
        }
        // `since` moves: the New divider's row is a different element.
        let since = (!rows.is_empty() && random.below(4) == 0)
            .then(|| rows[random.below(rows.len() as u64) as usize].id);
        let text = reply(&rows, since);
        let cached = reply_reusing(&text, &shape, &kept).unwrap();
        let full = reply_from_json_text(&text, &shape).unwrap();
        equal(&cached, &full, &format!("step {step}"));
        assert!(
            kept.borrow().len() <= rows.len(),
            "step {step}: one reply's elements at most"
        );
        // A second decode of the same reply reuses every element.
        let again = reply_reusing(&text, &shape, &kept).unwrap();
        equal(&again, &full, &format!("step {step}, again"));
        let (first, second) = (records(&cached), records(&again));
        assert!(
            first.iter().zip(&second).all(|(a, b)| Items::ptr_eq(a, b)),
            "step {step}: unchanged rows are the same values"
        );
        reused += first.len();
    }
    assert!(reused > 10_000);
}

#[test]
fn a_reply_that_fails_keeps_nothing_and_the_next_decodes_in_full() {
    let shape = window();
    let kept = RefCell::new(Rows::default());
    let rows: Vec<Row> = (0..20)
        .map(|i| Row {
            id: i,
            text: "t".into(),
            at: 1.0,
            edited: None,
            reactions: vec![],
        })
        .collect();
    let text = reply(&rows, None);
    reply_reusing(&text, &shape, &kept).unwrap();
    assert_eq!(kept.borrow().len(), 20);
    // Malformed after a row the cache holds: the decode's error is the full decode's.
    let broken = text.replacen(r#"],"firstId""#, r#",]"firstId""#, 1);
    let cached = reply_reusing(&broken, &shape, &kept)
        .err()
        .map(|e| e.to_string());
    let full = reply_from_json_text(&broken, &shape)
        .err()
        .map(|e| e.to_string());
    assert!(cached.is_some());
    assert_eq!(cached, full);
    assert_eq!(kept.borrow().len(), 0, "a failed reply drops what was kept");
    // A row whose bytes end early (no closing brace) is the full decode's error too.
    let cut = &text[..text.len() - 40];
    let kept_again = RefCell::new(Rows::default());
    reply_reusing(&text, &shape, &kept_again).unwrap();
    assert_eq!(
        reply_reusing(cut, &shape, &kept_again)
            .err()
            .map(|e| e.to_string()),
        reply_from_json_text(cut, &shape)
            .err()
            .map(|e| e.to_string())
    );
    // A row of the wrong shape is that shape error, not a reused value.
    let wrong = text.replacen(r#""at":1"#, r#""at":"one""#, 1);
    let kept_third = RefCell::new(Rows::default());
    reply_reusing(&text, &shape, &kept_third).unwrap();
    let cached = reply_reusing(&wrong, &shape, &kept_third).unwrap();
    let full = reply_from_json_text(&wrong, &shape).unwrap();
    equal(&cached, &full, "a wrong-shaped row");
    assert!(cached.value.is_err());
    // Its well-shaped rows are kept (each decoded whole), and the next good reply decodes as in full.
    assert_eq!(kept_third.borrow().len(), 19);
    let again = reply_reusing(&text, &shape, &kept_third).unwrap();
    equal(
        &again,
        &reply_from_json_text(&text, &shape).unwrap(),
        "after a wrong-shaped reply",
    );
}

#[test]
fn elements_alike_in_bytes_under_two_shapes_negative_zero_escapes_and_repeated_keys_decode_as_in_full(
) {
    // Two lists in one reply: the same element bytes under a record of a number and of an optional number.
    let a = Shape::Record(vec![
        ("n".into(), Shape::Number),
        ("s".into(), Shape::String),
    ]);
    let b = Shape::Record(vec![
        ("n".into(), Shape::Option(Box::new(Shape::Number))),
        ("s".into(), Shape::String),
    ]);
    let shape = Shape::Record(vec![
        ("a".into(), Shape::List(Box::new(a))),
        ("b".into(), Shape::List(Box::new(b))),
        (
            "c".into(),
            Shape::List(Box::new(Shape::Option(Box::new(Shape::Record(vec![(
                "n".into(),
                Shape::Number,
            )]))))),
        ),
    ]);
    // -0, \u escapes (one a quote), a repeated key (the last stands), and a list of optional records.
    let element = |i: u32| {
        format!(
            r#"{{"n":-0,"s":"\u0041\u0022 {i} \\ \/ and enough text to pass 64 bytes","n":{i}}}"#
        )
    };
    let list: Vec<String> = (0..20).map(element).collect();
    let text = format!(
        r#"{{"tag":0,"value":{{"a":[{}],"b":[{}],"c":[{}]}}}}"#,
        list.join(","),
        list.join(","),
        (0..20)
            .map(|i| if i % 3 == 0 {
                "null".to_string()
            } else {
                format!(r#"{{"n":{i}}}"#)
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    let kept = RefCell::new(Rows::default());
    for round in 0..3 {
        let cached = reply_reusing(&text, &shape, &kept).unwrap();
        let full = reply_from_json_text(&text, &shape).unwrap();
        equal(&cached, &full, &format!("round {round}"));
    }
    // Both record lists are kept, each under its own shape; the optional records aren't.
    assert_eq!(kept.borrow().len(), 40);
    // -0 survives a reused value: a kept record (past 64 bytes) holding a lone -0 decodes to -0 again.
    let negative = format!(
        r#"{{"tag":0,"value":{{"a":[{{"n":-0,"s":"{}"}}],"b":[],"c":[]}}}}"#,
        "x".repeat(64)
    );
    let first = reply_reusing(&negative, &shape, &kept).unwrap();
    let again = reply_reusing(&negative, &shape, &kept).unwrap();
    equal(
        &again,
        &reply_from_json_text(&negative, &shape).unwrap(),
        "-0",
    );
    let row = |reply: &exact_js_value::Reply| match &reply.value {
        Ok(Value::Record(window)) => match &window.as_slice()[0] {
            Value::List(a) => match &a.as_slice()[0] {
                Value::Record(row) => row.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let (one, two) = (row(&first), row(&again));
    assert!(Items::ptr_eq(&one, &two), "the record is reused");
    assert!(matches!(two.as_slice()[0], Value::Number(z) if z == 0.0 && z.is_sign_negative()));
}

#[test]
fn an_element_under_64_bytes_is_not_kept() {
    let row = Shape::Record(vec![("id".into(), Shape::String)]);
    let shape = Shape::List(Box::new(row));
    let kept = RefCell::new(Rows::default());
    let short: Vec<String> = (0..20).map(|i| format!(r#"{{"id":"{i}"}}"#)).collect();
    let long: Vec<String> = (0..20)
        .map(|i| format!(r#"{{"id":"{i} {}"}}"#, "x".repeat(64)))
        .collect();
    for (rows, expected) in [(&short, 0), (&long, 20)] {
        let text = format!(r#"{{"tag":0,"value":[{}]}}"#, rows.join(","));
        let cached = reply_reusing(&text, &shape, &kept).unwrap();
        equal(
            &cached,
            &reply_from_json_text(&text, &shape).unwrap(),
            "decoded",
        );
        assert_eq!(kept.borrow().len(), expected);
    }
}
