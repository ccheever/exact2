//! Opt-in is literal and the first implementation refuses ambiguous row flow.
fn source(list: &str, rows: &str) -> String {
    format!("component App\n  state yes = true\n  resource rows = rows() as shape list<number>\n  view\n    {list}\n{rows}\n")
}
const ROW: &str = "      each x in rows key=x\n        text `${x}`";
#[test]
fn literal_opt_in_and_ordinary_each_compile() {
    for list in [
        "list virtualized=true height=200",
        "list virtualized=true estimated-item-height=400 height=200",
        "list virtualized=false height=200",
        "list virtualized=false estimated-item-height=400 height=200",
        "list estimated-item-height=400 height=200",
        "scroll height=200",
        "column",
    ] {
        contract::compile(&source(list, ROW)).unwrap();
    }
    let nested = "      each x in rows key=x\n        column\n          when yes\n            text `${x}`\n          else\n            text \"off\"";
    contract::compile(&source("list virtualized=true height=200", nested)).unwrap();
}
#[test]
fn invalid_opt_in_shape_and_layout_are_rejected_with_stable_ids() {
    for (list, row, id) in [
        (
            "list virtualized=yes height=200",
            ROW,
            "lower-collection-opt-in",
        ),
        (
            "column virtualized=true height=200",
            ROW,
            "lower-collection-opt-in",
        ),
        (
            "list virtualized=1 height=200",
            ROW,
            "lower-collection-opt-in",
        ),
        (
            "list virtualized=true estimated-item-height=0 height=200",
            ROW,
            "lower-list-height",
        ),
        (
            "list virtualized=true estimated-item-height=-10 height=200",
            ROW,
            "lower-list-height",
        ),
        (
            "list virtualized=true estimated-item-height=yes height=200",
            ROW,
            "lower-list-height",
        ),
        (
            "list virtualized=true item-height=400 height=200",
            ROW,
            "lower-list-height",
        ),
        (
            "list virtualized=true estimated-item-height=400 item-height=400 height=200",
            ROW,
            "lower-list-height",
        ),
        ("column estimated-item-height=400", ROW, "lower-list-height"),
        (
            "list virtualized=true estimated-item-height=400",
            ROW,
            "lower-collection-unbounded",
        ),
        (
            "list virtualized=true estimated-item-height=400 height=200",
            "      text \"no each\"",
            "lower-collection-template",
        ),
        ("list virtualized=true", ROW, "lower-collection-unbounded"),
        (
            "list virtualized=true height=200",
            "      text \"no each\"",
            "lower-collection-template",
        ),
        (
            "list virtualized=true height=200 gap=8",
            ROW,
            "lower-collection-flow",
        ),
        (
            "list virtualized=true height=200 display=\"grid\"",
            ROW,
            "lower-collection-flow",
        ),
        (
            "list virtualized=true height=200",
            "      each x in rows key=x\n        text \"a\"\n        text \"b\"",
            "lower-collection-template",
        ),
        (
            "list virtualized=true height=200",
            "      each x in rows key=x\n        text \"a\" position=\"absolute\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true height=200",
            "      each x in rows key=x\n        text \"a\" margin-top=-10",
            "lower-collection-flow",
        ),
    ] {
        let error = contract::compile(&source(list, row))
            .unwrap_err()
            .to_string();
        assert!(error.contains(id), "{error}: expected {id}");
    }
}

