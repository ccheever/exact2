//! A box whose padding the presenter cannot resolve from its style rows (a
//! percentage, of the containing block's width) is sent the content box
//! layout resolved (`fieldContent`), which the presenter's content box, and
//! so its pointer offsets, read: the canvas drawn in it and its input share
//! one source. A change to the containing block alone sends it again.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

const SRC: &str = r##"component App
  state narrow = false
  state fixed = false
  action shrink
    narrow = true
  action fixPadding
    fixed = true
  view
    column width=(narrow ? 300 : 400)
      button press=shrink testId="narrow" height=20
        text "Narrow"
      button press=fixPadding testId="fixed" height=20
        text "Fixed"
      box testId="padded" width=200 height=100 box-sizing="border-box" padding-left=(fixed ? "10px" : "5%") padding-top=4
      box testId="plain" width=200 height=100 padding-left=10
"##;

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The `rect` of the last `fieldContent` op for `id` in the batch.
fn content(batch: &str, id: u32) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()?
        .iter()
        .rfind(|op| op["op"] == "fieldContent" && op["id"] == id)
        .map(|op| op["rect"].clone())
}

#[test]
fn a_box_with_a_percentage_padding_is_sent_its_resolved_content_box() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    let (padded, plain) = (view(&host, "padded"), view(&host, "plain"));
    assert_eq!(
        content(&first, padded),
        Some(serde_json::json!([20, 4, 180, 96]))
    );
    assert_eq!(content(&first, plain), None, "the presenter reads points");
    let narrowed = host.dispatch_at(view(&host, "narrow"), Event::Press, 0.0);
    assert_eq!(
        content(&narrowed, padded),
        Some(serde_json::json!([15, 4, 185, 96]))
    );
    let fixed = host.dispatch_at(view(&host, "fixed"), Event::Press, 0.0);
    assert_eq!(
        content(&fixed, padded),
        Some(serde_json::Value::Null),
        "point padding clears the published box"
    );
}
