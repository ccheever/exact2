//! LLP 1034 §8 on Linux: a `light-dark()` colour under a `color-scheme`
//! subtree resolves in that scheme while the app's stays light, and a
//! nested `light` wins below it; a colour moving under the subtree moves in
//! its scheme too.

use crate::borders::NoData;
use crate::pin_font;
use exact_linux::Presenter;
use std::path::PathBuf;

const APP: &str = r##"component App
  state on = false
  action toggle
    on = not on
  view
    column width=200 height=300 background-color="light-dark(#ffffff, #000000)"
      button testId="toggle" press=toggle width=40 height=20
      column testId="sheet" color-scheme="dark" width=200 height=100 background-color="light-dark(#ffffff, #000000)"
        box testId="leaf" width=50 height=50 background-color=(on ? "light-dark(#00ff00, #0000ff)" : "light-dark(#ffffff, #ff0000)") transition="background-color 1s linear"
        column color-scheme="light" width=50 height=40 background-color="light-dark(#00ff00, #ff00ff)"
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

fn pixel(p: &mut Presenter<NoData>, x: u32, y: u32) -> [u8; 4] {
    let c = p.frame().pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn a_subtree_paints_in_its_color_scheme() {
    let mut p = boot();
    // The page: light. The sheet (y 20..120): dark. Its leaf (0..50 x
    // 20..70): dark's red. The nested light column (y 70..110): light's green.
    assert_eq!(
        pixel(&mut p, 150, 200),
        [255, 255, 255, 255],
        "the page, light"
    );
    assert_eq!(pixel(&mut p, 150, 30), [0, 0, 0, 255], "the sheet, dark");
    assert_eq!(pixel(&mut p, 10, 30), [255, 0, 0, 255], "its leaf, dark");
    assert_eq!(
        pixel(&mut p, 10, 80),
        [0, 255, 0, 255],
        "a nested light subtree"
    );
    let k = p.host().kernel();
    let toggle = k.node_by_key(k.find_by_test_id("toggle")[0]).unwrap().id;
    let _ = p.tap(toggle).unwrap();
    p.tick(500.0);
    // Red to blue (the dark halves), halfway; never towards the light halves.
    let [r, g, b, _] = pixel(&mut p, 10, 30);
    assert!(
        r > 100 && b > 100 && g < 20,
        "moving in its scheme: {r} {g} {b}"
    );
    p.tick(1000.0);
    assert_eq!(
        pixel(&mut p, 10, 30),
        [0, 0, 255, 255],
        "arrived at dark's blue"
    );
}
