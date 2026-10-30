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

#[test]
fn a_resize_retires_running_layout_in_its_own_batch() {
    for width in ["100", "\"100%\""] {
        let mut host = boot_source(&format!(
            "component App\n  state moved = false\n  action move\n    moved = ! moved\n  view\n    column\n      button \"Move\" press=move testId=\"move\"\n      view height=(moved ? 80 : 20)\n      view testId=\"box\" width={width} height=(moved ? 100 : 50) -exact-layout-transition=\"1s linear\"\n"
        ));
        let box_id = view(&host, "box").unwrap();
        let move_id = view(&host, "move").unwrap();
        host.dispatch_at(move_id, Event::Press, 0.0);
        let mid = layout(&host.tick(500.0), box_id).unwrap();
        assert_ne!(mid[1], 0.0);
        assert_ne!(mid[3], 0.0);
        let before = host.runner().kernel().node(box_id).unwrap().frame;
        let resized = host.resize(300.0, 700.0);
        let after = host.runner().kernel().node(box_id).unwrap().frame;
        assert_eq!(before == after, width == "100");
        assert_eq!(
            layout(&resized, box_id),
            Some([0.0; 4]),
            "width={width}: the resize itself resets the presentation: {resized}"
        );
        assert!(resized.contains("\"motion\":false"), "{resized}");
        let next = host.tick(600.0);
        assert!(layout(&next, box_id).is_none_or(|v| v == [0.0; 4]));
        let moved = host.dispatch_at(move_id, Event::Press, 600.0);
        assert_ne!(layout(&moved, box_id).unwrap(), [0.0; 4]);
    }
}

/// The last `present` of `layout` for `id`: offset and size difference,
/// `[dx, dy, dw, dh]`.
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
fn a_box_that_grows_starts_its_surface_at_its_old_size() {
    let mut host = boot_source(
        "component App\n  state open = false\n  action toggle\n    open = ! open\n  view\n    column\n      button press=toggle testId=\"toggle\"\n        text \"Toggle\"\n      column testId=\"card\" -exact-layout-transition=\"400ms linear\"\n        text \"Title\"\n        if open\n          view height=120\n",
    );
    let (card, toggle) = (view(&host, "card").unwrap(), view(&host, "toggle").unwrap());
    let before = host.runner().kernel().node(card).unwrap().frame;
    let open = host.dispatch_at(toggle, Event::Press, 100.0);
    let after = host.runner().kernel().node(card).unwrap().frame;
    assert!(after.height > before.height + 100.0);
    // An accordion opens: it starts at its old size, not its new one.
    let [dx, dy, dw, dh] = layout(&open, card).expect("the card's size moves");
    assert_eq!((dx, dy, dw), (0.0, 0.0, 0.0));
    assert!(
        (dh - (before.height - after.height) as f64).abs() < 1e-3,
        "{dh}"
    );
    let mid = host.tick(300.0);
    let [.., half] = layout(&mid, card).unwrap();
    assert!(half > dh && half < 0.0, "{half}");
    let settled = host.tick(501.0);
    assert_eq!(layout(&settled, card), Some([0.0; 4]));
}

// GitHub #940: a clipping box closing to zero size shows the size it closes
// from, which a ratio of the laid-out size could not carry.

#[test]
fn a_clipping_box_closes_to_zero_width_from_the_width_it_had() {
    let shown = clip_shown(0, 0.0, "260ms linear", None, &[100.0, 230.0, 361.0]);
    assert_eq!(shown, [200.0, 100.0, 0.0]);
}

#[test]
fn a_clipping_box_closes_to_zero_height_from_the_height_it_had() {
    let shown = clip_shown(1, 0.0, "260ms linear", None, &[100.0, 230.0, 361.0]);
    assert_eq!(shown, [200.0, 100.0, 0.0]);
}

#[test]
fn a_clipping_box_closes_to_a_nonzero_width() {
    let shown = clip_shown(0, 20.0, "260ms linear", None, &[100.0, 230.0, 361.0]);
    assert_eq!(shown, [200.0, 110.0, 20.0]);
}

