//! Allocation-identity discriminator; original-source behavioral RED is preserved.
//! No native entry, async source, or timing claim belongs to these tests.
use std::{cell::Cell, rc::Rc};

use exact_kernel::{Frame, Kernel, Offer};
use exact_plan::Value;
use exact_runner::{Answer, CollectionFeedback, DataError, DataSource, Runner, Store};
use messages_stress_data::MessagesStress;

// Baseline RED used MessagesStress here with these same assertions.
type Candidate = messages_stress_data::ReusableMessagesStress;

fn args(
    count: usize,
    revision: usize,
    batch: usize,
    echo: &str,
    offset: usize,
    full: bool,
) -> Vec<Value> {
    vec![
        Value::Number(count as f64),
        Value::Number(revision as f64),
        Value::Number(batch as f64),
        Value::str(echo),
        Value::Number(offset as f64),
        Value::Bool(full),
    ]
}

fn record(value: &Value) -> &Rc<Vec<Value>> {
    let Value::Record(fields) = value else {
        panic!("expected canonical record")
    };
    fields
}

fn rows(value: &Value) -> &Rc<Vec<Value>> {
    let Value::List(items) = &record(value)[0] else {
        panic!("expected canonical history rows")
    };
    items
}

fn shared_record(a: &Value, b: &Value) -> bool {
    Rc::ptr_eq(record(a), record(b))
}

fn shared_rows(a: &Value, b: &Value) -> usize {
    rows(a)
        .iter()
        .zip(rows(b).iter())
        .filter(|(a, b)| shared_record(a, b))
        .count()
}

fn query(source: &mut impl DataSource, args: &[Value]) -> Value {
    source.query("history", args).unwrap()
}

fn canonical(source: &mut impl DataSource, args: &[Value]) -> Value {
    let actual = query(source, args);
    let expected = query(&mut MessagesStress, args);
    assert_eq!(actual, expected, "full rows, metadata and order must match");
    assert_eq!(actual.to_bytes(), expected.to_bytes(), "canonical bytes");
    actual
}

#[test]
fn one_tick_reuses_9968_records_and_replaces_only_the_32_changed_bodies() {
    let mut source = Candidate::default();
    let before = canonical(&mut source, &args(10_000, 0, 32, "", 9900, true));
    let frozen_bytes = before.to_bytes();
    let after = canonical(&mut source, &args(10_000, 1, 32, "", 9900, true));
    assert_eq!(rows(&after).len(), 10_000);
    assert!(
        !Rc::ptr_eq(rows(&before), rows(&after)),
        "fresh list of handles"
    );
    assert_eq!(shared_rows(&before, &after), 9968);
    for i in 9968..10_000 {
        assert!(!shared_record(&rows(&before)[i], &rows(&after)[i]));
        for field in [0, 1, 4] {
            let (Value::Str(a), Value::Str(b)) = (
                &record(&rows(&before)[i])[field],
                &record(&rows(&after)[i])[field],
            ) else {
                panic!("string field")
            };
            assert!(Rc::ptr_eq(a, b), "unchanged id/sender/meta stay shared");
        }
    }
    assert_eq!(
        before.to_bytes(),
        frozen_bytes,
        "accepted old value immutable"
    );
}

#[test]
fn same_full_args_reuse_the_whole_history_even_with_new_echo_argument_storage() {
    let mut source = Candidate::default();
    let before = canonical(&mut source, &args(10_000, 8, 32, "🦀 e\u{301}", 9900, true));
    let after = canonical(&mut source, &args(10_000, 8, 32, "🦀 e\u{301}", 9900, true));
    assert!(shared_record(&before, &after));
    assert!(Rc::ptr_eq(rows(&before), rows(&after)));
}

