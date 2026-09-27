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

const INHERITING: &str = r##"component App
  state on = false
  action toggle writes on
    on = not on
  view
    column testId="box" color=(on ? "#ffffff" : "#000000") transition="color 1s linear" border-width=2 border-style="solid" border-left-color="#ff0000"
      button press=toggle testId="toggle"
        text "Go"
      text testId="para"
        text "plain " testId="plain"
        text "red" color="#ff0000" testId="red"
"##;

fn boot(src: &str) -> Host<NoData> {
    let plan = contract::compile(src).unwrap();
    Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap()
    .0
}

/// A `currentcolor` border side paints the animating `color` frame by
/// frame, as CSS's used value does; a side with its own colour keeps it.
/// An inline run that inherits the colour follows it too, through its
/// paragraph (LLP 1062 D5).
#[test]
fn currentcolor_borders_and_inline_runs_follow_an_animating_color() {
    let mut host = boot(INHERITING);
    let (boxed, toggle, para, plain, red) = (
        view(&host, "box"),
        view(&host, "toggle"),
        view(&host, "para"),
        view(&host, "plain"),
        view(&host, "red"),
    );
    host.dispatch_at(toggle, Event::Press, 0.0);
    let mid = host.tick(500.0);
    let grey = "\"x\":127.5,\"y\":127.5,\"w\":127.5,\"h\":255";
    for side in ["top", "right", "bottom"] {
        let op = paint(&mid, boxed, &format!("border_color_{side}")).unwrap();
        assert!(op.contains(grey), "{side}: {op}");
    }
    assert_eq!(
        paint(&mid, boxed, "border_color_left"),
        None,
        "its own colour"
    );
    // The run is no view: its paragraph paints it.
    let run = mid
        .split("{\"op\":")
        .find(|op| op.contains(&format!("\"id\":{para},\"run\":{plain},")))
        .unwrap_or_else(|| panic!("{mid}"));
    assert!(run.contains(grey), "{run}");
    assert!(!mid.contains(&format!("\"run\":{red},")), "{mid}");
    let done = host.tick(1000.0);
    let op = paint(&done, boxed, "border_color_top").unwrap();
    assert!(op.starts_with("\"unpresent\""), "{done}");
    assert!(
        done.contains(&format!(
            "{{\"op\":\"unpresent\",\"id\":{para},\"run\":{plain},\"property\":\"text_color\"}}"
        )),
        "{done}"
    );
}

/// An exit animation animates a colour its node never transitioned: the
/// exit's keyframes own it while the node lives (LLP 1063).
#[test]
fn an_exit_animates_a_colour_the_node_never_transitioned() {
    let mut host = boot(
        "keyframes leave\n  to\n    background-color=\"#0000ff\"\ncomponent App\n  state shown = true\n  action hide writes shown\n    shown = false\n  view\n    column\n      button press=hide testId=\"hide\"\n        text \"Hide\"\n      when shown\n        column testId=\"gone\" height=40 background-color=\"#ff0000\" exit-animation=\"leave 1s linear both\"\n",
    );
    let (hide, gone) = (view(&host, "hide"), view(&host, "gone"));
    let off = host.dispatch_at(hide, Event::Press, 0.0);
    assert!(
        off.contains(&format!("{{\"op\":\"exit\",\"id\":{gone}}}")),
        "{off}"
    );
    let mid = host.tick(500.0);
    let bg = paint(&mid, gone, "background_color").unwrap_or_else(|| panic!("{mid}"));
    assert!(
        bg.contains("\"x\":127.5,\"y\":0,\"w\":127.5,\"h\":255"),
        "{bg}"
    );
}

/// LLP 1062 D4: a view whose own appearance differs from the session's (a
/// sheet with an override, say) reports it, and its node's `light-dark()`
/// colours resolve by it: the first report corrects in place, keyframes
/// included; one that agrees with the session again transitions back.
#[test]
fn a_view_in_its_own_appearance_resolves_by_it() {
    let mut host = boot(
        "keyframes lit\n  from\n    color=\"light-dark(#000000, #ffffff)\"\n  to\n    color=\"light-dark(#ff0000, #0000ff)\"\ncomponent App\n  view\n    column\n      text \"lit\" testId=\"word\" animation=\"lit 1s linear both\"\n      column testId=\"page\" height=10 background-color=\"light-dark(#ffffff, #000000)\" transition=\"background-color 1s linear\"\n",
    );
    let (word, page) = (view(&host, "word"), view(&host, "page"));
    host.set_scheme(false);
    host.tick(500.0);
    // The word's view is dark: its playing keyframes take the dark pair now.
    let own = host.set_view_scheme(word, true);
    let ink = paint(&own, word, "text_color").unwrap_or_else(|| panic!("{own}"));
    assert!(
        ink.contains("\"x\":127.5,\"y\":127.5,\"w\":255,\"h\":255"),
        "{ink}"
    );
    assert!(
        host.set_view_scheme(word, true).contains("\"ops\":[]"),
        "an unchanged report is nothing"
    );
    // The page's view reports dark too: its background is corrected, with
    // no motion, and the style row it hands back resolves by the view.
    let corrected = host.set_view_scheme(page, true);
    assert!(
        paint(&corrected, page, "background_color")
            .is_none_or(|op| op.starts_with("\"unpresent\"")),
        "{corrected}"
    );
    // Agreeing with the session again is an appearance change: it moves.
    let back = host.set_view_scheme(page, false);
    assert!(back.contains("\"motion\":true"), "{back}");
    let mid = host.tick(750.0);
    let bg = paint(&mid, page, "background_color").unwrap_or_else(|| panic!("{mid}"));
    assert!(
        bg.contains("\"x\":63.75,\"y\":63.75,\"w\":63.75,\"h\":255"),
        "{bg}"
    );
}

