//! Apple collection transport and host commits, using the actual runner window.
use super::*;
use exact_runner::{CollectionFeedback, CollectionSnapshot, DataError, RowMeasurement, Value};
use std::cell::Cell as Counter;

const SOURCE: &str = r#"
component App
  state showing = true
  resource rows = rows() as shape list<number>
  action toggle writes showing
    showing = !showing
  view
    column
      button press=toggle testId="toggle"
        text "toggle"
      when showing
        list virtualized=true height=160 width=240 testId="history"
          each row in rows key=row
            text `${row}` testId=`row-${row}`
"#;

#[derive(Default)]
struct Rows {
    queries: Rc<Counter<usize>>,
}

impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        self.queries.set(self.queries.get() + 1);
        Ok(Value::list(
            (0..1000).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}

fn fixture(source: &str) -> (Bridge<Rows>, String, Rc<Counter<usize>>) {
    let data = Rows::default();
    let queries = data.queries.clone();
    let plan = contract::compile(source).unwrap().encode();
    let (host, batch) = Host::boot(
        &plan,
        data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap();
    // Feedback does not run requests. Keep these tests independent of worker
    // admission and avoid consuming the scheduler's process-wide worker budget.
    let mut bridge = Bridge::new();
    bridge.host = Some(host);
    (bridge, batch, queries)
}

fn snapshot(bridge: &Bridge<Rows>) -> CollectionSnapshot {
    bridge
        .host
        .as_ref()
        .unwrap()
        .runner()
        .collections()
        .remove(0)
}

fn facts(snapshot: &CollectionSnapshot, top: f64) -> CollectionFeedback {
    CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: snapshot.scroll_sequence + 1,
        scroll_top: top,
        port_width: 240.,
        port_height: 160.,
        row_width: 240.,
        measurements: Vec::new(),
        focus_view: None,
        interaction_view: None,
    }
}

fn send(bridge: &mut Bridge<Rows>, facts: &CollectionFeedback, now_ms: f64) -> String {
    let bytes = facts.encode().unwrap();
    bridge.input_write(&bytes);
    let len = bridge.collection_feedback(bytes.len(), now_ms);
    String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
}

fn state(bridge: &Bridge<Rows>) -> (String, String, f64, f64) {
    let host = bridge.host.as_ref().unwrap();
    (
        host.runner().collections_json(),
        host.agent(r#"{"op":"tree"}"#),
        host.engine().now(),
        host.runner().now_ms(),
    )
}

#[test]
fn collection_boot_uses_common_json_after_frames_and_ticks_do_not_emit_it() {
    let (mut bridge, batch, queries) = fixture(SOURCE);
    let snapshot = snapshot(&bridge);
    assert_eq!(snapshot.count, 1000);
    assert!(snapshot.rows.len() < 64);
    let host = bridge.host.as_ref().unwrap();
    let op = format!(
        "{{\"op\":\"collections\",\"items\":{}}}",
        host.runner().collections_json()
    );
    assert!(batch.contains(&op), "{batch}");
    assert!(batch.rfind("\"op\":\"frame\"").unwrap() < batch.find(&op).unwrap());
    let queries_before = queries.get();
    for time in [1.0, 10.0, 20.0] {
        let len = bridge.tick(time);
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
        assert!(!batch.contains("\"op\":\"collections\""), "{batch}");
    }
    // An unchanged layout also avoids retransmitting the identical metadata.
    let len = bridge.resize(400., 600.);
    assert!(!String::from_utf8_lossy(bridge.output_bytes(len as usize))
        .contains("\"op\":\"collections\""));
    assert_eq!(queries.get(), queries_before);
}

#[test]
fn collection_scroll_and_measurements_commit_without_requerying_resources() {
    let (mut bridge, _, queries) = fixture(SOURCE);
    let before = snapshot(&bridge);
    let queries_before = queries.get();
    let batch = send(&mut bridge, &facts(&before, 16_000.), 10.);
    assert!(batch.contains("\"error\":null"), "{batch}");
    for op in ["destroy", "create", "children", "frame", "collections"] {
        assert!(
            batch.contains(&format!("\"op\":\"{op}\"")),
            "missing {op}: {batch}"
        );
    }
    let after = snapshot(&bridge);
    assert!(after.rows.iter().any(|row| row.index == 500));
    assert!(after.rows.iter().all(|row| row.index > 0));
    assert!(after.rows.len() < 64);
    let host = bridge.host.as_ref().unwrap();
    assert!(host.runner().kernel().find_by_test_id("row-0").is_empty());
    assert_eq!(host.runner().last_instance_work().rows_keyed, 0);
    assert_eq!(host.engine().now(), 0.01);
    let mut measurement = facts(&after, 16_000.);
    measurement.measurements.push(RowMeasurement {
        view: after.rows[0].view,
        epoch: after.rows[0].epoch,
        height: 43.,
    });
    let batch = send(&mut bridge, &measurement, 20.);
    assert!(batch.contains("\"error\":null"), "{batch}");
    assert!(snapshot(&bridge)
        .rows
        .iter()
        .any(|row| row.height == 43. && row.measured));
    assert_eq!(queries.get(), queries_before);
}

#[test]
fn collection_stale_feedback_changes_neither_tree_nor_motion_clock() {
    let (mut bridge, _, queries) = fixture(SOURCE);
    let old = facts(&snapshot(&bridge), 16_000.);
    send(&mut bridge, &old, 10.);
    let before = state(&bridge);
    let count = queries.get();
    let batch = send(&mut bridge, &old, 1000.);
    assert!(batch.starts_with("{\"ops\":[]"), "{batch}");
    assert!(batch.contains("\"error\":null"), "{batch}");
    assert_eq!(state(&bridge), before);
    let mut unknown = facts(&snapshot(&bridge), 0.);
    unknown.view = u32::MAX;
    assert!(send(&mut bridge, &unknown, 1000.).starts_with("{\"ops\":[]"));
    assert_eq!(state(&bridge), before);
    assert_eq!(queries.get(), count);
}

#[test]
fn collection_input_bounds_and_malformed_bytes_are_nonmutating() {
    let (mut bridge, _, queries) = fixture(SOURCE);
    let bytes = facts(&snapshot(&bridge), 16_000.).encode().unwrap();
    let before = state(&bridge);
    let count = queries.get();
    for len in [0, bytes.len() - 1, bytes.len() + 1, usize::MAX] {
        bridge.input_write(&bytes);
        let len = bridge.collection_feedback(len, 1000.);
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
        assert!(batch.starts_with("{\"ops\":[]"), "{batch}");
        assert!(batch.contains("malformed collection feedback"), "{batch}");
        assert_eq!(state(&bridge), before);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    let mut nonfinite = bytes.clone();
    nonfinite[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
    for malformed in [trailing, nonfinite] {
        let len = bridge.input_write(&malformed);
        let len = bridge.collection_feedback(len, 1000.);
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
        assert!(batch.contains("malformed collection feedback"), "{batch}");
        assert_eq!(state(&bridge), before);
    }
    assert_eq!(queries.get(), count);
    assert!(!bridge.host.as_ref().unwrap().runner().is_poisoned());
    // Rejection never consumes otherwise-valid feedback.
    let feedback = facts(&snapshot(&bridge), 16_000.);
    let batch = send(&mut bridge, &feedback, 20.);
    assert!(batch.contains("\"op\":\"collections\""), "{batch}");
    assert_eq!(bridge.host.as_ref().unwrap().engine().now(), 0.02);
}

#[test]
fn collection_feedback_rejects_invalid_time_before_mutation() {
    let (mut bridge, _, _) = fixture(SOURCE);
    let feedback = facts(&snapshot(&bridge), 16_000.);
    let before = state(&bridge);
    for time in [
        f64::NAN,
        f64::INFINITY,
        -1.0,
        exact_runner::MAX_CLOCK_MS + 1.,
    ] {
        let batch = send(&mut bridge, &feedback, time);
        assert!(batch.starts_with("{\"ops\":[]"), "{batch}");
        assert!(
            batch.contains("invalid collection feedback time"),
            "{batch}"
        );
        assert_eq!(state(&bridge), before);
    }
}

#[test]
fn collection_removal_clears_snapshot_and_recreation_rejects_old_feedback() {
    let (mut bridge, _, _) = fixture(SOURCE);
    let old = facts(&snapshot(&bridge), 16_000.);
    let host = bridge.host.as_ref().unwrap();
    let kernel = host.runner().kernel();
    let toggle = kernel
        .node_by_key(kernel.find_by_test_id("toggle")[0])
        .unwrap()
        .id;
    let len = bridge.dispatch(toggle, 0, 0, 10.);
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(
        batch.contains("{\"op\":\"collections\",\"items\":[]}"),
        "{batch}"
    );
    assert!(bridge
        .host
        .as_ref()
        .unwrap()
        .runner()
        .collections()
        .is_empty());
    bridge.dispatch(toggle, 0, 0, 20.);
    assert_ne!(snapshot(&bridge).view, old.view);
    let before = state(&bridge);
    assert!(send(&mut bridge, &old, 1000.).starts_with("{\"ops\":[]"));
    assert_eq!(state(&bridge), before);
}

#[test]
fn ordinary_eager_list_has_no_collection_metadata() {
    let (mut bridge, batch, _) = fixture(&SOURCE.replace("virtualized=true", "virtualized=false"));
    assert!(!batch.contains("\"op\":\"collections\""));
    let host = bridge.host.as_ref().unwrap();
    assert!(host.runner().collections().is_empty());
    assert_eq!(host.runner().kernel().find_by_test_id("row-999").len(), 1);
    let before = state(&bridge);
    let feedback = CollectionFeedback {
        view: 1,
        revision: 0,
        scroll_sequence: 1,
        scroll_top: 10.,
        port_width: 240.,
        port_height: 160.,
        row_width: 240.,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    };
    assert!(send(&mut bridge, &feedback, 1000.).starts_with("{\"ops\":[]"));
    assert_eq!(state(&bridge), before);
}

#[test]
fn collection_end_follow_is_runner_owned_while_eager_lists_keep_the_native_prop() {
    let source = SOURCE.replace("height=160", "scrollFollowEnd=true height=160");
    let (bridge, batch, _) = fixture(&source);
    let host = bridge.host.as_ref().unwrap();
    let node = host.runner().kernel().node(snapshot(&bridge).view).unwrap();
    assert_eq!(
        node.props.bool(exact_kernel::PropId::ScrollFollowEnd),
        Some(true)
    );
    assert!(!batch.contains("\"scrollFollowEnd\""), "{batch}");
    let (_, eager_batch, _) = fixture(&source.replace("virtualized=true", "virtualized=false"));
    assert!(
        eager_batch.contains("\"scrollFollowEnd\":\"true\""),
        "{eager_batch}"
    );
}

// Instantiate the actual export, including the runtime identity/reentrancy guard.
#[allow(unsafe_code)]
mod exports {
    use super::*;
    crate::host!(Rows, &[], "{}");

    #[test]
    fn collection_export_uses_the_runtime_buffer_and_refuses_dead_or_busy_handles() {
        let rt = exact_create();
        let (bridge, _, _) = fixture(SOURCE);
        let bytes = facts(&snapshot(&bridge), 16_000.).encode().unwrap();
        with_entry(&EXACT_RUNTIMES, rt, |entry| {
            entry.bridge = bridge;
            entry.bridge.input_write(&bytes);
        });
        let len = exact_collection_feedback(rt, bytes.len(), 10.);
        let entry = EXACT_RUNTIMES.with(|r| r.borrow().get(rt)).unwrap();
        let guard = entry.borrow();
        assert!(
            String::from_utf8_lossy(guard.bridge.output_bytes(len as usize))
                .contains("\"op\":\"collections\"")
        );
        let before = state(&guard.bridge);
        exact_collection_feedback(rt, bytes.len(), 1000.);
        REFUSAL.with(|r| assert!(String::from_utf8_lossy(&r.borrow()).contains("busy")));
        assert_eq!(state(&guard.bridge), before);
        drop(guard);
        exact_destroy(rt);
        exact_collection_feedback(rt, bytes.len(), 1000.);
        REFUSAL.with(|r| assert!(String::from_utf8_lossy(&r.borrow()).contains("no such runtime")));
    }
}
