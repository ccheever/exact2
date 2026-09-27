//! Keyframe animations on Apple (LLP 1057 D4): the host's engine samples the
//! `animation` row under its clock and the presenter paints the values, as
//! it does a transition. An endless one keeps frames coming; settling never
//! waits for it.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The last `present` value of `property` for `id` in a batch.
fn presented(batch: &str, id: u32, property: &str) -> Option<f64> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"{property}\",\"x\":");
    let at = batch.rfind(&marker)?;
    batch[at + marker.len()..]
        .split([',', '}'])
        .next()?
        .parse()
        .ok()
}

fn close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("presented");
    assert!((actual - expected).abs() < 1e-5, "{actual} vs {expected}");
}

#[test]
fn an_animation_is_presented_from_boot_and_stops_when_its_row_does() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/keyframes.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let (mark, added, toggle) = (
        view(&host, "mark"),
        view(&host, "added"),
        view(&host, "toggle"),
    );
    assert!(first.contains("\"motion\":true"), "{first}");
    close(presented(&first, mark, "opacity"), 0.4);
    close(presented(&first, mark, "scale"), 0.9);
    close(presented(&first, added, "opacity"), 0.0);
    close(presented(&first, added, "translate"), 0.0);
    // The finite one settles at 300 ms; the endless one is not waited for.
    assert_eq!(host.agent("{\"op\":\"settle\"}"), "{\"settle\":300}");

    // 400 ms into 1.6 s: a quarter through, halfway through the first
    // interval, where ease-in-out is exactly one half.
    let tick = host.tick(400.0);
    close(presented(&tick, mark, "opacity"), 0.7);
    // `both` holds the last keyframe: the implicit `to`, the row's own 1.
    close(presented(&tick, added, "opacity"), 1.0);
    assert_eq!(host.agent("{\"op\":\"settle\"}"), "{\"settle\":null}");
    assert!(tick.contains("\"motion\":true"));

    // `none`: the row's own opacity again, and the frames stop.
    let off = host.dispatch_at(toggle, Event::Press, 500.0);
    close(presented(&off, mark, "opacity"), 1.0);
    close(presented(&off, mark, "scale"), 1.0);
    assert!(off.contains("\"motion\":false"), "{off}");
    // Named again, it starts again from its first keyframe.
    let on = host.dispatch_at(toggle, Event::Press, 600.0);
    close(presented(&on, mark, "opacity"), 0.4);
    assert!(
        presented(&on, added, "opacity").is_none(),
        "untouched: {on}"
    );
}

/// LLP 1061 D4: the batch says whether what moves changes place or size.
/// A breathing fade keeps frames coming but not at the panel's full rate; a
/// slide asks for it.
#[test]
fn a_fade_is_motion_and_a_slide_is_spatial() {
    let src = "keyframes breathe\n  from\n    opacity=0.6\n  50%\n    opacity=1\n  to\n    opacity=0.6\ncomponent A\n  state sliding = false\n  action slide writes sliding\n    sliding = not sliding\n  view\n    column\n      button press=slide testId=\"slide\"\n        text \"Slide\"\n      text \"Breathe\" animation=\"breathe 4.2s ease-in-out infinite\"\n      text \"Card\" transition=\"translate 300ms ease\" translate=(sliding ? \"0px 40px\" : \"0px 0px\")\n";
    let plan = contract::compile(src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(
        first.contains("\"motion\":true") && !first.contains("\"spatial\""),
        "{first}"
    );
    let slid = host.dispatch_at(view(&host, "slide"), Event::Press, 100.0);
    assert!(slid.contains("\"spatial\":true"), "{slid}");
    let settled = host.tick(1000.0);
    assert!(
        settled.contains("\"motion\":true") && !settled.contains("\"spatial\""),
        "{settled}"
    );
}