use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, Event, Runner};
struct Rows {
    queries: usize,
}
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        Ok(Value::list(
            (0..25_000).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}
const INTERACTIVE: &str = r#"
component App
  state draft = ""
  state suffix = 0
  resource rows = rows() as shape list<number>
  action edit(value) writes draft
    draft = value
  action revise writes suffix
    suffix = suffix + 1
  view
    column
      input value=draft change=edit testId="echo"
      list virtualized=true height=320 testId="collection"
        each x in rows key=x
          Counter(id=x, suffix=suffix)
component Counter
  props
    id: number
    suffix: number
  state n = 0
  action increment writes n
    n = n + 1
  view
    button press=increment testId=`row-${id}`
      text `${n} ${suffix}` testId=`label-${id}`
"#;
fn feedback(r: &Runner<Rows>, top: f64) -> CollectionFeedback {
    let c = r.collections().pop().unwrap();
    CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        scroll_top: top,
        port_width: 640.0,
        port_height: 320.0,
        row_width: 640.0,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
}
#[test]
fn runner_scroll_has_no_resource_queries_and_row_handlers_use_owned_slots() {
    let mut r = Runner::boot(
        contract::compile(INTERACTIVE).unwrap(),
        Rows { queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.last_instance_work().rows_keyed, 25_000);
    assert_eq!(r.collections()[0].rows.len(), 16);
    let queries = r.data_ref().queries;
    let facts = feedback(&r, 32_000.0);
    r.collection_feedback_bytes(&facts.encode().unwrap())
        .unwrap();
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert!(r.kernel().find_by_test_id("row-0").is_empty());
    let key = r.kernel().find_by_test_id("row-1000")[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    assert_eq!(r.handlers_of(id), r.handlers()[&id]);
    assert!(!r.handlers_of(id).is_empty());
    r.dispatch(id, Event::Press).unwrap();
    let label = r.kernel().find_by_test_id("label-1000")[0];
    assert_eq!(
        r.kernel()
            .node_by_key(label)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("1 0")
    );
    r.act("edit", vec![Value::str("hello")]).unwrap();
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    r.act("revise", vec![]).unwrap();
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert_eq!(
        r.kernel()
            .node_by_key(label)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("1 1")
    );
    r.collection_feedback(feedback(&r, 0.0)).unwrap();
    assert!(r.handlers_of(id).is_empty());
    r.collection_feedback(feedback(&r, 32_000.0)).unwrap();
    for (view, events) in r.handlers() {
        assert_eq!(r.handlers_of(view), events);
    }
    let label = r.kernel().find_by_test_id("label-1000")[0];
    assert_eq!(
        r.kernel()
            .node_by_key(label)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("0 1")
    );
    assert_eq!(r.data_ref().queries, queries);
    let invalid = r.collection_feedback_bytes(&[0]);
    assert!(invalid.is_err());
    assert!(!r.is_poisoned());
}
#[test]
fn false_keeps_eager_semantics_and_no_collection_metadata() {
    let source = INTERACTIVE.replace("virtualized=true", "virtualized=false");
    let r = Runner::boot(
        contract::compile(&source).unwrap(),
        Rows { queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(r.collections().is_empty());
    assert!(!r.kernel().find_by_test_id("row-24999").is_empty());
}

#[test]
fn inherited_typography_changes_measurement_epochs_without_rekeying() {
    let source = r#"component App
  state size = 16
  resource rows = rows() as shape list<number>
  action revise writes size
    size = size + 1
  view
    column font-size=size
      list virtualized=true height=320
        each x in rows key=x
          text `${x}`
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Rows { queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.collection_feedback(feedback(&r, 640.0)).unwrap();
    let before = r.collections()[0].clone();
    r.act("revise", vec![]).unwrap();
    let after = &r.collections()[0];
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert!(after
        .rows
        .iter()
        .zip(&before.rows)
        .all(|(a, b)| a.root == b.root && a.epoch != b.epoch));
    let mut old = feedback(&r, 640.0);
    old.measurements = before
        .rows
        .iter()
        .map(|r| exact_runner::RowMeasurement {
            view: r.view,
            epoch: r.epoch,
            height: 500.0,
        })
        .collect();
    assert!(r.collection_feedback(old).unwrap().receipts.is_empty());
}

#[test]
fn tall_estimate_bounds_bootstrap_and_actual_measurements_replace_it() {
    let source = INTERACTIVE.replace(
        "virtualized=true height=320",
        "virtualized=true estimated-item-height=400 height=320",
    );
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        Rows { queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.collections()[0].rows.len(), 2);
    assert_eq!(r.collections()[0].total_extent, 10_000_000.0);
    let queries = r.data_ref().queries;
    let initial = feedback(&r, 0.0);
    r.collection_feedback_bytes(&initial.encode().unwrap())
        .unwrap();
    let row = r.collections()[0].rows[0].clone();
    let mut measured = feedback(&r, 0.0);
    measured.measurements.push(exact_runner::RowMeasurement {
        view: row.view,
        epoch: row.epoch,
        height: 800.0,
    });
    r.collection_feedback_bytes(&measured.encode().unwrap())
        .unwrap();
    assert_eq!(r.collections()[0].rows[0].height, 800.0);
    let distant = feedback(&r, 40_000.0);
    r.collection_feedback_bytes(&distant.encode().unwrap())
        .unwrap();
    assert!(r.collections()[0].rows.len() < 5);
    assert!(r.collections()[0].rows[0].index > 90);
    let top = feedback(&r, 0.0);
    r.collection_feedback_bytes(&top.encode().unwrap()).unwrap();
    assert_eq!(r.collections()[0].rows[0].index, 0);
    assert_eq!(r.collections()[0].rows[0].height, 800.0);
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
}

#[test]
fn row_height_hints_select_exactly_one_window_owner() {
    for opt in ["", "virtualized=false", "virtualized=true"] {
        for hint in ["item-height=400", "estimated-item-height=400"] {
            if opt == "virtualized=true" && hint == "item-height=400" {
                continue; // Rejected by the compiler cases above.
            }
            let source = source(
                &format!("list {opt} {hint} height=200 testId=\"rows\""),
                ROW,
            );
            let r = Runner::boot(
                contract::compile(&source).unwrap(),
                Rows { queries: 0 },
                Kernel::with_monospace(),
                Default::default(),
                "/",
            )
            .unwrap();
            let key = r.kernel().find_by_test_id("rows")[0];
            let id = r.kernel().node_by_key(key).unwrap().id;
            let shared = opt == "virtualized=true";
            assert_eq!(r.collections().len(), usize::from(shared), "{opt} {hint}");
            assert_eq!(r.list_status(id).is_some(), !shared, "{opt} {hint}");
        }
    }
}