#[test]
fn unicode_limit_and_output_ignored_offset_still_use_the_exact_full_args_key() {
    let mut source = Candidate::default();
    let echo = "🦀".repeat(512);
    let a = canonical(&mut source, &args(10_000, 8, 32, &echo, 9900, true));
    let b = canonical(&mut source, &args(10_000, 8, 32, &echo, 9800, true));
    assert_eq!(a, b, "full history ignores page offset only in its output");
    assert!(
        !shared_record(&a, &b),
        "full argument key includes that offset"
    );
    assert_eq!(shared_rows(&a, &b), 10_001);
    let c = canonical(&mut source, &args(10_000, 8, 32, &echo, 9800, true));
    assert!(shared_record(&b, &c));
}

#[test]
fn full_10k_revision_boundaries_and_batch_transitions_equal_the_stateless_control() {
    let mut source = Candidate::default();
    for revision in [0, 1, 8, 11, 12, 119, 120, 0] {
        for batch in [1, 8, 32, 8, 1] {
            canonical(&mut source, &args(10_000, revision, batch, "", 9900, true));
        }
    }
}

#[test]
fn shrinking_batch_then_reset_regenerates_the_old_and_new_suffix_union() {
    let mut source = Candidate::default();
    let a = canonical(&mut source, &args(10_000, 8, 32, "", 9900, true));
    let b = canonical(&mut source, &args(10_000, 8, 1, "", 9900, true));
    assert_eq!(shared_rows(&a, &b), 9969, "31 bodies lose the old suffix");
    let c = canonical(&mut source, &args(10_000, 0, 1, "", 9900, true));
    assert_eq!(shared_rows(&b, &c), 9999, "final changed body resets");
    let d = canonical(&mut source, &args(10_000, 0, 32, "", 9900, true));
    assert_eq!(shared_rows(&c, &d), 10_000, "revision zero has no suffix");
    assert_eq!(record(&d)[3], Value::Number(0.0));
}

#[test]
fn suffix_updates_do_not_rebuild_an_unaffected_visible_manual_page() {
    let mut source = Candidate::default();
    let a = canonical(&mut source, &args(10_000, 8, 32, "", 0, false));
    let b = canonical(&mut source, &args(10_000, 9, 1, "", 0, false));
    assert_eq!(rows(&b).len(), 100);
    assert_eq!(shared_rows(&a, &b), 100);
    assert_eq!(record(&b)[3], Value::Number(0.0));
}

#[test]
fn range_count_and_full_mode_changes_keep_exact_order_metadata_and_keys() {
    let mut source = Candidate::default();
    for request in [
        args(100, 0, 1, "", 0, false),
        args(10_000, 8, 32, "", 9900, false),
        args(10_000, 8, 32, "", 9800, false),
        args(10_000, 8, 32, "", 9800, true),
        args(10_000, 8, 32, "", 9900, true),
        args(1000, 120, 8, "", 900, false),
        args(100, 0, 32, "", 0, false),
    ] {
        canonical(&mut source, &request);
    }
}

#[test]
fn echo_add_replace_remove_preserves_all_base_rows_and_same_echo_identity() {
    let mut source = Candidate::default();
    let plain = canonical(&mut source, &args(10_000, 8, 32, "", 9900, true));
    let one = canonical(&mut source, &args(10_000, 8, 32, "hi 🦀", 9900, true));
    assert_eq!(shared_rows(&plain, &one), 10_000);
    let tick = canonical(&mut source, &args(10_000, 9, 32, "hi 🦀", 9900, true));
    assert!(shared_record(&rows(&one)[10_000], &rows(&tick)[10_000]));
    let next = canonical(
        &mut source,
        &args(10_000, 9, 32, "e\u{301} 你好", 9900, true),
    );
    assert_eq!(shared_rows(&tick, &next), 10_000);
    assert!(!shared_record(&rows(&tick)[10_000], &rows(&next)[10_000]));
    let empty = canonical(&mut source, &args(10_000, 9, 32, "", 9900, true));
    assert_eq!(shared_rows(&next, &empty), 10_000);
    assert_eq!(rows(&empty).len(), 10_000);
}

