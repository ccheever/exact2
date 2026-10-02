//! Real Runner collection ownership/preview, with no host or secondary item graph.
use exact_kernel::{Kernel, NodeKey};
use exact_plan::{Items, Value};
use exact_runner::{
    CollectionFeedback, DataError, DataSource, ReorderProgress, RowMeasurement, Runner,
};

struct Rows {
    n: usize,
    queries: usize,
}
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        Ok(Value::list(
            (0..self.n).map(|i| Value::str(&i.to_string())).collect(),
        ))
    }
}
const SOURCE: &str = r#"
component App
  state draft = ""
  state dropped = "initial"
  state count = 0
  state disabled = false
  resource rows = rows() as shape list<string>
  action edit(v: string)
    draft = v
  action block
    disabled = true
  action receive(item: string, before: option<string>)
    dropped = item
    count = count + 1
  view
    column
      input value=draft change=edit
      list id="arrange" testId="list" virtualized=true height=100 reorderdrop=receive
        each x in rows key=x
          column reorderFor="arrange" disabled=disabled testId=`grip-${x}` height=20
            text x
"#;
fn boot(n: usize) -> Runner<Rows> {
    Runner::boot(
        contract::compile(SOURCE).unwrap(),
        Rows { n, queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn key<D: DataSource>(r: &Runner<D>, name: &str) -> NodeKey {
    r.kernel().find_by_test_id(name)[0]
}
fn feedback<D: DataSource>(r: &Runner<D>, pin: Option<NodeKey>, top: f64) -> CollectionFeedback {
    let c = r.collections().remove(0);
    CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: top,
        port_cross: 320.,
        port_main: 100.,
        cross: 320.,
        measurements: c
            .rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: 20.,
            })
            .collect(),
        focus_view: None,
        interaction_view: pin.map(|k| r.kernel().node_by_key(k).unwrap().id),
    }
}
fn ready<D: DataSource>(r: &mut Runner<D>) -> NodeKey {
    let handle = key(r, "grip-0");
    for _ in 0..3 {
        r.collection_feedback(feedback(r, Some(handle), 0.))
            .unwrap();
    }
    handle
}

#[test]
fn preview_is_absolute_bounded_and_metadata_typing_keeps_capability() {
    let mut r = boot(25_000);
    let handle = ready(&mut r);
    let binding = r.reorder_binding(handle).unwrap();
    let g = r.reorder_geometry(binding.list).unwrap();
    let extent = r.collections()[0].total_extent;
    let start = r.begin_reorder(binding, g.clone()).unwrap().unwrap();
    let queries = r.data_ref().queries;
    for y in [80., 35., 80.] {
        let g = r.reorder_geometry(binding.list).unwrap();
        assert!(matches!(
            r.preview_reorder(start.token, g, y).unwrap(),
            ReorderProgress::Accepted { .. }
        ));
        assert_eq!(r.collections()[0].total_extent, extent);
        assert!(r.collections()[0].rows.len() < 40);
        assert_eq!(r.data_ref().queries, queries);
    }
    let shifted: Vec<_> = r.collections()[0]
        .rows
        .iter()
        .map(|row| {
            (
                row.index,
                r.kernel().node(row.view).unwrap().style.translate.y,
            )
        })
        .collect();
    assert!(shifted.iter().any(|&(i, y)| i == 1 && y == -20.));
    r.act("edit", vec![Value::str("unrelated typing")]).unwrap();
    assert!(r.has_reorder(start.token));
    r.cancel_reorder(start.token).unwrap().unwrap();
    assert!(!r.has_reorder(start.token));
    for row in &r.collections()[0].rows {
        assert_eq!(r.kernel().node(row.view).unwrap().style.translate.y, 0.);
    }
    assert!(r.cancel_reorder(start.token).unwrap().is_none());
}

#[test]
fn geometry_facts_not_just_revision_sequence_guard_preview_and_terminal() {
    let mut r = boot(100);
    let handle = ready(&mut r);
    let b = r.reorder_binding(handle).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    let mut f = feedback(&r, Some(handle), 1.);
    f.scroll_sequence = g.scroll_sequence;
    r.collection_feedback(f).unwrap();
    assert!(matches!(
        r.preview_reorder(token, g.clone(), f64::NAN).unwrap(),
        ReorderProgress::Stale
    ));
    assert!(r.drop_reorder(token, g).unwrap().is_none());
    assert_eq!(r.slot("count"), Some(&Value::Number(0.)));
    assert!(r.has_reorder(token));
}

