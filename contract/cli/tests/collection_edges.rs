//! Compiled Contract edge handlers through the real collection feedback seam.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Advanced, CollectionFeedback, DataError, DataSource, Runner};

#[derive(Default)]
struct Rows {
    queries: usize,
}
impl DataSource for Rows {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        let [Value::Number(start), Value::Number(count), Value::Number(revision), Value::Bool(refused)] =
            args
        else {
            panic!("fixture arguments")
        };
        if *refused {
            return Err(DataError::BadArguments("edge refused".into()));
        }
        if source == "interior" {
            return Ok(Value::list(vec![
                Value::Number(0.),
                Value::Number(revision + 10.),
                Value::Number(2.),
            ]));
        }
        Ok(Value::list(
            (*start as usize..(*start + *count) as usize)
                .map(|i| Value::Number(i as f64))
                .collect(),
        ))
    }
}
const SOURCE: &str = r#"component App
  state start = 0
  state count = 200
  state revision = 0
  state starts = 0
  state ends = 0
  state refused = false
  state fail = false
  resource rows = rows(start, count, revision, refused) as shape list<number>
  action onStart writes starts, refused
    starts = starts + 1
    refused = fail
  action onEnd writes ends
    ends = ends + 1
  action change(first: number, size: number) writes start, count
    start = first
    count = size
  action revise writes revision
    revision = revision + 1
  action armFailure writes fail
    fail = true
  view
    list virtualized=true height=320 reachstart=onStart reachend=onEnd
      each x in rows key=x
        text `${x}` testId=`row-${x}` height=32
"#;
fn boot(source: &str) -> Runner<Rows> {
    Runner::boot(
        contract::compile(source).unwrap(),
        Rows::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn facts(r: &Runner<Rows>, top: f64) -> CollectionFeedback {
    let c = &r.collections()[0];
    CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        scroll_top: top,
        port_width: 640.,
        port_height: 320.,
        row_width: 640.,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
}
fn send(r: &mut Runner<Rows>, top: f64) -> Advanced {
    let result = r.collection_feedback(facts(r, top)).unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    result
}
fn hits(r: &Runner<Rows>) -> (f64, f64) {
    (
        r.slot("starts").unwrap().as_number().unwrap(),
        r.slot("ends").unwrap().as_number().unwrap(),
    )
}

#[test]
fn bootstrap_then_edges_rearm_on_leaving_or_endpoint_key_change() {
    let mut r = boot(SOURCE);
    assert_eq!(r.collections()[0].rows.len(), 16);
    assert_eq!(hits(&r), (0., 0.));
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.));
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.));
    r.act("revise", vec![]).unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.), "same keys must stay disarmed");
    send(&mut r, 1000.);
    let queries = r.data_ref().queries;
    send(&mut r, 1100.);
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (2., 0.));
    r.act("change", vec![Value::Number(1.), Value::Number(200.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (3., 0.));
    send(&mut r, 6080.);
    send(&mut r, 6080.);
    assert_eq!(hits(&r), (3., 1.));
    r.act("change", vec![Value::Number(1.), Value::Number(201.)])
        .unwrap();
    send(&mut r, 6112.);
    assert_eq!(hits(&r), (3., 2.));
    send(&mut r, 1000.);
    send(&mut r, 6112.);
    assert_eq!(hits(&r), (3., 3.));
}

#[test]
fn both_edges_dispatch_start_then_end_once_and_empty_never_dispatches() {
    let mut r = boot(SOURCE);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    assert_eq!(hits(&r), (0., 0.));
    assert!(send(&mut r, 0.).receipts.len() <= 3);
    assert_eq!(hits(&r), (1., 1.));
    let edges: Vec<_> = r.journal().filter(|line| line.contains("reach")).collect();
    assert!(edges[0].contains("reachstart"));
    assert!(edges[1].contains("reachend"));
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
    r.act("change", vec![Value::Number(0.), Value::Number(0.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
}

#[test]
fn no_op_start_does_not_require_another_host_report_for_end() {
    let mut r = boot(&SOURCE.replace("starts = starts + 1", "starts = starts"));
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (0., 1.));
    send(&mut r, 0.);
    assert_eq!(hits(&r), (0., 1.));
    assert_eq!(
        r.journal()
            .filter(|line| line.contains("reachstart view"))
            .count(),
        1
    );
}

#[test]
fn same_keys_after_start_refresh_still_dispatch_end_in_the_same_call() {
    let source = SOURCE
        .replace(
            "action onStart writes starts, refused",
            "action onStart writes starts, refused, revision",
        )
        .replace(
            "refused = fail",
            "refused = fail\n    revision = revision + 1",
        );
    let mut r = boot(&source);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    let queries = r.data_ref().queries;
    send(&mut r, 0.);
    assert_eq!(r.data_ref().queries, queries + 1);
    assert_eq!(hits(&r), (1., 1.));
}

#[test]
fn changed_interior_membership_defers_end_even_when_endpoints_are_unchanged() {
    let source = SOURCE
        .replace("resource rows = rows(", "resource rows = interior(")
        .replace(
            "action onStart writes starts, refused",
            "action onStart writes starts, refused, revision",
        )
        .replace(
            "refused = fail",
            "refused = fail\n    revision = revision + 1",
        );
    let mut r = boot(&source);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.));
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    send(&mut r, 0.);
    assert_eq!(
        hits(&r),
        (1., 1.),
        "end stays armed for the new membership's feedback"
    );
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
}

#[test]
fn tiny_rows_shift_only_once_per_feedback_whether_start_or_end_loads() {
    for start_loads in [true, false] {
        let source = SOURCE
            .replace("state count = 200", "state count = 8")
            .replace("height=32", "height=1")
            .replace(
                "action onStart writes starts, refused",
                "action onStart writes starts, refused, start",
            )
            .replace(
                "refused = fail",
                if start_loads {
                    "refused = fail\n    start = start + 8"
                } else {
                    "refused = fail"
                },
            )
            .replace(
                "action onEnd writes ends",
                "action onEnd writes ends, start",
            )
            .replace("ends = ends + 1", "ends = ends + 1\n    start = start + 8");
        let mut r = boot(&source);
        let initial_queries = r.data_ref().queries;
        for n in 1..=20 {
            let mut f = facts(&r, 0.);
            f.measurements = r.collections()[0]
                .rows
                .iter()
                .map(|row| exact_runner::RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    height: 1.,
                })
                .collect();
            let result = r.collection_feedback(f).unwrap();
            assert!(result.error.is_none());
            assert!(result.receipts.len() <= if start_loads { 2 } else { 3 });
            assert_eq!(r.slot("start"), Some(&Value::Number((n * 8) as f64)));
            assert_eq!(r.data_ref().queries, initial_queries + n);
            assert_eq!(
                hits(&r),
                (n as f64, if start_loads { 0. } else { n as f64 })
            );
        }
    }
}

