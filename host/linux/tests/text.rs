//! The text engine's font matching (LLP 1015 §3): a weight the family lacks
//! resolves to the family's nearest face — never to another family that
//! happens to cover the weight — the browser's rule (family first, then
//! weight), measured against the pinned font's own advances so the numbers
//! are the same on every machine.

use exact_kernel::TextAlign;
use exact_linux::presenter::PainterChoice;
use exact_linux::text::{Run, Spec};
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;

struct NoData;
impl DataSource for NoData {
    fn app_id(&self) -> &str {
        "com.example"
    }

    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn a_rejected_reload_keeps_the_running_font_catalog() {
    pin_font();
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
    let plan = contract::compile_path(&assets.join("app.contract")).unwrap();
    let (mut presenter, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 844.0),
        1.0,
        assets,
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let before = presenter
        .text()
        .borrow_mut()
        .declared_face_id(8, 400, false)
        .unwrap();

    let mut refused = contract::compile(WEIGHTS).unwrap();
    refused.app_id = "com.foreign".into();
    assert!(presenter.reload(&refused.encode(), NoData).is_err());
    assert_eq!(
        presenter
            .text()
            .borrow_mut()
            .declared_face_id(8, 400, false),
        Some(before)
    );
}

/// The pinned font (LLP 1015 §3), set once before the first engine is made.
fn pin_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var(
            "EXACT_FONTS",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ),
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

#[test]
fn declared_bytes_are_the_resolved_faces_and_the_painted_geometry() {
    pin_font();
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
    let plan = contract::compile_path(&assets.join("app.contract")).unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 844.0),
        1.0,
        assets,
        PainterChoice::Cpu,
    )
    .unwrap();

    let text = p.text().clone();
    let mut text = text.borrow_mut();
    let book = text.declared_face_id(8, 400, false).unwrap();
    let bold = text.declared_face_id(8, 700, false).unwrap();
    assert_ne!(book, bold);
    assert_eq!(text.resolved_face_id(8, 400, false), Some(book));
    assert_eq!(text.resolved_face_id(8, 500, false), Some(book));
    assert_eq!(text.resolved_face_id(8, 600, false), Some(bold));
    assert_eq!(text.resolved_face_id(8, 700, false), Some(bold));

    for (weight, expected) in [(400, book), (500, book), (600, bold), (700, bold)] {
        let paragraph = text.paragraph(
            &Spec {
                runs: vec![Run {
                    text: "Change station".into(),
                    size: 13.0,
                    weight,
                    family: 8,
                    italic: false,
                    line_height: 0.0,
                    letter_spacing: 0.0,
                }],
                align: TextAlign::Left,
                line_clamp: 0,
            },
            None,
        );
        let shaped = paragraph
            .buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .next()
            .unwrap()
            .font_id;
        assert_eq!(
            shaped, expected,
            "the {weight} run shaped from its declared bytes"
        );
    }
    drop(text);

    let (w400, w600, w700) = (
        width(&mut p, "font-400"),
        width(&mut p, "font-600"),
        width(&mut p, "font-700"),
    );
    assert!((w400 - 98.6).abs() < 1.5, "Book geometry: {w400}");
    assert!((w700 - 111.1).abs() < 1.5, "Bold geometry: {w700}");
    assert_eq!(w600, w700);

    let frame = p.frame();
    for id in ["font-400", "font-600", "font-700"] {
        let kernel = p.host().kernel();
        let node = kernel
            .node_by_key(kernel.find_by_test_id(id)[0])
            .unwrap()
            .id;
        let rect = p.boxes().iter().find(|b| b.id == node).unwrap().rect;
        let mut ink = 0usize;
        for y in rect.1 as u32..(rect.1 + rect.3).ceil() as u32 {
            for x in rect.0 as u32..(rect.0 + rect.2).ceil() as u32 {
                let pixel = frame.pixel(x, y).unwrap().demultiply();
                ink += usize::from(pixel.red() < 128);
            }
        }
        assert!(
            ink > 40,
            "{id} paints glyph geometry from the shaped buffer: {ink} dark pixels"
        );
    }
}
