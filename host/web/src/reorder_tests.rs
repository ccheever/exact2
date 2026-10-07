use super::*;
use exact_runner::{CollectionFeedback, DataError, RowMeasurement, Value as DataValue};
struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
        Ok(DataValue::list(
            (0..100).map(|i| DataValue::str(&i.to_string())).collect(),
        ))
    }
}
const SOURCE: &str = r#"component App
  state count = 0
  state disabled = false
  resource rows = rows() as shape list<string>
  action receive(item: string, before: option<string>)
    count = count + 1
  action block
    disabled = true
  view
    column
      text `${count}` testId="count"
      button testId="block" press=block width=60 height=20
      column testId="other-motion" width=20 height=20 scale=1 transition="scale -exact-spring(300,30,1)"
      list id="arrange" testId="list" virtualized=true height=100 reorderdrop=receive
        each x in rows key=x
          column reorderFor="arrange" disabled=disabled testId=`grip-${x}` height=20
            text x
"#;
fn fixture() -> (Host<Rows>, ReorderBinding) {
    crate::link::link_for_tests();
    let (mut h, _) = Host::boot(
        &contract::compile(SOURCE).unwrap().encode(),
        Rows,
        Default::default(),
        "/",
    )
    .unwrap();
    let handle = h.runner.kernel().find_by_test_id("grip-0")[0];
    for _ in 0..3 {
        let c = h.runner.collections().remove(0);
        let f = CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: 0.,
            port_cross: 320.,
            port_main: 100.,
            cross: 320.,
            focus_view: None,
            interaction_view: Some(h.runner.kernel().node_by_key(handle).unwrap().id),
            measurements: c
                .rows
                .iter()
                .map(|r| RowMeasurement {
                    view: r.view,
                    epoch: r.epoch,
                    size: 20.,
                })
                .collect(),
        };
        let reply = h.collection_feedback(&f.encode().unwrap());
        assert!(reply.contains("\"error\":null"), "{reply}");
    }
    let b = h.runner.reorder_binding(handle).unwrap();
    (h, b)
}
fn packet(h: &Host<Rows>, b: ReorderBinding, op: u32, now: f64, y: f64) -> Vec<u8> {
    let g = h.runner.reorder_geometry(b.list).unwrap();
    let active = h.reorder_drags.active.as_ref();
    let token = active.map_or(0, |a| a.token.serial());
    let rows = if op == 17 || op == 18 || op == 19 {
        active
            .and_then(|a| h.runner.reorder_frame(a.token))
            .unwrap()
            .wrappers
    } else {
        vec![]
    };
    let mut v = Vec::new();
    for n in [3u32, op] {
        v.extend(n.to_le_bytes());
    }
    for n in [
        h.reorder_drags.runtime,
        motion_node(b.handle),
        motion_node(b.list),
        motion_node(b.wrapper),
        motion_node(b.root),
        b.row_epoch,
        token,
        g.revision,
        g.scroll_sequence,
    ] {
        v.extend(n.to_le_bytes());
    }
    for n in [rows.len() as u32, 0] {
        v.extend(n.to_le_bytes());
    }
    for n in [
        g.scroll_top,
        g.port_width,
        g.port_height,
        g.row_width,
        g.total_extent,
        y,
        0.,
        y,
        0.,
        0.,
        now,
    ] {
        v.extend(n.to_le_bytes());
    }
    for row in rows {
        v.extend(motion_node(row.wrapper).to_le_bytes());
        let serial = active
            .and_then(|a| a.holds.get(&row.wrapper))
            .filter(|t| h.springs.token(t.serial()) == Some(**t))
            .map_or(0, |s| s.serial());
        v.extend(serial.to_le_bytes());
        v.extend(0f64.to_le_bytes());
        v.extend(row.offset.to_le_bytes());
    }
    v
}
fn accepted(s: &str) {
    assert!(s.contains("\"accepted\":true"), "{s}");
}
fn begin(h: &mut Host<Rows>, b: ReorderBinding) {
    let p = packet(h, b, 15, 100., 0.);
    accepted(&h.reorder_motion(&p));
}
#[test]
fn wire_stale_runtime_precedes_bad_clock_and_replacement_cannot_finish() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let mut old = packet(&h, b, 16, f64::NAN, f64::NAN);
    old[8..16].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(h.reorder_motion(&old), stale());
    assert_eq!(h.springs.now(), 0.1);
    let p = packet(&h, b, 18, 110., 0.);
    accepted(&h.reorder_motion(&p));
    let p = packet(&h, b, 19, 110., 0.);
    accepted(&h.reorder_motion(&p));
    let old = packet(&h, b, 20, 110., 0.);
    accepted(&h.reorder_motion(&old));
    assert!(h.reorder_drags.active.is_none());
    assert_eq!(h.reorder_motion(&old), stale());
}
#[test]
fn needs_measurement_final_sample_never_drops_previous_certified_gap() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let p = packet(&h, b, 16, 110., 50.);
    accepted(&h.reorder_motion(&p));
    let p = packet(&h, b, 17, 120., 800_000.);
    let reply = h.reorder_motion(&p);
    accepted(&reply);
    assert!(reply.contains("\"dispatched\":false"), "{reply}");
    assert_eq!(h.runner.slot("count"), Some(&DataValue::Number(0.)));
    assert!(
        h.runner
            .reorder_frame(h.reorder_drags.active.as_ref().unwrap().token)
            .unwrap()
            .terminal
    );
    assert!(h
        .runner
        .reorder_frame(h.reorder_drags.active.as_ref().unwrap().token)
        .is_some());
}
#[test]
fn malformed_last_wrapper_cannot_update_first_hold_or_dispatch() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let before = h.springs.now();
    let mut p = packet(&h, b, 17, 200., 50.);
    let len = p.len();
    p[len - 8..].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(h.reorder_motion(&p).contains("error"));
    assert_eq!(h.springs.now(), before);
    assert_eq!(h.runner.slot("count"), Some(&DataValue::Number(0.)));
    assert!(h
        .runner
        .has_reorder(h.reorder_drags.active.as_ref().unwrap().token));
}
#[test]
fn terminal_action_once_while_source_held_rebase_then_finish_retires_only_owned_pin() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let source = *h
        .reorder_drags
        .active
        .as_ref()
        .unwrap()
        .holds
        .get(&b.wrapper)
        .unwrap();
    let p = packet(&h, b, 17, 200., 50.);
    let reply = h.reorder_motion(&p);
    accepted(&reply);
    assert!(reply.contains("\"dispatched\":true"));
    assert!(h.has_hold(source.serial()));
    assert_eq!(h.runner.slot("count"), Some(&DataValue::Number(1.)));
    assert!(h
        .runner
        .reorder_frame(h.reorder_drags.active.as_ref().unwrap().token)
        .is_some());
    assert_eq!(h.reorder_motion(&p), stale());
    let p = packet(&h, b, 19, 200., 0.);
    accepted(&h.reorder_motion(&p));
    assert!(!h.has_hold(source.serial()));
    assert!(h
        .runner
        .reorder_frame(h.reorder_drags.active.as_ref().unwrap().token)
        .is_some());
    let p = packet(&h, b, 20, 200., 0.);
    accepted(&h.reorder_motion(&p));
    assert!(h.reorder_drags.active.is_none());
}
#[test]
fn refused_missing_source_pin_does_not_advance_clock_or_take_property() {
    let (mut h, b) = fixture();
    let c = h.runner.collections().remove(0);
    let f = CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: 0.,
        port_cross: 320.,
        port_main: 100.,
        cross: 320.,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    };
    h.collection_feedback(&f.encode().unwrap());
    assert_eq!(h.runner.reorder_binding(b.handle), Some(b));
    let clock = h.springs.now();
    let p = packet(&h, b, 15, 200., 25.);
    assert!(h.reorder_motion(&p).contains("\"accepted\":false"));
    assert_eq!(h.springs.now(), clock);
    assert!(h.reorder_drags.active.is_none());
}
#[test]
fn cancel_after_source_property_takeover_preserves_successor() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let view = h.runner.kernel().node_by_key(b.wrapper).unwrap().id;
    let newer = h
        .begin_hold(view, Property::Translate, Value::new(0., 42.), 110.)
        .unwrap()
        .unwrap()
        .0;
    let p = packet(&h, b, 18, 120., 0.);
    accepted(&h.reorder_motion(&p));
    let mut p = packet(&h, b, 19, 120., 0.);
    // Rebase contains exactly the surviving originals, excluding the successor.
    let rows = p[176..]
        .chunks_exact(32)
        .filter(|r| u64::from_le_bytes(r[..8].try_into().unwrap()) != motion_node(b.wrapper))
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    p.truncate(176);
    p[80..84].copy_from_slice(&(rows.len() as u32 / 32).to_le_bytes());
    p.extend(rows);
    accepted(&h.reorder_motion(&p));
    assert!(h.has_hold(newer.token.serial()));
    let p = packet(&h, b, 20, 120., 0.);
    accepted(&h.reorder_motion(&p));
    assert!(h.has_hold(newer.token.serial()));
}
#[test]
fn unrelated_pending_scale_lowered_once_on_reorder_preview() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let view = h
        .runner
        .kernel()
        .node_by_key(h.runner.kernel().find_by_test_id("other-motion")[0])
        .unwrap()
        .id;
    let s = h
        .begin_hold(view, Property::Scale, Value::scalar(2.), 110.)
        .unwrap()
        .unwrap()
        .0;
    h.springs
        .end_hold(
            s.token.serial(),
            HoldEnd::Release {
                velocity: Value::ZERO,
            },
            0.11,
        )
        .unwrap();
    let p = packet(&h, b, 16, 120., 800_000.);
    let reply = h.reorder_motion(&p);
    accepted(&reply);
    assert!(reply.contains("\"property\":\"scale\""), "{reply}");
    let p = packet(&h, b, 16, 120., 800_000.);
    let reply = h.reorder_motion(&p);
    assert!(!reply.contains("\"property\":\"scale\""), "{reply}");
}
#[test]
fn disabled_handle_receipt_enters_no_action_terminal_and_can_cleanup() {
    let (mut h, b) = fixture();
    begin(&mut h, b);
    let view = h
        .runner
        .kernel()
        .node_by_key(h.runner.kernel().find_by_test_id("block")[0])
        .unwrap()
        .id;
    let batch = h.dispatch_at(view, exact_runner::Event::Press, 150.);
    assert!(batch.contains("\"terminal\":true"));
    let p = packet(&h, b, 17, f64::NAN, f64::NAN);
    assert_eq!(h.reorder_motion(&p), stale());
    let p = packet(&h, b, 18, 150., 0.);
    accepted(&h.reorder_motion(&p));
    let p = packet(&h, b, 19, 150., 0.);
    accepted(&h.reorder_motion(&p));
    let p = packet(&h, b, 20, 150., 0.);
    accepted(&h.reorder_motion(&p));
    assert_eq!(h.runner.slot("count"), Some(&DataValue::Number(0.)));
}
#[test]
fn abi_kind18_uses_common_binary_codec_and_refuses_before_clock() {
    let mut bridge = crate::abi::Bridge::new();
    crate::link::link_for_tests();
    let n = bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        Rows,
        320.,
        200.,
        "/",
    );
    let batch = String::from_utf8(bridge.output_bytes(n as usize).to_vec()).unwrap();
    let create = batch
        .split("\"op\":\"create\"")
        .find(|part| {
            part.split("\"op\"")
                .next()
                .unwrap()
                .contains("\"data-testid\":\"list\"")
        })
        .unwrap();
    let list = create
        .split("\"id\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let bytes = exact_runner::Event::ReorderDrop {
        item: "a,\0💠".into(),
        before: Some("".into()),
    }
    .reorder_drop_bytes()
    .unwrap();
    bridge.input_write(&bytes);
    let n = bridge.dispatch(list, 18, bytes.len(), 100.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"1\""), "{out}");
    bridge.input_write(&[1, 2]);
    let n = bridge.dispatch(list, 18, 2, f64::NAN);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid reorder event"));
    let mut bad = vec![0u8; 176];
    bad[0..4].copy_from_slice(&3u32.to_le_bytes());
    bad[4..8].copy_from_slice(&15u32.to_le_bytes());
    bad[80..84].copy_from_slice(&4097u32.to_le_bytes());
    bridge.input_write(&bad);
    let n = bridge.motion(176);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("malformed reorder input"));
}
/// A remeasure mid-drag: the collection's rows, the source (`grip-0`'s row,
/// the first) at `source` and the rest at 20.
fn remeasure(h: &mut Host<Rows>, b: ReorderBinding, source: f64) {
    let c = h.runner.collections().remove(0);
    let f = CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence,
        offset: 0.,
        port_cross: 320.,
        port_main: 100.,
        cross: 320.,
        focus_view: None,
        interaction_view: Some(h.runner.kernel().node_by_key(b.handle).unwrap().id),
        measurements: c
            .rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: if r.index == 0 { source } else { 20. },
            })
            .collect(),
    };
    let reply = h.collection_feedback(&f.encode().unwrap());
    assert!(reply.contains("\"error\":null"), "{reply}");
}
#[test]
fn a_float_noise_remeasure_of_the_translated_source_keeps_the_preview() {
    // habits F10: the source row, measured through its preview translate,
    // came back 76 as 75.99998 and ended the drag; a real change still does.
    for (size, drops) in [(20. - 1.5e-5, true), (25., false)] {
        let (mut h, b) = fixture();
        begin(&mut h, b);
        accepted(&h.reorder_motion(&packet(&h, b, 16, 110., 50.)));
        remeasure(&mut h, b, size);
        let token = h.reorder_drags.active.as_ref().unwrap().token;
        assert_eq!(h.runner.reorder_frame(token).unwrap().terminal, !drops);
        let reply = h.reorder_motion(&packet(&h, b, 17, 200., 50.));
        let count = if drops { 1. } else { 0. };
        assert_eq!(
            h.runner.slot("count"),
            Some(&DataValue::Number(count)),
            "{reply}"
        );
    }
}