#[test]
fn true_end_is_not_window_end_and_disabled_source_cannot_dispatch() {
    let mut r = boot(25_000);
    let handle = ready(&mut r);
    let b = r.reorder_binding(handle).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g).unwrap().unwrap().token;
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(matches!(
        r.preview_reorder(token, g, 800_000.).unwrap(),
        ReorderProgress::NeedsMeasurement
    ));
    r.act("block", vec![]).unwrap();
    assert!(!r.has_reorder(token));
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(r.drop_reorder(token, g).unwrap().is_none());
    assert_eq!(r.slot("count"), Some(&Value::Number(0.)));
    assert!(!r.is_poisoned());
}

struct EditingRows {
    order: Items,
    calls: usize,
    mode: &'static str,
}
impl DataSource for EditingRows {
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls += 1;
        if name == "move" && self.mode != "refuse" {
            let item = args[0].as_str().unwrap();
            let mut next = self.order.to_vec();
            let source = next.iter().position(|v| v.as_str() == Some(item)).unwrap();
            let value = next.remove(source);
            if self.mode != "delete" {
                let before = match &args[1] {
                    Value::Option(Some(v)) if self.mode != "end" => v.as_str(),
                    _ => None,
                };
                let at = before
                    .and_then(|key| next.iter().position(|v| v.as_str() == Some(key)))
                    .unwrap_or(next.len());
                next.insert(at, value);
            }
            self.order = Items::from(next);
        }
        Ok(Value::List(self.order.clone()))
    }
}
fn editing(mode: &'static str) -> Runner<EditingRows> {
    let source = SOURCE.replace("resource rows = rows() as shape list<string>",
        "resource initial = rows() as shape list<string>\n  mutation changed as shape list<string>\n  derive rows = match changed { case some(value) => value, case none => initial }")
        .replace("    dropped = item", "    send changed = move(item, before)\n    dropped = item");
    Runner::boot(
        contract::compile(&source).unwrap(),
        EditingRows {
            order: Items::from(
                (0..25_000)
                    .map(|i| Value::str(&i.to_string()))
                    .collect::<Vec<_>>(),
            ),
            calls: 0,
            mode,
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn edit_feedback(r: &mut Runner<EditingRows>, handle: NodeKey, top: f64) {
    let c = r.collections().remove(0);
    r.collection_feedback(CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: top,
        port_cross: 320.,
        port_main: 100.,
        cross: 320.,
        focus_view: None,
        interaction_view: r.kernel().node_by_key(handle).map(|n| n.id),
        measurements: c
            .rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: 20.,
            })
            .collect(),
    })
    .unwrap();
}
#[test]
fn own_structural_drop_pins_source_until_explicit_finish_and_dispatches_once() {
    let mut r = editing("end");
    let h = r.kernel().find_by_test_id("grip-1")[0];
    for _ in 0..3 {
        edit_feedback(&mut r, h, 0.);
    }
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g).unwrap().unwrap().token;
    let old = r.data_ref().order.clone();
    let calls = r.data_ref().calls;
    let g = r.reorder_geometry(b.list).unwrap();
    r.preview_reorder(token, g, 90.).unwrap();
    assert!(Items::ptr_eq(&old, &r.data_ref().order));
    assert_eq!(r.data_ref().calls, calls);
    let before = r.reorder_frame(token).unwrap();
    assert!(!before.terminal);
    let g = r.reorder_geometry(b.list).unwrap();
    let receipt = r.drop_reorder(token, g).unwrap().unwrap();
    assert!(!receipt.destroyed.contains(&b.wrapper));
    assert_eq!(r.slot("count"), Some(&Value::Number(1.)));
    assert!(!r.has_reorder(token));
    assert!(r.reorder_frame(token).unwrap().terminal);
    assert_eq!(r.kernel().node_by_key(b.wrapper).unwrap().key, b.wrapper);
    assert_eq!(
        r.collections()[0]
            .rows
            .iter()
            .find(|row| row.view == r.kernel().node_by_key(b.wrapper).unwrap().id)
            .unwrap()
            .index,
        24_999
    );
    edit_feedback(&mut r, h, 0.);
    edit_feedback(&mut r, h, 0.);
    assert!(
        r.kernel().node_by_key(b.wrapper).is_some(),
        "layout/feedback must retain terminal source"
    );
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(r.drop_reorder(token, g).unwrap().is_none());
    assert_eq!(r.slot("count"), Some(&Value::Number(1.)));
    r.finish_reorder(token).unwrap();
    assert!(
        r.kernel().node_by_key(b.wrapper).is_none(),
        "finish releases offscreen source"
    );
    assert!(r.reorder_frame(token).is_none());
    assert!(r.finish_reorder(token).unwrap().is_none());
}
#[test]
fn deletion_and_normal_model_refusal_both_have_bounded_terminal_cleanup() {
    for mode in ["delete", "refuse"] {
        let mut r = editing(mode);
        let h = r.kernel().find_by_test_id("grip-0")[0];
        for _ in 0..3 {
            edit_feedback(&mut r, h, 0.);
        }
        let b = r.reorder_binding(h).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let token = r.begin_reorder(b, g).unwrap().unwrap().token;
        let old = r.data_ref().order.clone();
        let g = r.reorder_geometry(b.list).unwrap();
        r.preview_reorder(token, g, 90.).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        r.drop_reorder(token, g).unwrap().unwrap();
        assert_eq!(r.slot("count"), Some(&Value::Number(1.)));
        if mode == "refuse" {
            assert!(Items::ptr_eq(&old, &r.data_ref().order));
        } else {
            assert!(r.kernel().node_by_key(h).is_none());
        }
        r.finish_reorder(token).unwrap();
        assert!(r.reorder_frame(token).is_none());
        assert!(!r.is_poisoned());
    }
}
#[test]
fn successor_pin_and_token_survive_old_finish_and_width_cancellation() {
    let mut r = boot(25_000);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let old = r.begin_reorder(b, g).unwrap().unwrap().token;
    let other = key(&r, "grip-1");
    r.collection_feedback(feedback(&r, Some(other), 0.))
        .unwrap();
    assert!(!r.has_reorder(old));
    r.finish_reorder(old).unwrap();
    let b2 = r.reorder_binding(other).unwrap();
    let g = r.reorder_geometry(b2.list).unwrap();
    let new = r.begin_reorder(b2, g).unwrap().unwrap().token;
    assert!(r.finish_reorder(old).unwrap().is_none());
    assert!(r.has_reorder(new));
    let mut f = feedback(&r, Some(other), 0.);
    f.cross = 300.;
    r.collection_feedback(f).unwrap();
    assert!(!r.has_reorder(new));
    assert!(r.reorder_frame(new).unwrap().terminal);
    r.finish_reorder(new).unwrap();
    assert!(r.reorder_frame(new).is_none());
}
#[test]
fn live_bad_sample_is_atomic_and_new_window_receives_absolute_targets_without_feedback_loop() {
    let mut r = boot(25_000);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    let epoch = r.kernel().epoch();
    let frame = r.reorder_frame(token);
    assert!(r.preview_reorder(token, g, f64::NAN).is_err());
    assert_eq!(r.kernel().epoch(), epoch);
    assert_eq!(r.reorder_frame(token), frame);
    for _ in 0..3 {
        r.collection_feedback(feedback(&r, Some(h), 80.)).unwrap();
    }
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(matches!(
        r.preview_reorder(token, g, 170.).unwrap(),
        ReorderProgress::Accepted { .. }
    ));
    for row in &r.collections()[0].rows {
        let offset = r.kernel().node(row.view).unwrap().style.translate.y;
        if (1..9).contains(&row.index) {
            assert_eq!(offset, -20.);
        }
        if row.index > 9 {
            assert_eq!(offset, 0.);
        }
    }
    let f = feedback(&r, Some(h), 80.);
    assert!(
        r.collection_feedback(f).unwrap().receipts.is_empty(),
        "equal feedback does not emit unchanged wrapper styles"
    );
}

