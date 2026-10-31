//! LLP 1121 D8 on the Linux host's CPU painter (Android's Canvas reader
//! replays what it records; its layers are not exercised here): `:active` is
//! the press paint, eased on the press's curve,
//! its opacity an absolute target from what shows; a `:hover` look shows at
//! once while a mouse hovers the node (or a child of it), its opacity in
//! place of the node's, under the press's paint; a disabled node takes none.
use super::*;

const APP: &str = r##"component App
  state presses = 0
  action go
    presses = presses + 1
  view
    column width=400 height=400 background-color="#ffffff"
      box press=go background-color="#00ff00" border="8px solid #000000" width=300 height=120 testId="card" scale=(presses > 1 ? 1.01 : 1) transition="scale 200ms"
        :active
          background-color = "#0000ff"
          opacity = 0.5
        :hover
          background-color = "#ffff00"
          border-color = "#ff0000"
          opacity = 0.8
        text "c" testId="child"
      box press=go background-color="#00ff00" width=300 height=40 testId="focused"
        :focus-visible
          background-color = "#ff00ff"
      box background-color="#00ff00" width=300 height=40 testId="clears"
        :hover
          background-color = "unset"
      box press=go disabled=true background-color="#00ff00" width=300 height=60 testId="off"
        :hover
          background-color = "#ffff00"
"##;

fn boot() -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP)
            .unwrap_or_else(|e| panic!("{e}"))
            .encode(),
        (),
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn rect(p: &mut Presenter<()>, test_id: &str) -> (f32, f32, f32, f32) {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    p.rect_of(id).unwrap()
}

fn pixel(p: &mut Presenter<()>, x: f32, y: f32) -> [u8; 3] {
    let c = p.frame().pixel(x as u32, y as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2)
}

#[test]
fn the_active_look_is_the_press_and_the_hover_look_shows_beneath_it() {
    let mut p = boot();
    let b = rect(&mut p, "card");
    let ground = (b.0 + b.2 - 20., b.1 + 30.);
    let border = (b.0 + b.2 / 2., b.1 + 3.);
    assert_eq!(pixel(&mut p, ground.0, ground.1), [0, 255, 0]);
    assert_eq!(pixel(&mut p, border.0, border.1), [0, 0, 0]);
    // A mouse over it: the hover look at once, yellow at 0.8 over the white page.
    p.pointer_move(ground.0, ground.1, 0.).unwrap();
    assert!(
        near(pixel(&mut p, ground.0, ground.1), [255, 255, 51]),
        "{:?}",
        pixel(&mut p, ground.0, ground.1)
    );
    assert!(
        near(pixel(&mut p, border.0, border.1), [255, 51, 51]),
        "{:?}",
        pixel(&mut p, border.0, border.1)
    );
    // Pressed: eased in, then blue at 0.5 (not 0.5 of 0.8), the hover's border beneath.
    assert!(p.pointer_down(ground.0, ground.1, 0.).unwrap());
    p.clock(30.);
    let part = pixel(&mut p, ground.0, ground.1);
    assert!(part[0] < 250 && part[0] > 127, "eased in: {part:?}");
    p.clock(120.);
    assert!(
        near(pixel(&mut p, ground.0, ground.1), [127, 127, 255]),
        "{:?}",
        pixel(&mut p, ground.0, ground.1)
    );
    assert!(
        near(pixel(&mut p, border.0, border.1), [255, 127, 127]),
        "{:?}",
        pixel(&mut p, border.0, border.1)
    );
    // Lifted: the hover again, eased back.
    p.pointer_up(ground.0, ground.1, 120.).unwrap();
    p.clock(240.);
    assert!(
        near(pixel(&mut p, ground.0, ground.1), [255, 255, 51]),
        "{:?}",
        pixel(&mut p, ground.0, ground.1)
    );
    // Over its child still hovers it; away, its own again.
    let c = rect(&mut p, "child");
    p.pointer_move(c.0 + 1., c.1 + 1., 250.).unwrap();
    assert!(near(pixel(&mut p, ground.0, ground.1), [255, 255, 51]));
    p.pointer_move(390., 390., 260.).unwrap();
    assert_eq!(pixel(&mut p, ground.0, ground.1), [0, 255, 0]);
    assert_eq!(pixel(&mut p, border.0, border.1), [0, 0, 0]);
    assert!(!p.host.motion());
}

