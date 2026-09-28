//! Allocation-identity discriminator; original-source behavioral RED is preserved.
//! No native entry, async source, or timing claim belongs to these tests.
use std::{cell::Cell, rc::Rc};

use exact_kernel::{Frame, Kernel, Offer};
use exact_plan::{Items, Value};
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

/// Whether this is the last holder of `items`; it is dropped either way.
fn sole(items: Items) -> bool {
    Items::strong_count(&items) == 1
}

fn record(value: &Value) -> &Items {
    let Value::Record(fields) = value else {
        panic!("expected canonical record")
    };
    fields
}

fn rows(value: &Value) -> &Items {
    let Value::List(items) = &record(value)[0] else {
        panic!("expected canonical history rows")
    };
    items
}

fn shared_record(a: &Value, b: &Value) -> bool {
    Items::ptr_eq(record(a), record(b))
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
        !Items::ptr_eq(rows(&before), rows(&after)),
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
            assert!(
                exact_plan::Str::ptr_eq(a, b),
                "unchanged id/sender/meta stay shared"
            );
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
    assert!(Items::ptr_eq(rows(&before), rows(&after)));
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
    // Held here too, outermost first: each count says whether anything
    // besides this test (and the one holder above it, dropped first) does.
    let old_history = record(&a).clone();
    let old_list = rows(&a).clone();
    let old_first = record(&rows(&a)[0]).clone();
    let b = canonical(&mut source, &args(1000, 9, 32, "new", 900, false));
    drop(a);
    assert!(sole(old_history));
    assert!(sole(old_list));
    assert!(sole(old_first), "unvisited old page must be released");
    let latest = record(&b).clone();
    drop(b);
    assert!(
        Items::strong_count(&latest) > 1,
        "one latest output belongs to source"
    );
    drop(source);
    assert!(sole(latest), "source drop releases final cache");
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
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
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
        offset: (snapshot.total_extent - 320.0).max(0.0),
        port_cross: width as f64,
        port_main: 320.0,
        cross: width as f64,
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
    let held = record(&accepted).clone();
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
    assert!(sole(held), "no bounded output history");
    let full_held = record(&full).clone();
    let bounded = canonical(&mut source, &request);
    drop(full);
    assert!(sole(full_held), "no full output history");
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

// The test envelope is deliberately private to this module.
mod region_admission {
    // Admission/capacity regressions; no worker or native painting claim.
    // This is the real Contract/full data source, not a shortened transcript.
    // Monospace is a kernel geometry control, NOT a native paint/worker proof.
    use exact_kernel::{
        AxisOffer, ContentRegion, Frame, Kernel, MonospaceMeasurer, NodeKey, NodeType, Offer,
        ParagraphStamp, TextMeasureRequest, TextMeasurer, TextMetrics, ViewId,
    };
    use exact_plan::Value;
    use exact_runner::{CollectionFeedback, RowMeasurement, Runner};
    use messages_stress_data::{MessagesStress, ReusableMessagesStress};
    use std::{cell::RefCell, rc::Rc};

    const APP: &str = include_str!("../../../app.contract");
    type Live = Runner<ReusableMessagesStress>;

    fn boot(source: &str) -> Live {
        let plan = contract::bake(contract::compile(source).unwrap(), MessagesStress).unwrap();
        let mut r = Runner::boot(
            plan,
            ReusableMessagesStress::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.act("toggleWindowed", vec![]).unwrap();
        r.act("chooseCount", vec![Value::Number(10_000.)]).unwrap();
        r.act("chooseBatch", vec![Value::Number(32.)]).unwrap();
        assert_eq!(r.collections()[0].count, 10_000);
        r
    }

    fn key(r: &Live, test_id: &str) -> NodeKey {
        let keys = r.kernel().find_by_test_id(test_id);
        assert_eq!(keys.len(), 1, "{test_id}");
        keys[0]
    }

    // Registration needs two direct branches. Add ONLY its test envelope: the
    // original List, every row/component/action and the composer remain intact.
    // Both variants are admission fixtures, not a proposed application patch.
    fn envelope(fixed_control: bool) -> String {
        let start = APP.find("        list virtualized=true").unwrap();
        let end = start + APP[start..].find("      else\n").unwrap();
        assert_eq!(APP.matches("        list virtualized=true").count(), 1);
        let size = if fixed_control {
            "height=500"
        } else {
            "flex=1 min-height=80"
        };
        let mut out = APP[..start].to_owned();
        out.push_str(&format!("        column testId=\"region-owner\" width=\"100%\" {size} overflow-x=\"hidden\" overflow-y=\"hidden\"\n"));
        for line in APP[start..end].lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("          text \"Preparing messages…\" testId=\"region-pending\" position=\"absolute\"\n");
        out.push_str(&APP[end..]);
        out
    }

    fn binding(r: &Live) -> ContentRegion {
        ContentRegion {
            owner: key(r, "region-owner"),
            content: key(r, "transcript"),
            pending: key(r, "region-pending"),
        }
    }

    fn view(r: &Live, test_id: &str) -> ViewId {
        r.kernel().node_by_key(key(r, test_id)).unwrap().id
    }

    fn descendants(k: &Kernel, root: ViewId) -> Vec<ViewId> {
        let mut result = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            result.push(id);
            stack.extend(k.node(id).unwrap().children().into_iter().rev());
        }
        result
    }

    fn composer(r: &Live) -> ViewId {
        let field = r.kernel().node_by_key(key(r, "stress-input")).unwrap();
        r.kernel()
            .node(field.parent.unwrap())
            .unwrap()
            .parent
            .unwrap()
    }

    // Mirror the existing shell operation using public layout primitives:
    // rebuild a TEST-ONLY cloned arena, optionally remove this engine child
    // edge, and compute. Authored nodes/styles/sources remain identical. This
    // neither relaxes production admission nor returns pending metrics.
    fn shell_frames(r: &Live, width: f32, cut: bool) -> Vec<(ViewId, Frame)> {
        let k = r.kernel();
        let owner = view(r, "region-owner");
        let mut wanted = vec![owner];
        wanted.extend(descendants(k, composer(r)));
        let mut arena = k.arena().clone();
        let mut tree = exact_kernel::layout::LayoutTree::rebuild(&mut arena);
        if cut {
            let engine = arena.taffy(arena.slot_of(owner).unwrap()).unwrap();
            tree.set_children(engine, &[]);
        }
        let root = arena.taffy(arena.slot_of(r.roots()[0]).unwrap()).unwrap();
        tree.compute(
            root,
            Offer::definite(width, 820.),
            &arena,
            &mut MonospaceMeasurer::default(),
        )
        .unwrap();
        wanted
            .into_iter()
            .map(|id| {
                let slot = arena.slot_of(id).unwrap();
                let mut chain = vec![slot];
                let mut parent = arena.parent(slot);
                while let Some(slot) = parent {
                    chain.push(slot);
                    parent = arena.parent(slot);
                }
                let (mut x, mut y) = (0., 0.);
                for slot in chain.into_iter().rev() {
                    let l = tree.layout(arena.taffy(slot).unwrap());
                    x += l.location.x;
                    y += l.location.y;
                }
                let l = tree.layout(arena.taffy(slot).unwrap());
                (
                    id,
                    Frame {
                        x,
                        y,
                        width: l.size.width,
                        height: l.size.height,
                    },
                )
            })
            .collect()
    }

    fn assert_frame(actual: Frame, expected: Frame, context: &str) {
        assert!(
            actual.bits_eq(expected),
            "{context}: {actual:?} != {expected:?}"
        );
    }

    fn configure_composer(r: &mut Live, reply: bool, draft: &str) {
        if reply {
            let index = r.collections()[0].rows[0].index;
            let Value::Record(fields) = &super::rows(r.resource("history").unwrap())[index] else {
                panic!("actual Message record");
            };
            let id = fields[0].clone();
            r.act("replyTo", vec![id]).unwrap();
        } else {
            r.act("cancelReply", vec![]).unwrap();
        }
        r.act("editDraft", vec![Value::str(draft)]).unwrap();
        assert_eq!(r.slot("draft"), Some(&Value::str(draft)));
        assert_eq!(
            r.kernel().find_by_test_id("reply-target").len(),
            usize::from(reply)
        );
    }

    #[test]
    fn real_messages_test_envelope_preserves_the_full_authored_list_block() {
        let start = APP.find("        list virtualized=true").unwrap();
        let end = start + APP[start..].find("      else\n").unwrap();
        for fixed in [false, true] {
            let source = envelope(fixed);
            assert!(source.starts_with(&APP[..start]));
            assert!(source.ends_with(&APP[end..]));
            let list = source.find("          list virtualized=true").unwrap();
            let pending = source[list..]
                .find("          text \"Preparing messages…\"")
                .unwrap()
                + list;
            let restored = source[list..pending]
                .lines()
                .map(|line| format!("{}\n", line.strip_prefix("  ").unwrap()))
                .collect::<String>();
            assert_eq!(restored, APP[start..end]);
        }
    }

    #[test]
    fn real_messages_flex_cut_preserves_owner_and_entire_composer_at_both_widths() {
        let mut original = boot(APP);
        let mut wrapped = boot(&envelope(false));
        let multiline = "First line while the worker is blocked: retain every bubble, sender and Reply button.\nSecond line keeps the composer editable while the transcript reflows at another width.\nThird line must use current text and the actual root offer, not old-width metrics.";
        let mut empty_height = None;
        for (reply, draft) in [
            (false, ""),
            (true, ""),
            (true, multiline),
            (false, multiline),
        ] {
            configure_composer(&mut original, reply, draft);
            configure_composer(&mut wrapped, reply, draft);
            for width in [980., 896.] {
                ordinary_layout(&mut original, width);
                ordinary_layout(&mut wrapped, width);
                assert_eq!(original.resource("history"), wrapped.resource("history"));
                assert_eq!(
                    original.collections()[0].rows.len(),
                    wrapped.collections()[0].rows.len()
                );
                for id in ["transcript", "stress-input", "stress-echo", "send"] {
                    assert_frame(
                        wrapped
                            .kernel()
                            .node_by_key(key(&wrapped, id))
                            .unwrap()
                            .frame,
                        original
                            .kernel()
                            .node_by_key(key(&original, id))
                            .unwrap()
                            .frame,
                        id,
                    );
                }
                let owner = wrapped
                    .kernel()
                    .node(view(&wrapped, "region-owner"))
                    .unwrap();
                assert_eq!(owner.style.height, exact_kernel::Dimension::Auto);
                assert_eq!(owner.style.flex_grow, 1.);
                assert_eq!(owner.style.min_height, exact_kernel::Dimension::Points(80.));
                assert_frame(
                    owner.frame,
                    original
                        .kernel()
                        .node_by_key(key(&original, "transcript"))
                        .unwrap()
                        .frame,
                    "envelope must not change the real transcript allocation",
                );
                let ordinary = shell_frames(&wrapped, width, false);
                let cut = shell_frames(&wrapped, width, true);
                assert_eq!(ordinary.len(), cut.len());
                for ((id, before), (other, after)) in ordinary.into_iter().zip(cut) {
                    assert_eq!(id, other);
                    assert_frame(
                        before,
                        wrapped.kernel().node(id).unwrap().frame,
                        "derived ordinary control must equal published kernel frame",
                    );
                    assert_frame(after, before, "cut must preserve owner/composer geometry");
                }
                let height = wrapped
                    .kernel()
                    .node(composer(&wrapped))
                    .unwrap()
                    .frame
                    .height;
                if !reply && draft.is_empty() {
                    empty_height = Some(height);
                }
                if reply || !draft.is_empty() {
                    assert!(
                        height > empty_height.unwrap(),
                        "composer control must really change height"
                    );
                }
                eprintln!("flex-shell width={width} reply={reply} draft_bytes={} owner={:?} composer_height={height}",
                    draft.len(), owner.frame);
            }
        }
    }

    #[test]
    fn real_messages_cut_oracle_detects_content_sized_owner_instead_of_blessing_all_auto_heights() {
        let mut r = boot(&envelope(false));
        let mut patch = exact_kernel::StyleProps::default();
        for style in [
            exact_kernel::StyleId::FlexGrow,
            exact_kernel::StyleId::FlexBasis,
            exact_kernel::StyleId::MinHeight,
        ] {
            patch.mask.set(style);
        }
        patch.flex_grow = 0.;
        patch.flex_basis = exact_kernel::Dimension::Auto;
        patch.min_height = exact_kernel::Dimension::Auto;
        let owner = view(&r, "region-owner");
        r.kernel_mut()
            .apply(
                0,
                0,
                &[exact_kernel::Op::SetStyle {
                    id: owner,
                    patch: Box::new(patch),
                }],
            )
            .unwrap();
        ordinary_layout(&mut r, 980.);
        let ordinary = shell_frames(&r, 980., false);
        let cut = shell_frames(&r, 980., true);
        assert!(ordinary[0].1.height > cut[0].1.height,
            "content-sized owner must lose height when its real children are cut: {ordinary:?} / {cut:?}");
    }

    #[test]
    fn real_messages_fixed_envelope_is_an_admission_control_only() {
        let mut r = boot(&envelope(true));
        let b = binding(&r);
        assert_eq!(
            r.kernel().node_by_key(b.content).unwrap().node_type,
            NodeType::List
        );
        assert_eq!(
            r.collections()[0].rows.len(),
            16,
            "unchanged bootstrap, no row omission"
        );
        assert!(r.kernel_mut().set_content_region(Some(b)).unwrap());
    }

    #[test]
    fn real_messages_flex_envelope_must_admit_without_fixed_transcript_height() {
        let mut r = boot(&envelope(false));
        let b = binding(&r);
        let owner = r.kernel().node_by_key(b.owner).unwrap();
        assert_eq!(owner.style.height, exact_kernel::Dimension::Auto);
        assert_eq!(r.collections()[0].rows.len(), 16);
        let result = r.kernel_mut().set_content_region(Some(b));
        // Expected first behavioral RED: current independent-size policy refuses.
        // No absent API/import is needed to expose it. Do not change this fixture
        // to fixed height to turn this assertion green.
        assert!(
            result.is_ok(),
            "complete flex transcript refused: {result:?}"
        );
    }

    fn ordinary_layout(r: &mut Live, width: f32) {
        r.set_viewport(width as f64, 820.).unwrap();
        for root in r.roots() {
            r.kernel_mut()
                .compute_layout(root, Offer::definite(width, 820.))
                .unwrap();
        }
    }

    fn tail_feedback(r: &mut Live, width: f32) {
        ordinary_layout(r, width);
        let c = r.collections().remove(0);
        let port = r.kernel().node(c.view).unwrap().frame;
        let row_width = r.kernel().node(c.rows[0].view).unwrap().frame.width;
        let measurements = c
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: r.kernel().node(row.view).unwrap().frame.height as f64,
            })
            .collect();
        r.collection_feedback(CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset: (c.total_extent - port.height as f64).max(0.),
            port_cross: port.width as f64,
            port_main: port.height as f64,
            cross: row_width as f64,
            measurements,
            focus_view: None,
            interaction_view: None,
        })
        .unwrap();
        ordinary_layout(r, width);
    }

    type Tuples = Rc<RefCell<Vec<(ParagraphStamp, Offer)>>>;
    struct CountExact(Tuples);
    impl TextMeasurer for CountExact {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(r)
        }
        fn measure_identified(
            &mut self,
            stamp: &ParagraphStamp,
            r: &TextMeasureRequest<'_>,
        ) -> TextMetrics {
            let value = (
                stamp.clone(),
                Offer {
                    width: r.width,
                    height: r.height,
                },
            );
            let mut tuples = self.0.borrow_mut();
            if !tuples.contains(&value) {
                tuples.push(value);
            }
            drop(tuples);
            self.measure(r)
        }
    }

    #[test]
    fn real_messages_reports_complete_mounted_tuple_pressure_without_changing_cap() {
        let mut r = boot(APP);
        for width in [980., 896.] {
            if width == 896. {
                r.act("step", vec![]).unwrap();
                let Value::Record(history) = r.resource("history").unwrap() else {
                    panic!("actual History record");
                };
                assert_eq!(
                    history[3],
                    Value::Number(32.),
                    "real full-history 32-body update"
                );
            }
            // Real nested port and all selected wrappers; no fixed window-height
            // stand-in, truncation to 16, or deletion of Reply/sender/meta text.
            tail_feedback(&mut r, width);
            tail_feedback(&mut r, width);
            let c = r.collections().remove(0);
            let tuples: Tuples = Default::default();
            let mut cold = r.kernel().rehydrate(Box::new(CountExact(tuples.clone())));
            for root in cold.roots() {
                cold.compute_layout(root, Offer::definite(width, 820.))
                    .unwrap();
            }
            let mut paragraphs = Vec::new();
            for row in &c.rows {
                let mut stack = vec![row.root];
                let before = paragraphs.len();
                while let Some(id) = stack.pop() {
                    let node = cold.node(id).unwrap();
                    if let Some(stamp) = node.paragraph_stamp() {
                        assert_eq!(node.node_type, NodeType::Text);
                        paragraphs.push((stamp, node.frame.width));
                    }
                    stack.extend(node.children());
                }
                assert_eq!(paragraphs.len() - before, 4, "sender/body/meta/Reply");
            }
            let mut required: Vec<_> = tuples
                .borrow()
                .iter()
                .filter(|(s, _)| paragraphs.iter().any(|(p, _)| p == s))
                .cloned()
                .collect();
            let layout_tuples = required.len();
            for (stamp, inner_width) in &paragraphs {
                // These actual four text declarations have zero padding/border.
                let value = (
                    stamp.clone(),
                    Offer {
                        width: AxisOffer::Definite(*inner_width),
                        height: AxisOffer::MaxContent,
                    },
                );
                if !required.contains(&value) {
                    required.push(value);
                }
            }
            assert_eq!(c.count, 10_000);
            assert_eq!(paragraphs.len(), c.rows.len() * 4);
            assert_eq!(exact_kernel::region::REGION_OFFERS, 64);
            eprintln!("width={width} rows={} paragraphs={} layout_tuples={layout_tuples} including_final_paint={} cap=64",
                c.rows.len(), paragraphs.len(), required.len());
            for (stamp, offer) in required {
                eprintln!("{:?} {offer:?}", stamp.owner());
            }
        }
    }

    #[test]
    fn full_messages_fixed_control_refuses_complete_offer_overflow_without_publishing_partial_rows()
    {
        let mut r = boot(&envelope(true));
        let b = binding(&r);
        r.kernel_mut().set_content_region(Some(b)).unwrap();
        let root = r.roots()[0];
        let input = exact_kernel::RegionInputs {
            catalog: 1,
            consumer_revision: r.collections()[0].revision,
        };
        let mut delivered = 0;
        loop {
            let before: Vec<_> = r
                .kernel()
                .arena()
                .iter_live()
                .map(|slot| {
                    let key = r.kernel().arena().key(slot);
                    let node = r.kernel().node_by_key(key).unwrap();
                    (key, node.frame, node.content)
                })
                .collect();
            let collection_before = r.collections();
            match r
                .kernel_mut()
                .compute_region_layout(root, Offer::definite(980., 820.), input)
            {
                Err(exact_kernel::KernelError::Layout(
                    exact_kernel::LayoutError::ContentRegion("exact-offer/source budget exhausted"),
                )) => {
                    assert_eq!(delivered, 64);
                    assert_eq!(r.kernel().region_retention().accepted_offers, 0);
                    assert_eq!(r.kernel().region_retention().candidate_offers, 64);
                    assert!(r.kernel().region_text_request().is_none());
                    assert_eq!(r.collections(), collection_before);
                    for (key, frame, content) in before {
                        let node = r.kernel().node_by_key(key).unwrap();
                        assert_frame(
                            node.frame,
                            frame,
                            "overflow must not publish partial row frames",
                        );
                        assert_eq!(node.content, content);
                    }
                    break;
                }
                Err(error) => panic!("unexpected refusal: {error:?}"),
                Ok(receipt) => {
                    assert!(
                        !receipt.current,
                        "unexpected fit: retain recorded exact tuples before repricing"
                    );
                    let request = r.kernel().region_text_request().unwrap().clone();
                    eprintln!(
                        "region tuple {delivered}: {:?} {:?}",
                        request.stamp().owner(),
                        request.offer()
                    );
                    let metrics = request.with_request(|q| MonospaceMeasurer::default().measure(q));
                    assert!(r
                        .kernel_mut()
                        .resolve_region_text(&request, metrics, Rc::new(()))
                        .unwrap());
                    delivered += 1;
                    assert!(delivered <= 64);
                }
            }
        }
    }
    // Tests-first split-profile draft; retains the real existing helper.
    include!("reuse/region_split.rs");
}
