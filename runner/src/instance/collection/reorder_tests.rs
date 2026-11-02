use crate::{CollectionFeedback, DataError, DataSource, ReorderProgress, RowMeasurement, Runner};
use exact_kernel::{Kernel, NodeKey};
use exact_plan::Value;

struct Rows(Vec<String>);
impl DataSource for Rows {
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        if name == "refuse" {
            return Err(DataError::BadArguments("refused drop".into()));
        }
        if name == "trim" {
            self.0.drain(..10);
        }
        if name == "empty" {
            return Ok(Value::list(vec![]));
        }
        if name == "move" || name == "remove" {
            let at = self
                .0
                .iter()
                .position(|k| Some(k.as_str()) == args[0].as_str())
                .unwrap();
            let item = self.0.remove(at);
            let before = match &args[1] {
                Value::Option(Some(v)) => v.as_str(),
                _ => None,
            };
            let to = self
                .0
                .iter()
                .position(|k| Some(k.as_str()) == before)
                .unwrap_or(self.0.len());
            if name == "move" {
                self.0.insert(to, item);
            }
        }
        Ok(Value::list(self.0.iter().map(|k| Value::str(k)).collect()))
    }
}
const SOURCE: &str = r#"component App
  resource initial = rows() as shape string[]
  mutation changed as shape string[]
  derive rows = match changed { case some(value) => value, case none => initial }
  action receive(item: string, before: option<string>)
    send changed = move(item, before)
  view
    list id="arrange" testId="list" virtualized=true estimated-item-height=20 height=100 reorderdrop=receive
      for x in rows key=x
        column reorderFor="arrange" testId=`grip-${x}` height=20
          text x
"#;
fn report(r: &mut Runner<Rows>, offset: f64, pin: Option<NodeKey>) {
    for c in r.collections() {
        let source = c.view == r.collections()[0].view;
        r.collection_feedback(CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: if source { offset } else { 0. },
            port_cross: 320.,
            port_main: 100.,
            cross: 320.,
            padding: [0.; 2],
            focus_view: None,
            interaction_view: pin
                .filter(|_| source)
                .map(|k| r.kernel().node_by_key(k).unwrap().id),
            measurements: c
                .rows
                .iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: 20.,
                })
                .collect(),
        })
        .unwrap();
    }
}
fn fixture(offset: f64) -> (Runner<Rows>, NodeKey) {
    fixture_with(SOURCE, offset, 10)
}
fn fixture_with(source: &str, offset: f64, item: usize) -> (Runner<Rows>, NodeKey) {
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Rows((0..30).map(|i| i.to_string()).collect()),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for _ in 0..3 {
        report(&mut r, 0., None);
    }
    report(&mut r, offset, None);
    let grip = r.kernel().find_by_test_id(&format!("grip-{item}"))[0];
    for _ in 0..2 {
        report(&mut r, offset, Some(grip));
    }
    (r, grip)
}

#[test]
fn dropped_top_row_anchors_its_visible_successor() {
    let (mut r, grip) = fixture(200.);
    let b = r.reorder_binding(grip).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    assert!(matches!(
        r.preview_reorder(token, g.clone(), 300.).unwrap(),
        ReorderProgress::Accepted { .. }
    ));
    assert!(r.drop_reorder(token, g).unwrap().is_some());
    assert_eq!(r.data_ref().0[14], "10");
    assert_eq!(r.collections()[0].correction.unwrap().offset, 180.);
    assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 180.);
}

