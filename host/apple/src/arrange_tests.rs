//! Arrange on the Apple host, against the web (`host/web/src/reorder_tests.rs`)
//! and Linux (`host/linux/tests/it/arrange.rs`) sequences.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{CollectionFeedback, DataError, RowMeasurement, Value as DataValue};

struct Rows(Vec<String>);
impl DataSource for Rows {
    fn query(&mut self, name: &str, args: &[DataValue]) -> Result<DataValue, DataError> {
        if name == "move" {
            let item = args[0].as_str().unwrap().to_owned();
            let at = self.0.iter().position(|k| *k == item).unwrap();
            self.0.remove(at);
            let before = match &args[1] {
                DataValue::Option(Some(v)) => v.as_str().map(str::to_owned),
                _ => None,
            };
            let to = before
                .and_then(|b| self.0.iter().position(|k| *k == b))
                .unwrap_or(self.0.len());
            self.0.insert(to, item);
        }
        Ok(DataValue::list(
            self.0.iter().map(|k| DataValue::str(k)).collect(),
        ))
    }
}

const SOURCE: &str = r#"component App
  state count = 0
  state disabled = false
  resource initial = rows() as shape list<string>
  mutation changed as shape list<string>
  derive rows = match changed { case some(value) => value, case none => initial }
  action receive(item: string, before: option<string>)
    send changed = move(item, before)
    count = count + 1
  action block
    disabled = true
  view
    column
      button testId="block" press=block width=60 height=20
      list id="arrange" testId="list" virtualized=true height=100 reorderdrop=receive
        each x in rows key=x
          column reorderFor="arrange" disabled=disabled testId=`grip-${x}` height=20
            text x
"#;

fn view(h: &Host<Rows>, test_id: &str) -> ViewId {
    let key = h.runner.kernel().find_by_test_id(test_id)[0];
    h.runner.kernel().node_by_key(key).unwrap().id
}

/// Report the port three times, with the grip as the interaction pin, as the
/// Swift collection host does after `holdPointer`.
fn feedback(h: &mut Host<Rows>, top: f64, pin: Option<ViewId>) {
    for _ in 0..3 {
        let c = h.runner.collections().remove(0);
        let f = CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: top,
            port_cross: 320.,
            port_main: 100.,
            cross: 320.,
            focus_view: None,
            interaction_view: pin,
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
        let reply = h.collection_feedback(&f.encode().unwrap(), 0.);
        assert!(reply.contains("\"error\":null"), "{reply}");
    }
}

fn fixture() -> (Host<Rows>, ViewId) {
    let rows = Rows((0..100).map(|i| i.to_string()).collect());
    let plan = contract::compile(SOURCE).unwrap().encode();
    let (mut h, _) = Host::boot(
        &plan,
        rows,
        Box::new(MonospaceMeasurer::default()),
        320.,
        400.,
    )
    .unwrap();
    let grip = view(&h, "grip-0");
    feedback(&mut h, 0., Some(grip));
    (h, grip)
}

fn count(h: &Host<Rows>) -> Option<&DataValue> {
    h.runner.slot("count")
}

/// The decimal serial of the batch's last `reorder` op, and its phase.
fn state(batch: &str) -> (u64, String) {
    let op = batch.rsplit("\"op\":\"reorder\"").next().unwrap();
    let field = |name: &str| {
        op.split(&format!("\"{name}\":\""))
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned()
    };
    (field("token").parse().unwrap(), field("phase"))
}

fn begin(h: &mut Host<Rows>, grip: ViewId) -> u64 {
    let reply = h.reorder_begin(grip, 0., 100.);
    assert!(reply.contains("\"error\":null"), "{reply}");
    let (token, phase) = state(&reply);
    assert_eq!(phase, "active", "{reply}");
    token
}

fn source(h: &Host<Rows>) -> (u64, HoldToken) {
    let a = h.arrange.as_ref().unwrap();
    (motion_node(a.binding.wrapper), a.hold)
}

#[test]
fn drop_dispatches_once_while_the_source_is_held_then_settles_and_finishes() {
    let (mut h, grip) = fixture();
    let token = begin(&mut h, grip);
    let (node, hold) = source(&h);
    let reply = h.reorder_move(token, 50., 0., true, 110.);
    assert_eq!(state(&reply).1, "active", "{reply}");
    assert_eq!(
        h.engine.value(node, Property::Translate).map(|v| v.y),
        Some(50.)
    );
    assert!(h.engine.has_hold(hold));
    let reply = h.reorder_end(token, true, 50., 0., true, 400., 120.);
    assert!(reply.contains("\"error\":null"), "{reply}");
    assert_eq!(state(&reply).1, "settling", "{reply}");
    assert!(reply.contains("\"dispatched\":true"), "{reply}");
    assert_eq!(count(&h), Some(&DataValue::Number(1.)));
    assert!(!h.engine.has_hold(hold), "released after the action");
    // A second end with the old token is stale: no second action.
    let again = h.reorder_end(token, true, 50., 0., true, 0., 130.);
    assert_eq!(state(&again).1, "settling");
    assert_eq!(count(&h), Some(&DataValue::Number(1.)));
    assert!(h
        .runner
        .reorder_frame(h.arrange.as_ref().unwrap().token)
        .is_some());
    let settled = h.tick(5000.);
    assert_eq!(state(&settled).1, "finished", "{settled}");
    assert!(h.arrange.is_none());
    assert_eq!(
        state(&h.reorder_move(token, 0., 0., true, 5000.)).1,
        "finished"
    );
}

