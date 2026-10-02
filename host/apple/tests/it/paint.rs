//! Paint motion on the Apple host's batch (LLP 1055.000 D6, LLP 1062): a box
//! whose paint the engine is moving is re-sent its `style` with the
//! presented values over its rows; when they arrive, the rows show again.
//! An inheriting view — and an inline run, through its paragraph — shows an
//! animating `color` too, and the presenter's appearance re-targets a
//! `light-dark()` colour: the first report without motion, the next with.

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
  action toggle
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

/// The last `style` op for `id` in the batch: a frame of paint motion
/// re-sends it.
fn style(batch: &str, id: u32) -> Option<String> {
    batch
        .split("{\"op\":")
        .filter(|op| op.starts_with(&format!("\"style\",\"id\":{id},")))
        .last()
        .map(str::to_owned)
}

/// The style `id` shows `key` as `value` in the batch.
fn shows(batch: &str, id: u32, key: &str, value: &str) -> bool {
    style(batch, id).is_some_and(|s| s.contains(&format!("\"{key}\":{value}")))
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
    assert!(
        first.contains("\"background_color\":[255,0,0,255]"),
        "boot shows rows"
    );
    let started = host.dispatch_at(toggle, Event::Press, 0.0);
    assert!(style(&started, toggle).is_some(), "{started}");
    let mid = host.tick(500.0);
    // Red to transparent blue: red fading, straight channels.
    assert!(
        shows(&mid, toggle, "background_color", "[255,0,0,128]"),
        "{mid}"
    );
    // The label inherits the colour; a sibling with none of its own does not.
    assert!(
        shows(&mid, label, "text_color", "[128,128,128,255]"),
        "{mid}"
    );
    assert_eq!(style(&mid, still), None);
    // `none` to a shadow: geometry and colour from zero together.
    assert!(
        shows(&mid, toggle, "box_shadow", "[{\"o\":[0,2],\"b\":6,"),
        "{mid}"
    );
    let done = host.tick(1000.0);
    // Arrived: the rows show again.
    assert!(
        shows(&done, toggle, "background_color", "[0,0,255,0]"),
        "{done}"
    );
    assert!(
        shows(&done, label, "text_color", "[255,255,255,255]"),
        "{done}"
    );
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
        !shows(&first, page, "background_color", "[0,0,0,255]"),
        "{first}"
    );
    let back = host.set_scheme(false);
    assert!(back.contains("\"motion\":true"), "{back}");
    let mid = host.tick(250.0);
    assert!(
        shows(&mid, page, "background_color", "[64,64,64,255]"),
        "{mid}"
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
        "fn accent(): string = \"light-dark(#000000, #ffffff)\"\nkeyframes lit\n  from color=accent()\n  to color=\"light-dark(#ff0000, #0000ff)\"\ncomponent App\n  view\n    text \"lit\" testId=\"word\" animation=\"lit 1s linear both\"\n",
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
    assert!(
        shows(&dark, word, "text_color", "[255,255,255,255]"),
        "{dark}"
    );
    let mid = host.tick(500.0);
    assert!(
        shows(&mid, word, "text_color", "[128,128,255,255]"),
        "{mid}"
    );
    let flipped = host.set_scheme(false);
    if style(&flipped, word).is_some() {
        assert!(
            shows(&flipped, word, "text_color", "[128,128,255,255]"),
            "{flipped}"
        );
    }
}

const INHERITING: &str = r##"component App
  state on = false
  action toggle
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
    let grey = "[128,128,128,255]";
    for side in ["top", "right", "bottom"] {
        assert!(
            shows(&mid, boxed, &format!("border_color_{side}"), grey),
            "{side}: {mid}"
        );
    }
    assert!(
        shows(&mid, boxed, "border_color_left", "[255,0,0,255]"),
        "its own colour"
    );
    // The run is no view: its paragraph paints it, in the colour it inherits.
    let paragraph = mid
        .split("{\"op\":")
        .find(|op| op.starts_with(&format!("\"paragraph\",\"id\":{para},")))
        .unwrap_or_else(|| panic!("{mid}"));
    let run = paragraph
        .split(&format!("\"id\":{plain},"))
        .nth(1)
        .unwrap_or_else(|| panic!("{paragraph}"));
    assert!(
        run.contains(&format!("\"text_color\":{grey}")),
        "{paragraph}"
    );
    let own = paragraph.split(&format!("\"id\":{red},")).nth(1).unwrap();
    assert!(own.contains("\"text_color\":[255,0,0,255]"), "{paragraph}");
    let done = host.tick(1000.0);
    assert!(
        shows(&done, boxed, "border_color_top", "[255,255,255,255]"),
        "{done}"
    );
    assert!(
        done.contains(&format!("\"paragraph\",\"id\":{para},")),
        "{done}"
    );
}

/// An exit animation animates a colour its node never transitioned: the
/// exit's keyframes own it while the node lives (LLP 1063).
#[test]
fn an_exit_animates_a_colour_the_node_never_transitioned() {
    let mut host = boot(
        "keyframes leave\n  to background-color=\"#0000ff\"\ncomponent App\n  state shown = true\n  action hide\n    shown = false\n  view\n    column\n      button press=hide testId=\"hide\"\n        text \"Hide\"\n      when shown\n        column testId=\"gone\" height=40 background-color=\"#ff0000\" exit-animation=\"leave 1s linear both\"\n",
    );
    let (hide, gone) = (view(&host, "hide"), view(&host, "gone"));
    let off = host.dispatch_at(hide, Event::Press, 0.0);
    assert!(
        off.contains(&format!("{{\"op\":\"exit\",\"id\":{gone}}}")),
        "{off}"
    );
    let mid = host.tick(500.0);
    assert!(
        shows(&mid, gone, "background_color", "[128,0,128,255]"),
        "{mid}"
    );
}

