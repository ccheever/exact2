//! Compiled Contract edge handlers through the real collection feedback seam.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Advanced, CollectionFeedback, DataError, DataSource, Runner};

#[derive(Default)]
struct Rows {
    queries: usize,
    deferred: bool,
}
impl DataSource for Rows {
    fn ready(&self) -> bool {
        !self.deferred
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        if self.deferred {
            return Err(DataError::BadArguments(
                "data executor not activated".into(),
            ));
        }
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
        offset: top,
        port_cross: 640.,
        port_main: 320.,
        cross: 640.,
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
fn measurements(r: &Runner<Rows>, height: f64) -> Vec<exact_runner::RowMeasurement> {
    r.collections()[0]
        .rows
        .iter()
        .map(|row| exact_runner::RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            size: height,
        })
        .collect()
}
fn measure(r: &mut Runner<Rows>, top: f64, height: f64) {
    let mut feedback = facts(r, top);
    feedback.measurements = measurements(r, height);
    assert!(r.collection_feedback(feedback).unwrap().error.is_none());
}
fn hits(r: &Runner<Rows>) -> (f64, f64) {
    (
        r.slot("starts").unwrap().as_number().unwrap(),
        r.slot("ends").unwrap().as_number().unwrap(),
    )
}

#[test]
fn bootstrap_then_edges_rearm_only_after_leaving_the_geometric_window() {
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
    measure(&mut r, 1100., 32.); // Confirm the edge exit after mounting this window.
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (2., 0.));
    r.act("change", vec![Value::Number(1.), Value::Number(200.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(
        hits(&r),
        (2., 0.),
        "a changed first key inside the window stays disarmed"
    );
    send(&mut r, 6080.);
    send(&mut r, 6080.);
    assert_eq!(hits(&r), (2., 1.));
    r.act("change", vec![Value::Number(1.), Value::Number(201.)])
        .unwrap();
    send(&mut r, 6112.);
    assert_eq!(
        hits(&r),
        (2., 1.),
        "a changed last key inside the window stays disarmed"
    );
    send(&mut r, 1000.);
    measure(&mut r, 1000., 32.);
    send(&mut r, 6112.);
    assert_eq!(hits(&r), (2., 2.));
}

#[test]
fn stateful_start_defers_end_once_and_empty_never_dispatches() {
    let mut r = boot(SOURCE);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    assert_eq!(hits(&r), (0., 0.));
    assert!(send(&mut r, 0.).receipts.len() <= 2);
    assert_eq!(hits(&r), (1., 0.));
    send(&mut r, 0.);
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
fn same_keys_after_start_refresh_defer_end_until_another_report() {
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
    assert_eq!(hits(&r), (1., 0.));
    send(&mut r, 0.);
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
fn tiny_rows_shift_at_most_once_per_report_then_stay_disarmed() {
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
                    size: 1.,
                })
                .collect();
            let result = r.collection_feedback(f).unwrap();
            assert!(result.error.is_none());
            assert!(result.receipts.len() <= 2);
            let shifts = if start_loads {
                n.min(2)
            } else {
                usize::from(n >= 2)
            };
            assert_eq!(r.slot("start"), Some(&Value::Number((shifts * 8) as f64)));
            assert_eq!(r.data_ref().queries, initial_queries + shifts);
            assert_eq!(hits(&r), (1., if n >= 2 { 1. } else { 0. }));
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
    // Certify the middle window, retaining both offscreen pins. Pins must not
    // make the endpoint part of the geometric window or prevent re-arming.
    let mut f = facts(&r, 3200.);
    f.focus_view = Some(first);
    f.interaction_view = Some(last);
    f.measurements = measurements(&r, 32.);
    assert!(r.collection_feedback(f).unwrap().error.is_none());
    let mut f = facts(&r, 0.);
    f.port_main = 0.;
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
        size: 32.,
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
    // Each later host report retries once; a refusal never loops in one call.
    for _ in 0..3 {
        let queries = r.data_ref().queries;
        let result = r.collection_feedback(facts(&r, 0.)).unwrap();
        assert!(result.error.is_some());
        assert_eq!(r.data_ref().queries, queries + 1);
    }
    assert_eq!(hits(&r), (0., 0.));
}

#[test]
fn refused_second_edge_retries_once_without_repeating_the_successful_start() {
    let source = SOURCE
        .replace("refused = fail", "refused = false")
        .replace(
            "action onEnd writes ends",
            "action onEnd writes ends, refused",
        )
        .replace("ends = ends + 1", "ends = ends + 1\n    refused = fail")
        .replace("fail = true", "fail = not fail");
    let mut r = boot(&source);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    r.act("armFailure", vec![]).unwrap();
    send(&mut r, 0.); // The successful stateful start defers end.
    for _ in 0..3 {
        let queries = r.data_ref().queries;
        let result = r.collection_feedback(facts(&r, 0.)).unwrap();
        assert!(result.error.is_some());
        assert_eq!(r.data_ref().queries, queries + 1);
        assert_eq!(hits(&r), (1., 0.));
    }
    r.act("armFailure", vec![]).unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
}

#[test]
fn an_edge_before_activation_commits_and_activation_asks_its_arguments_once() {
    let source = SOURCE
        .replace("starts, refused", "starts, refused, revision")
        .replace(
            "starts = starts + 1",
            "starts = starts + 1\n    revision = revision + 1",
        );
    let plan = contract::bake(contract::compile(&source).unwrap(), Rows::default()).unwrap();
    let mut r = Runner::boot(
        plan,
        Rows {
            deferred: true,
            ..Default::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let initial = r.collections()[0].count;
    let queries = r.data_ref().queries;
    // The compiled placeholder stands until activation (LLP 1038 D5, LLP
    // 1027 D4): the edge's action commits and the source is not asked.
    let result = r.collection_feedback(facts(&r, 0.)).unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(hits(&r), (1., 0.));
    assert_eq!(r.data_ref().queries, queries);
    r.data().deferred = false;
    r.data_ready().unwrap();
    assert_eq!(
        r.data_ref().queries,
        queries + 1,
        "asked once, at activation"
    );
    assert_eq!(
        r.resource_args("rows"),
        Some(
            &[
                Value::Number(0.),
                Value::Number(200.),
                Value::Number(1.),
                Value::Bool(false)
            ][..]
        ),
        "with the arguments the edge's action left"
    );
    assert_eq!(r.collections()[0].count, initial);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.), "the edge is neither lost nor run again");
}

#[test]
fn edge_handlers_are_list_only_and_take_their_bound_arguments() {
    for source in [
        SOURCE.replace("list virtualized=true height=320", "column"),
        // `change` takes two parameters: bare, or with one, is the wrong arity.
        SOURCE.replace("reachstart=onStart", "reachstart=change"),
        SOURCE.replace("reachstart=onStart", "reachstart=change(0)"),
        SOURCE.replace("reachstart=onStart", "reachstart=onStart(1)"),
    ] {
        assert!(contract::compile(&source).is_err());
    }
    contract::compile(&SOURCE.replace("virtualized=true", "virtualized=false")).unwrap();
    contract::compile(&SOURCE.replace("reachstart=onStart", "reachstart=change(0, 2)")).unwrap();
}

#[test]
fn an_edge_evaluates_its_arguments_when_it_dispatches() {
    // LLP 1054.000.006: the value the handler sees is the one at dispatch.
    let source = SOURCE
        .replace("reachend=onEnd", "reachend=endAt(mark)")
        .replace(
            "  action revise writes revision\n",
            "  state mark = 0\n  action endAt(n: number) writes ends\n    ends = n\n  action setMark(n: number) writes mark\n    mark = n\n  action revise writes revision\n",
        );
    let mut r = boot(&source.replace("reachstart=onStart ", ""));
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    r.act("setMark", vec![Value::Number(7.)]).unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (0., 7.));
}

#[test]
fn a_deferred_edge_reads_its_arguments_when_it_finally_dispatches() {
    // A start defers the end (see stateful_start_defers_end_once); the end's
    // bound argument is read when it runs, after the start changed it.
    let source = SOURCE
        .replace("reachend=onEnd", "reachend=endAt(mark)")
        .replace(
            "  action onStart writes starts, refused\n    starts = starts + 1\n",
            "  state mark = 0\n  action onStart writes starts, refused, mark\n    starts = starts + 1\n    mark = 5\n",
        )
        .replace(
            "  action revise writes revision\n",
            "  action endAt(n: number) writes ends\n    ends = n\n  action revise writes revision\n",
        );
    let mut r = boot(&source);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 0.), "the start ran and the end waited");
    send(&mut r, 0.);
    assert_eq!(
        hits(&r),
        (1., 5.),
        "the end read mark after the start set it"
    );
}

