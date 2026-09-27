//! Keying a changed answer from the previous keys where items are the same
//! objects equals a full re-key (LLP 1053 §0 G8): the same keys, repeats,
//! identity texts, retained heights and measurement generations, and the
//! same window. Each case reports how many rows it keyed.
use super::*;
use crate::instance::collection::rekey::FULL_REKEY;

const N: usize = 1_000;

/// A list of `{id}` records keyed by `id + key_dep`.
fn record_plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let num = b.primitive(TypeKind::Number);
    let item = b.record("Item", &[("id", num), ("bump", num)]);
    let list = b.list(item);
    let zero = b.constant(&Value::Number(0.0));
    // The harness supplies the slots; a record literal needs code.
    let initial = b.constant(&Value::list(Vec::new()));
    let data = b.slot("items", list, initial);
    let _body = b.slot("body", num, zero);
    let key_dep = b.slot("key_dep", num, zero);
    let enabled = b.constant(&Value::Bool(true));
    let height = b.constant(&Value::Number(320.0));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[
            binding(BindingKind::Prop, PropId::Virtualized as u16, enabled),
            binding(
                BindingKind::Style,
                exact_kernel::StyleId::Height as u16,
                height,
            ),
        ],
        &[],
        None,
    );
    let subject = code(&mut b, |a| {
        a.load_slot(data);
    });
    let key = code(&mut b, |a| {
        a.load_item(0)
            .field(0)
            .load_slot(key_dep)
            .op(Opcode::Add, &[]);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let text = code(&mut b, |a| {
        a.load_item(0).field(0).call(exact_plan::Stdlib::ToString);
    });
    b.node(
        NodeType::Text as u8,
        None,
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, text)],
        &[],
        None,
    );
    b.finish().unwrap()
}

fn record(id: usize) -> Value {
    Value::record(vec![Value::Number(id as f64), Value::Number(0.0)])
}

/// The row `id` with new content: a new object under the same key.
fn bumped(id: usize) -> Value {
    Value::record(vec![Value::Number(id as f64), Value::Number(1.0)])
}

fn records(ids: std::ops::Range<usize>) -> Value {
    Value::list(ids.map(record).collect())
}

/// A harness scrolled to the middle of the first rows with them measured,
/// so retained heights and tokens are visible in the comparison.
fn harness() -> Harness {
    let plan = record_plan();
    let mut h = Harness::from_parts(
        plan,
        vec![records(0..N), Value::Number(0.0), Value::Number(0.0)],
    );
    h.send(h.feedback(640.0));
    let s = h.snapshot();
    let mut feedback = h.feedback(640.0);
    feedback.measurements = s
        .rows
        .iter()
        .map(|r| RowMeasurement {
            view: r.view,
            epoch: r.epoch,
            height: 20.0 + r.index as f64,
        })
        .collect();
    h.send(feedback);
    h
}

fn items(h: &Harness) -> Vec<Value> {
    let Value::List(items) = &h.slots[0] else {
        unreachable!()
    };
    items.to_vec()
}

/// Everything a re-key decides.
fn state(h: &Harness) -> String {
    let c = h.collection();
    format!(
        "{:?}\n{:?}\n{}\n{}\n{:?}",
        c.keys,
        c.dups,
        c.string_keys,
        c.index.fingerprint(),
        h.snapshot()
    )
}

/// Apply `edit` to the previous answer's items (sharing them) on two equal
/// harnesses, one forced down the full path; the states must be equal.
/// Returns the rows the shared path keyed.
fn keyed(edit: impl Fn(&mut Vec<Value>)) -> usize {
    let mut results = Vec::new();
    for full in [true, false] {
        let mut h = harness();
        let mut next = items(&h);
        edit(&mut next);
        h.slots[0] = Value::list(next);
        FULL_REKEY.with(|f| f.set(full));
        let result = h.update();
        FULL_REKEY.with(|f| f.set(false));
        result.unwrap();
        results.push((state(&h), h.tree.last_work.rows_keyed));
    }
    assert_eq!(
        results[0].0, results[1].0,
        "the shared re-key equals a full one"
    );
    results[1].1
}

#[test]
fn an_insert_at_the_top_keys_one_row() {
    assert_eq!(keyed(|v| v.insert(0, record(5_000))), 1);
}

#[test]
fn an_insert_in_the_middle_keys_one_row() {
    assert_eq!(keyed(|v| v.insert(N / 2, record(5_000))), 1);
    assert_eq!(keyed(|v| v.push(record(5_000))), 1);
}