#[test]
fn drop_anchors_preserve_partial_visibility_start_and_end_policy() {
    for (offset, item, y, expected) in [
        (207., 10, 300., 187.),
        (0., 0, 100., 0.),
        (500., 25, 600., 480.),
        (180., 10, 280., 180.),
    ] {
        for grouped in [false, true] {
            let source = SOURCE.replace(
                "virtualized=true",
                if grouped {
                    "virtualized=true scrollFollowEnd=true reorderGroup=\"cards\""
                } else {
                    "virtualized=true scrollFollowEnd=true"
                },
            );
            let (mut r, grip) = fixture_with(&source, offset, item);
            let b = r.reorder_binding(grip).unwrap();
            let g = r.reorder_geometry(b.list).unwrap();
            let start = if grouped {
                r.begin_group_reorder(b, g.clone(), false)
            } else {
                r.begin_reorder(b, g.clone())
            };
            let token = start.unwrap().unwrap().token;
            assert!(matches!(
                r.preview_reorder(token, g.clone(), y).unwrap(),
                ReorderProgress::Accepted { .. }
            ));
            r.drop_reorder(token, g).unwrap().unwrap();
            assert_eq!(
                r.reorder_geometry(b.list).unwrap().scroll_top,
                expected,
                "{offset}, grouped={grouped}"
            );
        }
    }
}

#[test]
fn cancellation_ordinary_moves_and_preview_measurements_keep_their_anchor() {
    for preview in [false, true] {
        let (mut r, grip) = fixture(200.);
        let b = r.reorder_binding(grip).unwrap();
        if preview {
            let g = r.reorder_geometry(b.list).unwrap();
            let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
            r.preview_reorder(token, g, 300.).unwrap();
            r.cancel_reorder(token).unwrap();
        }
        r.act(
            "receive",
            vec![Value::str("10"), Value::some(Value::str("15"))],
        )
        .unwrap();
        assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 280.);
    }
    let (mut r, grip) = fixture(200.);
    let b = r.reorder_binding(grip).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    r.preview_reorder(token, g, 300.).unwrap();
    let c = r.collections().remove(0);
    let source = c.rows.iter().find(|row| row.index == 10).unwrap();
    r.collection_feedback(CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence,
        offset: 200.,
        port_cross: 320.,
        port_main: 100.,
        cross: 320.,
        padding: [0.; 2],
        focus_view: None,
        interaction_view: Some(r.kernel().node_by_key(grip).unwrap().id),
        measurements: vec![RowMeasurement {
            view: source.view,
            epoch: source.epoch,
            size: 40.,
        }],
    })
    .unwrap();
    assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 200.);
}

#[test]
fn outgoing_group_drop_keeps_the_deleted_source_neighbour_fallback() {
    let mut source = SOURCE
        .replace(
            "  resource initial",
            "  resource empty = empty() as shape string[]\n  resource initial",
        )
        .replace("move(item, before)", "remove(item, before)")
        .replace(
            "virtualized=true",
            "virtualized=true reorderGroup=\"cards\"",
        );
    let (head, body) = source.split_once("  view\n").unwrap();
    source = format!(
        "{head}  view\n    column\n{}",
        body.lines().map(|l| format!("  {l}\n")).collect::<String>()
    );
    source.push_str(r#"      list id="target" testId="target" virtualized=true reorderGroup="cards" reorderdrop=receive height=100
        for x in empty key=x
          text x
"#);
    let (mut r, grip) = fixture_with(&source, 207., 10);
    let b = r.reorder_binding(grip).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_group_reorder(b, g, true).unwrap().unwrap().token;
    let target = r.kernel().find_by_test_id("target")[0];
    let g = r.reorder_geometry(target).unwrap();
    assert!(matches!(
        r.preview_reorder_into(token, target, g.clone(), 0.)
            .unwrap(),
        ReorderProgress::Accepted { .. }
    ));
    r.drop_reorder(token, g).unwrap().unwrap();
    assert!(!r.data_ref().0.iter().any(|k| k == "10"));
    assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 207.);
    assert_eq!(
        r.reorder_frame(token).unwrap().ending,
        Some(crate::ReorderEnding::Gone)
    );
}

#[test]
fn deleting_rows_above_the_anchor_keeps_the_correction_relative() {
    let source = SOURCE.replace(
        "  view",
        "  action trim()\n    send changed = trim()\n  view",
    );
    let (mut r, _) = fixture_with(&source, 200., 10);
    r.act("trim", vec![]).unwrap();
    let correction = r.collections()[0].correction.unwrap();
    assert_eq!(correction.offset, 0.);
    assert_eq!(correction.from, Some(200.));
}