#[test]
fn a_disabled_node_takes_no_look_and_a_finger_hovers_none() {
    let mut p = boot();
    let off = rect(&mut p, "off");
    let at = (off.0 + 20., off.1 + 20.);
    p.pointer_move(at.0, at.1, 0.).unwrap();
    assert_eq!(pixel(&mut p, at.0, at.1), [0, 255, 0]);
    let b = rect(&mut p, "card");
    p.pointer_move(390., 390., 1.).unwrap();
    p.set_pointer_kind(PointerKind::Touch);
    p.pointer_down(b.0 + 20., b.1 + 30., 2.).unwrap();
    p.pointer_up(b.0 + 20., b.1 + 30., 3.).unwrap();
    p.clock(200.);
    assert_eq!(
        pixel(&mut p, b.0 + b.2 - 20., b.1 + 30.),
        [0, 255, 0],
        "no hover left behind"
    );
}

/// A look's opacity is the host's, read at every frame, never kept: a
/// transition that starts while a look holds leaves the node's own opacity
/// when the look goes. The agent's hover shows the look too.
#[test]
fn a_looks_opacity_is_never_kept() {
    let mut p = boot();
    let b = rect(&mut p, "card");
    let ground = (b.0 + b.2 - 20., b.1 + 30.);
    let k = p.host().kernel();
    let card = k.node_by_key(k.find_by_test_id("card")[0]).unwrap().id;
    p.hover(card).unwrap();
    assert!(
        near(pixel(&mut p, ground.0, ground.1), [255, 255, 51]),
        "{:?}",
        pixel(&mut p, ground.0, ground.1)
    );
    // Two taps start a scale transition under the hover (the engine's cache takes the node then).
    for t in [0., 200.] {
        p.pointer_down(ground.0, ground.1, t).unwrap();
        p.pointer_up(ground.0, ground.1, t + 10.).unwrap();
        p.clock(t + 150.);
    }
    p.pointer_move(390., 390., 400.).unwrap();
    p.clock(700.);
    assert_eq!(
        pixel(&mut p, ground.0, ground.1),
        [0, 255, 0],
        "its own opacity, 1, again"
    );
}

#[test]
fn a_step_equal_to_hover_is_kept_when_the_pointer_leaves_between_steps() {
    let plan = crate::test_plan(
        r##"keyframes blink
  from opacity=1
  to opacity=0
component App
  view
    box width=80 height=40 background-color="#ffffff"
      box testId="caret" width=20 height=20 background-color="#000000" animation="blink 1s steps(2) infinite"
        :hover
          opacity = 0.5
"##,
    );
    let (mut p, error) = Presenter::boot_with(
        &plan,
        (),
        (80., 40.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    assert_eq!(pixel(&mut p, 10., 10.), [0, 0, 0]);
    p.pointer_move(10., 10., 0.).unwrap();
    assert!(near(pixel(&mut p, 10., 10.), [128, 128, 128]));
    p.tick(500.);
    p.frame();
    p.pointer_move(60., 30., 750.).unwrap();
    // No animation tick on pointer leave: the held step must already be kept.
    assert!(near(pixel(&mut p, 10., 10.), [128, 128, 128]));
}

/// A row a look clears (`unset`) is CSS's initial value: a transparent
/// background shows the page.
#[test]
fn a_looks_clear_is_the_initial_value() {
    let mut p = boot();
    let c = rect(&mut p, "clears");
    let at = (c.0 + 20., c.1 + 20.);
    assert_eq!(pixel(&mut p, at.0, at.1), [0, 255, 0]);
    p.pointer_move(at.0, at.1, 0.).unwrap();
    assert_eq!(pixel(&mut p, at.0, at.1), [255, 255, 255]);
}

/// A `:focus-visible` look shows where the focus's ring would: not after a
/// press, and once a key is pressed.
#[test]
fn a_focus_look_shows_with_the_ring() {
    let mut p = boot();
    let f = rect(&mut p, "focused");
    let at = (f.0 + 20., f.1 + 20.);
    p.pointer_down(at.0, at.1, 0.).unwrap();
    p.pointer_up(at.0, at.1, 10.).unwrap();
    p.pointer_move(390., 390., 20.).unwrap();
    p.clock(200.);
    let pressed = pixel(&mut p, at.0, at.1);
    assert_ne!(pressed, [255, 0, 255], "a press's focus shows neither");
    p.key_down("ArrowDown", 210.);
    p.clock(400.);
    assert_eq!(
        pixel(&mut p, at.0, at.1),
        [255, 0, 255],
        "a key shows the ring, and the look"
    );
}