#[test]
fn changed_measured_source_extent_retires_old_preview_before_reusing_height() {
    let mut r = boot(100);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g).unwrap().unwrap().token;
    let g = r.reorder_geometry(b.list).unwrap();
    r.preview_reorder(token, g, 80.).unwrap();
    let wrapper = r.kernel().node_by_key(b.wrapper).unwrap().id;
    let mut f = feedback(&r, Some(h), 0.);
    f.measurements
        .iter_mut()
        .find(|m| m.view == wrapper)
        .unwrap()
        .size = 40.;
    r.collection_feedback(f).unwrap();
    assert!(!r.has_reorder(token));
    assert!(r.reorder_frame(token).unwrap().terminal);
    r.finish_reorder(token).unwrap();
}
#[test]
fn measured_far_focus_pin_does_not_certify_remote_end_as_visible_gap() {
    let mut r = boot(25_000);
    let h = ready(&mut r);
    for _ in 0..4 {
        let end = (r.collections()[0].total_extent - 100.).max(0.);
        r.collection_feedback(feedback(&r, Some(h), end)).unwrap();
    }
    let last = key(&r, "grip-24999");
    let last_id = r.kernel().node_by_key(last).unwrap().id;
    for _ in 0..3 {
        let mut f = feedback(&r, Some(h), 0.);
        f.focus_view = Some(last_id);
        r.collection_feedback(f).unwrap();
    }
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let end = g.total_extent;
    let token = r.begin_reorder(b, g).unwrap().unwrap().token;
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(matches!(
        r.preview_reorder(token, g, end).unwrap(),
        ReorderProgress::NeedsMeasurement
    ));
    assert!(r.collections()[0].rows.len() < 40);
}
#[test]
fn zero_scrollport_cannot_admit_and_deleted_source_finish_publishes_pin_cleanup() {
    let mut r = boot(100);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let mut f = feedback(&r, Some(h), 0.);
    f.port_main = 0.;
    r.collection_feedback(f).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    assert!(r.begin_reorder(b, g).unwrap().is_none());
    let mut r = editing("delete");
    let h = r.kernel().find_by_test_id("grip-0")[0];
    for _ in 0..3 {
        edit_feedback(&mut r, h, 0.);
    }
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g).unwrap().unwrap().token;
    let g = r.reorder_geometry(b.list).unwrap();
    r.preview_reorder(token, g, 90.).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    r.drop_reorder(token, g).unwrap().unwrap();
    let rev = r.collections()[0].revision;
    assert!(r.finish_reorder(token).unwrap().is_some());
    assert!(
        r.collections()[0].revision > rev,
        "cleared dead handle is a new pin publication"
    );
}