#[test]
fn same_slot_drop_does_not_anchor_a_later_move() {
    for grouped in [false, true] {
        let source = SOURCE.replace(
            "virtualized=true",
            if grouped {
                "virtualized=true reorderGroup=\"cards\""
            } else {
                "virtualized=true"
            },
        );
        let (mut r, grip) = fixture_with(&source, 200., 10);
        let b = r.reorder_binding(grip).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let token = if grouped {
            r.begin_group_reorder(b, g.clone(), false)
        } else {
            r.begin_reorder(b, g.clone())
        }
        .unwrap()
        .unwrap()
        .token;
        r.preview_reorder(token, g.clone(), 200.).unwrap();
        r.drop_reorder(token, g).unwrap().unwrap();
        r.act(
            "receive",
            vec![Value::str("10"), Value::some(Value::str("12"))],
        )
        .unwrap();
        assert_eq!(
            r.reorder_geometry(b.list).unwrap().scroll_top,
            220.,
            "grouped={grouped}"
        );
    }
}

#[test]
fn refused_or_cancelled_drop_does_not_anchor_a_later_move() {
    for (cancel, grouped, refused) in [
        (false, false, true),
        (true, false, true),
        (false, true, true),
        (true, false, false),
    ] {
        let source = SOURCE.replace("send changed = move(item, before)", if refused { "send changed = refuse(item, before)" } else { "send changed = unchanged()" })
            .replace("  view", "  action move_now(item: string, before: option<string>)\n    send changed = move(item, before)\n  view");
        let source = source.replace(
            "virtualized=true",
            if grouped {
                "virtualized=true reorderGroup=\"cards\""
            } else {
                "virtualized=true"
            },
        );
        let (mut r, grip) = fixture_with(&source, 200., 10);
        let b = r.reorder_binding(grip).unwrap();
        let g = r.reorder_geometry(b.list).unwrap();
        let token = if grouped {
            r.begin_group_reorder(b, g.clone(), false)
        } else {
            r.begin_reorder(b, g.clone())
        }
        .unwrap()
        .unwrap()
        .token;
        r.preview_reorder(token, g.clone(), 300.).unwrap();
        assert_eq!(r.drop_reorder(token, g).is_err(), refused);
        if grouped && refused {
            let frame = r.reorder_frame(token).unwrap();
            assert_eq!(frame.phase, crate::ReorderPhase::Settling);
            assert_eq!(frame.ending, None);
        }
        if cancel {
            r.cancel_reorder(token).unwrap();
        }
        r.act(
            "move_now",
            vec![Value::str("10"), Value::some(Value::str("15"))],
        )
        .unwrap();
        assert_eq!(
            r.reorder_geometry(b.list).unwrap().scroll_top,
            280.,
            "cancel={cancel}, grouped={grouped}, refused={refused}"
        );
    }
}

#[test]
fn pending_drop_anchors_only_the_dispatched_destination() {
    let source = SOURCE.replace("send changed = move(item, before)", "send changed = unchanged()")
        .replace("  view", "  action move_now(item: string, before: option<string>)\n    send changed = move(item, before)\n  view");
    let (mut r, grip) = fixture_with(&source, 200., 10);
    let b = r.reorder_binding(grip).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    let token = r.begin_reorder(b, g.clone()).unwrap().unwrap().token;
    r.preview_reorder(token, g.clone(), 300.).unwrap();
    r.drop_reorder(token, g).unwrap().unwrap();
    r.act(
        "move_now",
        vec![Value::str("10"), Value::some(Value::str("12"))],
    )
    .unwrap();
    assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 220.);
    report(&mut r, 220., Some(grip));
    r.act(
        "move_now",
        vec![Value::str("10"), Value::some(Value::str("15"))],
    )
    .unwrap();
    assert_eq!(r.reorder_geometry(b.list).unwrap().scroll_top, 200.);
}