/// Two grouped lists (LLP 1094 D5): a ghost's begin, op 21 naming the
/// target by its view id, and the drop on it.
struct Board(Vec<(String, String)>);
impl DataSource for Board {
    fn query(&mut self, name: &str, args: &[DataValue]) -> Result<DataValue, DataError> {
        if name == "move" {
            let item = args[0].as_str().unwrap().to_owned();
            let at = self.0.iter().position(|(id, _)| *id == item).unwrap();
            self.0.remove(at);
            self.0
                .insert(0, (item, args[1].as_str().unwrap().to_owned()));
        }
        Ok(DataValue::list(
            self.0
                .iter()
                .map(|(id, col)| DataValue::record(vec![DataValue::str(id), DataValue::str(col)]))
                .collect(),
        ))
    }
}
const BOARD: &str = r#"shape Card
  id: string
  col: string
component App
  state log = ""
  resource initial = cards() as shape list<Card>
  mutation moved as shape list<Card>
  derive cards = match moved { case some(v) => v, case none => initial }
  action dropA(item: string, before: option<string>, e: ReorderEvent)
    log = `${item} ${e.from}>${e.to}`
    send moved = move(item, "a")
  action dropB(item: string, before: option<string>, e: ReorderEvent)
    log = `${item} ${e.from}>${e.to}`
    send moved = move(item, "b")
  view
    row
      list id="a" testId="a" virtualized=true reorderGroup="g" reorderdrop=dropA height=100 width=100
        each c in filter(cards, k => k.col == "a") key=c.id
          column reorderFor="a" testId=`grip-${c.id}` height=20
      list id="b" testId="b" virtualized=true reorderGroup="g" reorderdrop=dropB height=100 width=100
        each c in filter(cards, k => k.col == "b") key=c.id
          column reorderFor="b" testId=`grip-${c.id}` height=20
