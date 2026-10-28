//! A prepend moves every kept row (LLP 1062 D8): a row re-evaluates for its
//! move only when its body reads its own position (`each item, i in list`);
//! one that reads none keeps its bindings, and still updates for every other
//! input it reads (an outside slot), moved or not.
use super::*;

/// A virtualized list over `items` keyed by item; each row a text of its
/// item, a text of its position when `index`, and a text of the outside slot
/// `mark` when `mark`.
fn rows_plan(n: usize, index: bool, mark: bool) -> Plan {
    plan_with(n, index, mark, true, false)
}

/// [`rows_plan`]'s rows, in a virtualized list or a plain `for` in a view,
/// and when `nested` with a last text from a `for` inside the row over one
/// item that shows the OUTER row's position.
fn plan_with(n: usize, index: bool, mark: bool, virtualized: bool, nested: bool) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let num = b.primitive(TypeKind::Number);
    let list = b.list(num);
    let zero = b.constant(&Value::Number(0.0));
    let initial = b.constant(&values(n));
    let items = b.slot("items", list, initial);
    let marked = b.slot("mark", num, zero);
    let enabled = b.constant(&Value::Bool(true));
    let height = b.constant(&Value::Number(320.0));
    let root = if virtualized {
        b.node(
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
        )
    } else {
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None)
    };
    let subject = code(&mut b, |a| {
        a.load_slot(items);
    });
    let key = code(&mut b, |a| {
        a.load_item(0);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let row = b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    let order = std::cell::Cell::new(0);
    let text = |b: &mut PlanBuilder, emit: &dyn Fn(&mut Asm)| {
        let expr = code(b, |a| {
            emit(a);
            a.call(exact_plan::Stdlib::ToString);
        });
        b.node(
            NodeType::Text as u8,
            Some(row),
            Some(arms[0]),
            order.replace(order.get() + 1),
            &[binding(BindingKind::Prop, PropId::Text as u16, expr)],
            &[],
            None,
        );
    };
    text(&mut b, &|a: &mut Asm| {
        a.load_item(0);
    });
    if index {
        text(&mut b, &|a: &mut Asm| {
            a.load_index(0);
        });
    }
    if mark {
        text(&mut b, &|a: &mut Asm| {
            a.load_slot(marked);
        });
    }
    if nested {
        let subject = b.constant(&values(1));
        let key = code(&mut b, |a| {
            a.load_item(0);
        });
        let (_, inner) = b.region(
            RegionKind::Each,
            Some(row),
            Some(arms[0]),
            order.replace(order.get() + 1),
            subject,
            key,
            1,
        );
        let expr = code(&mut b, |a| {
            a.load_index(1).call(exact_plan::Stdlib::ToString);
        });
        b.node(
            NodeType::Text as u8,
            None,
            Some(inner[0]),
            0,
            &[binding(BindingKind::Prop, PropId::Text as u16, expr)],
            &[],
            None,
        );
    }
    b.finish().unwrap()
}

fn harness(n: usize, index: bool, mark: bool) -> Harness {
    Harness::from_parts(
        rows_plan(n, index, mark),
        vec![values(n), Value::Number(0.0)],
    )
}

/// The list with one item put before item 0.
fn prepended(n: usize) -> Value {
    Value::list(
        std::iter::once(Value::Number(-1.0))
            .chain((0..n).map(|i| Value::Number(i as f64)))
            .collect(),
    )
}

/// The texts under `id`, in order.
fn walk(h: &Harness, id: u32, out: &mut Vec<String>) {
    let node = h.kernel.node(id).unwrap();
    if let Some(s) = node.props.str(PropId::Text) {
        out.push(s.to_string());
    }
    for c in node.children() {
        walk(h, c, out);
    }
}

/// Each mounted row's root and the texts under it, in order, by kernel id.
fn texts(h: &Harness) -> Vec<(u32, Vec<String>)> {
    h.snapshot()
        .rows
        .iter()
        .map(|r| {
            let mut out = Vec::new();
            walk(h, r.root, &mut out);
            (r.root, out)
        })
        .collect()
}

/// Mounted rows kept across the last update (the same wrapper before and after).
fn kept(before: &[(u32, Vec<String>)], after: &[(u32, Vec<String>)]) -> usize {
    after
        .iter()
        .filter(|(root, _)| before.iter().any(|(r, _)| r == root))
        .count()
}

#[test]
fn a_prepend_reevaluates_no_kept_row_that_reads_no_position() {
    let mut h = harness(100, false, false);
    h.send(h.feedback(640.0));
    let before = texts(&h);
    h.slots[0] = prepended(100);
    h.update().unwrap();
    let after = texts(&h);
    let kept = kept(&before, &after);
    assert!(kept >= 8, "the window keeps its rows: {kept}");
    // Only rows mounted by this update evaluate, one binding each.
    let mounted_now = after.len() - kept;
    assert!(
        h.tree.last_work.bindings_evaluated <= mounted_now,
        "{} bindings evaluated for {mounted_now} new rows and {kept} kept ones",
        h.tree.last_work.bindings_evaluated
    );
    // Each kept row still shows its item.
    for (root, t) in &after {
        if let Some((_, was)) = before.iter().find(|(r, _)| r == root) {
            assert_eq!(t, was);
        }
    }
}

#[test]
fn a_prepend_reevaluates_and_renumbers_every_kept_row_that_reads_its_position() {
    let mut h = harness(100, true, false);
    h.send(h.feedback(640.0));
    let before = texts(&h);
    h.slots[0] = prepended(100);
    h.update().unwrap();
    let after = texts(&h);
    let kept = kept(&before, &after);
    assert!(kept >= 8);
    assert!(
        h.tree.last_work.bindings_evaluated >= kept,
        "{} for {kept} kept rows",
        h.tree.last_work.bindings_evaluated
    );
    // Every kept row shows its item and its NEW position: one more than before.
    for (root, t) in &after {
        if let Some((_, was)) = before.iter().find(|(r, _)| r == root) {
            assert_eq!(t[0], was[0]);
            assert_eq!(
                t[1].parse::<f64>().unwrap(),
                was[1].parse::<f64>().unwrap() + 1.0,
                "{t:?} after {was:?}"
            );
        }
    }
}

#[test]
fn a_row_reading_outside_state_updates_when_it_changes_and_its_item_does_not_moved_or_not() {
    // theme2's condition: "unchanged" is every input a row reads, not its item alone.
    let mut h = harness(100, false, true);
    h.send(h.feedback(640.0));
    // Unmoved: the outside slot alone changes.
    h.slots[1] = Value::Number(7.0);
    h.update().unwrap();
    assert!(
        texts(&h).iter().all(|(_, t)| t[1] == "7"),
        "{:?}",
        texts(&h)
    );
    // Moved in the same commit as the outside slot changes.
    h.slots[0] = prepended(100);
    h.slots[1] = Value::Number(8.0);
    h.update().unwrap();
    assert!(
        texts(&h).iter().all(|(_, t)| t[1] == "8"),
        "{:?}",
        texts(&h)
    );
}

#[test]
fn a_row_whose_item_changed_and_moved_shows_its_new_item() {
    let mut h = harness(100, false, false);
    h.send(h.feedback(640.0));
    // Item 25 becomes 25.5 (a new key: a new row) and the rest move by one: kept rows keep their text,
    // and nothing shows an old value.
    let mut items: Vec<Value> = std::iter::once(Value::Number(-1.0))
        .chain((0..100).map(|i| Value::Number(i as f64)))
        .collect();
    items[26] = Value::Number(25.5);
    h.slots[0] = Value::list(items.clone());
    h.update().unwrap();
    for (_, t) in texts(&h) {
        let v: f64 = t[0].parse().unwrap();
        assert!(
            items.iter().any(|i| *i == Value::Number(v)),
            "{v} is not in the list"
        );
    }
}

#[test]
fn a_kept_row_that_reads_no_position_still_publishes_its_new_place() {
    // The wrapper's aria-posinset and aria-setsize are the collection's, published on every settle whatever the
    // row's bindings (views::publish_position): a prepend moves them on.
    let mut h = harness(100, false, false);
    h.send(h.feedback(640.0));
    let at = |h: &Harness, view: u32| {
        let w = h.kernel.node(view).unwrap();
        (
            w.props.get(PropId::AccessibilityPosInSet).cloned(),
            w.props.get(PropId::AccessibilitySetSize).cloned(),
        )
    };
    let row = h.snapshot().rows[3].view;
    let (Some(PropValue::Int(was)), Some(PropValue::Int(100))) = at(&h, row) else {
        panic!("{:?}", at(&h, row))
    };
    h.slots[0] = prepended(100);
    h.update().unwrap();
    assert_eq!(
        at(&h, row),
        (Some(PropValue::Int(was + 1)), Some(PropValue::Int(101)))
    );
}

#[test]
fn a_grouped_lists_old_first_row_is_no_longer_first_after_a_prepend() {
    // A grouped list's first and last rows (their corners and separators) are the kernel's, from the children's order
    // (Kernel::grouped_list): a prepend's new first row is first there, though the old first row reads no position.
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let num = b.primitive(TypeKind::Number);
    let list = b.list(num);
    let initial = b.constant(&values(5));
    let items = b.slot("items", list, initial);
    let grouped = b.constant(&Value::str("inset-grouped"));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[binding(
            BindingKind::Prop,
            PropId::ListStyle as u16,
            grouped,
        )],
        &[],
        None,
    );
    let section = b.node(NodeType::View as u8, Some(root), None, 0, &[], &[], None);
    let part = b.node(NodeType::View as u8, Some(section), None, 0, &[], &[], None);
    let subject = code(&mut b, |a| {
        a.load_slot(items);
    });
    let key = code(&mut b, |a| {
        a.load_item(0);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(part), None, 0, subject, key, 1);
    let text = code(&mut b, |a| {
        a.load_item(0).call(exact_plan::Stdlib::ToString);
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
    let mut h = Harness::from_parts(b.finish().unwrap(), vec![values(5)]);
    let list_view = |h: &Harness| match &h.tree.children[0] {
        Child::Node(n) => n.view,
        _ => panic!(),
    };
    let rows = |h: &Harness| {
        h.kernel.grouped_list(list_view(h)).unwrap().sections[0]
            .rows
            .iter()
            .map(|r| r.view)
            .collect::<Vec<_>>()
    };
    let before = rows(&h);
    assert_eq!(before.len(), 5);
    h.slots[0] = prepended(5);
    h.update().unwrap();
    let after = rows(&h);
    assert_eq!(after.len(), 6);
    assert!(!before.contains(&after[0]), "the new row is first");
    assert_eq!(after[1], before[0], "the old first row is second");
    assert_eq!(after[5], before[4], "and the last row is still last");
}

/// The texts of each row of a plain `for` in the root view, by kernel id.
fn plain_texts(h: &Harness) -> Vec<(u32, Vec<String>)> {
    let Child::Node(root) = &h.tree.children[0] else {
        panic!()
    };
    h.kernel
        .node(root.view)
        .unwrap()
        .children()
        .into_iter()
        .map(|row| {
            let mut out = Vec::new();
            walk(h, row, &mut out);
            (row, out)
        })
        .collect()
}

fn plain(n: usize, index: bool, mark: bool, nested: bool) -> Harness {
    Harness::from_parts(
        plan_with(n, index, mark, false, nested),
        vec![values(n), Value::Number(0.0)],
    )
}

#[test]
fn a_plain_for_prepend_reevaluates_no_kept_row_that_reads_no_position() {
    let mut h = plain(20, false, false, false);
    let before = plain_texts(&h);
    h.slots[0] = prepended(20);
    h.update().unwrap();
    let after = plain_texts(&h);
    assert_eq!(kept(&before, &after), 20);
    assert!(
        h.tree.last_work.bindings_evaluated <= 1,
        "{} bindings evaluated for 1 new row",
        h.tree.last_work.bindings_evaluated
    );
    assert_eq!(after[0].1, ["-1"]);
    assert_eq!(&after[1..], &before[..]);
}

#[test]
fn a_plain_for_prepend_renumbers_every_kept_row_that_reads_its_position() {
    let mut h = plain(20, true, false, false);
    let before = plain_texts(&h);
    h.slots[0] = prepended(20);
    h.update().unwrap();
    let after = plain_texts(&h);
    assert_eq!(kept(&before, &after), 20);
    for (i, (_, t)) in after.iter().enumerate() {
        assert_eq!(t[1], i.to_string(), "{after:?}");
    }
}

#[test]
fn a_plain_for_row_reading_outside_state_updates_when_it_changes_and_its_item_does_not_moved_or_not(
) {
    let mut h = plain(20, false, true, false);
    h.slots[1] = Value::Number(7.0);
    h.update().unwrap();
    assert!(plain_texts(&h).iter().all(|(_, t)| t[1] == "7"));
    h.slots[0] = prepended(20);
    h.slots[1] = Value::Number(8.0);
    h.update().unwrap();
    assert!(
        plain_texts(&h).iter().all(|(_, t)| t[1] == "8"),
        "{:?}",
        plain_texts(&h)
    );
}

#[test]
fn a_row_whose_inner_for_reads_the_outer_position_renumbers_after_a_prepend() {
    // The outer row's body reads no position itself; its inner `for` reads it one scope out.
    for virtualized in [false, true] {
        let mut h = Harness::from_parts(
            plan_with(100, false, false, virtualized, true),
            vec![values(100), Value::Number(0.0)],
        );
        let read = |h: &Harness| {
            if virtualized {
                texts(h)
            } else {
                plain_texts(h)
            }
        };
        if virtualized {
            // Mounting rows keys each one's inner `for`.
            h.try_send_filled(h.feedback(640.0), CollectionFill::default())
                .unwrap();
        }
        let before = read(&h);
        h.slots[0] = prepended(100);
        h.update().unwrap();
        let after = read(&h);
        assert!(kept(&before, &after) >= 8);
        for (root, t) in &after {
            if let Some((_, was)) = before.iter().find(|(r, _)| r == root) {
                assert_eq!(t[0], was[0]);
                assert_eq!(
                    t[1].parse::<f64>().unwrap(),
                    was[1].parse::<f64>().unwrap() + 1.0,
                    "virtualized {virtualized}: {t:?} after {was:?}"
                );
            }
        }
    }
}

/// A plain `for` over `{id, body}` records keyed by `id`; each row a text of
/// its `id` and a text of its `body`.
fn record_plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let num = b.primitive(TypeKind::Number);
    let item = b.record("Item", &[("id", num), ("body", num)]);
    let list = b.list(item);
    let initial = b.constant(&Value::list(Vec::new()));
    let data = b.slot("items", list, initial);
    let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let subject = code(&mut b, |a| {
        a.load_slot(data);
    });
    let key = code(&mut b, |a| {
        a.load_item(0).field(0);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let row = b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    for field in 0..2 {
        let text = code(&mut b, |a| {
            a.load_item(0)
                .field(field)
                .call(exact_plan::Stdlib::ToString);
        });
        b.node(
            NodeType::Text as u8,
            Some(row),
            Some(arms[0]),
            field as u32,
            &[binding(BindingKind::Prop, PropId::Text as u16, text)],
            &[],
            None,
        );
    }
    b.finish().unwrap()
}

fn records(items: impl Iterator<Item = (i32, i32)>) -> Value {
    Value::list(
        items
            .map(|(id, body)| {
                Value::record(vec![Value::Number(id as f64), Value::Number(body as f64)])
            })
            .collect(),
    )
}

#[test]
fn a_moved_row_whose_item_changed_reevaluates_only_its_changed_fields_readers() {
    let mut h = Harness::from_parts(record_plan(), vec![records((0..20).map(|i| (i, 0)))]);
    let before = plain_texts(&h);
    // A row prepended, and row 5's body changed in the same commit.
    h.slots[0] =
        records(std::iter::once((-1, 0)).chain((0..20).map(|i| (i, if i == 5 { 1 } else { 0 }))));
    h.update().unwrap();
    let after = plain_texts(&h);
    assert_eq!(kept(&before, &after), 20);
    // The new row's two texts and row 5's body: not row 5's id, nor any other kept row.
    assert_eq!(h.tree.last_work.bindings_evaluated, 3);
    assert_eq!(after[6].1, ["5", "1"]);
    assert_eq!(after[7].1, ["6", "0"]);
}

#[test]
fn after_a_prepend_the_rows_equal_a_fresh_build_of_the_new_list() {
    for (index, mark, nested) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let mut h = plain(20, index, mark, nested);
        h.slots[1] = Value::Number(3.0);
        h.update().unwrap();
        h.slots[0] = prepended(20);
        h.update().unwrap();
        // Built from the new list and slot at creation: no row of it was ever updated.
        let fresh = Harness::from_parts(
            plan_with(20, index, mark, false, nested),
            vec![prepended(20), Value::Number(3.0)],
        );
        let shown = |h: &Harness| {
            plain_texts(h)
                .into_iter()
                .map(|(_, t)| t)
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&fresh).len(), 21);
        assert_eq!(
            shown(&h),
            shown(&fresh),
            "index {index}, mark {mark}, nested {nested}"
        );
    }
}
