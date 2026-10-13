//! A box with padding or a border is sent the content box layout resolved
//! (`fieldContent`), a percentage of the containing block's width included,
//! which the presenter's content box, and so its pointer offsets, read: the
//! canvas drawn in it and its input share one source. A change to the
//! containing block alone sends it again.

use exact_apple::Host;
use exact_kernel::{HostCover, MonospaceMeasurer};
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
      box testId="safe" width=200 height=100 padding-left="env(safe-area-inset-left)"
      scroll testId="list" width=400 height=300 box-sizing="border-box" padding-left="5%"
        box height=600
"##;

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The `rect` of the last `fieldContent` op for `id` in the batch.
fn content(batch: &str, id: u32) -> Option<serde_json::Value> {
    field(batch, id, "rect")
}

/// A field of the last `fieldContent` op for `id` in the batch.
fn field(batch: &str, id: u32, name: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()?
        .iter()
        .rfind(|op| op["op"] == "fieldContent" && op["id"] == id)
        .map(|op| op[name].clone())
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
    assert_eq!(
        field(&first, padded, "padding"),
        Some(serde_json::json!([4, 0, 0, 20])),
        "the padding resolved, top, right, bottom, left, which insets read with the box"
    );
    assert_eq!(
        content(&first, plain),
        Some(serde_json::json!([10, 0, 200, 100])),
        "points too: the one source a sheet or a failed layout holds"
    );
    // A scroller UIKit insets under its bars (LLP 1075.003 §3.5) keeps its
    // CSS padding: the platform adds those insets itself
    let list = view(&host, "list");
    let covered = host.set_covers(&[(list, Some(HostCover::Inset([140.0, 34.0])))]);
    assert_eq!(content(&covered, list), None, "cover: {covered}");
    // A box whose content box is its border box (an `env()` inset of 0) sends none
    assert_eq!(content(&first, view(&host, "safe")), None, "env: {first}");
    let narrowed = host.dispatch_at(view(&host, "narrow"), Event::Press, 0.0);
    assert_eq!(
        content(&narrowed, padded),
        Some(serde_json::json!([15, 4, 185, 96]))
    );
    let fixed = host.dispatch_at(view(&host, "fixed"), Event::Press, 0.0);
    assert_eq!(
        content(&fixed, padded),
        Some(serde_json::json!([10, 4, 190, 96])),
        "a padding back in points"
    );
}

const BUTTONS: &str = r##"style Padded
  padding-left=40
style Plain
  height=34
component App
  state plain = false
  action clear
    plain = true
  view
    column width=400
      button press=clear testId="clear" height=20
        text "Clear"
      button "Go" testId="go" class=(plain ? Plain : Padded)
      button "Zero" testId="zero" padding-left=0
      button "Unsaid" testId="unsaid"
"##;

/// A native button's padding goes with which sides its author wrote: UIKit
/// keeps its own inset on an unsaid side (`null`), and a written `0` is
/// sent, so a sheet or a failed layout holds the choice with the values.
#[test]
fn a_native_buttons_box_says_which_paddings_its_author_wrote() {
    let plan = contract::compile(BUTTONS).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    let go = view(&host, "go");
    let null = serde_json::Value::Null;
    assert_eq!(
        field(&first, go, "padding"),
        Some(serde_json::json!([null, null, null, 40])),
        "{first}"
    );
    assert_eq!(
        field(&first, view(&host, "zero"), "padding"),
        Some(serde_json::json!([null, null, null, 0])),
        "a written 0 is sent: {first}"
    );
    assert_eq!(content(&first, view(&host, "unsaid")), None, "{first}");
    let cleared = host.dispatch_at(view(&host, "clear"), Event::Press, 0.0);
    assert_eq!(content(&cleared, go), Some(null), "{cleared}");
}
