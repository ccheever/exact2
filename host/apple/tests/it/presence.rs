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

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> Option<u32> {
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

/// The last `present` of `layout` for `id`: offset and scale, `[dx, dy, sx, sy]`.
fn layout(batch: &str, id: u32) -> Option<[f64; 4]> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"layout\",");
    let at = batch.rfind(&marker)?;
    let end = batch[at..].find('}')? + at;
    let mut values = [0.0; 4];
    for (value, name) in values.iter_mut().zip(["x", "y", "w", "h"]) {
        let field = format!("\"{name}\":");
        let from = batch[at..end].find(&field)? + at + field.len();
        *value = batch[from..end].split(',').next()?.parse().ok()?;
    }
    Some(values)
}

fn boot_source(source: &str) -> Host<NoData> {
    let plan = contract::compile(source).unwrap();
    Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap()
    .0
}

#[test]
fn a_box_that_grows_scales_from_its_old_size_while_it_moves() {
    let mut host = boot_source(
        "component App\n  state open = false\n  action toggle writes open\n    open = not open\n  view\n    column\n      button press=toggle testId=\"toggle\"\n        text \"Toggle\"\n      column testId=\"card\" layout-transition=\"400ms linear\"\n        text \"Title\"\n        when open\n          view height=120\n",
    );
    let (card, toggle) = (view(&host, "card").unwrap(), view(&host, "toggle").unwrap());
    let before = host.runner().kernel().node(card).unwrap().frame;
    let open = host.dispatch_at(toggle, Event::Press, 100.0);
    let after = host.runner().kernel().node(card).unwrap().frame;
    assert!(after.height > before.height + 100.0);
    // An accordion opens: it starts at its old size, not its new one.
    let [dx, dy, sx, sy] = layout(&open, card).expect("the card's size moves");
    assert_eq!((dx, dy, sx), (0.0, 0.0, 1.0));
    assert!(
        (sy - (before.height / after.height) as f64).abs() < 1e-6,
        "{sy}"
    );
    let mid = host.tick(300.0);
    let [.., half] = layout(&mid, card).unwrap();
    assert!(half > sy && half < 1.0, "{half}");
    let settled = host.tick(501.0);
    assert_eq!(layout(&settled, card), Some([0.0, 0.0, 1.0, 1.0]));
}

#[test]
fn a_node_that_gains_the_row_animates_its_first_move() {
    let mut host = boot_source(
        "component App\n  state on = false\n  action go writes on\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      when not on\n        view height=50\n      text \"B\" testId=\"b\" layout-transition=(on ? \"200ms linear\" : \"none\")\n",
    );
    let (b, go) = (view(&host, "b").unwrap(), view(&host, "go").unwrap());
    // One commit gains the row and moves the node: as a CSS transition
    // gained in the same style change, it runs.
    let moved = host.dispatch_at(go, Event::Press, 100.0);
    let [dx, dy, ..] = layout(&moved, b).expect("the first move slides");
    assert_eq!(dx, 0.0);
    assert!((dy - 50.0).abs() < 1e-3, "{dy}");
}

struct Keys;
impl DataSource for Keys {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        let short = matches!(args.first(), Some(Value::Bool(true)));
        let keys: &[&str] = if short { &["a", "c"] } else { &["a", "b", "c"] };
        Ok(Value::list(
            keys.iter()
                .map(|k| Value::record(vec![Value::str(*k)]))
                .collect(),
        ))
    }
}

#[test]
fn a_windowed_row_whose_item_left_exits_and_the_rows_after_it_slide() {
    let plan = contract::compile(
        "keyframes leave\n  to\n    opacity=0\n\nshape Item\n  id: string\n\ncomponent App\n  state short = false\n  action cut writes short\n    short = true\n  resource keys = keys(short) as shape list<Item>\n  view\n    column\n      button press=cut testId=\"cut\"\n        text \"Cut\"\n      list height=300 item-height=40 testId=\"list\"\n        each k in keys key=k.id\n          text k.id testId=`row-${k.id}` layout-transition=\"200ms linear\" exit-animation=\"leave 100ms linear both\"\n",
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        Keys,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let find = |host: &Host<Keys>, name: &str| {
        let k = host.runner().kernel();
        let key = *k.find_by_test_id(name).first()?;
        Some(k.node_by_key(key).unwrap()).map(|n| (n.id, n.parent.unwrap()))
    };
    let list = view(&host, "list").unwrap();
    let viewport = exact_apple::ListViewport {
        height: 300.0,
        width: 390.0,
        ..Default::default()
    };
    host.list_viewport(list, viewport);
    let (_, b_wrapper) = find(&host, "row-b").expect("mounted");
    let (c, _) = find(&host, "row-c").unwrap();
    let cut = view(&host, "cut").unwrap();
    let out = host.dispatch_at(cut, Event::Press, 100.0);
    assert!(find(&host, "row-b").is_none(), "its item left: {out}");
    // The row leaves where the window placed it, as its wrapper, with its
    // root's exit; the row after it slides up from where it was.
    assert!(op(&out, "exit", b_wrapper), "{out}");
    assert!(!op(&out, "destroy", b_wrapper), "{out}");
    let [_, dy, ..] = layout(&out, c).expect("the next row slides");
    assert!((dy - 40.0).abs() < 1e-3, "{dy}");
    let ended = host.tick(201.0);
    assert!(op(&ended, "destroy", b_wrapper), "{ended}");
}