#[test]
fn lists_in_an_each_say_which_one_reached_its_end() {
    let source = r#"component App
  state count = 2
  state revision = 0
  state refused = false
  state last = -1
  state ends = 0
  resource groups = rows(0, 2, revision, refused) as shape list<number>
  resource rows = rows(0, count, revision, refused) as shape list<number>
  action onEnd(which: number) writes last, ends
    last = which
    ends = ends + 1
  view
    column
      each g in groups key=g
        list virtualized=true height=320 reachend=onEnd(g)
          each x in rows key=x
            text `${x}` height=32
"#;
    let mut r = boot(source);
    assert_eq!(r.collections().len(), 2);
    for (i, expected) in [(1usize, 1.), (0, 0.)] {
        let c = &r.collections()[i];
        let feedback = CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: 0.,
            port_cross: 640.,
            port_main: 320.,
            cross: 640.,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        };
        assert!(r.collection_feedback(feedback).unwrap().error.is_none());
        assert_eq!(r.slot("last").unwrap().as_number(), Some(expected));
    }
    assert_eq!(r.slot("ends").unwrap().as_number(), Some(2.));
}

#[test]
fn bidirectional_tiny_rows_do_not_rearm_on_endpoint_key_changes() {
    let source = SOURCE
        .replace("state count = 200", "state count = 2")
        .replace("height=32", "height=1")
        .replace(
            "action onStart writes starts, refused",
            "action onStart writes starts, refused, start",
        )
        .replace(
            "refused = fail",
            "refused = fail\n    if start > 0\n      start = 2 - start",
        )
        .replace(
            "action onEnd writes ends",
            "action onEnd writes ends, start",
        )
        .replace(
            "ends = ends + 1",
            "ends = ends + 1\n    if start < 2\n      start = 2",
        );
    let mut r = boot(&source);
    for _ in 0..12 {
        send(&mut r, 0.);
    }
    assert_eq!(r.slot("start"), Some(&Value::Number(2.)));
    assert_eq!(hits(&r), (1., 1.), "an all-fitting window must become idle");
}