#[test]
fn invalid_full_args_and_source_preserve_the_exact_latest_cache() {
    let mut source = Candidate::default();
    let valid = args(10_000, 8, 32, "accepted", 9900, true);
    let accepted = canonical(&mut source, &valid);
    let mut invalid = Vec::new();
    for (index, values) in [
        (
            0,
            vec![f64::NAN, f64::INFINITY, -1.0, 100.5, 101.0, 100_001.0],
        ),
        (1, vec![-1.0, 0.5, 121.0]),
        (2, vec![0.0, 2.0, 33.0]),
        (4, vec![-1.0, 1.0, 10_000.0]),
    ] {
        for number in values {
            let mut request = valid.clone();
            request[index] = Value::Number(number);
            invalid.push(request);
        }
    }
    let mut too_long = valid.clone();
    too_long[3] = Value::str(&"🦀".repeat(513));
    invalid.push(too_long);
    let mut wrong_bool = valid.clone();
    wrong_bool[5] = Value::Number(1.0);
    invalid.push(wrong_bool);
    invalid.push(vec![]);
    for request in invalid {
        assert!(source.query("history", &request).is_err());
        let current = query(&mut source, &valid);
        assert!(
            shared_record(&accepted, &current),
            "invalid request cannot replace cache"
        );
    }
    assert!(source.query("missing", &valid).is_err());
    assert!(shared_record(&accepted, &query(&mut source, &valid)));
}

#[test]
fn one_latest_result_does_not_keep_old_result_or_visited_page_history_alive() {
    let mut source = Candidate::default();
    let a = canonical(&mut source, &args(1000, 8, 32, "old", 0, false));
    let old_history = Rc::downgrade(record(&a));
    let old_list = Rc::downgrade(rows(&a));
    let old_first = Rc::downgrade(record(&rows(&a)[0]));
    let b = canonical(&mut source, &args(1000, 9, 32, "new", 900, false));
    drop(a);
    assert!(old_history.upgrade().is_none());
    assert!(old_list.upgrade().is_none());
    assert!(
        old_first.upgrade().is_none(),
        "unvisited old page must be released"
    );
    let latest = Rc::downgrade(record(&b));
    drop(b);
    assert!(
        latest.upgrade().is_some(),
        "one latest output belongs to source"
    );
    drop(source);
    assert!(
        latest.upgrade().is_none(),
        "source drop releases final cache"
    );
}

#[test]
fn accepted_old_result_outlives_source_drop_without_mutation() {
    let mut source = Candidate::default();
    let old = canonical(&mut source, &args(10_000, 1, 32, "old", 9900, true));
    let bytes = old.to_bytes();
    let new = canonical(&mut source, &args(10_000, 2, 32, "new", 9900, true));
    drop(new);
    drop(source);
    assert_eq!(old.to_bytes(), bytes);
}

#[test]
fn candidate_uses_existing_synchronous_answer_without_pending_requests() {
    let mut source = Candidate::default();
    let mut store = Store::new("", []);
    let request = args(10_000, 8, 32, "", 9900, true);
    let answer = source.answer(&mut store, "history", &request).unwrap();
    let Answer::Now(value) = answer else {
        panic!("candidate must not introduce continuation timing")
    };
    assert_eq!(value, query(&mut MessagesStress, &request));
    assert_eq!(source.app_id(), MessagesStress.app_id());
}

struct Counted<D> {
    source: D,
    calls: Rc<Cell<usize>>,
}

impl<D: DataSource> DataSource for Counted<D> {
    fn app_id(&self) -> &str {
        self.source.app_id()
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.set(self.calls.get() + 1);
        self.source.query(source, args)
    }
}

fn runner<D: DataSource>(source: D) -> (Runner<Counted<D>>, Rc<Cell<usize>>) {
    let plan = contract::compile(include_str!("../../app.contract")).unwrap();
    let plan = contract::bake(plan, MessagesStress).unwrap();
    let calls = Rc::new(Cell::new(0));
    let data = Counted {
        source,
        calls: calls.clone(),
    };
    let mut runner = Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.act("toggleWindowed", vec![]).unwrap();
    runner
        .act("chooseCount", vec![Value::Number(10_000.0)])
        .unwrap();
    runner
        .act("chooseBatch", vec![Value::Number(32.0)])
        .unwrap();
    (runner, calls)
}

