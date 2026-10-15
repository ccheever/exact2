use super::*;
use exact_plan::Items;

fn rows(value: &Value) -> &[Value] {
    match &fields(value)[0] {
        Value::List(rows) => rows,
        _ => panic!("feed rows"),
    }
}
fn baked() -> Value {
    // Exercise the same canonical Value decoder used for the baked resource.
    let initial = Heavy::default().query("messages", &[]).unwrap();
    Value::from_bytes(&initial.to_bytes()).unwrap()
}
fn same_record(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Record(a), Value::Record(b)) => Items::ptr_eq(a, b),
        _ => false,
    }
}

#[test]
fn adopted_feed_is_ready_and_uses_the_one_baked_allocation() {
    let seed = baked();
    let mut h = Heavy::default();
    assert!(h.feed.is_none());
    h.adopt("messages", &[], &seed);
    assert!(h.feed.is_some());
    let Rows::Adopted(owned) = &h.feed.as_ref().unwrap().rows else {
        panic!("whole adopted lease")
    };
    assert!(same_record(owned, &seed));
    let got = h.query("messages", &[]).unwrap();
    assert_eq!(rows(&got).len(), 10_000);
    assert_eq!(got, seed);
    assert!(rows(&got)
        .iter()
        .zip(rows(&seed))
        .all(|(a, b)| same_record(a, b)));
    assert!(h.feed.as_ref().unwrap().live.is_empty());
    assert_eq!(h.feed.as_ref().unwrap().ticks, 0);
}

#[test]
fn react_and_live_keep_unmodified_record_and_field_ownership() {
    let seed = baked();
    let mut h = Heavy::default();
    h.adopt("messages", &[], &seed);
    let first = rows(&seed)
        .iter()
        .find(|m| !Feed::reactions(m).is_empty())
        .unwrap();
    let id = fields(first)[ID].as_str().unwrap();
    let emoji = fields(&Feed::reactions(first)[0])[0].as_str().unwrap();
    let old_count = fields(&Feed::reactions(first)[0])[1].as_number().unwrap();
    let reacted = h
        .query("react", &[Value::str(id), Value::str(emoji)])
        .unwrap();
    let changed: Vec<_> = rows(&seed)
        .iter()
        .zip(rows(&reacted))
        .filter(|(a, b)| !same_record(a, b))
        .collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(
        fields(&Feed::reactions(changed[0].1)[0])[1].as_number(),
        Some(old_count + 1.0)
    );
    for at in 0..REACTIONS {
        assert!(exact_runner::compare::same(
            &fields(changed[0].0)[at],
            &fields(changed[0].1)[at]
        ));
    }
    let ticked = h.query("tick", &[]).unwrap();
    assert_eq!(rows(&ticked).len(), 10_001);
    assert_eq!(fields(&rows(&ticked)[0])[ID].as_str(), Some("live-1"));
    assert_eq!(
        fields(&rows(&ticked)[0])[MINUTES_AGO].as_number(),
        Some(0.0)
    );
    for at in [1, 2, 4, 5, 6, 7, 8] {
        assert!(exact_runner::compare::same(
            &fields(&rows(&ticked)[0])[at],
            &fields(&rows(&reacted)[37])[at]
        ));
    }
    let next = h.query("tick", &[]).unwrap();
    assert_eq!(rows(&next).len(), 10_002);
    assert_eq!(
        rows(&ticked)
            .iter()
            .zip(&rows(&next)[1..])
            .filter(|(a, b)| !same_record(a, b))
            .count(),
        1
    );
    let live = rows(&next)
        .iter()
        .take(2)
        .find(|m| !Feed::reactions(m).is_empty());
    if let Some(live) = live {
        let id = fields(live)[ID].as_str().unwrap();
        let e = fields(&Feed::reactions(live)[0])[0].as_str().unwrap();
        assert!(h.feed.as_mut().unwrap().react(id, e));
    }
    // Adopting again cannot reset changed runtime state.
    let before = h.query("messages", &[]).unwrap();
    h.adopt("messages", &[], &seed);
    assert_eq!(before, h.query("messages", &[]).unwrap());
}

#[test]
fn unbaked_and_unrelated_adoption_preserve_query_contract() {
    let mut h = Heavy::default();
    h.adopt("config", &[], &Value::record(vec![Value::Bool(false)]));
    h.adopt("messages", &[Value::Number(1.0)], &baked());
    assert!(h.feed.is_none());
    assert!(h.query("messages", &[Value::Number(1.0)]).is_err());
    assert!(h.query("tick", &[Value::Number(1.0)]).is_err());
    assert!(h.query("unknown", &[]).is_err());
    let fallback = h.query("messages", &[]).unwrap();
    assert_eq!(fallback, baked());
    let before = fallback.clone();
    let after = h
        .query("react", &[Value::str("missing"), Value::str("missing")])
        .unwrap();
    assert!(rows(&before)
        .iter()
        .zip(rows(&after))
        .all(|(a, b)| same_record(a, b)));
}
