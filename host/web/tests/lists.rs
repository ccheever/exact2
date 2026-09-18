//! LLP 1010: list lifetime, geometry, state and host batches.
use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value, Viewport};
use exact_web::Host;
use std::cell::Cell;
use std::rc::Rc;

const SOURCE: &str = r#"shape Item
  id: number
component App
  state reverse = false
  state skip = 0
  state jump = 0
  resource rows = rows(reverse, skip) as shape list<Item>
  action reorder writes reverse
    reverse = !reverse
  action trim(n: number) writes skip
    skip = n
  action go(n: number) writes jump
    jump = n
  view
    list item-height=24 height=240 width=390 testId="list" scrollTop=jump
      each item in rows key=item.id
        Cell(item=item)
component Cell
  props
    item: Item
  state count = 0
  action bump writes count
    count = count + 1
  view
    button press=bump testId=`row-${item.id}` height=24
      text `${item.id}:${count}` testId=`value-${item.id}`
"#;

#[derive(Clone)]
struct Data {
    count: usize,
    calls: Rc<Cell<usize>>,
}
impl DataSource for Data {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.set(self.calls.get() + 1);
        let skip = match args.get(1) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let mut rows: Vec<_> = (skip..self.count)
            .map(|id| Value::record(vec![Value::Number(id as f64)]))
            .collect();
        if args.first() == Some(&Value::Bool(true)) {
            rows.reverse();
        }
        Ok(Value::list(rows))
    }
}
fn data(count: usize) -> Data {
    Data {
        count,
        calls: Rc::new(Cell::new(0)),
    }
}
fn view(r: &Runner<Data>, name: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn text(r: &Runner<Data>, name: &str) -> String {
    r.kernel()
        .node(view(r, name))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

#[test]
fn scrolling_has_bounded_live_and_retained_nodes_without_data_calls() {
    let plan = contract::compile(SOURCE).unwrap();
    let mut middle_counts = Vec::new();
    for count in [1000, 25000] {
        let d = data(count);
        let calls = d.calls.clone();
        let mut r = Runner::boot(
            plan.clone(),
            d,
            Kernel::with_monospace(),
            Viewport::default(),
            "/",
        )
        .unwrap();
        assert!(r.kernel().live_count() < 20, "boot never expands N rows");
        let list = view(&r, "list");
        r.list_viewport(
            list,
            exact_runner::ListViewport {
                top: 2400.0,
                height: 240.0,
                width: 390.0,
                origin: 0.0,
                pins: [0, 0],
                rows: &[],
            },
        )
        .unwrap();
        middle_counts.push(r.kernel().live_count());
        for _ in 0..20 {
            // Visit every record of the 1,000-row case; long-list jumps
            // exercise arbitrary ranges without a scan of intervening rows.
            let step = if count == 1000 { 10 } else { 500 };
            for row in (0..count)
                .step_by(step)
                .chain((0..count).step_by(step).rev())
            {
                r.list_viewport(
                    list,
                    exact_runner::ListViewport {
                        top: (row * 24) as f64,
                        height: 240.0,
                        width: 390.0,
                        origin: 0.0,
                        pins: [0, 0],
                        rows: &[],
                    },
                )
                .unwrap();
                assert!(r.kernel().live_count() <= 92);
                assert!(
                    r.kernel().arena().slot_count() <= 182,
                    "retired kernel storage is reused"
                );
            }
        }
        assert_eq!(calls.get(), 1, "scrolling never re-enters data settlement");
        r.list_viewport(
            list,
            exact_runner::ListViewport {
                top: ((count - 10) * 24) as f64,
                height: 240.0,
                width: 390.0,
                origin: 0.0,
                pins: [0, 0],
                rows: &[],
            },
        )
        .unwrap();
        assert_eq!(
            text(&r, &format!("value-{}", count - 1)),
            format!("{}:0", count - 1)
        );
    }
    assert_eq!(middle_counts[0], middle_counts[1]);
}

#[test]
fn a_pinned_row_keeps_identity_and_state_then_releases_both() {
    let mut r = Runner::boot(
        contract::compile(SOURCE).unwrap(),
        data(1000),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    let first = view(&r, "row-0");
    r.dispatch(first, Event::Press).unwrap();
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 2400.0,
            height: 240.0,
            width: 390.0,
            origin: 0.0,
            pins: [first, first],
            rows: &[],
        },
    )
    .unwrap();
    assert_eq!(view(&r, "row-0"), first);
    assert_eq!(text(&r, "value-0"), "0:1");
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 2400.0,
            height: 240.0,
            width: 390.0,
            origin: 0.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    assert!(r.kernel().node(first).is_none());
    assert!(r.dispatch(first, Event::Press).is_err());
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 0.0,
            height: 240.0,
            width: 390.0,
            origin: 0.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    assert_ne!(view(&r, "row-0"), first);
    assert_eq!(text(&r, "value-0"), "0:0");
}

#[test]
fn reorder_and_deleted_anchor_preserve_the_reading_position() {
    let mut r = Runner::boot(
        contract::compile(SOURCE).unwrap(),
        data(1000),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 2407.0,
            height: 240.0,
            width: 390.0,
            origin: 0.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    let row = view(&r, "row-100");
    r.dispatch(row, Event::Press).unwrap();
    r.act("reorder", vec![]).unwrap();
    assert_eq!(view(&r, "row-100"), row);
    assert_eq!(text(&r, "value-100"), "100:1");
    let top = r
        .kernel()
        .node(list)
        .unwrap()
        .props
        .get(PropId::ScrollTop)
        .unwrap()
        .as_float()
        .unwrap();
    assert_eq!(top, 899.0 * 24.0 + 7.0);
    r.act("reorder", vec![]).unwrap();
    r.act("trim", vec![Value::Number(110.0)]).unwrap();
    assert!(r.kernel().node(row).is_none());
    assert_eq!(
        r.kernel()
            .node(list)
            .unwrap()
            .props
            .get(PropId::ScrollTop)
            .unwrap()
            .as_float(),
        Some(0.0)
    );
    assert_eq!(text(&r, "value-110"), "110:0");
}

#[test]
fn resize_scroll_commands_and_refused_geometry_are_coherent() {
    let mut r = Runner::boot(
        contract::compile(SOURCE).unwrap(),
        data(1000),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 0.0,
            height: 240.0,
            width: 390.0,
            origin: 12.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    let before = r.kernel().live_count();
    assert!(r
        .list_viewport(
            list,
            exact_runner::ListViewport {
                top: f64::NAN,
                height: 240.0,
                width: 390.0,
                origin: 0.0,
                pins: [0, 0],
                rows: &[]
            }
        )
        .is_err());
    assert!(!r.is_poisoned());
    assert_eq!(before, r.kernel().live_count());
    r.act("go", vec![Value::Number(4812.0)]).unwrap();
    assert_eq!(text(&r, "value-200"), "200:0");
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 4812.0,
            height: 480.0,
            width: 390.0,
            origin: 12.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    assert!(r.kernel().live_count() > before);
    r.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 0.0,
            height: 0.0,
            width: 390.0,
            origin: 0.0,
            pins: [0, 0],
            rows: &[],
        },
    )
    .unwrap();
    assert_eq!(
        r.kernel().live_count(),
        2,
        "hidden scrollport retains no unpinned rows"
    );
}

#[test]
fn web_batches_retire_rows_and_report_logical_accessibility_positions() {
    let (mut host, first) = Host::boot(
        &contract::compile(SOURCE).unwrap().encode(),
        data(25000),
        Viewport::default(),
        "/",
    )
    .unwrap();
    assert!(first.contains("data-itemheight"));
    assert!(first.contains("\"aria-setsize\":\"25000\""));
    let list = view(host.runner(), "list");
    let batch = host.list_viewport(
        list,
        exact_runner::ListViewport {
            top: 2400.0,
            height: 240.0,
            width: 390.0,
            origin: 0.0,
            pins: [0, 0],
            rows: &[],
        },
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(batch.contains("\"op\":\"destroy\""));
    assert!(batch.contains("\"aria-posinset\":\"101\""));
    assert!(first.contains("height:600000px"));
}

#[test]
fn list_shape_and_all_keys_are_checked_even_outside_the_window() {
    for (from, to, error) in [
        ("item-height=24", "item-height=0", "lower-list-height"),
        ("item-height=24", "item-height=jump", "lower-list-height"),
        (
            "list item-height=24",
            "scroll item-height=24",
            "lower-list-height",
        ),
    ] {
        assert_eq!(
            contract::compile(&SOURCE.replace(from, to)).unwrap_err().id,
            error
        );
    }
    let duplicate =
        contract::compile(&SOURCE.replace("key=item.id", "key=(item.id < 999 ? item.id : 0)"))
            .unwrap();
    let error = Runner::boot(
        duplicate,
        data(1000),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .err()
    .unwrap();
    assert!(format!("{error:?}").contains("DuplicateKey"));
}

fn wrapper(r: &Runner<Data>, item: usize) -> u32 {
    r.kernel()
        .node(view(r, &format!("row-{item}")))
        .unwrap()
        .parent
        .unwrap()
}

fn commanded_top(r: &Runner<Data>, list: u32) -> f64 {
    r.kernel()
        .node(list)
        .unwrap()
        .props
        .get(PropId::ScrollTop)
        .unwrap()
        .as_float()
        .unwrap()
}

#[test]
fn measured_rows_keep_the_anchor_and_state_through_growth_resize_and_reorder() {
    use exact_runner::ListViewport;
    let source = SOURCE.replace("item-height=24", "estimated-item-height=24");
    let d = data(25000);
    let calls = d.calls.clone();
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        d,
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    assert!(r.kernel().live_count() < 20);
    let list = view(&r, "list");
    let geometry = ListViewport {
        top: 2407.0,
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    r.list_viewport(list, geometry).unwrap();
    let row = view(&r, "row-100");
    r.dispatch(row, Event::Press).unwrap();
    let sizes = [(wrapper(&r, 95), 84.0), (wrapper(&r, 100), 48.0)];
    r.list_viewport(
        list,
        ListViewport {
            rows: &sizes,
            ..geometry
        },
    )
    .unwrap();
    assert_eq!(commanded_top(&r, list), 2467.0);
    assert_eq!(view(&r, "row-100"), row);
    assert_eq!(text(&r, "value-100"), "100:1");
    assert!(r.kernel().live_count() <= 95);
    assert_eq!(calls.get(), 1, "measurement never settles data");

    r.act("reorder", vec![]).unwrap();
    assert_eq!(commanded_top(&r, list), 24899.0 * 24.0 + 7.0);
    r.act("reorder", vec![]).unwrap();
    assert_eq!(commanded_top(&r, list), 2467.0);
    r.list_viewport(
        list,
        ListViewport {
            top: 2467.0,
            width: 200.0,
            ..geometry
        },
    )
    .unwrap();
    assert_eq!(
        commanded_top(&r, list),
        2407.0,
        "resize invalidates old-width measurements"
    );
    assert_eq!(view(&r, "row-100"), row);
    assert_eq!(text(&r, "value-100"), "100:1");
}

#[test]
fn measured_zero_height_rows_advance_the_window_and_bad_samples_are_atomic() {
    use exact_runner::ListViewport;
    let source = SOURCE.replace("item-height=24", "estimated-item-height=24");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        data(1000),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    let geometry = ListViewport {
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    r.list_viewport(list, geometry).unwrap();
    let sizes: Vec<_> = (0..10).map(|i| (wrapper(&r, i), 0.0)).collect();
    r.list_viewport(
        list,
        ListViewport {
            rows: &sizes,
            ..geometry
        },
    )
    .unwrap();
    assert!(r.kernel().find_by_test_id("row-0").is_empty());
    assert_eq!(text(&r, "value-10"), "10:0");
    assert_eq!(text(&r, "value-29"), "29:0");
    assert!(r.kernel().live_count() <= 62);
    let before = r.kernel().live_count();
    let invalid = [(wrapper(&r, 10), f64::NAN)];
    assert!(r
        .list_viewport(
            list,
            ListViewport {
                rows: &invalid,
                ..geometry
            }
        )
        .is_err());
    assert_eq!(before, r.kernel().live_count());
    assert!(!r.is_poisoned());
    assert!(contract::compile(
        &SOURCE.replace("item-height=24", "item-height=24 estimated-item-height=30")
    )
    .is_err());
}

#[test]
fn logical_text_copy_spans_unmounted_rows_without_changing_the_window() {
    use exact_runner::{ListTextPosition, ListViewport};
    let d = data(1000);
    let calls = d.calls.clone();
    let mut r = Runner::boot(
        contract::compile(SOURCE).unwrap(),
        d,
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    r.list_viewport(
        list,
        ListViewport {
            top: 12000.0,
            height: 240.0,
            width: 390.0,
            ..Default::default()
        },
    )
    .unwrap();
    r.dispatch(view(&r, "row-500"), Event::Press).unwrap();
    let count = r.kernel().live_count();
    let slots = r.kernel().arena().slot_count();
    let copy = r.list_text(list, None).unwrap();
    let expected = (0..1000)
        .map(|i| format!("{i}:{}", usize::from(i == 500)))
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_eq!(copy, expected);
    assert_eq!(count, r.kernel().live_count());
    assert_eq!(slots, r.kernel().arena().slot_count());
    assert_eq!(calls.get(), 1);
    assert!(r.kernel().find_by_test_id("row-0").is_empty());
    let a = ListTextPosition {
        key: "n:498",
        paragraph: 0,
        offset: 2,
    };
    let b = ListTextPosition {
        key: "n:502",
        paragraph: 0,
        offset: 3,
    };
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        "8:0\n\n499:0\n\n500:1\n\n501:0\n\n502"
    );
    assert_eq!(
        r.list_text(list, Some((b, a))).unwrap(),
        r.list_text(list, Some((a, b))).unwrap()
    );
    r.act("reorder", vec![]).unwrap();
    assert_eq!(r.list_index(list, "n:498"), Some(501));
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        ":0\n\n501:0\n\n500:1\n\n499:0\n\n49"
    );
    r.act("trim", vec![Value::Number(500.0)]).unwrap();
    assert!(r.list_text(list, Some((a, b))).is_err());
    assert!(!r.is_poisoned());
}

#[test]
fn logical_text_joins_inline_runs_skips_hidden_text_and_uses_utf16_positions() {
    use exact_runner::ListTextPosition;
    let source = SOURCE.replace(
        "      text `${item.id}:${count}` testId=`value-${item.id}`",
        r#"      text
        text "A🦊"
        text `${item.id}`
      view display="none"
        text "secret"
      text "tail""#,
    );
    let r = Runner::boot(
        contract::compile(&source).unwrap(),
        data(3),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    assert_eq!(
        r.list_text(list, None).unwrap(),
        "A🦊0\n\ntail\n\nA🦊1\n\ntail\n\nA🦊2\n\ntail"
    );
    let a = ListTextPosition {
        key: "n:0",
        paragraph: 0,
        offset: 1,
    };
    let b = ListTextPosition {
        key: "n:2",
        paragraph: 1,
        offset: 2,
    };
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        "🦊0\n\ntail\n\nA🦊1\n\ntail\n\nA🦊2\n\nta"
    );
}
