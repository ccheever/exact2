//! LLP 1057.001 §8 on the Android reader: a fling the reader recognized goes,
//! before the lift, to the innermost swipe on the touch's first hit's path
//! that names its direction at its count, and ends the contact. A live
//! `press` node, control or text input between keeps it, a scroller that
//! scrolls on its axis keeps it, a node that is hidden or disabled passes it
//! on, also when the up handlers of the contact the fling ended made it so,
//! and a gesture of the presenter's that took the contact leaves none.
use super::super::*;

const SOURCE: &str = r#"component App
  state seen = ""
  state x = 0
  state hide = "block"
  state veil = "block"
  state off = false
  state shown = true
  action outer(e: SwipeEvent)
    seen = `${seen}outer-${e.direction} `
  action inner(e: SwipeEvent)
    seen = `${seen}inner-${e.direction} `
  action both(e: SwipeEvent)
    seen = `${seen}both-${e.direction} `
  action pair(e: SwipeEvent)
    seen = `${seen}pair-${e.direction} `
  action moved(dx: number, dy: number)
    x = x + dx
  action pressed
    x = x + 1000
  action menu
    x = x + 7
  action hideSelf
    hide = "none"
  action hideParent
    veil = "none"
  action disable
    off = true
    x = x + 1
  action remove
    shown = false
  view
    column width=400 height=700
      box testId="outer" width=400 swipe=outer swipe-direction="left right down"
        box testId="inner" width=400 height=30 swipe=inner swipe-direction="left"
        box testId="pan" width=400 height=30 pan=moved touch-action="none"
        button testId="button" press=pressed width=200 height=30
          text "press"
        box testId="both" width=400 height=30 press=pressed swipe=both swipe-direction="left" -exact-press-scale=0.9
        box testId="off" width=400 height=30 swipe=inner swipe-direction="left" disabled=true
        box width=400 height=30 disabled=true
          box testId="under" width=400 height=30
        input testId="field" width=400 height=30 value=""
        input testId="check" type="checkbox" width=400 height=30
        box testId="menu" width=400 height=30 contextmenu=menu
        box testId="list" width=400 height=60 overflow-y="auto"
          box width=400 height=200
        box testId="hides" width=400 height=30 swipe=inner swipe-direction="left" display=hide pointerdown=hideSelf
        box width=400 display=veil pointerdown=hideParent
          box testId="veiled" width=400 height=30 swipe=inner swipe-direction="left"
        box testId="fickle" width=400 height=30 swipe=inner swipe-direction="left" disabled=off pointerup=disable
        if shown
          box testId="fleeting" width=400 height=30 swipe=inner swipe-direction="left" pointerup=remove
      box testId="pair" width=400 height=50 swipe=pair swipe-direction="up" touch-count=2
      text seen testId="seen"
      text `${x}` testId="x"
"#;
fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(SOURCE),
        (),
        (400., 700.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // Name every touch a finger and serve the top layer, as the reader does
    p.set_pointer_kind(PointerKind::Touch);
    p.serve_overlays();
    p.frame();
    p
}
fn text(p: &Presenter<()>, name: &str) -> String {
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id;
    k.node(id)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap_or("")
        .to_owned()
}
fn seen(p: &Presenter<()>) -> String {
    text(p, "seen")
}
fn mid(p: &mut Presenter<()>, name: &str) -> f32 {
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id;
    let (_, y, _, h) = p.rect_of(id).unwrap();
    y + h / 2.
}
/// One finger down on node `name`, 60 px sideways and the reader's fling, as
/// it sends it before the lift; then the lift. Whether a swipe took the fling.
fn flung(p: &mut Presenter<()>, name: &str, direction: &str, pointers: u32) -> bool {
    let (t, y) = (p.host.now() + 100., mid(p, name));
    p.pointer_down(100., y, t).unwrap();
    p.pointer_move(70., y, t + 10.).unwrap();
    p.pointer_move(40., y, t + 20.).unwrap();
    let took = p.fling(direction, pointers, t + 30.).unwrap();
    p.pointer_up(40., y, t + 30.).unwrap();
    took
}
/// One finger down on node `name` and the reader's fling before it moves;
/// then the lift. Whether a swipe took the fling.
fn flicked(p: &mut Presenter<()>, name: &str, direction: &str) -> bool {
    let (t, y) = (p.host.now() + 100., mid(p, name));
    p.pointer_down(100., y, t).unwrap();
    let took = p.fling(direction, 1, t + 10.).unwrap();
    p.pointer_up(100., y, t + 10.).unwrap();
    took
}
/// One finger down and up on node `name`, still.
fn tapped(p: &mut Presenter<()>, name: &str) {
    let (t, y) = (p.host.now() + 100., mid(p, name));
    p.pointer_down(100., y, t).unwrap();
    p.pointer_up(100., y, t + 30.).unwrap();
}

#[test]
fn a_fling_goes_to_the_innermost_swipe_that_names_its_direction() {
    let mut p = boot();
    assert!(flung(&mut p, "inner", "left", 1));
    assert!(
        flung(&mut p, "inner", "right", 1),
        "the inner one names no right"
    );
    assert!(
        !flung(&mut p, "inner", "up", 1),
        "nor does the outer one name up"
    );
    assert!(flung(&mut p, "inner", "down", 1));
    assert_eq!(seen(&p), "inner-left outer-right outer-down ");
    assert!(
        !p.fling("down", 1, p.host.now()).unwrap(),
        "one fling a touch"
    );
}

