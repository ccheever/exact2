//! Layout transition on Linux, and its refusal of exit animation (LLP 1063):
//! a sibling that takes a removed node's place is painted from where it was;
//! the removed node leaves at once and the journal says why.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
use exact_runner::Event;

fn boot(source: &str) -> Host<()> {
    let plan = contract::compile(source).unwrap().encode();
    Host::boot(
        &plan,
        (),
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0
}

fn view(h: &Host<()>, name: &str) -> u32 {
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
        (),
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let second = view(&h, "second");
    assert_eq!(h.presented(second).layout, [0.0; 4]);
    assert!(!h.motion());
    h.dispatch_at(view(&h, "toggle"), Event::Press, 100.);
    assert!(h.kernel().find_by_test_id("first").is_empty());
    let [_, from, ..] = h.presented(second).layout;
    assert!(from > 0.0, "painted where it was: {from}");
    h.tick(200.);
    let [_, mid, ..] = h.presented(second).layout;
    assert!(mid > 0.0 && mid < from, "{mid} of {from}");
    h.tick(421.);
    assert_eq!(h.presented(second).layout, [0.0; 4]);
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
        "component App\n  state on = false\n  action go\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      if not on\n        view height=50\n      column testId=\"card\" -exact-layout-transition=(on ? \"200ms linear\" : \"none\")\n        text \"Title\"\n        if on\n          view height=150\n",
    );
    let card = view(&h, "card");
    let before = h.kernel().node(card).unwrap().frame.height;
    // One commit gives the card the row, moves it up and grows it.
    h.dispatch_at(view(&h, "go"), Event::Press, 100.);
    let after = h.kernel().node(card).unwrap().frame.height;
    let [dx, dy, dw, dh] = h.presented(card).layout;
    assert_eq!((dx, dw), (0.0, 0.0));
    assert!((dy - 50.0).abs() < 1e-3, "{dy}");
    assert!((dh - (before - after)).abs() < 1e-3, "{dh}");
    h.tick(301.);
    assert_eq!(h.presented(card).layout, [0.0; 4]);
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
        assert_ne!(mid[3], 0.0);
        let before = h.kernel().node(box_id).unwrap().frame;
        assert!(h.resize(300.0, 700.0).is_none());
        let after = h.kernel().node(box_id).unwrap().frame;
        assert_eq!(before == after, width == "100");
        assert_eq!(
            h.presented(box_id).layout,
            [0.0; 4],
            "width={width}: the resize itself resets the presentation"
        );
        assert!(!h.motion());
        h.tick(600.0);
        assert_eq!(h.presented(box_id).layout, [0.0; 4]);
        h.dispatch_at(move_id, Event::Press, 600.0);
        assert_ne!(h.presented(box_id).layout, [0.0; 4]);
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
            "component App\n  state open = false\n  action toggle\n    open = not open\n  view\n    column\n      button press=toggle testId=\"toggle\"\n        text \"Toggle\"\n      column testId=\"card\" width=200 -exact-layout-transition=\"1000ms linear\" background-color=\"#0000ff\"{overflow}\n        view testId=\"title\" height=20 width=100 background-color=\"#ff0000\"\n        if open\n          view height=180 width=100 background-color=\"#00ff00\"\n"
        );
        let plan = contract::compile(&source).unwrap().encode();
        let (mut p, _) = Presenter::boot_with(
            &plan,
            (),
            (400., 600.),
            1.,
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            PainterChoice::Cpu,
        )
        .unwrap();
        let id = |p: &Presenter<()>, name: &str| {
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

#[test]
fn a_clipping_box_closing_to_zero_width_shrinks_its_clip_to_zero() {
    use exact_linux::{presenter::PainterChoice, Presenter};
    let source = "component App\n  state closed = false\n  action close\n    closed = true\n  view\n    column\n      button press=close testId=\"close\"\n        text \"Close\"\n      view testId=\"clip\" width=(closed ? 0 : 200) height=120 overflow=\"hidden\" -exact-layout-transition=\"1000ms linear\"\n        view testId=\"child\" width=200 height=120 background-color=\"#ff0000\"\n";
    let plan = contract::compile(source).unwrap().encode();
    let (mut p, _) = Presenter::boot_with(
        &plan,
        (),
        (400., 600.),
        1.,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let id = |p: &Presenter<()>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let clip = id(&p, "clip");
    p.tap(id(&p, "close")).unwrap();
    p.tick(500.);
    assert_eq!(p.host().kernel().node(clip).unwrap().frame.width, 0.0);
    let state: serde_json::Value =
        serde_json::from_str(&exact_linux::agent::handle(&mut p, r#"{"op":"state"}"#)).unwrap();
    let observed = state["presence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == clip)
        .unwrap();
    assert_eq!(observed["w"], 100.0, "half way from 200 to 0: {observed}");
    let (x, y, ..) = p.rect_of(clip).unwrap();
    let frame = p.frame();
    // The child keeps its 200 points; the clip shows its first 100.
    assert!(is(&frame, x + 50., y + 60., 0), "clipped to the shown box");
    assert!(!is(&frame, x + 150., y + 60., 0), "and no further");
    p.tick(1001.);
    let frame = p.frame();
    assert!(!is(&frame, x + 50., y + 60., 0), "closed");
}

/// `clock +N` as the agent runs it (`clock_stepped`): the request stops it
/// took.
fn agent_jump(h: &mut Host<()>, to: f64) -> usize {
    let mut stops = 0;
    while stops < 4096 && h.timer_due_ms().is_some_and(|d| d <= to) {
        h.advance_until_request(to);
        stops += 1;
    }
    h.advance(to);
    stops
}

/// A box a timer moves while another timer and a frame task run: one
/// `clock +N` crosses all of them.
const CAPSULE: &str = "component App
  state small = false
  state go = false
  state beats = 0
  state frames = 0
  action start
    go = true
  action shrink
    small = true
  action beat
    beats = beats + 1
  action frame
    frames = frames + 1
  task later when go
    after(100, shrink)
  task beating when go
    every(200, beat)
  task framing when go
    every(frame, frame)
  view
    column width=\"100%\" height=\"100%\" position=\"relative\"
      button press=start testId=\"go\"
        text \"Go\"
      row testId=\"capsule\" position=\"absolute\" right=(small ? 68 : 14) bottom=14 -exact-layout-transition=\"400ms linear\"
        view width=(small ? 48 : 180) height=48
";

#[test]
fn a_box_a_timer_moves_inside_an_agents_jump_starts_moving_at_the_timer() {
    let mut h = boot(CAPSULE);
    let capsule = view(&h, "capsule");
    h.dispatch_at(view(&h, "go"), Event::Press, 0.);
    // Right-anchored, it shrinks at 100 ms: its left edge goes from 206 to
    // 284, half way (39 short of it) at 300 ms, whatever else fired.
    assert_eq!(agent_jump(&mut h, 300.), 1, "no request stopped it");
    assert_eq!(h.kernel().node(capsule).unwrap().frame.x, 284.0);
    let [dx, ..] = h.presented(capsule).layout;
    assert!((dx + 39.0).abs() < 1e-3, "half way at 300 ms, not {dx}");
    agent_jump(&mut h, 1000.);
    assert_eq!(h.presented(capsule).layout, [0.0; 4]);
    // A frame task's every frame for two minutes is one call, not a request
    // stop each (the agent's 4,096 would end at 68 s).
    assert_eq!(agent_jump(&mut h, 121_000.), 1);
    assert_eq!(h.runner().now_ms(), 121_000.);
}

/// Each `fetchOne(word)` a request whose reply is `word`.
#[derive(Default)]
struct Fetches;
impl exact_runner::DataSource for Fetches {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        let word = args[0].as_str().unwrap_or_default();
        Ok(exact_runner::Answer::Later(exact_runner::Request::get(
            &format!("https://fixture.invalid/{word}"),
        )))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[exact_plan::Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        let exact_runner::Outcome::Storage(bytes) = outcome else {
            panic!("a storage outcome")
        };
        Ok(exact_runner::Answer::Now(exact_plan::Value::record(vec![
            exact_plan::Value::str(std::str::from_utf8(&bytes).unwrap()),
        ])))
    }
}

/// A box under a layout transition, and two timers that each send.
const SENDS: &str = "shape Reply
  text: string
component A
  state go = false
  state small = false
  state seen = \"\"
  mutation loaded as shape Reply then note
  action start
    go = true
  action load
    small = true
    send loaded = fetchOne(\"a\")
  action loadAgain
    send loaded = fetchOne(\"b\")
  action note
    match loaded
      case some(r)
        seen = `${seen}${r.text}`
      case none
        seen = seen
  task beating when go
    every(1000, load)
  task again when go
    after(1500, loadAgain)
  view
    column width=\"100%\" height=\"100%\" position=\"relative\"
      button testId=\"go\" press=start
        text \"Go\"
      row testId=\"capsule\" position=\"absolute\" right=(small ? 68 : 14) bottom=14 -exact-layout-transition=\"400ms linear\"
        view width=(small ? 48 : 180) height=48
";

#[test]
fn a_jump_stepping_for_a_layout_transition_still_stops_at_each_send() {
    let plan = contract::compile(SENDS).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        Fetches,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let go = {
        let k = h.kernel();
        k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id
    };
    h.dispatch_at(go, Event::Press, 0.);
    // `clock +1800` as the agent runs it: each stop's request is answered
    // before the jump goes on (LLP 1016 D5).
    let mut stops = Vec::new();
    while h.timer_due_ms().is_some_and(|d| d <= 1800.) {
        h.advance_until_request(1800.);
        for r in h.take_requests() {
            let word = r.request.url.rsplit('/').next().unwrap().to_string();
            stops.push((h.runner().now_ms(), word.clone()));
            let now = h.runner().now_ms();
            let landed = h
                .runner_mut()
                .fulfill(r.ticket, exact_runner::Outcome::Storage(word.into_bytes()))
                .unwrap();
            assert!(landed.is_some(), "the reply at {now} commits");
        }
    }
    h.advance(1800.);
    assert_eq!(stops, [(1000., "a".to_string()), (1500., "b".to_string())]);
    assert_eq!(
        h.runner().slot("seen"),
        Some(&exact_plan::Value::str("ab")),
        "both replies landed"
    );
}