#[test]
fn rebase_keeps_every_surviving_wrapper_where_it_was_presented() {
    let (mut h, grip) = fixture();
    let token = begin(&mut h, grip);
    h.reorder_move(token, 30., 0., true, 110.);
    h.reorder_move(token, 50., 0., true, 400.);
    let a = h.arrange.as_ref().unwrap();
    let wrappers = h.runner.reorder_frame(a.token).unwrap().wrappers;
    let presented = |h: &Host<Rows>, key: NodeKey| {
        let node = h.runner.kernel().node_by_key(key)?;
        let p = h.engine.value(motion_node(key), Property::Translate)?;
        Some((node.frame.x as f64 + p.x, node.frame.y as f64 + p.y))
    };
    let before: Vec<_> = wrappers
        .iter()
        .filter_map(|w| Some((w.wrapper, presented(&h, w.wrapper)?)))
        .collect();
    let reply = h.reorder_end(token, true, 50., 0., true, 0., 400.);
    assert!(reply.contains("\"dispatched\":true"), "{reply}");
    let mut moved = 0;
    for (key, old) in before {
        let Some(new) = presented(&h, key) else {
            continue;
        };
        assert!(
            (new.0 - old.0).abs() < 0.01 && (new.1 - old.1).abs() < 0.01,
            "{key:?}: {old:?} -> {new:?}"
        );
        let frame = h.runner.kernel().node_by_key(key).unwrap().frame;
        moved += usize::from(new.1 != frame.y as f64);
    }
    assert!(moved > 1, "a rebase with no displaced wrapper is vacuous");
}

#[test]
fn an_unproved_final_sample_cannot_drop_the_previous_gap() {
    let (mut h, grip) = fixture();
    let token = begin(&mut h, grip);
    h.reorder_move(token, 50., 0., true, 110.);
    // The pointer left the port: the last certified gap is no longer eligible.
    let reply = h.reorder_end(token, true, 70., 0., false, 0., 120.);
    assert!(reply.contains("\"dispatched\":false"), "{reply}");
    assert_eq!(count(&h), Some(&DataValue::Number(0.)));
    h.tick(5000.);
    assert!(h.arrange.is_none());
    // The next contact reports its pin again, as `holdPointer` does.
    feedback(&mut h, 0., Some(grip));
    let token = begin(&mut h, grip);
    h.reorder_move(token, 50., 0., true, 5100.);
    // A scrollTop the runner has not accepted is unproved too.
    let reply = h.reorder_end(token, true, 50., 12., true, 0., 5110.);
    assert!(reply.contains("\"dispatched\":false"), "{reply}");
    assert_eq!(count(&h), Some(&DataValue::Number(0.)));
}

#[test]
fn a_receipt_that_disables_the_handle_cancels_at_receipt_time() {
    let (mut h, grip) = fixture();
    let token = begin(&mut h, grip);
    let (node, hold) = source(&h);
    h.reorder_move(token, 40., 0., true, 110.);
    let batch = h.dispatch_at(view(&h, "block"), Event::Press, 150.);
    assert!(batch.contains("\"phase\":\"settling\""), "{batch}");
    assert!(!h.engine.has_hold(hold));
    assert_eq!(
        h.engine
            .spring_descriptor(node, Property::Translate)
            .unwrap()
            .start,
        0.15
    );
    let reply = h.reorder_end(token, true, 40., 0., true, 0., 160.);
    assert_eq!(state(&reply).1, "settling");
    assert_eq!(count(&h), Some(&DataValue::Number(0.)));
    h.tick(5000.);
    assert!(h.arrange.is_none());
}

#[test]
fn refusals_take_no_hold_and_invalid_samples_change_nothing() {
    let (mut h, grip) = fixture();
    // A scrollTop the runner has not accepted, and a handle without the pin.
    assert_eq!(state(&h.reorder_begin(grip, 12., 100.)).1, "refused");
    let other = view(&h, "grip-1");
    assert_eq!(state(&h.reorder_begin(other, 0., 100.)).1, "refused");
    assert!(h.arrange.is_none());
    let token = begin(&mut h, grip);
    assert_eq!(state(&h.reorder_begin(grip, 0., 100.)).1, "refused");
    let (node, _) = source(&h);
    h.reorder_move(token, 20., 0., true, 110.);
    let clock = h.engine.now();
    let value = h.engine.value(node, Property::Translate);
    let reply = h.reorder_move(token, f64::NAN, 0., true, 200.);
    assert!(reply.contains("invalid reorder sample"), "{reply}");
    assert_eq!(h.engine.now(), clock);
    assert_eq!(h.engine.value(node, Property::Translate), value);
    let reply = h.reorder_end(token, false, 0., 0., true, 0., 200.);
    assert!(reply.contains("\"dispatched\":false"), "{reply}");
    assert_eq!(count(&h), Some(&DataValue::Number(0.)));
}

#[test]
fn eager_lists_do_not_admit_physical_reorder() {
    let source = SOURCE.replace("virtualized=true", "virtualized=false");
    let plan = contract::compile(&source).unwrap().encode();
    let rows = Rows((0..10).map(|i| i.to_string()).collect());
    let (mut h, _) = Host::boot(
        &plan,
        rows,
        Box::new(MonospaceMeasurer::default()),
        320.,
        400.,
    )
    .unwrap();
    let grip = view(&h, "grip-0");
    assert_eq!(state(&h.reorder_begin(grip, 0., 100.)).1, "refused");
    assert!(h.arrange.is_none());
}