fn layout<D: DataSource>(r: &mut Runner<D>, width: f32) -> Vec<(usize, Frame)> {
    r.set_viewport(width as f64, 900.0).unwrap();
    let snapshot = r.collections().remove(0);
    r.collection_feedback(CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: snapshot.scroll_sequence + 1,
        scroll_top: (snapshot.total_extent - 320.0).max(0.0),
        port_width: width as f64,
        port_height: 320.0,
        row_width: width as f64,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    })
    .unwrap();
    for root in r.roots() {
        r.kernel_mut()
            .compute_layout(root, Offer::definite(width, 900.0))
            .unwrap();
    }
    r.collections()
        .remove(0)
        .rows
        .iter()
        .map(|row| (row.index, r.kernel().node(row.root).unwrap().frame))
        .collect()
}

#[test]
fn actual_contract_typing_and_width_do_zero_queries_and_ticks_key_only_changed_rows() {
    let (mut cached, calls) = runner(Candidate::default());
    let (mut control, control_calls) = runner(MessagesStress);
    assert_eq!(layout(&mut cached, 640.0), layout(&mut control, 640.0));
    calls.set(0);
    control_calls.set(0);
    cached
        .act("editDraft", vec![Value::str("typing 🦀")])
        .unwrap();
    control
        .act("editDraft", vec![Value::str("typing 🦀")])
        .unwrap();
    assert_eq!(layout(&mut cached, 520.0), layout(&mut control, 520.0));
    assert_eq!(calls.get(), 0, "typing and width cannot ask history");
    assert_eq!(control_calls.get(), 0);
    cached.act("start", vec![]).unwrap();
    control.act("start", vec![]).unwrap();
    calls.set(0);
    control_calls.set(0);
    for step in 1..=3 {
        let old = cached.resource("history").unwrap().clone();
        cached.advance(step as f64 * 250.0).unwrap();
        control.advance(step as f64 * 250.0).unwrap();
        assert_eq!(calls.get(), step);
        assert_eq!(control_calls.get(), step);
        assert_eq!(cached.resource("history"), control.resource("history"));
        assert_eq!(shared_rows(&old, cached.resource("history").unwrap()), 9968);
        assert_eq!(
            cached.last_instance_work().rows_keyed,
            32,
            "only changed immutable records need key evaluation"
        );
        assert_eq!(
            control.last_instance_work().rows_keyed,
            10_000,
            "fresh control records still require every key evaluation"
        );
        assert_eq!(layout(&mut cached, 640.0), layout(&mut control, 640.0));
        assert!(!cached.is_poisoned());
    }
}

#[test]
fn explicit_none_keeps_full_10k_reuse_and_all_nine_canonical_fields() {
    let mut source = Candidate::default();
    let mut request = args(10_000, 0, 32, "", 9900, true);
    request.push(Value::Option(None));
    let before = canonical(&mut source, &request);
    assert_eq!(record(&before).len(), 9);
    request[1] = Value::Number(1.);
    let after = canonical(&mut source, &request);
    assert_eq!(rows(&after).len(), 10_000);
    assert_eq!(shared_rows(&before, &after), 9968);
    assert_eq!(&record(&after)[5..], &record(&before)[5..]);
    assert!(shared_record(&after, &canonical(&mut source, &request)));

    request.pop();
    let omitted = canonical(&mut source, &request);
    assert_eq!(omitted, after);
    assert!(shared_record(&omitted, &after), "omitted cursor is none");
    assert_eq!(shared_rows(&omitted, &after), 10_000);
    request[5] = Value::Bool(false);
    assert_eq!(rows(&canonical(&mut source, &request)).len(), 100);
}