#[test]
fn replacement_estimates_do_not_rearm_bidirectional_measured_tiny_rows() {
    let source = SOURCE
        .replace("height=32", "height=1")
        .replace("state start = 0", "state start = 200")
        .replace(
            "action onStart writes starts, refused",
            "action onStart writes starts, refused, start",
        )
        .replace(
            "refused = fail",
            "refused = fail\n    if start > 0\n      start = start - 100",
        )
        .replace(
            "action onEnd writes ends",
            "action onEnd writes ends, start",
        )
        .replace(
            "ends = ends + 1",
            "ends = ends + 1\n    if start < 200\n      start = start + 100",
        );
    let mut r = boot(&source);
    let mut top = 0.;
    for turn in 0..120 {
        let snapshot = r.collections().remove(0);
        if let Some(correction) = snapshot.correction {
            top = correction.offset;
        }
        let mut feedback = facts(&r, top);
        feedback.scroll_sequence = 1; // no reader scroll, only layout feedback
        feedback.measurements = measurements(&r, 1.);
        let result = r.collection_feedback(feedback).unwrap();
        assert!(result.error.is_none(), "{:?}", result.error);
        if turn > 40 {
            assert!(
                result.receipts.is_empty(),
                "settled feedback must stay idle"
            );
        }
    }
    assert_eq!(
        hits(&r),
        (1., 1.),
        "provisional replacement heights must not manufacture an edge exit"
    );
    assert_eq!(r.slot("start"), Some(&Value::Number(200.)));
    assert_eq!(r.data_ref().queries, 3);
    assert_eq!(r.collections()[0].total_extent, 200.);
}

#[test]
fn hiding_the_scrollport_does_not_rearm_an_edge() {
    let mut r = boot(SOURCE);
    send(&mut r, 0.);
    measure(&mut r, 0., 32.);
    let mut feedback = facts(&r, 0.);
    feedback.port_main = 0.;
    assert!(r.collection_feedback(feedback).unwrap().error.is_none());
    send(&mut r, 0.);
    measure(&mut r, 0., 32.);
    assert_eq!(hits(&r), (1., 0.));
}

#[test]
fn an_empty_replacement_rearms_new_data_without_dispatching_an_empty_edge() {
    let mut r = boot(SOURCE);
    r.act("change", vec![Value::Number(0.), Value::Number(2.)])
        .unwrap();
    send(&mut r, 0.);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
    r.act("change", vec![Value::Number(0.), Value::Number(0.)])
        .unwrap();
    send(&mut r, 0.);
    assert_eq!(hits(&r), (1., 1.));
    r.act("change", vec![Value::Number(10.), Value::Number(2.)])
        .unwrap();
    send(&mut r, 0.);
    send(&mut r, 0.);
    assert_eq!(hits(&r), (2., 2.));
}
