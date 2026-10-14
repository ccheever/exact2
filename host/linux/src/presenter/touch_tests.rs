//! A finger is a finger (LLP 1115.001 D7.4, D7.6): what a shell that names
//! the pointer changes, and that a mouse, which is what the pointer is until
//! one does, is as it was.
use super::*;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.boxes();
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn log(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_string()
}

/// Down and up at a point: a tap.
fn tap(p: &mut Presenter<()>, x: f32, y: f32, at: f64) {
    p.pointer_down(x, y, at).unwrap();
    p.pointer_up(x, y, at + 1.).unwrap();
}

const PAD: &str = r#"component App
  state seen = ""
  action down(e: PointerEvent)
    seen = `${seen} ${e.pointerType}${e.pointerId}/${e.pressure}`
  action hot(over)
    seen = `${seen} ${over ? "in" : "out"}`
  view
    column width=400 height=400
      box pointerdown=down testId="pad" width=200 height=100
      box hover=hot testId="row" width=200 height=100
      text seen testId="log" height=20
"#;

#[test]
fn a_finger_is_reported_as_a_touch_and_the_pointer_is_a_mouse_until_told() {
    let mut p = boot(PAD);
    tap(&mut p, 20., 20., 1.);
    assert_eq!(log(&p), " mouse1/0.5");
    p.set_pointer_kind(PointerKind::Touch);
    tap(&mut p, 20., 20., 3.);
    p.set_pointer_kind(PointerKind::Pen);
    tap(&mut p, 20., 20., 5.);
    assert_eq!(log(&p), " mouse1/0.5 touch2/0.5 pen2/0.5");
}

#[test]
fn a_finger_hovers_nothing_and_leaves_no_hover_behind() {
    let mut p = boot(PAD);
    // A mouse over the row hovers it, and stays hovering after a click there.
    p.pointer_move(20., 150., 1.).unwrap();
    tap(&mut p, 20., 150., 2.);
    assert_eq!(log(&p), " in");
    p.pointer_move(20., 350., 4.).unwrap();
    assert_eq!(log(&p), " in out");
    // A finger on the row, dragged within it and lifted: never hovered.
    p.set_pointer_kind(PointerKind::Touch);
    p.pointer_down(20., 150., 5.).unwrap();
    p.pointer_move(22., 152., 6.).unwrap();
    p.pointer_up(22., 152., 7.).unwrap();
    assert_eq!(log(&p), " in out");
    assert!(p.hovered.is_empty() && p.hover_point.is_none());
    // Nor does a repaint hover what is under where it lifted.
    let _ = p.frame();
    p.follow_pointer();
    assert_eq!(log(&p), " in out");
    // What a mouse left hovered goes with a finger's touch (by its lift).
    p.set_pointer_kind(PointerKind::Mouse);
    p.pointer_move(20., 150., 8.).unwrap();
    assert_eq!(log(&p), " in out in");
    p.set_pointer_kind(PointerKind::Touch);
    tap(&mut p, 20., 20., 9.);
    assert_eq!(log(&p), " in out in touch2/0.5 out");
    assert!(p.hovered.is_empty());
}

const BUTTONS: &str = r##"component App
  state pressed = 0
  action press
    pressed = pressed + 1
  view
    column width=400 height=300 background-color="white" align-items="flex-start"
      button "Native" appearance="auto" press=press width=120 height=40 accent-color="#ff0000" testId="native"
      button appearance="none" press=press width=120 height=40 accent-color="#ff0000" testId="bare"
        box width="100%" height="100%" background-color="blue"
          text "Bare"
      text `${pressed}` testId="log" height=20
"##;

/// The pixel on the top edge of a button, where its focus ring is drawn.
fn edge(p: &mut Presenter<()>, name: &str) -> [u8; 4] {
    let k = p.host().kernel();
    let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
    let c = p.frame().pixel(f.x as u32 + 60, f.y as u32).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn middle(p: &Presenter<()>, name: &str) -> (f32, f32) {
    let k = p.host().kernel();
    let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
    (f.x + f.width / 2., f.y + f.height / 2.)
}

const RING: [u8; 4] = [255, 0, 0, 255];

#[test]
fn a_fingers_press_leaves_no_focus_ring_and_a_mouses_does() {
    for name in ["native", "bare"] {
        let other = if name == "native" { "bare" } else { "native" };
        // A mouse: the pressed button holds the focus and shows its ring.
        let mut p = boot(BUTTONS);
        let (x, y) = middle(&p, name);
        tap(&mut p, x, y, 1.);
        assert_eq!(p.focus, Some(id(&p, name)));
        assert_eq!(edge(&mut p, name), RING, "{name}: a mouse's press rings");
        // A finger: the focus, and no ring.
        let mut p = boot(BUTTONS);
        p.set_pointer_kind(PointerKind::Touch);
        tap(&mut p, x, y, 1.);
        assert_eq!(log(&p), "1", "{name}: the press went through");
        assert_eq!(p.focus, Some(id(&p, name)), "{name}: still the focus");
        assert_ne!(
            edge(&mut p, name),
            RING,
            "{name}: a finger's press rings nothing"
        );
        // A focus a key moves shows its ring, in touch mode too.
        let next = id(&p, other);
        assert!(p.set_focus(Some(next), 3.).is_none());
        assert_eq!(edge(&mut p, other), RING, "{other}: a key's focus rings");
        assert_ne!(edge(&mut p, name), RING);
        // And a finger pressing that one takes its ring away.
        let (x, y) = middle(&p, other);
        tap(&mut p, x, y, 4.);
        assert_ne!(edge(&mut p, other), RING, "{other}: pressed by a finger");
    }
}

#[test]
fn a_tap_that_drifts_inside_the_systems_slop_still_presses() {
    let phone = Device {
        night: false,
        font_scale: 1.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        touch_slop: 8.0,
    };
    let drift = |p: &mut Presenter<()>, by: f32, at: f64| {
        let (x, y) = middle(p, "bare");
        p.pointer_down(x, y, at).unwrap();
        p.pointer_move(x + by, y, at + 1.).unwrap();
        p.pointer_up(x + by, y, at + 2.).unwrap();
    };
    // The shared slop is 4 points: a contact that moves 6 is no tap.
    let mut p = boot(BUTTONS);
    drift(&mut p, 6., 1.);
    assert_eq!(log(&p), "0");
    drift(&mut p, 3., 5.);
    assert_eq!(log(&p), "1");
    // A phone says its own (Android's is 8 dp): 6 points is still a tap.
    let mut p = boot(BUTTONS);
    assert!(p.set_device(&phone).is_none());
    p.set_pointer_kind(PointerKind::Touch);
    drift(&mut p, 6., 1.);
    assert_eq!(log(&p), "1");
    drift(&mut p, 10., 5.);
    assert_eq!(log(&p), "1", "past the slop it is not");
    // A device with no touch screen keeps the shared one, whatever it says.
    let mut p = boot(BUTTONS);
    let desk = Device {
        touch: false,
        ..phone
    };
    assert!(p.set_device(&desk).is_none());
    drift(&mut p, 6., 1.);
    assert_eq!(log(&p), "0");
}
