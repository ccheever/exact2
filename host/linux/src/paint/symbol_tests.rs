//! A symbol role in the platform's shape (LLP 1115.001 D14): `back` is
//! Windows' and Android's arrow and `more` their bare dots, where the Linux
//! desktop draws the web's chevron and circled ellipsis.
use crate::presenter::{PainterChoice, Presenter};
use exact_kernel::SymbolSet;
use std::path::PathBuf;

const APP: &str = r##"component App
  view
    row width=300 height=100 background-color="#ffffff" color="#000000"
      image "symbol:back" width=96 height=96 font-size=96 testId="back"
      image "symbol:more" width=96 height=96 font-size=96 testId="more"
      image "symbol:share" width=96 height=96 font-size=96 testId="share"
"##;

/// A presenter whose painter draws `set`'s shapes from its first frame.
fn boot(set: SymbolSet) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (300., 100.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.brush.symbols = set;
    p
}

/// Whether the symbol `test_id` inks the point (`u`, `v`) of its 24-unit box.
fn inked(p: &mut Presenter<()>, test_id: &str, (u, v): (f32, f32)) -> bool {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    let (x, y, w, h) = p.rect_of(id).unwrap();
    let (px, py) = (x + u / 24. * w, y + v / 24. * h);
    let c = p.frame().pixel(px as u32, py as u32).unwrap().demultiply();
    c.red() < 128
}

#[test]
fn windows_and_android_draw_their_own_back_and_more() {
    // The table's: a chevron's point, no shaft; a ring round three dots.
    let mut web = boot(SymbolSet::Web);
    assert!(inked(&mut web, "back", (8., 12.)) && !inked(&mut web, "back", (18., 12.)));
    assert!(inked(&mut web, "more", (12., 3.)) && !inked(&mut web, "more", (6., 12.)));
    // Fluent: an arrow's shaft; three dots in a row with no ring.
    let mut fluent = boot(SymbolSet::Fluent);
    assert!(inked(&mut fluent, "back", (18., 12.)) && inked(&mut fluent, "back", (5., 12.)));
    assert!(inked(&mut fluent, "back", (7.5, 8.5)) && !inked(&mut fluent, "back", (12., 8.5)));
    assert!(inked(&mut fluent, "more", (6., 12.)) && inked(&mut fluent, "more", (18., 12.)));
    assert!(!inked(&mut fluent, "more", (12., 3.)) && !inked(&mut fluent, "more", (12., 6.)));
    // Material: an arrow; three dots in a column.
    let mut material = boot(SymbolSet::Material);
    assert!(inked(&mut material, "back", (18., 12.)) && inked(&mut material, "back", (8., 8.)));
    assert!(inked(&mut material, "more", (12., 6.)) && inked(&mut material, "more", (12., 18.)));
    assert!(!inked(&mut material, "more", (6., 12.)) && !inked(&mut material, "more", (12., 3.)));
    // Material's share: three filled discs, linked, open between the links.
    for disc in [(6., 12.), (18., 5.), (18., 19.)] {
        assert!(inked(&mut material, "share", disc), "{disc:?}");
    }
    assert!(!inked(&mut material, "share", (14., 12.)));
}
