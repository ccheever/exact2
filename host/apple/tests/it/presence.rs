//! Exit animation and layout transition on Apple (LLP 1063): the leaving
//! view is kept and animated until its exit ends, then destroyed once; the
//! sibling that takes its place slides there as a `layout` offset.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view(host: &Host<NoData>, test_id: &str) -> Option<u32> {
    let k = host.runner().kernel();
    let key = *k.find_by_test_id(test_id).first()?;
    Some(k.node_by_key(key).unwrap().id)
}

/// The last `present` value of `property` for `id` in a batch, as (x, y).
fn presented(batch: &str, id: u32, property: &str) -> Option<(f64, f64)> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"{property}\",\"x\":");
    let at = batch.rfind(&marker)?;
    let mut rest = batch[at + marker.len()..].split([',', '}']);
    let x = rest.next()?.parse().ok()?;
    let y = rest.next()?.strip_prefix("\"y\":")?.parse().ok()?;
    Some((x, y))
}

fn op(batch: &str, op: &str, id: u32) -> bool {
    batch.contains(&format!("{{\"op\":\"{op}\",\"id\":{id}}}"))
}

fn boot() -> Host<NoData> {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/presence.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    // First seen: nothing moves at boot.
    assert!(!first.contains("\"property\":\"layout\""), "{first}");
    assert!(first.contains("\"motion\":false"), "{first}");
    host
}

#[test]
fn a_removed_view_leaves_with_its_exit_and_its_sibling_slides_into_its_place() {
    let mut host = boot();
    let (first, second, toggle) = (
        view(&host, "first").unwrap(),
        view(&host, "second").unwrap(),
        view(&host, "toggle").unwrap(),
    );
    let off = host.dispatch_at(toggle, Event::Press, 100.0);
    assert!(op(&off, "exit", first), "{off}");
    assert!(
        !op(&off, "destroy", first),
        "held until the exit ends: {off}"
    );
    assert!(off.contains("\"motion\":true"));
    assert!(view(&host, "first").is_none(), "gone from the tree at once");
    // `second` moved up by the height `first` took; it starts where it was.
    let (dx, dy) = presented(&off, second, "layout").expect("a layout offset");
    assert_eq!(dx, 0.0);
    assert!(dy > 0.0, "{dy}");
    // The exit ends at 300 ms, the slide at 420 ms (as f32 seconds, like
    // every row on the wire): settle waits for both.
    let settle = host.agent("{\"op\":\"settle\"}");
    let settle: f64 = settle[10..settle.len() - 1].parse().unwrap();
    assert!((settle - 420.0).abs() < 1e-3, "{settle}");

    let mid = host.tick(200.0);
    let (_, half) = presented(&mid, second, "layout").unwrap();
    assert!(half > 0.0 && half < dy, "{half} of {dy}");
    let (opacity, _) = presented(&mid, first, "opacity").expect("the leaving view animates");
    assert!(opacity > 0.0 && opacity < 1.0, "{opacity}");
    assert!(!op(&mid, "destroy", first));

    let ended = host.tick(301.0);
    assert!(op(&ended, "destroy", first), "{ended}");
    let settled = host.tick(421.0);
    assert_eq!(presented(&settled, second, "layout"), Some((0.0, 0.0)));
    assert!(settled.contains("\"motion\":false"), "{settled}");
    assert!(!op(&settled, "destroy", first), "destroyed once");

    // Back: a new view, first seen, so it does not exit or slide in; its
    // sibling slides down out of its way.
    let on = host.dispatch_at(toggle, Event::Press, 1000.0);
    let again = view(&host, "first").unwrap();
    assert_ne!(again, first);
    assert!(presented(&on, again, "layout").is_none());
    let (_, down) = presented(&on, second, "layout").unwrap();
    assert!((down + dy).abs() < 1e-3, "{down} vs {dy}");
}

#[test]
fn toggling_back_mid_exit_keeps_one_leaving_view_and_one_live_one() {
    let mut host = boot();
    let (first, toggle) = (
        view(&host, "first").unwrap(),
        view(&host, "toggle").unwrap(),
    );
    host.dispatch_at(toggle, Event::Press, 100.0);
    let on = host.dispatch_at(toggle, Event::Press, 150.0);
    let again = view(&host, "first").unwrap();
    assert!(op(&on, "create", again) || on.contains(&format!("\"id\":{again},")));
    assert!(!op(&on, "destroy", first), "still leaving: {on}");
    let ended = host.tick(301.0);
    assert!(op(&ended, "destroy", first));
    assert!(!op(&ended, "destroy", again));
}

#[test]
fn a_resize_takes_new_places_without_sliding() {
    let mut host = boot();
    let second = view(&host, "second").unwrap();
    let resized = host.resize(200.0, 844.0);
    assert!(
        presented(&resized, second, "layout").is_none_or(|o| o == (0.0, 0.0)),
        "{resized}"
    );
}