#[test]
fn pinned_endpoints_do_not_qualify_and_zero_port_has_no_geometric_window() {
    let mut r = boot(SOURCE);
    send(&mut r, 0.);
    let first = r.collections()[0].rows[0].root;
    let mut f = facts(&r, 6080.);
    f.focus_view = Some(first);
    r.collection_feedback(f).unwrap();
    let last = r.collections()[0].rows.last().unwrap().root;
    let mut f = facts(&r, 3200.);
    f.focus_view = Some(first);
    f.interaction_view = Some(last);
    r.collection_feedback(f).unwrap();
    assert!(r.collections()[0].rows.iter().any(|row| row.root == first));
    assert!(r.collections()[0].rows.iter().any(|row| row.root == last));
    assert_eq!(hits(&r), (1., 1.));
    let mut f = facts(&r, 0.);
    f.port_height = 0.;
    f.focus_view = Some(first);
    r.collection_feedback(f).unwrap();
    assert_eq!(hits(&r), (1., 1.));
    send(&mut r, 0.);
    assert_eq!(hits(&r), (2., 1.));
}

#[test]
fn stale_revision_sequence_and_measurement_epoch_do_not_dispatch() {
    let mut r = boot(SOURCE);
    let old = facts(&r, 0.);
    send(&mut r, 3200.);
    assert!(r.collection_feedback(old).unwrap().receipts.is_empty());
    let mut old = facts(&r, 0.);
    old.scroll_sequence = 0;
    assert!(r.collection_feedback(old).unwrap().receipts.is_empty());
    let mut old = facts(&r, 0.);
    let row = &r.collections()[0].rows[0];
    old.measurements.push(exact_runner::RowMeasurement {
        view: row.view,
        epoch: row.epoch + 1,
        height: 32.,
    });
    assert!(r.collection_feedback(old).unwrap().receipts.is_empty());
    assert_eq!(hits(&r), (0., 0.));
}

#[test]
fn absent_handlers_never_dispatch_and_end_does_not_wait_for_absent_start() {
    let mut r = boot(&SOURCE.replace(" reachstart=onStart reachend=onEnd", ""));
    let queries = r.data_ref().queries;
    send(&mut r, 0.);
    send(&mut r, 6080.);
    assert_eq!(hits(&r), (0., 0.));
    assert_eq!(r.data_ref().queries, queries);
    let mut r = boot(&SOURCE.replace(" reachstart=onStart", ""));
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (0., 1.));
}

#[test]
fn refused_edge_action_returns_committed_feedback_and_rolls_back_action_only() {
    let mut r = boot(SOURCE);
    send(&mut r, 3200.);
    r.act("armFailure", vec![]).unwrap();
    let result = r.collection_feedback(facts(&r, 0.)).unwrap();
    assert!(matches!(
        result.error,
        Some(exact_runner::RunnerError::Data { .. })
    ));
    assert_eq!(result.receipts.len(), 1);
    assert!(!result.receipts[0].receipt.created.is_empty());
    assert!(!r.kernel().find_by_test_id("row-0").is_empty());
    assert_eq!(hits(&r), (0., 0.));
    assert_eq!(r.slot("refused"), Some(&Value::Bool(false)));
    assert!(!r.is_poisoned());
    // The attempted edge is disarmed even when the action refuses.
    send(&mut r, 0.);
    assert_eq!(hits(&r), (0., 0.));
}

#[test]
fn edge_handlers_are_list_only_and_take_no_arguments() {
    for source in [
        SOURCE.replace("list virtualized=true height=320", "column"),
        SOURCE.replace("reachstart=onStart", "reachstart=change(0, 2)"),
        SOURCE.replace("reachstart=onStart", "reachstart=change"),
    ] {
        assert!(contract::compile(&source).is_err());
    }
    contract::compile(&SOURCE.replace("virtualized=true", "virtualized=false")).unwrap();
}