/// CSS: a side that stays `currentcolor` never transitions on its own — its
/// computed value is the keyword — so it follows `color` even under a faster
/// `border-color` transition; a side that becomes `currentcolor` from a
/// colour moves there under its own row (Chrome 153: red to currentcolor
/// with `color: blue` is `rgb(128, 0, 128)` halfway).
#[test]
fn a_side_that_stays_currentcolor_follows_color_and_one_that_becomes_it_moves() {
    let mut host = boot(
        r##"component App
  state on = false
  action toggle writes on
    on = not on
  view
    column
      button press=toggle testId="toggle"
        text "Go"
      column testId="stays" height=10 border-width=2 border-style="solid" color=(on ? "#ffffff" : "#000000") transition="color 1s linear, border-color 200ms linear"
      column testId="becomes" height=10 border-width=2 border-style="solid" color="#0000ff" border-top-color=(on ? "currentcolor" : "#ff0000") transition="border-color 1s linear"
"##,
    );
    let (toggle, stays, becomes) = (
        view(&host, "toggle"),
        view(&host, "stays"),
        view(&host, "becomes"),
    );
    host.dispatch_at(toggle, Event::Press, 0.0);
    let mid = host.tick(500.0);
    let side = paint(&mid, stays, "border_color_top").unwrap_or_else(|| panic!("{mid}"));
    assert!(
        side.contains("\"x\":127.5,\"y\":127.5,\"w\":127.5,\"h\":255"),
        "{side}"
    );
    let side = paint(&mid, becomes, "border_color_top").unwrap_or_else(|| panic!("{mid}"));
    assert!(
        side.contains("\"x\":127.5,\"y\":0,\"w\":127.5,\"h\":255"),
        "{side}"
    );
}

/// A path's `fill` and `stroke` are paint motion's (LLP 1065): presented
/// over their rows under the dictionary's own keys, reaching a path that
/// inherits them; `none` is discrete, so moving to it retires the motion.
#[test]
fn a_paths_fill_and_stroke_move_as_colours() {
    let src = r##"component App
  state on = false
  action toggle writes on
    on = not on
  view
    column
      button press=toggle testId="toggle"
        text "Go"
      column testId="group" stroke=(on ? "#0000ff" : "#ff0000") transition="stroke 1s linear"
        path testId="child" d="M0 0 H10" width=10 height=10
      path testId="own" d="M0 0 H10" width=10 height=10 fill=(on ? "#00ff00" : "#000000") transition="fill 1s linear"
      path testId="gone" d="M0 0 H10" width=10 height=10 fill=(on ? "none" : "#000000") transition="fill 1s linear"
"##;
    let plan = contract::compile(src).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let (toggle, group, child, own, gone) = (
        view(&host, "toggle"),
        view(&host, "group"),
        view(&host, "child"),
        view(&host, "own"),
        view(&host, "gone"),
    );
    host.dispatch_at(toggle, Event::Press, 0.0);
    let mid = host.tick(500.0);
    let fill = paint(&mid, own, "fill").unwrap();
    assert!(
        fill.contains("\"x\":0,\"y\":127.5,\"w\":0,\"h\":255"),
        "{fill}"
    );
    // The inheriting path paints the group's moving stroke.
    for id in [group, child] {
        let stroke = paint(&mid, id, "stroke").unwrap();
        assert!(
            stroke.contains("\"x\":127.5,\"y\":0,\"w\":127.5"),
            "{stroke}"
        );
    }
    assert_eq!(
        paint(&mid, gone, "fill"),
        None,
        "to `none` is no transition"
    );
    let done = host.tick(1000.0);
    for (id, key) in [(own, "fill"), (child, "stroke")] {
        let op = paint(&done, id, key).unwrap();
        assert!(op.starts_with("\"unpresent\""), "{op}");
    }
}

/// A path's markers cross placed (LLP 1065 D11): the path's length, each
/// marker's shapes once, and each instance's transform, viewport and fit; a
/// path in a path takes the outer one's view box (D12) and inherits its
/// markers.
#[test]
fn markers_and_a_path_of_paths_cross_as_props() {
    let src = r##"marker sq
  refX=5 refY=5 markerWidth=10 markerHeight=10 markerUnits="userSpaceOnUse"
  path d="M0 0 H10 V10 H0 Z" fill="context-stroke"
component App
  view
    column
      path testId="outer" width=100 height=50 viewBox="0 0 10 5" d="M0 1 H8" stroke="#ff0000" marker-start="url(#sq)" marker-end="url(#sq)"
        path testId="inner" d="M0 0 H10"
"##;
    let plan = contract::compile(src).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let (outer, inner) = (view(&host, "outer"), view(&host, "inner"));
    let create = |id: u32| {
        first
            .split("{\"op\":")
            .find(|op| op.starts_with("\"create\"") && op.contains(&format!("\"id\":{id},")))
            .unwrap()
            .to_owned()
    };
    let o = create(outer);
    assert!(
        o.contains(r#""markers":"L 8\nD 0\nS M 0 0 L 10 0 L 10 10 L 0 10 Z|context-stroke|none|1|butt|miter|4|nonzero\nI 0 0 1 0 0 1 -5 -4 0 0 10 10 1 1 0 0\nI 0 8 1 0 0 1 3 -4 0 0 10 10 1 1 0 0""#),
        "{o}"
    );
    let i = create(inner);
    assert!(i.contains(r#""viewBox":"0 0 10 5""#), "{i}");
    // Markers inherit, as SVG's do: the inner path has its own.
    assert!(
        i.contains(r#"I 0 10 1 0 0 1 5 -5 0 0 10 10 1 1 0 0""#),
        "{i}"
    );
}