#[test]
fn a_clipping_box_without_motion_closes_at_once() {
    let shown = clip_shown(0, 0.0, "none", None, &[100.0, 230.0, 361.0]);
    assert_eq!(shown, [0.0; 3]);
}

#[test]
fn a_clipping_box_opens_from_zero_width() {
    let times = [100.0, 361.0, 400.0, 530.0, 661.0];
    let shown = clip_shown(0, 0.0, "260ms linear", Some(400.0), &times);
    assert_eq!(shown, [200.0, 0.0, 0.0, 100.0, 200.0]);
}

#[test]
fn a_clipping_box_reopened_while_it_closes_goes_back_from_where_it_is() {
    let times = [100.0, 230.0, 300.0, 1000.0];
    let shown = clip_shown(0, 0.0, "260ms linear", Some(230.0), &times);
    assert_eq!(shown[..2], [200.0, 100.0], "no snap: {shown:?}");
    assert!(shown[2] > 100.0 && shown[2] < 200.0, "{shown:?}");
    assert_eq!(shown[3], 200.0);
}

/// The size along `axis` (0 width, 1 height) a 200-point clipping box's
/// surface shows at each of `times`, as `Surface.swift` sizes it from the
/// batches: the box's last `frame` plus its last `layout` difference. The box
/// closes to `to` at 100 ms and opens again at `open`. Its 200-point child
/// and both ids stay, and the batch agrees with the engine at every sample.
fn clip_shown(
    axis: usize,
    to: f64,
    transition: &str,
    open: Option<f64>,
    times: &[f64],
) -> Vec<f64> {
    let [w, h] = [0, 1].map(|a| {
        if a == axis {
            format!("(closed ? {to} : 200)")
        } else {
            "200".into()
        }
    });
    let mut host = boot_source(&format!(
        "component App\n  state closed = false\n  action toggle\n    closed = ! closed\n  view\n    column\n      button \"Toggle\" press=toggle testId=\"toggle\"\n      view testId=\"clip\" width={w} height={h} overflow=\"hidden\" -exact-layout-transition=\"{transition}\"\n        view testId=\"child\" width=200 height=200 background-color=\"red\"\n"
    ));
    let (clip, child, toggle) = (
        view(&host, "clip").unwrap(),
        view(&host, "child").unwrap(),
        view(&host, "toggle").unwrap(),
    );
    let node = exact_kernel::motion_node(host.runner().kernel().node(clip).unwrap().key);
    let (mut laid, mut resize) = (200.0, 0.0);
    let mut shown = Vec::new();
    for &time in times {
        let batch = if time == 100.0 || Some(time) == open {
            host.dispatch_at(toggle, Event::Press, time)
        } else {
            host.tick(time)
        };
        let json: serde_json::Value = serde_json::from_str(&batch).unwrap();
        assert!(json["error"].is_null(), "{batch}");
        for op in json["ops"].as_array().unwrap() {
            if op["op"] == "frame" && op["id"] == clip {
                laid = op[["w", "h"][axis]].as_f64().unwrap();
            }
        }
        if let Some(values) = layout(&batch, clip) {
            resize = values[2 + axis];
        }
        let size = |id| {
            let f = host.runner().kernel().node(id).unwrap().frame;
            [f.width, f.height][axis] as f64
        };
        assert_eq!(laid, size(clip), "at {time}: {batch}");
        assert_eq!(size(child), 200.0, "the child keeps its size");
        assert_eq!(view(&host, "clip"), Some(clip), "retained");
        assert_eq!(view(&host, "child"), Some(child), "retained");
        let at = f64::max(0.0, laid + resize);
        if let Some(engine) = host.engine().value(node, exact_motion::Property::Layout) {
            let engine = [engine.z, engine.w][axis].max(0.0);
            assert!(
                (at - engine).abs() < 1e-3,
                "at {time}: {at} vs the engine's {engine}: {batch}"
            );
        }
        shown.push((at * 1e3).round() / 1e3);
    }
    shown
}