#[test]
fn a_flick_on_a_node_that_presses_and_swipes_swipes_and_a_tap_presses() {
    let mut p = boot();
    assert!(flung(&mut p, "both", "left", 1));
    assert_eq!(
        (seen(&p).as_str(), text(&p, "x").as_str()),
        ("both-left ", "0"),
        "the swipe ended the armed press"
    );
    tapped(&mut p, "both");
    assert_eq!(text(&p, "x"), "1000");
}

#[test]
fn a_disabled_node_passes_the_fling_to_an_ancestors_swipe() {
    let mut p = boot();
    assert!(
        flung(&mut p, "off", "left", 1),
        "a disabled swipe takes none"
    );
    assert!(
        flung(&mut p, "under", "left", 1),
        "nor does a node inside a disabled one keep it"
    );
    assert_eq!(seen(&p), "outer-left outer-left ");
}

#[test]
fn a_press_control_or_field_between_keeps_the_fling_and_a_gesture_that_took_it_leaves_none() {
    let mut p = boot();
    assert!(!flung(&mut p, "button", "left", 1), "the button keeps it");
    assert!(!flung(&mut p, "field", "left", 1), "the field keeps it");
    assert!(!flung(&mut p, "check", "left", 1), "the checkbox keeps it");
    assert!(
        !flung(&mut p, "pan", "left", 1),
        "the pan began and took it"
    );
    assert_eq!(text(&p, "x"), "-60");
    // A long press that a node took
    let (t, y) = (p.host.now() + 100., mid(&mut p, "menu"));
    p.pointer_down(100., y, t).unwrap();
    assert!(p.long_press(100., y, t + 600.));
    assert!(
        !p.fling("left", 1, t + 700.).unwrap(),
        "the long press took it"
    );
    p.pointer_up(100., y, t + 700.).unwrap();
    assert_eq!(seen(&p), "");
}

#[test]
fn a_hidden_node_passes_the_fling_on() {
    let mut p = boot();
    assert!(
        flung(&mut p, "hides", "left", 1),
        "it hid itself at its down"
    );
    assert!(
        flung(&mut p, "veiled", "left", 1),
        "its parent hid at the down"
    );
    assert_eq!(seen(&p), "outer-left outer-left ");
}

#[test]
fn the_lift_after_a_fling_runs_the_up_handlers() {
    let mut p = boot();
    // The contact ended at the first move, past the slop
    assert!(flung(&mut p, "fickle", "left", 1));
    assert!(
        flung(&mut p, "fickle", "left", 1),
        "the first lift disabled it"
    );
    assert_eq!(seen(&p), "inner-left outer-left ");
    assert_eq!(text(&p, "x"), "1", "one up");
}

#[test]
fn a_second_fingers_cancel_after_the_slop_runs_the_up_handlers() {
    let mut p = boot();
    let (t, y) = (p.host.now() + 100., mid(&mut p, "fickle"));
    p.pointer_down(100., y, t).unwrap();
    p.pointer_move(40., y, t + 10.).unwrap();
    // The reader sends a second finger's coming as a cancel
    p.pointer_cancel(t + 20.).unwrap();
    assert!(p.fling("left", 2, t + 30.).unwrap());
    p.pointer_up(40., y, t + 30.).unwrap();
    assert_eq!(seen(&p), "outer-left ", "the cancel's up disabled it");
    assert_eq!(text(&p, "x"), "1", "one up");
}

#[test]
fn a_fling_that_ends_a_contact_finds_its_swipe_again_after_the_up_handlers() {
    let mut p = boot();
    assert!(flicked(&mut p, "fickle", "left"), "its up disabled it");
    assert!(!flicked(&mut p, "fleeting", "left"), "its up removed it");
    assert_eq!(seen(&p), "outer-left ");
}

#[test]
fn a_scroller_keeps_a_fling_on_its_axis_and_passes_on_the_other() {
    let mut p = boot();
    assert!(!flung(&mut p, "list", "down", 1), "the list scrolls down");
    assert!(flung(&mut p, "list", "left", 1), "but not sideways");
    assert_eq!(seen(&p), "outer-left ");
}

#[test]
fn a_count_takes_that_many_and_the_fling_outlives_the_contact_a_second_finger_ends() {
    let mut p = boot();
    assert!(
        !flung(&mut p, "pair", "up", 1),
        "one finger is not the pair's"
    );
    // The reader sends a second finger's coming as a cancel; the touch goes on
    let (t, y) = (p.host.now() + 100., mid(&mut p, "pair"));
    p.pointer_down(100., y, t).unwrap();
    p.pointer_cancel(t + 10.).unwrap();
    assert!(p.fling("up", 2, t + 30.).unwrap());
    // With no count, the fling takes any number of fingers
    let y = mid(&mut p, "inner");
    p.pointer_down(100., y, t + 40.).unwrap();
    p.pointer_cancel(t + 50.).unwrap();
    assert!(p.fling("right", 2, t + 60.).unwrap());
    assert_eq!(seen(&p), "pair-up outer-right ");
}
