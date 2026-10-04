//! Opt-in is literal and the first implementation refuses ambiguous row flow.
fn source(list: &str, rows: &str) -> String {
    format!("component App\n  state yes = true\n  resource rows = rows() as shape list<number>\n  view\n    {list}\n{rows}\n")
}
const ROW: &str = "      each x in rows key=x\n        text `${x}`";

fn index_keys(list: &str) {
    struct SharedRows(Vec<Value>);
    impl DataSource for SharedRows {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            let indices: &[usize] = match args[0].as_number().unwrap() as usize {
                0 => &[0, 1, 2],
                1 => &[3, 0, 1, 2],
                2 => &[1, 2],
                3 => &[2, 1],
                _ => &[3, 2, 1, 4],
            };
            Ok(Value::list(
                indices.iter().map(|i| self.0[*i].clone()).collect(),
            ))
        }
    }
    let src = format!("component App\n  state step = 0\n  resource rows = rows(step) as shape list<string>\n  action next\n    step = step + 1\n  view\n    {list} testId=\"list\"\n      each item, i in rows key=i\n        text `${{i}}:${{item}}` testId=`row-${{i}}`\n");
    let mut r = Runner::boot(
        contract::compile(&src).unwrap(),
        SharedRows(["a", "b", "c", "x", "d"].map(Value::str).to_vec()),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for (step, expected) in [
        vec!["a", "b", "c"],
        vec!["x", "a", "b", "c"],
        vec!["b", "c"],
        vec!["c", "b"],
        vec!["x", "c", "b", "d"],
    ]
    .iter()
    .enumerate()
    {
        if step > 0 {
            r.act("next", vec![]).unwrap();
        }
        let view = r
            .kernel()
            .node_by_key(r.kernel().find_by_test_id("list")[0])
            .unwrap()
            .id;
        for (i, item) in expected.iter().enumerate() {
            let key = r.kernel().find_by_test_id(&format!("row-{i}"))[0];
            assert_eq!(
                r.kernel().node_by_key(key).unwrap().props.str(PropId::Text),
                Some(format!("{i}:{item}").as_str())
            );
            if list.starts_with("list") {
                assert_eq!(
                    r.list_index(view, &format!("n:{i}")),
                    Some(i),
                    "step {step}"
                );
            }
        }
    }
}

#[test]
fn plain_each_keys_can_read_the_index() {
    index_keys("column");
}

#[test]
fn virtualized_list_keys_follow_positions_when_shared_items_move() {
    index_keys("list virtualized=true estimated-item-height=20 height=200");
}
#[test]
fn literal_opt_in_and_ordinary_each_compile() {
    for list in [
        "list virtualized=true height=200",
        "list virtualized=true estimated-item-height=400 height=200",
        "list virtualized=false height=200",
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
        // The windowed list is deleted (LLP 1070 stage 1): its hints say how
        // to move to the collection.
        ("list item-height=400 height=200", ROW, "lower-list-height"),
        (
            "list estimated-item-height=400 height=200",
            ROW,
            "lower-list-virtualized",
        ),
        (
            "list virtualized=false estimated-item-height=400 height=200",
            ROW,
            "lower-list-virtualized",
        ),
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
  action edit(value)
    draft = value
  action revise
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
  action increment
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
        offset: top,
        port_cross: 640.0,
        port_main: 320.0,
        cross: 640.0,
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
  action revise
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
            size: 500.0,
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
        size: 800.0,
    });
    r.collection_feedback_bytes(&measured.encode().unwrap())
        .unwrap();
    assert_eq!(r.collections()[0].rows[0].size, 800.0);
    let distant = feedback(&r, 40_000.0);
    r.collection_feedback_bytes(&distant.encode().unwrap())
        .unwrap();
    assert!(r.collections()[0].rows.len() < 5);
    assert!(r.collections()[0].rows[0].index > 90);
    let top = feedback(&r, 0.0);
    r.collection_feedback_bytes(&top.encode().unwrap()).unwrap();
    assert_eq!(r.collections()[0].rows[0].index, 0);
    assert_eq!(r.collections()[0].rows[0].size, 800.0);
    assert_eq!(r.data_ref().queries, queries);
    assert_eq!(r.last_instance_work().rows_keyed, 0);
}

#[test]
fn duplicate_keys_index_and_copy_on_a_virtualized_list() {
    use exact_kernel::Kernel;
    use exact_plan::Value;
    use exact_runner::{DataError, DataSource, Runner};
    struct Data;
    impl DataSource for Data {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            let values = match args {
                [Value::Number(1.0)] => [1.0, 1.0],
                [Value::Number(2.0)] => [2.0, 3.0],
                _ => [1.0, 2.0],
            };
            Ok(Value::list(values.into_iter().map(Value::Number).collect()))
        }
    }
    let source = "component App\n  state version = 0\n  resource rows = rows(version) as shape list<number>\n  action change(next: number)\n    version = next\n  view\n    list virtualized=true estimated-item-height=20 height=100 overflow-x=\"hidden\" testId=\"list\"\n      each x in rows key=x\n        text `${x}`\n";
    let boot = |source: &str| {
        Runner::boot(
            contract::compile(source).unwrap(),
            Data,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
    };
    // Duplicates are told apart by order, from the first report on.
    assert!(boot(&source.replace("version = 0", "version = 1")).is_ok());
    let mut runner = boot(source).unwrap();
    let view = runner
        .kernel()
        .node_by_key(runner.kernel().find_by_test_id("list")[0])
        .unwrap()
        .id;
    runner.act("change", vec![Value::Number(1.0)]).unwrap();
    assert_eq!(runner.list_index(view, "n:1"), Some(0));
    assert_eq!(runner.list_index(view, "d1:n:1"), Some(1));
    assert_eq!(
        runner.list_text(view, None).unwrap().matches('1').count(),
        2
    );
    runner.act("change", vec![Value::Number(2.0)]).unwrap();
}
/// A transform, a relative offset and `z-index` paint a row elsewhere and
/// leave its flow, as in CSS: a dragged row lifts with them (files F14:
/// the runner refused `translate="0px 0px"` and the list drew no rows).
#[test]
fn a_row_lifted_by_translate_and_z_index_keeps_its_place_in_the_index() {
    let src = r#"component App
  state lifted = 1
  resource rows = rows() as shape list<number>
  action drop
    lifted = -1
  view
    list virtualized=true height=300 estimated-item-height=30
      each k in rows key=k
        row height=30 translate=(k == lifted ? "0px 10px" : "0px 0px") rotate=(k == lifted ? 2 : 0) top=(k == lifted ? 4 : 0) position="relative" z-index=(k == lifted ? 2 : 0) press=drop testId=`row-${k}`
          text `${k}`
"#;
    let mut r = Runner::boot(
        contract::compile(src).unwrap(),
        Rows { queries: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.collections()[0].rows.len(), 16);
    let lifted = r.kernel().find_by_test_id("row-1")[0];
    let id = r.kernel().node_by_key(lifted).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
    assert_eq!(r.collections()[0].rows.len(), 16);
}