#[test]
fn bounded_cursors_ticks_echo_and_ignored_page_controls_equal_the_canonical_source() {
    let mut source = Candidate::default();
    for count in [100, 1000, 10_000, 100_000] {
        for cursor in ["", "0", "300", "999", "999999999999999999999999999999999"] {
            for (revision, batch, echo) in [(0, 32, ""), (8, 32, "🦀"), (8, 1, "é"), (0, 1, "")]
            {
                // Bounded selection ignores page alignment/range and eager;
                // integer validation still applies, exactly as in the control.
                let mut request = args(count, revision, batch, echo, 99_999, true);
                request.push(Value::some(Value::str(cursor)));
                let value = canonical(&mut source, &request);
                assert_eq!(record(&value).len(), 9);
                assert!(rows(&value).len() <= 201);
                assert!(shared_record(&value, &canonical(&mut source, &request)));
            }
        }
    }
}

#[test]
fn bounded_refusals_preserve_latest_and_mode_changes_release_only_unowned_outputs() {
    let mut source = Candidate::default();
    let mut request = args(10_000, 8, 32, "accepted 🦀", 99_999, true);
    request.push(Value::some(Value::str("")));
    let accepted = canonical(&mut source, &request);
    let bytes = accepted.to_bytes();
    let weak = Rc::downgrade(record(&accepted));
    for selection in [
        Value::some(Value::str("-1")),
        Value::some(Value::str(" 1")),
        Value::some(Value::str("١")),
        Value::some(Value::Number(1.)),
        Value::str(""),
        Value::Option(None), // Invalid manual page offset once cursor is absent.
    ] {
        let mut invalid = request.clone();
        invalid[6] = selection;
        assert!(source.query("history", &invalid).is_err());
        assert!(MessagesStress.query("history", &invalid).is_err());
        assert!(shared_record(&accepted, &query(&mut source, &request)));
    }
    let mut extra = request.clone();
    extra.push(Value::Option(None));
    assert!(source.query("history", &extra).is_err());
    let mut too_long = request.clone();
    too_long[3] = Value::str(&"🦀".repeat(513));
    assert!(source.query("history", &too_long).is_err());
    assert!(shared_record(&accepted, &query(&mut source, &request)));

    let full = canonical(&mut source, &args(10_000, 8, 32, "full", 9900, true));
    assert_eq!(rows(&full).len(), 10_001);
    assert_eq!(
        accepted.to_bytes(),
        bytes,
        "accepted old result is immutable"
    );
    drop(accepted);
    assert!(weak.upgrade().is_none(), "no bounded output history");
    let full_weak = Rc::downgrade(record(&full));
    let bounded = canonical(&mut source, &request);
    drop(full);
    assert!(full_weak.upgrade().is_none(), "no full output history");
    drop(source);
    assert_eq!(bounded.to_bytes(), bytes, "accepted result outlives source");
}

#[test]
fn actual_contract_reusable_bounded_mode_preserves_controls_and_zero_query_typing() {
    let (mut cached, calls) = runner(Candidate::default());
    let (mut control, _) = runner(MessagesStress);
    for action in [
        "toggleBounded",
        "step",
        "reachEarlier",
        "step",
        "reachLater",
        "latest",
    ] {
        cached.act(action, vec![]).unwrap();
        control.act(action, vec![]).unwrap();
        assert_eq!(cached.resource("history"), control.resource("history"));
        assert!(rows(cached.resource("history").unwrap()).len() <= 200);
        assert!(cached.last_instance_work().rows_keyed <= 200);
        assert!(!cached.is_poisoned());
    }
    let queries = calls.get();
    cached
        .act("editDraft", vec![Value::str("bounded 🦀")])
        .unwrap();
    cached.set_viewport(520., 900.).unwrap();
    assert_eq!(calls.get(), queries, "typing/width do not query history");
    cached.act("sendDraft", vec![]).unwrap();
    assert_eq!(rows(cached.resource("history").unwrap()).len(), 201);
    cached.act("toggleWindowed", vec![]).unwrap();
    assert_eq!(rows(cached.resource("history").unwrap()).len(), 10_001);
    cached.act("reset", vec![]).unwrap();
    assert_eq!(rows(cached.resource("history").unwrap()).len(), 100);
    assert_eq!(cached.slot("bounded"), Some(&Value::Bool(false)));
}