#[test]
fn a_node_that_gains_the_row_animates_its_first_move() {
    let mut host = boot_source(
        "component App\n  state on = false\n  action go\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      if ! on\n        view height=50\n      text \"B\" testId=\"b\" -exact-layout-transition=(on ? \"200ms linear\" : \"none\")\n",
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
                .map(|k| Value::record(vec![Value::str(k)]))
                .collect(),
        ))
    }
}

#[test]
fn a_virtualized_row_whose_item_left_exits_and_the_rows_after_it_slide() {
    let plan = contract::compile(
        "keyframes leave\n  to opacity=0\n\nshape Item\n  id: string\n\ncomponent App\n  state short = false\n  action cut\n    short = true\n  resource keys = keys(short) as shape Item[]\n  view\n    column\n      button press=cut testId=\"cut\"\n        text \"Cut\"\n      list virtualized=true height=300 estimated-item-height=40 overflow-x=\"hidden\" testId=\"list\"\n        for k in keys key=k.id\n          text k.id testId=`row-${k.id}` height=40 -exact-layout-transition=\"200ms linear\" -exact-exit-animation=\"leave 100ms linear both\"\n",
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
    // A row is its content's height, 40, as the host measures it. The first
    // report sets the list's width, which retires every measurement epoch
    // published before it; the rows are measured in the next.
    let report = |host: &mut Host<Keys>| {
        let snapshot = host.runner().collections().remove(0);
        let feedback = exact_runner::CollectionFeedback {
            view: snapshot.view,
            revision: snapshot.revision,
            scroll_sequence: snapshot.scroll_sequence + 1,
            offset: 0.0,
            port_cross: 390.0,
            port_main: 300.0,
            cross: 390.0,
            padding: [0.0; 2],
            measurements: snapshot
                .rows
                .iter()
                .map(|r| exact_runner::RowMeasurement {
                    view: r.view,
                    epoch: r.epoch,
                    size: 40.0,
                })
                .collect(),
            focus_view: None,
            interaction_view: None,
        };
        host.collection_feedback(&feedback.encode().unwrap(), 0.0);
    };
    report(&mut host);
    report(&mut host);
    let rows = host.runner().collections().remove(0).rows;
    assert!(
        rows.iter().all(|r| r.measured && r.size == 40.0),
        "{rows:?}"
    );
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

#[test]
fn removing_a_layout_transition_presents_identity_mid_move() {
    let mut host = boot_source(
        "component App\n  state moved = false\n  state animate = true\n  action move\n    moved = true\n  action stop\n    animate = false\n  view\n    column\n      button \"Move\" press=move testId=\"move\"\n      button \"Stop\" press=stop testId=\"stop\"\n      view height=(moved ? 80 : 20)\n      view testId=\"box\" height=(moved ? 100 : 50) -exact-layout-transition=(animate ? \"1s linear\" : \"none\")\n",
    );
    let box_id = view(&host, "box").unwrap();
    host.dispatch_at(view(&host, "move").unwrap(), Event::Press, 0.0);
    let mid = layout(&host.tick(500.0), box_id).unwrap();
    assert_ne!(mid[1], 0.0);
    assert_ne!(mid[3], 0.0);
    let stopped = host.dispatch_at(view(&host, "stop").unwrap(), Event::Press, 500.0);
    assert_eq!(layout(&stopped, box_id), Some([0.0; 4]), "{stopped}");
    assert!(layout(&host.tick(1000.0), box_id).is_none());
}

#[test]
fn hiding_an_ancestor_retires_a_descendants_layout_presentation() {
    let mut host = boot_source(
        "component App\n  state moved = false\n  state hidden = false\n  action move\n    moved = true\n  action toggle\n    hidden = ! hidden\n  view\n    column\n      button \"Move\" press=move testId=\"move\"\n      button \"Hide\" press=toggle testId=\"toggle\"\n      column display=(hidden ? \"none\" : \"flex\")\n        view height=(moved ? 80 : 20)\n        view testId=\"box\" height=(moved ? 100 : 50) -exact-layout-transition=\"1s linear\"\n",
    );
    let box_id = view(&host, "box").unwrap();
    host.dispatch_at(view(&host, "move").unwrap(), Event::Press, 0.0);
    assert_ne!(layout(&host.tick(500.0), box_id).unwrap(), [0.0; 4]);
    let hidden = host.dispatch_at(view(&host, "toggle").unwrap(), Event::Press, 500.0);
    assert_eq!(layout(&hidden, box_id), Some([0.0; 4]), "{hidden}");
    let shown = host.dispatch_at(view(&host, "toggle").unwrap(), Event::Press, 600.0);
    assert!(
        layout(&shown, box_id).is_none_or(|v| v == [0.0; 4]),
        "{shown}"
    );
    assert!(shown.contains("\"motion\":false"), "{shown}");
}

#[test]
fn a_layout_springs_size_overshoot_is_clamped_to_zero() {
    let mut host = boot_source(
        "component App\n  state small = false\n  action shrink\n    small = true\n  view\n    column\n      button \"Shrink\" press=shrink testId=\"shrink\"\n      view testId=\"box\" width=(small ? 10 : 200) height=(small ? 10 : 200) -exact-layout-transition=\"-exact-spring(100, 1, 1)\"\n",
    );
    let box_id = view(&host, "box").unwrap();
    host.dispatch_at(view(&host, "shrink").unwrap(), Event::Press, 0.0);
    let mid = host.tick(300.0);
    let [_, _, dw, dh] = layout(&mid, box_id).unwrap();
    assert_eq!((dw, dh), (-10.0, -10.0), "shown at zero, not below: {mid}");
}

/// Stage 14 B: a box going into the top layer (here its author's sticky
/// dropped, so its popover lifts it) moves into the viewport's block, a
/// change of space, not of place: no layout motion across it, as on the
/// Mac's painted popover, and none back.
#[test]
fn a_box_crossing_into_its_block_does_not_move() {
    let mut host = boot_source(
        "component App\n  state inflow = true\n  action lift\n    inflow = ! inflow\n  view\n    column\n      button \"Lift\" press=lift testId=\"lift\"\n      view height=100\n      view testId=\"box\" popover=\"manual\" position=(inflow ? \"sticky\" : \"absolute\") height=50 -exact-layout-transition=\"1s linear\"\n",
    );
    let box_id = view(&host, "box").unwrap();
    let kernel = |host: &Host<NoData>| host.runner().kernel().host_block(box_id).is_some();
    assert!(!kernel(&host), "sticky: in flow");
    for at in [0.0, 2000.0] {
        let lifted = host.dispatch_at(view(&host, "lift").unwrap(), Event::Press, at);
        assert_eq!(kernel(&host), at == 0.0, "lifted, then sticky again");
        let mid = host.tick(at + 500.0);
        for batch in [&lifted, &mid] {
            assert!(
                layout(batch, box_id).is_none_or(|v| v == [0.0; 4]),
                "{batch}"
            );
        }
    }
    assert!(!kernel(&host), "and back");
}

/// A transition declared on a box already in its block (an open popover)
/// plays: it crossed nothing, so its old box is where it moves from.
#[test]
fn a_transition_added_inside_a_block_plays() {
    let mut host = boot_source(
        "component App\n  state go = false\n  action start\n    go = true\n  view\n    column\n      button \"Go\" press=start testId=\"go\"\n      view testId=\"box\" popover=\"manual\" position=\"absolute\" height=50 width=(go ? 200 : 100) -exact-layout-transition=(go ? \"1s linear\" : \"none\")\n",
    );
    let box_id = view(&host, "box").unwrap();
    assert!(host.runner().kernel().host_block(box_id).is_some());
    host.dispatch_at(view(&host, "go").unwrap(), Event::Press, 0.0);
    let mid = layout(&host.tick(500.0), box_id).expect("it moves");
    assert_ne!(mid[2], 0.0, "its width grows over the second: {mid:?}");
}