#[test]
fn a_removal_keys_nothing() {
    assert_eq!(keyed(|v| drop(v.remove(300))), 0);
    assert_eq!(keyed(|v| drop(v.remove(0))), 0);
    assert_eq!(keyed(|v| v.truncate(10)), 0);
    assert_eq!(keyed(|v| v.clear()), 0);
}

#[test]
fn a_replacement_keys_the_replaced_row() {
    // The same length keeps the positional path; a live tick adds a row.
    assert_eq!(keyed(|v| v[700] = bumped(700)), 1);
    assert_eq!(keyed(|v| v[12] = bumped(12)), 1);
    assert_eq!(
        keyed(|v| {
            v[12] = bumped(12);
            v.insert(0, record(5_000));
        }),
        2
    );
    // A new object under a new key, beside a removal.
    assert_eq!(
        keyed(|v| {
            v[12] = record(5_000);
            v.remove(400);
        }),
        1
    );
}

#[test]
fn a_reorder_keys_what_it_moved() {
    // A changed length finds moved objects by identity: nothing keyed.
    assert_eq!(
        keyed(|v| {
            v.reverse();
            v.pop();
        }),
        0
    );
    assert_eq!(
        keyed(|v| {
            let moved = v.remove(5);
            v.insert(900, moved);
            v.push(record(5_000));
        }),
        1
    );
    // The same length keeps the positional path: it keys the rows that
    // moved between the two positions.
    assert_eq!(
        keyed(|v| {
            let moved = v.remove(5);
            v.insert(15, moved);
        }),
        11
    );
}

#[test]
fn a_repeated_key_takes_the_full_path_and_the_same_order() {
    // A new object repeating a kept key, a kept object twice, and a repeat
    // among the new rows: each is decided by the full path.
    keyed(|v| v.insert(0, record(10)));
    keyed(|v| {
        let again = v[3].clone();
        v.insert(0, again);
    });
    keyed(|v| {
        v.insert(0, record(5_000));
        v.insert(0, record(5_000));
    });
    // Replacing a repeated key's first row re-decides the repeat.
    let mut h = harness();
    let mut next = items(&h);
    next.insert(0, record(20));
    h.slots[0] = Value::list(next);
    h.update().unwrap();
    assert_eq!(h.collection().dups.len(), 1);
    let mut next = items(&h);
    next.remove(21);
    h.slots[0] = Value::list(next);
    h.update().unwrap();
    assert!(h.collection().dups.is_empty());
    assert_eq!(h.collection().index.key(0), Some("n:20"));
}

#[test]
fn a_bad_key_in_a_new_row_is_refused_as_before() {
    let mut h = harness();
    let mut next = items(&h);
    next.insert(
        0,
        Value::record(vec![Value::str("not a number"), Value::Number(0.0)]),
    );
    h.slots[0] = Value::list(next);
    assert!(h.update().is_err());
}

#[test]
fn an_answer_of_new_objects_takes_the_full_path() {
    // Nothing to share: the full path keys every row, as before.
    assert_eq!(
        keyed(|v| {
            *v = (0..N).map(bumped).collect();
            v.insert(0, record(5_000));
        }),
        N + 1
    );
}

/// Items changed in place (the same keys in the same order, as a live tick
/// changes prices): only the changed rows lose their measurements. The rest
/// keep heights and epochs, so the host measures again only what changed,
/// and a change to no mounted row publishes nothing new.
#[test]
fn an_in_place_change_invalidates_only_the_changed_rows() {
    let mut h = harness();
    let before = h.snapshot();
    assert!(before.rows.iter().all(|r| r.measured));
    let shown = before.rows[3].index;
    let mut next = items(&h);
    next[N - 1] = bumped(N - 1);
    h.slots[0] = Value::list(next);
    h.update().unwrap();
    assert_eq!(
        h.snapshot(),
        before,
        "an unmounted row's change publishes nothing"
    );
    let mut next = items(&h);
    next[shown] = bumped(shown);
    h.slots[0] = Value::list(next);
    h.update().unwrap();
    let after = h.snapshot();
    assert_eq!(before.rows.len(), after.rows.len());
    for (a, b) in before.rows.iter().zip(&after.rows) {
        assert_eq!((a.index, a.view, a.height), (b.index, b.view, b.height));
        if a.index == shown {
            assert!(
                !b.measured && a.epoch != b.epoch,
                "the changed row is measured again"
            );
        } else {
            assert!(
                b.measured && a.epoch == b.epoch,
                "row {} keeps its measurement",
                a.index
            );
        }
    }
    assert!(after.revision > before.revision);
    assert_eq!(h.tree.last_work.rows_keyed, 1);
}
