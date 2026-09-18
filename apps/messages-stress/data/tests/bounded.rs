//! S1: the shipped Contract, source and runner at three history cardinalities.
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, Runner};
use messages_stress_data::{model::WINDOW_SIZE, MessagesStress};

#[derive(Default)]
struct Counted {
    queries: usize,
}
impl DataSource for Counted {
    fn app_id(&self) -> &str {
        "com.exact.messages-stress"
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        MessagesStress.query(source, args)
    }
}
fn boot(count: usize) -> Runner<Counted> {
    let plan = contract::bake(
        contract::compile(include_str!("../../app.contract")).unwrap(),
        MessagesStress,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        Counted::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("chooseCount", vec![Value::Number(count as f64)])
        .unwrap();
    r.act("toggleBounded", vec![]).unwrap();
    r
}
fn fields(r: &Runner<Counted>) -> &[Value] {
    let Value::Record(fields) = r.resource("history").unwrap() else {
        panic!("history")
    };
    fields
}
fn rows(r: &Runner<Counted>) -> &[Value] {
    let Value::List(rows) = &fields(r)[0] else {
        panic!("rows")
    };
    rows
}
fn id(row: &Value) -> &str {
    let Value::Record(fields) = row else {
        panic!("row")
    };
    fields[0].as_str().unwrap()
}
fn has(r: &Runner<Counted>, earlier: bool) -> bool {
    fields(r)[if earlier { 7 } else { 8 }] == Value::Bool(true)
}
fn feed(r: &mut Runner<Counted>, top: f64) -> (f64, usize) {
    let c = r.collections().remove(0);
    let journal = r.journal_start() + r.journal().count();
    let result = r
        .collection_feedback(CollectionFeedback {
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
        })
        .unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    let events = r
        .journal()
        .skip(journal - r.journal_start())
        .filter(|line| line.contains("reachstart view") || line.contains("reachend view"))
        .count();
    assert!(events <= 1, "at most one edge dispatch per feedback call");
    let c = r.collections().remove(0);
    assert_eq!(c.count, rows(r).len());
    assert!(c.count <= WINDOW_SIZE + 1);
    assert!(c.rows.len() <= 34, "mounted {}", c.rows.len());
    (c.correction.map_or(top, |c| c.scroll_top), events)
}
fn at_tail(r: &mut Runner<Counted>) -> f64 {
    let end = r.collections()[0].total_extent - 320.;
    feed(r, end).0
}
fn bounded_work(r: &Runner<Counted>) {
    assert!(rows(r).len() <= WINDOW_SIZE + 1);
    assert!(r.last_instance_work().rows_keyed <= WINDOW_SIZE + 1);
}

#[test]
fn supplied_records_keying_and_offset_only_work_are_bounded_at_all_sizes() {
    for count in [1_000, 10_000, 100_000] {
        let mut r = boot(count);
        assert_eq!(rows(&r).len(), WINDOW_SIZE);
        assert_eq!(
            r.slot("cursor"),
            Some(&Value::str("")),
            "bootstrap must not dispatch"
        );
        at_tail(&mut r);
        r.act("start", vec![]).unwrap();
        r.advance(250.).unwrap();
        assert_eq!(fields(&r)[2], Value::Number(1.));
        bounded_work(&r);
        let stream_keys = r.last_instance_work().rows_keyed;
        feed(&mut r, 7.);
        bounded_work(&r);
        let shift_keys = r.last_instance_work().rows_keyed;
        let queries = r.data_ref().queries;
        let (_, events) = feed(&mut r, 3200.);
        assert_eq!(events, 0);
        assert_eq!(r.data_ref().queries, queries);
        assert_eq!(r.last_instance_work().rows_keyed, 0);
        println!("N={count}: supplied={}, stream keys={stream_keys}, shift keys={shift_keys}, offset queries=0, offset keys=0, mounted={}", rows(&r).len(), r.collections()[0].rows.len());
    }
}

#[test]
fn feedback_alone_traverses_tail_to_first_and_back_preserving_every_anchor() {
    for count in [1_000, 10_000, 100_000] {
        let mut r = boot(count);
        at_tail(&mut r);
        let mut shifts = [0; 2];
        let mut max_mounted = 0;
        for (direction, earlier) in [true, false].into_iter().enumerate() {
            while has(&r, earlier) {
                // Reach the overscan edge while preserving a seven-pixel
                // within-row offset. The later edge is short of end following.
                let top = if earlier {
                    7.
                } else {
                    r.collections()[0].total_extent - 320. - 25.
                };
                let index = (top / 32.).floor() as usize;
                let anchor = id(&rows(&r)[index]).to_owned();
                let offset = index as f64 * 32. - top;
                let queries = r.data_ref().queries;
                let (corrected, events) = feed(&mut r, top);
                assert_eq!(events, 1);
                assert_eq!(r.data_ref().queries, queries + 1);
                bounded_work(&r);
                let c = r.collections().remove(0);
                let new_index = rows(&r)
                    .iter()
                    .position(|row| id(row) == anchor)
                    .expect("surviving anchor");
                let row = c
                    .rows
                    .iter()
                    .find(|row| row.index == new_index)
                    .expect("anchor is mounted");
                assert!((row.top - corrected - offset).abs() <= 0.01,
                    "N={count}, earlier={earlier}, anchor={anchor}, offset={offset}, corrected={corrected}, row={row:?}");
                max_mounted = max_mounted.max(c.rows.len());
                // Accept the correction. A final clamped shift can still
                // geometrically contain the new endpoint: its newly armed edge
                // dispatches once, but hasEarlier/hasLater prevents a query.
                let queries = r.data_ref().queries;
                let terminal_edge = !has(&r, earlier)
                    && if earlier {
                        corrected - 320. < 32.
                    } else {
                        corrected + 640. > c.total_extent - 32.
                    };
                assert_eq!(feed(&mut r, corrected).1, usize::from(terminal_edge));
                assert_eq!(r.data_ref().queries, queries);
                assert_eq!(r.last_instance_work().rows_keyed, 0);
                shifts[direction] += 1;
                assert!(shifts[direction] <= count / 90 + 2);
            }
            if earlier {
                assert_eq!(id(&rows(&r)[0]), "m-000000");
                let queries = r.data_ref().queries;
                assert_eq!(feed(&mut r, 0.).1, 1);
                assert_eq!(feed(&mut r, 0.).1, 0);
                assert_eq!(r.data_ref().queries, queries);
                assert!(!has(&r, true));
            }
        }
        assert_eq!(id(rows(&r).last().unwrap()), format!("m-{:06}", count - 1));
        assert_ne!(r.slot("cursor"), Some(&Value::str("")));
        // Latest resets the resource cursor and requests the actual host's end.
        r.act("latest", vec![]).unwrap();
        assert_eq!(r.slot("cursor"), Some(&Value::str("")));
        let c = &r.collections()[0];
        assert_eq!(
            r.kernel()
                .node(c.view)
                .unwrap()
                .props
                .bool(PropId::ScrollFollowEnd),
            Some(true)
        );
        at_tail(&mut r);
        r.act("editDraft", vec![Value::str("exact bounded echo 🦀")])
            .unwrap();
        r.act("sendDraft", vec![]).unwrap();
        bounded_work(&r);
        let c = &r.collections()[0];
        assert_eq!(c.count, WINDOW_SIZE + 1);
        assert_eq!(id(rows(&r).last().unwrap()), "local-echo");
        assert!((c.correction.unwrap().scroll_top - (c.total_extent - 320.)).abs() <= 0.01);
        println!("N={count}: earlier shifts={}, later shifts={}, max mounted={max_mounted}; every anchor preserved; first/tail reached; latest follows echo", shifts[0], shifts[1]);
    }
}

#[test]
fn cursor_resolution_clamps_after_shrink_and_refuses_malformed_positions() {
    let query = |count: usize, cursor: &str| {
        MessagesStress.query(
            "history",
            &[
                Value::Number(count as f64),
                Value::Number(0.),
                Value::Number(8.),
                Value::str(""),
                Value::Number(0.),
                Value::Bool(false),
                Value::some(Value::str(cursor)),
            ],
        )
    };
    for cursor in [
        "",
        "999",
        "1000",
        "99999",
        "99999999999999999999999999999999999999",
    ] {
        let Value::Record(answer) = query(1000, cursor).unwrap() else {
            panic!("record")
        };
        let Value::List(rows) = &answer[0] else {
            panic!("rows")
        };
        assert_eq!(rows.len(), WINDOW_SIZE);
        assert_eq!(id(rows.last().unwrap()), "m-000999");
        assert_eq!(answer[8], Value::Bool(false));
    }
    for cursor in ["-1", "+1", " 1", "1 ", "1.0", "1e3", "oops", "١"] {
        assert!(
            matches!(query(1000, cursor), Err(DataError::BadArguments(_))),
            "{cursor}"
        );
    }
    let mut r = boot(100_000);
    feed(&mut r, 7.);
    r.act("chooseCount", vec![Value::Number(1000.)]).unwrap();
    assert_eq!(r.slot("cursor"), Some(&Value::str("")));
    assert_eq!(id(rows(&r).last().unwrap()), "m-000999");
    feed(&mut r, 7.);
    r.act("toggleWindowed", vec![]).unwrap();
    assert_eq!(r.slot("cursor"), Some(&Value::str("")));
    assert_eq!(r.collections()[0].count, 1000);
}
