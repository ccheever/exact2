//! LLP 1062 on Linux: colour and shadow transitions sampled by the engine and
//! painted over their rows; an appearance change (`setScheme`) re-targets a
//! `light-dark()` colour and transitions it, as Chrome's computed value does.

use crate::borders::NoData;
use crate::pin_font;
use exact_linux::Presenter;
use exact_motion::Property;
use std::path::PathBuf;

const APP: &str = r##"component App
  state on = false
  state dark = false
  action toggle writes on
    on = not on
  action scheme writes dark
    dark = not dark
    setScheme(dark ? "light" : "dark")
  view
    column testId="page" width=200 height=300 background-color="light-dark(#ffffff, #000000)" transition="background-color 1s linear"
      button testId="toggle" press=toggle width=100 height=40 border-width=4 border-style="solid" border-color=(on ? "#00ff00" : "#ff0000") background-color=(on ? "#0000ff00" : "#ff0000") color=(on ? "#ffffff" : "#000000") transition="background-color 1s linear, border-color 1s linear, color 1s linear"
        text "Go" testId="label"
      button testId="scheme" press=scheme width=40 height=20
"##;

fn boot() -> Presenter<NoData> {
    pin_font();
    let plan = contract::compile(APP).unwrap();
    let assets = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/caltrain/assets"
    ));
    let (p, error) = Presenter::boot(&plan.encode(), NoData, (200.0, 300.0), 1.0, assets).unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn view(p: &Presenter<NoData>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn pixel(p: &mut Presenter<NoData>, x: u32, y: u32) -> [u8; 4] {
    let c = p.frame().pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn a_colour_transition_paints_premultiplied_and_hands_its_row_back() {
    let mut p = boot();
    let (toggle, label) = (view(&p, "toggle"), view(&p, "label"));
    let _ = p.tap(toggle).unwrap();
    p.tick(500.0);
    let shown = p.host().presented(toggle).paint;
    // Red to transparent blue, halfway: red fading, never purple (Chrome's
    // premultiplied interpolation), painted over the white page.
    assert_eq!(
        shown.color(Property::BackgroundColor),
        Some([255, 0, 0, 128])
    );
    assert_eq!(pixel(&mut p, 50, 30), [255, 127, 127, 255]);
    assert_eq!(
        pixel(&mut p, 1, 20),
        [128, 128, 0, 255],
        "the border, halfway"
    );
    // The label inherits the button's colour and paints it as it moves.
    assert_eq!(
        p.host().presented(label).paint.color(Property::Color),
        Some([128, 128, 128, 255])
    );
    p.tick(1000.0);
    assert!(
        p.host().presented(toggle).paint.is_empty(),
        "arrived: the rows paint again"
    );
    assert!(p.host().presented(label).paint.is_empty());
    assert_eq!(pixel(&mut p, 1, 20), [0, 255, 0, 255]);
}

#[test]
fn an_appearance_change_transitions_a_light_dark_colour() {
    let mut p = boot();
    let page = view(&p, "page");
    assert_eq!(pixel(&mut p, 150, 250), [255, 255, 255, 255]);
    let _ = p.tap(view(&p, "scheme")).unwrap();
    p.run_commands(|| NoData);
    p.tick(250.0);
    assert_eq!(
        p.host()
            .presented(page)
            .paint
            .color(Property::BackgroundColor),
        Some([191, 191, 191, 255])
    );
    assert_eq!(pixel(&mut p, 150, 250), [191, 191, 191, 255]);
    p.tick(1000.0);
    assert_eq!(pixel(&mut p, 150, 250), [0, 0, 0, 255]);
}