#[test]
fn admission_requires_real_ancestor_handler_string_keys_and_unambiguous_enabled_path() {
    let cases = [
        SOURCE.replace("reorderdrop=receive", ""),
        SOURCE.replace("column\n      input", "column id=\"arrange\"\n      input"),
        SOURCE.replace("column\n      input", "column inert=true\n      input"),
        SOURCE.replace("disabled=disabled", "disabled=true"),
        SOURCE.replace("reorderFor=\"arrange\"", "reorderFor=\"wrong\""),
        SOURCE.replace(
            "reorderFor=\"arrange\"",
            "reorderFor=\"arrange\" heightDragFor=\"sheet\"",
        ),
    ];
    for source in cases {
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            Rows { n: 8, queries: 0 },
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let h = ready(&mut r);
        assert!(r.reorder_binding(h).is_none(), "{source}");
    }
    let mut r = boot(8);
    let h = ready(&mut r);
    let mut stale = h;
    stale.generation = stale.generation.wrapping_add(1);
    assert!(r.reorder_binding(stale).is_none());
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let old = r.begin_reorder(b, g).unwrap().unwrap().token;
    r.cancel_reorder(old).unwrap();
    r.finish_reorder(old).unwrap();
    r.collection_feedback(feedback(&r, Some(h), 0.)).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let new = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    assert_ne!(old.serial(), new.serial());
    assert!(r.drop_reorder(old, g).unwrap().is_none());
    assert!(r.has_reorder(new));
}

