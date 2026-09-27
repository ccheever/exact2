//! LLP 1062 on the Apple host's batch: a paint property the engine is moving
//! crosses as a `present` op in the style dictionary's units, over its row;
//! when it arrives, `unpresent` hands the row back. An inheriting view shows
//! an animating `color` too, and the presenter's appearance re-targets a
//! `light-dark()` colour — the first report without motion, the next with.

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
  state on = false
  action toggle writes on
    on = not on
  view
    column testId="page" background-color="light-dark(#ffffff, #000000)" transition="background-color 1s linear"
      button press=toggle testId="toggle" background-color=(on ? "#0000ff00" : "#ff0000") color=(on ? "#ffffff" : "#000000") box-shadow=(on ? "0 4px 12px #00000080" : "none") transition="background-color 1s linear, color 1s linear, box-shadow 1s linear"
        text "Go" testId="label"
      text "Still" testId="still"
"##;

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The `present`/`unpresent` op for `id`'s `property`, if the batch has one.
fn paint(batch: &str, id: u32, property: &str) -> Option<String> {
    batch
        .split("{\"op\":")
        .find(|op| {
            op.contains(&format!("\"id\":{id},"))
                && op.contains(&format!("\"property\":\"{property}\""))
        })
        .map(str::to_owned)
}

#[test]
fn a_moving_colour_is_presented_over_its_row_and_handed_back() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let (toggle, label, still) = (
        view(&host, "toggle"),
        view(&host, "label"),
        view(&host, "still"),
    );
    assert_eq!(
        paint(&first, toggle, "background_color"),
        None,
        "boot shows rows"
    );
    let started = host.dispatch_at(toggle, Event::Press, 0.0);
    assert!(
        paint(&started, toggle, "background_color").is_some(),
        "{started}"
    );
    let mid = host.tick(500.0);
    // Red to transparent blue: red fading, straight channels 0–255.
    let bg = paint(&mid, toggle, "background_color").unwrap();
    assert!(bg.contains("\"x\":255,\"y\":0,\"w\":0,\"h\":127.5"), "{bg}");
    // The label inherits the colour; a sibling with none of its own does not.
    let ink = paint(&mid, label, "text_color").unwrap();
    assert!(
        ink.contains("\"x\":127.5,\"y\":127.5,\"w\":127.5,\"h\":255"),
        "{ink}"
    );
    assert_eq!(paint(&mid, still, "text_color"), None);
    // `none` to a shadow: geometry and colour from zero together.
    let shadow = paint(&mid, toggle, "shadow_geometry").unwrap();
    assert!(shadow.contains("\"x\":0,\"y\":2,\"w\":6"), "{shadow}");
    let done = host.tick(1000.0);
    for (id, property) in [
        (toggle, "background_color"),
        (label, "text_color"),
        (toggle, "shadow_color"),
    ] {
        let op = paint(&done, id, property).unwrap_or_default();
        assert!(op.starts_with("\"unpresent\""), "{property}: {done}");
    }
}

#[test]
fn the_appearance_retargets_light_dark_first_quietly_then_moving() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let page = view(&host, "page");
    // Boot guessed light; the first report corrects it without a transition.
    let first = host.set_scheme(true);
    assert!(!first.contains("\"motion\":true"), "{first}");
    assert!(
        paint(&first, page, "background_color").is_none_or(|op| op.starts_with("\"unpresent\""))
    );
    let back = host.set_scheme(false);
    assert!(back.contains("\"motion\":true"), "{back}");
    let mid = host.tick(250.0);
    let bg = paint(&mid, page, "background_color").unwrap();
    assert!(
        bg.contains("\"x\":63.75,\"y\":63.75,\"w\":63.75,\"h\":255"),
        "{bg}"
    );
    assert!(
        host.set_scheme(false).contains("\"ops\":[]"),
        "an unchanged report is nothing"
    );
}

/// LLP 1062 D9: a keyframe's `light-dark()` colour is the presenter's
/// appearance when it starts; the first report corrects boot's light guess
/// in place, and a later flip leaves a playing animation as Chrome does.
#[test]
fn a_light_dark_keyframe_follows_the_presenter_appearance() {
    let plan = contract::compile(
        "fn accent(): string = \"light-dark(#000000, #ffffff)\"\nkeyframes lit\n  from\n    color=accent()\n  to\n    color=\"light-dark(#ff0000, #0000ff)\"\ncomponent App\n  view\n    text \"lit\" testId=\"word\" animation=\"lit 1s linear both\"\n",
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let word = view(&host, "word");
    let dark = host.set_scheme(true);
    let ink = paint(&dark, word, "text_color").unwrap();
    assert!(
        ink.contains("\"x\":255,\"y\":255,\"w\":255,\"h\":255"),
        "{ink}"
    );
    let mid = host.tick(500.0);
    let ink = paint(&mid, word, "text_color").unwrap();
    assert!(
        ink.contains("\"x\":127.5,\"y\":127.5,\"w\":255,\"h\":255"),
        "{ink}"
    );
    let flipped = host.set_scheme(false);
    if let Some(ink) = paint(&flipped, word, "text_color") {
        assert!(
            ink.contains("\"x\":127.5,\"y\":127.5,\"w\":255,\"h\":255"),
            "{ink}"
        );
    }
}