"#;
#[test]
fn op_21_moves_the_gap_into_another_grouped_list_and_drops_there() {
    crate::link::link_for_tests();
    let cards = [("a1", "a"), ("a2", "a"), ("b1", "b")]
        .map(|(i, c)| (i.to_owned(), c.to_owned()))
        .to_vec();
    let (mut h, _) = Host::boot(
        &contract::compile(BOARD).unwrap().encode(),
        Board(cards),
        Default::default(),
        "/",
    )
    .unwrap();
    let handle = h.runner.kernel().find_by_test_id("grip-a1")[0];
    let pin = h.runner.kernel().node_by_key(handle).unwrap().id;
    for _ in 0..3 {
        for c in h.runner.collections() {
            let f = CollectionFeedback {
                view: c.view,
                revision: c.revision,
                scroll_sequence: c.scroll_sequence + 1,
                offset: 0.,
                port_cross: 100.,
                port_main: 100.,
                cross: 100.,
                focus_view: None,
                interaction_view: c.rows.iter().any(|r| r.root == pin).then_some(pin),
                measurements: c
                    .rows
                    .iter()
                    .map(|r| RowMeasurement {
                        view: r.view,
                        epoch: r.epoch,
                        size: 20.,
                    })
                    .collect(),
            };
            h.collection_feedback(&f.encode().unwrap());
        }
    }
    let b = h.runner.reorder_binding(handle).unwrap();
    let target = h.runner.kernel().find_by_test_id("b")[0];
    let view = h.runner.kernel().node_by_key(target).unwrap().id;
    let wire = |h: &Host<Board>, op: u32, flags: u32, list: NodeKey, record: bool, y: f64| {
        let g = h.runner.reorder_geometry(list).unwrap();
        let token = h
            .reorder_drags
            .active
            .as_ref()
            .map_or(0, |a| a.token.serial());
        let mut v = Vec::new();
        for n in [3u32, op] {
            v.extend(n.to_le_bytes());
        }
        for n in [
            h.reorder_drags.runtime,
            motion_node(b.handle),
            motion_node(b.list),
            motion_node(b.wrapper),
            motion_node(b.root),
            b.row_epoch,
            token,
            g.revision,
            g.scroll_sequence,
        ] {
            v.extend(n.to_le_bytes());
        }
        for n in [u32::from(record), flags] {
            v.extend(n.to_le_bytes());
        }
        for n in [
            g.scroll_top,
            g.port_width,
            g.port_height,
            g.row_width,
            g.total_extent,
            y,
            0.,
            0.,
            0.,
            0.,
            100.,
        ] {
            v.extend(n.to_le_bytes());
        }
        if record {
            v.extend(u64::from(view).to_le_bytes());
            v.extend(0u64.to_le_bytes());
            v.extend(0f64.to_le_bytes());
            v.extend(0f64.to_le_bytes());
        }
        v
    };
    let r = h.reorder_motion(&wire(&h, 15, 1, b.list, false, 0.));
    assert!(r.contains("\"phase\":\"active\""), "{r}");
    // Op 21 is refused without its record; with it, b certifies its gap.
    assert!(h
        .reorder_motion(&wire(&h, 21, 0, target, false, 5.))
        .contains("error"));
    let r = h.reorder_motion(&wire(&h, 21, 0, target, true, 5.));
    assert!(
        r.contains("\"certified\":true") && r.contains(&format!("\"target\":{view}")),
        "{r}"
    );
    let r = h.reorder_motion(&wire(&h, 17, 0, target, true, 5.));
    assert!(
        r.contains("\"dispatched\":true") && r.contains("\"ending\":\"landed\""),
        "{r}"
    );
    assert_eq!(h.runner.slot("log"), Some(&DataValue::str("a1 a>b")));
    accepted(&h.reorder_motion(&wire(&h, 20, 0, b.list, false, 0.)));
    assert!(h.reorder_drags.active.is_none());
}