#[test]
fn numeric_keys_remain_valid_collections_but_cannot_admit_physical_reorder() {
    struct Numbers;
    impl DataSource for Numbers {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                (0..8).map(|i| Value::Number(i as f64)).collect(),
            ))
        }
    }
    let source = SOURCE
        .replace("shape list<string>", "shape list<number>")
        .replace("text x", "text `${x}`");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        Numbers,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let h = ready(&mut r);
    assert!(r.reorder_binding(h).is_none());
    assert_eq!(r.collections()[0].count, 8);
}

#[test]
fn physical_terminal_preserves_empty_unicode_destination_and_true_end() {
    struct ExactKeys;
    impl DataSource for ExactKeys {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(vec![
                Value::str("0"),
                Value::str(""),
                Value::str("🦀,\n"),
            ]))
        }
    }
    let source = SOURCE
        .replace(
            "state count = 0",
            "state count = 0\n  state destination = some(\"initial\")",
        )
        .replace(
            "    dropped = item",
            "    dropped = item\n    destination = before",
        );
    for (y, before) in [(20., Some("")), (40., Some("🦀,\n")), (60., None)] {
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            ExactKeys,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let h = ready(&mut r);
        let b = r.reorder_binding(h).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let t = r.begin_reorder(b, g).unwrap().unwrap().token;
        let g = r.reorder_geometry(b.list).unwrap();
        assert!(matches!(
            r.preview_reorder(t, g, y).unwrap(),
            ReorderProgress::Accepted { .. }
        ));
        let g = r.reorder_geometry(b.list).unwrap();
        r.drop_reorder(t, g).unwrap().unwrap();
        assert_eq!(r.slot("dropped"), Some(&Value::str("0")));
        assert_eq!(
            r.slot("destination"),
            Some(&before.map_or(Value::NONE, |s| Value::some(Value::str(s))))
        );
        r.finish_reorder(t).unwrap();
    }
}

