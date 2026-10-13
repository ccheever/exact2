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

/// The style rows the batch gives `test_id`, as it is made and restyled.
fn style(host: &Host<NoData>, ops: &[serde_json::Value], test_id: &str) -> serde_json::Value {
    let id = view(host, test_id);
    let mut rows = serde_json::Map::new();
    for op in ops
        .iter()
        .filter(|op| (op["op"] == "style" || op["op"] == "create") && op["id"] == id)
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

/// D10.2: a range and a determinate `progress` are framed as wide as what
/// holds them, by the plan's `width: 100%`: the presenter is sent that
/// frame and lays its slider or bar across it, widening nothing itself. The
/// row is the compiler's, so it is not among the written ones (LLP 1115.000
/// D2); an authored width is.
#[test]
fn a_range_and_a_determinate_progress_are_full_width() {
    let (host, ops) = boot(
        r#"component App
  view
    column width=400 testId="root"
      input type="range" testId="range"
      progress value=0.5 testId="bar"
      row testId="row"
        box width=100 height=10 flex-shrink=0
        input type="range" testId="row-range"
      input type="range" width=90 testId="narrow"
      row
        column
          input type="range" testId="fit-range"
        column
          progress value=0.5 testId="fit-bar"
"#,
    );
    let width = |id: &str| frame(&host, &ops, id)[2];
    assert_eq!(width("range"), 400.0);
    assert_eq!(width("bar"), 400.0);
    assert_eq!(frame(&host, &ops, "bar")[3], 16.0, "1em tall");
    assert_eq!(width("row-range"), 300.0, "what the row's box leaves");
    assert_eq!(width("narrow"), 90.0, "an authored width wins");
    assert_eq!(width("fit-range"), 129.0, "its own width");
    assert_eq!(width("fit-bar"), 160.0, "10em");
    let written = |id: &str| {
        let node = host.runner().kernel().node(view(&host, id)).unwrap();
        node.style.written.has(exact_kernel::StyleId::Width)
    };
    assert!(!written("range") && !written("bar"), "the compiler's row");
    assert!(written("narrow"), "the author's");
}

/// D10.3: an `hr` is one hairline with no margin, a point until the
/// presenter says its display's scale (`exact_set_display_scale`: UIKit's
/// `displayScale`, AppKit's `backingScaleFactor`). Telling it sends the
/// presenter the rule's new edge and the frames that move with it, in one
/// batch; the same scale again sends nothing.
#[test]
fn an_hr_is_one_device_pixel_of_the_scale_the_presenter_reports() {
    let (mut host, ops) = boot(
        r#"component App
  view
    column width=200 testId="root"
      box height=20 testId="above"
      hr testId="rule"
      box height=20 testId="below"
"#,
    );
    assert_eq!(frame(&host, &ops, "rule"), [0.0, 20.0, 200.0, 1.0]);
    assert_eq!(frame(&host, &ops, "below")[1], 21.0);
    let rule = style(&host, &ops, "rule");
    assert_eq!(rule["border_width_top"], 1.0, "{rule}");
    assert!(rule.get("margin_top").is_none(), "{rule}");
    // An iPhone's 3x display.
    let told: serde_json::Value = serde_json::from_str(&host.set_display_scale(3.0)).unwrap();
    let told = told["ops"].as_array().cloned().unwrap_or_default();
    let width = style(&host, &told, "rule")["border_width_top"]
        .as_f64()
        .expect("the rule is restyled");
    assert!((width - 1.0 / 3.0).abs() < 1e-4, "{width}");
    let [_, y, w, h] = frame(&host, &told, "rule");
    assert!(
        (y, w) == (20.0, 200.0) && (h - 1.0 / 3.0).abs() < 1e-4,
        "{h}"
    );
    let below = frame(&host, &told, "below")[1];
    assert!((below - (20.0 + 1.0 / 3.0)).abs() < 1e-4, "{below}");
    let again: serde_json::Value = serde_json::from_str(&host.set_display_scale(3.0)).unwrap();
    assert!(
        again["ops"].as_array().is_none_or(Vec::is_empty),
        "the same scale again: {again}"
    );
    assert!(host.set_display_scale(0.0).contains("display scale"));
}