/// A row's first measurement at its estimate, after the list's first report,
/// still publishes its grip's binding: a grip binds only once its row is
/// measured, and a report that only travelled (no commit) published nothing,
/// so no grip on the page could lift (synthetic-reorder, 2026-10-07).
#[test]
fn a_first_measurement_at_the_estimate_publishes_the_grip_binding() {
    crate::link::link_for_tests();
    let source = SOURCE.replace(
        "virtualized=true height=100",
        "virtualized=true height=100 estimated-item-height=20",
    );
    let (mut h, _) = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        Rows,
        Default::default(),
        "/",
    )
    .unwrap();
    let report = |h: &Host<Rows>, measured: bool| {
        let c = h.runner.collections().remove(0);
        CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: 0.,
            port_cross: 320.,
            port_main: 100.,
            cross: 320.,
            focus_view: None,
            interaction_view: None,
            measurements: if measured {
                c.rows
                    .iter()
                    .map(|r| RowMeasurement {
                        view: r.view,
                        epoch: r.epoch,
                        size: 20.,
                    })
                    .collect()
            } else {
                Vec::new()
            },
        }
        .encode()
        .unwrap()
    };
    let unmeasured = report(&h, false);
    h.collection_feedback(&unmeasured);
    let grip = h.runner.kernel().find_by_test_id("grip-0")[0];
    let id = h.runner.kernel().node_by_key(grip).unwrap().id;
    assert!(
        h.runner.reorder_binding(grip).is_none(),
        "unmeasured: no binding yet"
    );
    let measured = report(&h, true);
    let batch = h.collection_feedback(&measured);
    assert!(h.runner.reorder_binding(grip).is_some(), "measured: bound");
    let op = format!("\"op\":\"reorder-drag\",\"id\":{id},");
    let at = batch
        .find(&op)
        .unwrap_or_else(|| panic!("no reorder-drag op for the grip: {batch}"));
    assert!(
        !batch[at..]
            .split('}')
            .next()
            .unwrap()
            .contains("\"list\":null"),
        "the binding names its list: {batch}"
    );
}
