//! LLP 1121 D8 on the Linux host (and so on Android's, which paints what
//! this records): `:active` is the press paint, eased on the press's curve,
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
      box press=go background-color="#00ff00" border="8px solid #000000" width=300 height=120 testId="card"
        :active
          background-color = "#0000ff"
          opacity = 0.5
        :hover
          background-color = "#ffff00"
          border-color = "#ff0000"
          opacity = 0.8
        text "c" testId="child"
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