#[test]
fn unproved_final_sample_cannot_drop_the_previous_certified_gap() {
    let mut r = boot(100);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let t = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    assert!(matches!(
        r.preview_reorder(t, g.clone(), 80.).unwrap(),
        ReorderProgress::Accepted { .. }
    ));
    let displayed = r.reorder_frame(t).unwrap();
    assert!(matches!(
        r.preview_reorder(t, g.clone(), 2000.).unwrap(),
        ReorderProgress::NeedsMeasurement
    ));
    assert_eq!(r.reorder_frame(t), Some(displayed));
    assert!(r.drop_reorder(t, g).unwrap().is_none());
    assert_eq!(r.slot("count"), Some(&Value::Number(0.)));
    r.cancel_reorder(t).unwrap();
    r.finish_reorder(t).unwrap();
}
#[test]
fn source_exclusion_right_biases_and_certifies_zero_ties_after_normalization() {
    for y in [25., 35.] {
        let mut r = boot(4);
        let h = key(&r, "grip-1");
        for _ in 0..3 {
            let mut f = feedback(&r, Some(h), 0.);
            let rows = r.collections()[0].rows.clone();
            for m in &mut f.measurements {
                if rows.iter().any(|r| r.index == 2 && r.view == m.view) {
                    m.size = 0.;
                }
            }
            r.collection_feedback(f).unwrap();
        }
        let b = r.reorder_binding(h).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let t = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
        assert!(matches!(
            r.preview_reorder(t, g.clone(), y).unwrap(),
            ReorderProgress::Accepted { .. }
        ));
        // A zero row remains unmounted, but source-excluded right bias still
        // moves source to before row3, not before the coincident zero row2.
        let frame = r.reorder_frame(t).unwrap();
        assert_eq!(
            frame
                .wrappers
                .iter()
                .find(|w| w.wrapper == b.wrapper)
                .unwrap()
                .offset,
            0.
        );
        r.drop_reorder(t, g).unwrap().unwrap();
        assert_eq!(r.slot("count"), Some(&Value::Number(1.)));
        // Exact destination asserted through a second fixture's typed payload.
        let event_source = SOURCE
            .replace(
                "state count = 0",
                "state count = 0\n  state destination = some(\"initial\")",
            )
            .replace(
                "    dropped = item",
                "    dropped = item\n    destination = before",
            );
        let mut r = Runner::boot(
            contract::compile(&event_source).unwrap(),
            Rows { n: 4, queries: 0 },
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let h = key(&r, "grip-1");
        for _ in 0..3 {
            let rows = r.collections()[0].rows.clone();
            let mut f = feedback(&r, Some(h), 0.);
            for m in &mut f.measurements {
                if rows.iter().any(|r| r.index == 2 && r.view == m.view) {
                    m.size = 0.;
                }
            }
            r.collection_feedback(f).unwrap();
        }
        let b = r.reorder_binding(h).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let t = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
        r.preview_reorder(t, g.clone(), y).unwrap();
        r.drop_reorder(t, g).unwrap();
        assert_eq!(
            r.slot("destination"),
            Some(&Value::some(Value::str("3"))),
            "sample {y}"
        );
    }
}
#[test]
fn terminal_source_key_survives_grip_replacement_and_a_second_reconciliation() {
    let source=SOURCE.replace("state count = 0","state count = 0\n  state grip = true\n  action removeGrip\n    grip = false")
        .replace("column reorderFor=\"arrange\" disabled=disabled testId=`grip-${x}` height=20\n            text x",
            "column height=20\n            when grip\n              view reorderFor=\"arrange\" testId=`grip-${x}`\n            text `${x} ${disabled}`");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        Rows {
            n: 25_000,
            queries: 0,
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let t = r.begin_reorder(b, g).unwrap().unwrap().token;
    for _ in 0..3 {
        r.collection_feedback(feedback(&r, Some(h), 3000.)).unwrap();
    }
    let old_id = r.kernel().node_by_key(h).unwrap().id;
    r.act("removeGrip", vec![]).unwrap();
    assert!(r.kernel().node_by_key(h).is_none());
    assert!(r.kernel().node_by_key(b.wrapper).is_some());
    for _ in 0..2 {
        let mut f = feedback(&r, None, 3000.);
        f.interaction_view = Some(old_id);
        r.collection_feedback(f).unwrap();
        assert!(r.kernel().node_by_key(b.wrapper).is_some());
    }
    r.act("block", vec![]).unwrap();
    assert!(
        r.kernel().node_by_key(b.wrapper).is_some(),
        "second body pass must keep source key pin"
    );
    assert!(r.reorder_frame(t).unwrap().terminal);
    r.finish_reorder(t).unwrap();
    assert!(r.kernel().node_by_key(b.wrapper).is_none());
}

#[test]
fn transfer_away_then_back_to_same_grip_is_not_owned_by_old_terminal() {
    let mut r = boot(25_000);
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let old = r.begin_reorder(b, g).unwrap().unwrap().token;
    let other = key(&r, "grip-1");
    r.collection_feedback(feedback(&r, Some(other), 0.))
        .unwrap();
    r.collection_feedback(feedback(&r, Some(h), 0.)).unwrap();
    r.finish_reorder(old).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let new = r.begin_reorder(b, g).unwrap().unwrap().token;
    assert!(r.has_reorder(new));
    assert!(r.finish_reorder(old).unwrap().is_none());
    assert!(r.has_reorder(new));
}

fn beneath_empty_when_arms() -> String {
    let source = SOURCE.replace(
        "state disabled = false",
        "state disabled = false\n  state shown = true\n  state windowed = true\n  action hide\n    shown = false",
    );
    let (head, list) = source.split_once("      list").unwrap();
    let list = format!("      list{list}")
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect::<String>();
    format!("{head}      when shown\n        when windowed\n{list}")
}
fn boot_source(source: &str) -> Runner<Rows> {
    Runner::boot(
        contract::compile(source).unwrap(),
        Rows { n: 100, queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn empty_when_frames_keep_preview_and_measured_rows_on_unrelated_typing() {
    let mut r = boot_source(&beneath_empty_when_arms());
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let t = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    assert!(matches!(
        r.preview_reorder(t, g, 65.).unwrap(),
        ReorderProgress::Accepted { .. }
    ));
    let geometry = r.reorder_geometry(b.list).unwrap();
    let rows = r.collections()[0]
        .rows
        .iter()
        .map(|row| (row.view, row.epoch))
        .collect::<Vec<_>>();
    let queries = r.data_ref().queries;
    r.act("edit", vec![Value::str("unrelated draft")]).unwrap();
    assert!(
        r.has_reorder(t),
        "unchanged When frames must not end preview"
    );
    assert_eq!(r.reorder_binding(h), Some(b));
    assert_eq!(r.reorder_geometry(b.list), Some(geometry.clone()));
    assert_eq!(
        r.collections()[0]
            .rows
            .iter()
            .map(|row| (row.view, row.epoch))
            .collect::<Vec<_>>(),
        rows
    );
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert!(!r.reorder_frame(t).unwrap().terminal);
    assert!(r.drop_reorder(t, geometry.clone()).unwrap().is_some());
    assert_eq!(r.slot("count"), Some(&Value::Number(1.)));
    assert!(r.drop_reorder(t, geometry).unwrap().is_none());
    r.finish_reorder(t).unwrap();
}

#[test]
fn empty_when_frames_still_retire_for_changed_dependency_or_destroyed_arm() {
    for action in ["block", "hide"] {
        let mut r = boot_source(&beneath_empty_when_arms());
        let h = ready(&mut r);
        let b = r.reorder_binding(h).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let t = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
        r.act(action, vec![]).unwrap();
        assert!(!r.has_reorder(t));
        assert!(r.drop_reorder(t, g).unwrap().is_none());
        assert_eq!(r.slot("count"), Some(&Value::Number(0.)));
        if action == "hide" {
            assert!(r.kernel().node_by_key(b.wrapper).is_none());
        } else {
            assert!(r.reorder_frame(t).unwrap().terminal);
        }
        r.finish_reorder(t).unwrap();
    }
}

#[test]
fn contextful_match_binding_still_refreshes_collection_body() {
    let source = SOURCE.replace("state draft = \"\"", "state draft = \"before\"");
    let (head, list) = source.split_once("      list").unwrap();
    let list = format!("      list{list}")
        .replace("text x", "text label testId=`label-${x}`")
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect::<String>();
    let mut r = boot_source(&format!(
        "{head}      match some(draft)\n        case some(label)\n{list}        case none\n          text \"none\"\n"
    ));
    let h = ready(&mut r);
    let b = r.reorder_binding(h).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let t = r.begin_reorder(b, g).unwrap().unwrap().token;
    r.act("edit", vec![Value::str("after")]).unwrap();
    assert!(
        !r.has_reorder(t),
        "changed bound context must not be memoized away"
    );
    let text = r.kernel().node_by_key(key(&r, "label-0")).unwrap();
    assert_eq!(text.props.str(exact_kernel::PropId::Text), Some("after"));
    r.finish_reorder(t).unwrap();
}

#[test]
fn reorder_wrappers_contain_absolute_descendants_before_and_after_preview() {
    let source = SOURCE.replace("            text x", "            text x\n            box testId=`absolute-${x}` position=\"absolute\" right=0 bottom=0 width=5 height=5");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        Rows { n: 20, queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let handle = ready(&mut r);
    let binding = r.reorder_binding(handle).unwrap();
    let check = |r: &mut Runner<Rows>| {
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, exact_kernel::Offer::definite(320., 400.))
            .unwrap();
        let wrapper = r.kernel().node_by_key(binding.wrapper).unwrap();
        assert_eq!(
            wrapper.style.position_type,
            exact_kernel::PositionType::Relative
        );
        let child = r.kernel().node_by_key(key(r, "absolute-0")).unwrap();
        let (p, c) = (wrapper.frame, child.frame);
        assert_eq!((c.x, c.y), (p.x + p.width - 5., p.y + p.height - 5.));
    };
    check(&mut r);
    let g = r.reorder_geometry(binding.list).unwrap();
    let start = r.begin_reorder(binding, g.clone()).unwrap().unwrap();
    r.preview_reorder(start.token, g, 80.).unwrap();
    check(&mut r);
    r.cancel_reorder(start.token).unwrap();
    check(&mut r);
    let plain = source.replace(" reorderdrop=receive", "");
    let r = Runner::boot(
        contract::compile(&plain).unwrap(),
        Rows { n: 2, queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for row in &r.collections()[0].rows {
        assert_eq!(
            r.kernel().node(row.view).unwrap().style.position_type,
            exact_kernel::PositionType::Static
        );
    }
}
