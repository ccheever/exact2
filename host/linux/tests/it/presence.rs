//! Layout transition on Linux, and its refusal of exit animation (LLP 1063):
//! a sibling that takes a removed node's place is painted from where it was;
//! the removed node leaves at once and the journal says why.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
use exact_runner::{DataError, DataSource, Event};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn boot(source: &str) -> Host<NoData> {
    let plan = contract::compile(source).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0
}

fn view(h: &Host<NoData>, name: &str) -> u32 {
    let k = h.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

#[test]
fn a_sibling_slides_into_a_removed_nodes_place_which_leaves_at_once() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/presence.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let second = view(&h, "second");
    assert_eq!(h.presented(second).layout, [0.0, 0.0, 1.0, 1.0]);
    assert!(!h.motion());
    h.dispatch_at(view(&h, "toggle"), Event::Press, 100.);
    assert!(h.kernel().find_by_test_id("first").is_empty());
    let [_, from, ..] = h.presented(second).layout;
    assert!(from > 0.0, "painted where it was: {from}");
    h.tick(200.);
    let [_, mid, ..] = h.presented(second).layout;
    assert!(mid > 0.0 && mid < from, "{mid} of {from}");
    h.tick(421.);
    assert_eq!(h.presented(second).layout, [0.0, 0.0, 1.0, 1.0]);
    assert!(!h.motion());
    let logs = h.agent("{\"op\":\"logs\"}");
    assert!(
        logs.contains("-exact-exit-animation: refused on Linux"),
        "{logs}"
    );
}

#[test]
fn a_growing_box_starts_from_its_old_size_and_a_gained_row_animates_its_first_move() {
    let mut h = boot(
        "component App\n  state on = false\n  action go\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      when not on\n        view height=50\n      column testId=\"card\" -exact-layout-transition=(on ? \"200ms linear\" : \"none\")\n        text \"Title\"\n        when on\n          view height=150\n",
    );
    let card = view(&h, "card");
    let before = h.kernel().node(card).unwrap().frame.height;
    // One commit gives the card the row, moves it up and grows it.
    h.dispatch_at(view(&h, "go"), Event::Press, 100.);
    let after = h.kernel().node(card).unwrap().frame.height;
    let [dx, dy, sx, sy] = h.presented(card).layout;
    assert_eq!((dx, sx), (0.0, 1.0));
    assert!((dy - 50.0).abs() < 1e-3, "{dy}");
    assert!((sy - before / after).abs() < 1e-6, "{sy}");
    h.tick(301.);
    assert_eq!(h.presented(card).layout, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn a_resize_retires_running_layout_before_the_next_tick() {
    for width in ["100", "\"100%\""] {
        let mut h = boot(&format!(
            "component App\n  state moved = false\n  action move\n    moved = not moved\n  view\n    column\n      button \"Move\" press=move testId=\"move\"\n      view height=(moved ? 80 : 20)\n      view testId=\"box\" width={width} height=(moved ? 100 : 50) -exact-layout-transition=\"1s linear\"\n"
        ));
        let box_id = view(&h, "box");
        let move_id = view(&h, "move");
        h.dispatch_at(move_id, Event::Press, 0.0);
        h.tick(500.0);
        let mid = h.presented(box_id).layout;
        assert_ne!(mid[1], 0.0);
        assert_ne!(mid[3], 1.0);
        let before = h.kernel().node(box_id).unwrap().frame;
        assert!(h.resize(300.0, 700.0).is_none());
        let after = h.kernel().node(box_id).unwrap().frame;
        assert_eq!(before == after, width == "100");
        assert_eq!(
            h.presented(box_id).layout,
            [0.0, 0.0, 1.0, 1.0],
            "width={width}: the resize itself resets the presentation"
        );
        assert!(!h.motion());
        h.tick(600.0);
        assert_eq!(h.presented(box_id).layout, [0.0, 0.0, 1.0, 1.0]);
        h.dispatch_at(move_id, Event::Press, 600.0);
        assert_ne!(h.presented(box_id).layout, [0.0, 0.0, 1.0, 1.0]);
    }
}

/// Whether the pixel at `(x, y)` is mostly `channel` (0 red, 1 green, 2 blue).
fn is(frame: &tiny_skia::Pixmap, x: f32, y: f32, channel: usize) -> bool {
    let c = frame.pixel(x as u32, y as u32).unwrap().demultiply();
    let rgb = [c.red(), c.green(), c.blue()];
    rgb[channel] > 200 && (0..3).all(|i| i == channel || rgb[i] < 60)
}

#[test]
fn a_growing_box_reveals_its_content_at_its_final_size_and_never_scales_it() {
    use exact_linux::{presenter::PainterChoice, Presenter};
    for clips in [true, false] {
        let overflow = if clips { " overflow=\"hidden\"" } else { "" };
        let source = format!(
            "component App\n  state open = false\n  action toggle\n    open = not open\n  view\n    column\n      button press=toggle testId=\"toggle\"\n        text \"Toggle\"\n      column testId=\"card\" width=200 -exact-layout-transition=\"1000ms linear\" background-color=\"#0000ff\"{overflow}\n        view testId=\"title\" height=20 width=100 background-color=\"#ff0000\"\n        when open\n          view height=180 width=100 background-color=\"#00ff00\"\n"
        );
        let plan = contract::compile(&source).unwrap().encode();
        let (mut p, _) = Presenter::boot_with(
            &plan,
            NoData,
            (400., 600.),
            1.,
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            PainterChoice::Cpu,
        )
        .unwrap();
        let id = |p: &Presenter<NoData>, name: &str| {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
        };
        let card = id(&p, "card");
        p.tap(id(&p, "toggle")).unwrap();
        p.tick(500.);
        let state: serde_json::Value =
            serde_json::from_str(&exact_linux::agent::handle(&mut p, r#"{"op":"state"}"#)).unwrap();
        let observed = state["presence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == card)
            .unwrap();
        assert_eq!(observed["h"], 110.0, "surface, not laid-out height");
        assert_eq!(observed["opacity"], 1.0);
        assert_eq!(observed["exiting"], false);
        let (x, y, ..) = p.rect_of(card).unwrap();
        let frame = p.frame();
        // Half way from 20 to 200 high: the surface is 110 high; the title
        // keeps all 20 of its rows, and the new content keeps its own.
        assert!(is(&frame, x + 50., y + 19., 0), "the title is not squashed");
        assert!(is(&frame, x + 150., y + 100., 2), "the surface has grown");
        assert!(!is(&frame, x + 150., y + 130., 2), "and no further");
        assert!(is(&frame, x + 50., y + 100., 1), "revealed content");
        // A clipping box reveals the rest as it grows; one that does not clip
        // shows it at once, as CSS paints overflow.
        assert_eq!(is(&frame, x + 50., y + 150., 1), !clips, "clips: {clips}");
    }
}
