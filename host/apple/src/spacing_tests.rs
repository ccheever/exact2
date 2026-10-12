//! LLP 1115.000 D10 on the Apple host: what the Swift presenter is sent for
//! a control's spacing and size is the plan's, the same on every host. A
//! native control's alignment inset is inside its presentation and is no
//! frame of these.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::DataError;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(app: &str) -> (Host<NoData>, Vec<serde_json::Value>) {
    let plan = contract::compile(app)
        .unwrap_or_else(|e| panic!("{e}"))
        .encode();
    let (host, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        400.0,
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
    (host, v["ops"].as_array().cloned().unwrap_or_default())
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The frame the batch gives `test_id`, in its parent: x, y, width, height.
fn frame(host: &Host<NoData>, ops: &[serde_json::Value], test_id: &str) -> [f64; 4] {
    let id = view(host, test_id);
    let op = ops
        .iter()
        .rev()
        .find(|op| op["op"] == "frame" && op["id"] == id)
        .unwrap_or_else(|| panic!("no frame for `{test_id}`"));
    ["x", "y", "w", "h"].map(|k| op[k].as_f64().unwrap())
}

/// The style rows the batch gives `test_id`.
fn style(host: &Host<NoData>, ops: &[serde_json::Value], test_id: &str) -> serde_json::Value {
    let id = view(host, test_id);
    let mut rows = serde_json::Map::new();
    for op in ops
        .iter()
        .filter(|op| op["op"] == "style" && op["id"] == id)
    {
        if let Some(style) = op["style"].as_object() {
            rows.extend(style.clone());
        }
    }
    serde_json::Value::Object(rows)
}

/// D10.1: a checkbox, a radio and a range are framed where their row puts
/// them, and the presenter is sent no margin row for them (Chrome's UA
/// margins put the checkbox at 4, 3).
#[test]
fn a_checkbox_a_radio_and_a_range_have_no_margin_of_their_own() {
    let (host, ops) = boot(
        r#"component App
  view
    row width=400 testId="row"
      input type="checkbox" testId="check"
      input type="radio" testId="radio"
      input type="range" width=90 testId="range"
      box width=10 height=10 testId="after"
"#,
    );
    let [check, radio, range, after] =
        ["check", "radio", "range", "after"].map(|id| frame(&host, &ops, id));
    assert_eq!((check[0], check[1]), (0.0, 0.0), "the checkbox");
    assert_eq!(
        (radio[0], radio[1]),
        (check[2], 0.0),
        "the radio touches it"
    );
    assert_eq!((range[0], range[1]), (check[2] + radio[2], 0.0));
    assert_eq!(after[0], check[2] + radio[2] + 90.0, "and what follows");
    for id in ["check", "radio", "range"] {
        let rows = style(&host, &ops, id);
        let margins: Vec<&String> = rows
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with("margin"))
            .collect();
        assert!(margins.is_empty(), "{id}: {margins:?}");
    }
}