/// LLP 1062 D4: a view whose own appearance differs from the session's (a
/// sheet with an override, say) reports it, and its node's `light-dark()`
/// colours resolve by it: the first report corrects in place, keyframes
/// included; one that agrees with the session again transitions back.
#[test]
fn a_view_in_its_own_appearance_resolves_by_it() {
    let mut host = boot(
        "keyframes lit\n  from color=\"light-dark(#000000, #ffffff)\"\n  to color=\"light-dark(#ff0000, #0000ff)\"\ncomponent App\n  view\n    column\n      text \"lit\" testId=\"word\" animation=\"lit 1s linear both\"\n      column testId=\"page\" height=10 background-color=\"light-dark(#ffffff, #000000)\" transition=\"background-color 1s linear\"\n",
    );
    let (word, page) = (view(&host, "word"), view(&host, "page"));
    host.set_scheme(false);
    host.tick(500.0);
    // The word's view is dark: its playing keyframes take the dark pair now.
    let own = host.set_view_scheme(word, true);
    assert!(
        shows(&own, word, "text_color", "[128,128,255,255]"),
        "{own}"
    );
    assert!(
        host.set_view_scheme(word, true).contains("\"ops\":[]"),
        "an unchanged report is nothing"
    );
    // The page's view reports dark too: its background is corrected, with
    // no motion, and the style row it hands back resolves by the view.
    let corrected = host.set_view_scheme(page, true);
    assert_eq!(style(&corrected, page), None, "{corrected}");
    // Agreeing with the session again is an appearance change: it moves.
    let back = host.set_view_scheme(page, false);
    assert!(back.contains("\"motion\":true"), "{back}");
    let mid = host.tick(750.0);
    assert!(
        shows(&mid, page, "background_color", "[64,64,64,255]"),
        "{mid}"
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
  action toggle
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
    assert!(
        shows(&mid, stays, "border_color_top", "[128,128,128,255]"),
        "{mid}"
    );
    assert!(
        shows(&mid, becomes, "border_color_top", "[128,0,128,255]"),
        "{mid}"
    );
}

/// An SVG shape's `light-dark()` fill transitions when the appearance
/// changes, as a box's colour does (LLP 1055.000 D6, LLP 1062 D4): the scene
/// is re-sent with the moving value.
#[test]
fn an_svg_fill_moves_between_its_light_dark_pair() {
    let mut host = boot(
        "component App\n  view\n    svg testId=\"chart\" width=100 height=100 viewBox=\"0 0 100 100\"\n      circle cx=50 cy=50 r=40 fill=\"light-dark(#ffffff, #000000)\" transition=\"fill 1s linear\"\n",
    );
    let chart = view(&host, "chart");
    host.set_scheme(false);
    let flipped = host.set_scheme(true);
    assert!(flipped.contains("\"motion\":true"), "{flipped}");
    let mid = host.tick(500.0);
    let scene = mid
        .split("{\"op\":")
        .find(|op| op.starts_with(&format!("\"svg\",\"id\":{chart},")))
        .unwrap_or_else(|| panic!("{mid}"));
    assert!(scene.contains("\"f\":[128,128,128,255]"), "{scene}");
}

#[test]
fn an_exit_presents_text_color_over_the_last_style() {
    let mut host = boot(
        "keyframes leave\n  to color=\"#0000ff\"\ncomponent App\n  state shown = true\n  action hide\n    shown = false\n  view\n    column\n      button \"Hide\" press=hide testId=\"hide\"\n      when shown\n        text \"Leaving\" testId=\"gone\" color=\"#ff0000\" exit-animation=\"leave 1s linear both\"\n",
    );
    let (hide, gone) = (view(&host, "hide"), view(&host, "gone"));
    host.dispatch_at(hide, Event::Press, 0.0);
    let mid = host.tick(500.0);
    assert!(shows(&mid, gone, "text_color", "[128,0,128,255]"), "{mid}");
}

#[test]
fn cancelling_color_motion_restores_inheriting_views_and_inline_runs() {
    let source = INHERITING
        .replace(
            "  view\n",
            "  state moving = true\n  action stop\n    moving = false\n  view\n",
        )
        .replace(
            "transition=\"color 1s linear\"",
            "transition=(moving ? \"color 1s linear\" : \"none\")",
        )
        .replace(
            "      button press=toggle",
            "      button \"Stop\" press=stop testId=\"stop\"\n      button press=toggle",
        );
    let mut host = boot(&source);
    let (toggle, para, plain) = (
        view(&host, "toggle"),
        view(&host, "para"),
        view(&host, "plain"),
    );
    host.dispatch_at(toggle, Event::Press, 0.0);
    let mid = host.tick(500.0);
    assert!(
        shows(&mid, toggle, "text_color", "[128,128,128,255]"),
        "{mid}"
    );
    let stopped = host.dispatch_at(view(&host, "stop"), Event::Press, 500.0);
    assert!(
        shows(&stopped, toggle, "text_color", "[255,255,255,255]"),
        "{stopped}"
    );
    let batch: serde_json::Value = serde_json::from_str(&stopped).unwrap();
    let paragraph = batch["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "paragraph" && op["id"] == para)
        .expect("run repainted");
    let run = paragraph["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| run["id"] == plain)
        .unwrap();
    assert_eq!(
        run["style"]["text_color"],
        serde_json::json!([255, 255, 255, 255]),
        "{paragraph}"
    );
}
