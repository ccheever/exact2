//! The text engine's font matching (LLP 1015 §3): a weight the family lacks
//! resolves to the family's nearest face — never to another family that
//! happens to cover the weight — the browser's rule (family first, then
//! weight), measured against the pinned font's own advances so the numbers
//! are the same on every machine.

use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

/// The pinned font (LLP 1015 §3), set once before the first engine is made.
fn pin_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var(
            "EXACT_FONTS",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/fixtures/fonts"),
        );
        std::env::set_var("EXACT_FONT", "DejaVu Sans");
    });
}

/// One string at four weights, each text shrink-wrapped to its advance.
const WEIGHTS: &str = "component Weights
  view
    column gap=4 padding=10 align-items=\"flex-start\"
      text \"Change station\" font-size=13 font-weight=400 testId=\"w400\"
      text \"Change station\" font-size=13 font-weight=500 testId=\"w500\"
      text \"Change station\" font-size=13 font-weight=600 testId=\"w600\"
      text \"Change station\" font-size=13 font-weight=700 testId=\"w700\"
";

fn width(p: &mut Presenter<NoData>, test_id: &str) -> f32 {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    p.boxes().iter().find(|b| b.id == id).unwrap().rect.2
}

#[test]
fn a_weight_the_family_lacks_resolves_within_the_family() {
    pin_font();
    let plan = contract::compile(WEIGHTS).unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 844.0),
        1.0,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let (w400, w500, w600, w700) = (
        width(&mut p, "w400"),
        width(&mut p, "w500"),
        width(&mut p, "w600"),
        width(&mut p, "w700"),
    );
    // DejaVu Sans Book sets "Change station" at 13 pt 98.6 wide and Bold
    // 111.1 — the fonts' own advances (fonttools over the fixture files).
    assert!((w400 - 98.6).abs() < 1.5, "Book at 400: {w400}");
    assert!((w700 - 111.1).abs() < 1.5, "Bold at 700: {w700}");
    // 500 has no face: CSS takes the nearest below, Book; 600 takes the
    // nearest above, Bold. Before the snap a Mac set both in San Francisco
    // (85 and 87.5 wide), whose variable weight axis covers them.
    assert_eq!(w500, w400, "500 is Book");
    assert_eq!(w600, w700, "600 is Bold");
}
