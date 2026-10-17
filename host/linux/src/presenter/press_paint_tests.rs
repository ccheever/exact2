//! LLP 1061 D7 on the Linux host (and so on Android's and Windows's, which
//! paint through this presenter): a contact holding a node shows its
//! pressed paint, eased on the press's curve, and lets it go when it leaves
//! or lifts; the press colour reaches the text that takes the node's colour,
//! not one with its own.
use super::*;

const APP: &str = r##"component App
  state presses = 0
  action go
    presses = presses + 1
  view
    column width=400 height=400 background-color="#ffffff"
      box press=go -exact-press-opacity=0.5 -exact-press-background-color="#0000ff" -exact-press-color="#ff0000" background-color="#00ff00" color="#000000" width=300 height=120 testId="painted"
        text "IIIIIIII" font-size=40 testId="inherits"
        text "IIIIIIII" font-size=40 color="#000000" testId="keeps"
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

/// Whether any pixel of the box `r` is the press colour's red, over the
/// pressed ground at half opacity.
fn reddened(p: &mut Presenter<()>, r: (f32, f32, f32, f32)) -> bool {
    let frame = p.frame();
    (r.1 as u32..(r.1 + r.3) as u32).any(|y| {
        (r.0 as u32..(r.0 + r.2) as u32).any(|x| {
            let c = frame.pixel(x, y).unwrap().demultiply();
            c.red() > 200 && c.green() < 160 && c.blue() < 200
        })
    })
}

#[test]
fn a_held_contact_shows_the_pressed_paint_and_lets_it_go() {
    let mut p = boot();
    let b = rect(&mut p, "painted");
    let (inherits, keeps) = (rect(&mut p, "inherits"), rect(&mut p, "keeps"));
    // A point of the box's own ground, right of the texts.
    let ground = (b.0 + b.2 - 4., b.1 + 4.);
    assert_eq!(pixel(&mut p, ground.0, ground.1), [0, 255, 0]);
    assert!(!reddened(&mut p, inherits));

    assert!(p.pointer_down(ground.0, ground.1, 0.).unwrap());
    p.clock(30.);
    let part = pixel(&mut p, ground.0, ground.1);
    assert!(part[2] > 0 && part[2] < 255, "eased in: {part:?}");
    p.clock(120.);
    // Blue at half opacity over the white page.
    let held = pixel(&mut p, ground.0, ground.1);
    for (c, want) in held.into_iter().zip([127, 127, 255]) {
        assert!(c.abs_diff(want) <= 1, "{held:?}");
    }
    assert!(
        reddened(&mut p, inherits),
        "the text takes the press colour"
    );
    assert!(
        !reddened(&mut p, keeps),
        "a text with its own colour keeps it"
    );

    // Off the box it goes; back inside it shows again; the lift lets it go.
    p.pointer_move(b.0 + b.2 + 20., ground.1, 120.).unwrap();
    p.clock(240.);
    assert_eq!(pixel(&mut p, ground.0, ground.1), [0, 255, 0]);
    p.pointer_move(ground.0, ground.1, 240.).unwrap();
    p.clock(360.);
    assert_eq!(pixel(&mut p, ground.0, ground.1), held);
    p.pointer_up(ground.0, ground.1, 360.).unwrap();
    p.clock(480.);
    assert_eq!(pixel(&mut p, ground.0, ground.1), [0, 255, 0]);
    assert!(!reddened(&mut p, inherits));
    assert!(!p.host.motion(), "the paint needs no more frames");
}
