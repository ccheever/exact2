//! The text engine's font matching (LLP 1015 §3): a weight the family lacks
//! resolves to the family's nearest face — never to another family that
//! happens to cover the weight — the browser's rule (family first, then
//! weight), measured against the pinned font's own advances so the numbers
//! are the same on every machine.

use crate::pin_font;
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
fn agent_layout_adds_fragment_fields_only_for_flowed_paragraphs() {
    // @ref LLP 1043.000 §3 D7 — ordinary layout replies keep their old shape.
    pin_font();
    for wrap in ["auto", "both"] {
        let source = format!(
            "component App\n  view\n    view width=300 height=200\n      text \"one two three four five six seven eight nine ten\" width=300 height=200 testId=\"para\"\n      view position=\"absolute\" left=100 top=0 width=80 height=60 wrap-flow=\"{wrap}\"\n"
        );
        let plan = contract::compile(&source).unwrap();
        let (mut p, error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (300.0, 200.0),
            1.0,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let kernel = p.host().kernel();
        let id = kernel
            .node_by_key(kernel.find_by_test_id("para")[0])
            .unwrap()
            .id;
        let reply: serde_json::Value =
            serde_json::from_str(&p.layout_json(Some(id), false)).unwrap();
        let node = &reply["node"];
        assert_eq!(node["id"], id);
        if wrap == "auto" {
            assert!(node.get("fragments").is_none(), "{reply}");
            assert!(node.get("flow").is_none(), "{reply}");
            assert!(node.get("flow_shapes").is_none(), "{reply}");
            assert!(node.get("flow_skipped").is_none(), "{reply}");
        } else {
            assert!(!node["fragments"].as_array().unwrap().is_empty(), "{reply}");
            assert!(
                !node["flow_shapes"].as_array().unwrap().is_empty(),
                "{reply}"
            );
        }
    }
}

#[test]
fn emergency_wrapping_preserves_the_two_intrinsic_width_rules() {
    use exact_kernel::{AxisOffer, OverflowWrap, StyleProps};
    pin_font();
    let mut engine = exact_linux::text::TextEngine::new();
    let mut spec = exact_linux::paint::text_spec(&StyleProps::default(), &"W".repeat(30));
    let normal = engine.measure(&spec, AxisOffer::Definite(80.0));
    let minimum = engine.measure(&spec, AxisOffer::MinContent).width;
    assert!(normal.width > 80.0);
    spec.overflow_wrap = OverflowWrap::BreakWord;
    let broken = engine.measure(&spec, AxisOffer::Definite(80.0));
    assert!(broken.height > normal.height);
    assert!(broken.width <= 80.0);
    assert_eq!(engine.measure(&spec, AxisOffer::MinContent).width, minimum);
    spec.overflow_wrap = OverflowWrap::Anywhere;
    assert_eq!(engine.measure(&spec, AxisOffer::Definite(80.0)), broken);
    assert!(engine.measure(&spec, AxisOffer::MinContent).width < minimum / 10.0);
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
                strut: exact_linux::text::Run::from_style(
                    "",
                    exact_kernel::TextStyle::from_style(&exact_kernel::StyleProps::default()),
                ),
                runs: vec![Run {
                    text: "Change station".into(),
                    size: 13.0,
                    weight,
                    family: 8,
                    italic: false,
                    line_height: None,
                    letter_spacing: 0.0,
                }],
                align: TextAlign::Left,
                line_clamp: 0,
                overflow_wrap: exact_kernel::OverflowWrap::Normal,
                white_space: exact_kernel::WhiteSpace::Normal,
                direction: exact_kernel::Direction::Ltr,
            },
            None,
        );
        let shaped = paragraph
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

#[test]
fn line_height_resolves_zero_and_paragraph_minimum_in_the_painted_cache() {
    use exact_kernel::{AxisOffer, LineHeight, StyleProps};
    pin_font();
    let mut engine = exact_linux::text::TextEngine::new();
    for (line_height, height) in [
        (LineHeight::Number(1.5), 24.0),
        (LineHeight::Length(25.25), 25.25),
        (LineHeight::Number(0.0), 0.0),
    ] {
        let style = StyleProps {
            font_size: 16.0,
            line_height,
            ..StyleProps::default()
        };
        let mut spec = exact_linux::paint::text_spec(&style, "Hello");
        let measured = engine.measure(&spec, AxisOffer::MaxContent);
        assert!(
            (measured.height - height).abs() < 0.02,
            "{line_height:?}: {measured:?}"
        );
        assert_eq!(engine.paragraph(&spec, None).height, measured.height);
        if height > 0.0 {
            spec.runs[0].size = 8.0;
            spec.runs[0].line_height = Some(12.0);
            assert!((engine.measure(&spec, AxisOffer::MaxContent).height - height).abs() < 0.02);
        }
    }
}

#[test]
fn normal_paragraph_preserves_fractional_explicit_child_boxes() {
    use exact_kernel::{AxisOffer, StyleProps};
    pin_font();
    let mut engine = exact_linux::text::TextEngine::new();
    let style = StyleProps {
        font_size: 16.0,
        ..StyleProps::default()
    };
    for (text, count) in [("Child", 1.0), ("First\nSecond", 2.0)] {
        let mut spec = exact_linux::paint::text_spec(&style, text);
        spec.runs[0].line_height = Some(60.25);
        let measured = engine.measure(&spec, AxisOffer::MaxContent);
        assert!(
            (measured.height - 60.25 * count).abs() < 0.02,
            "{measured:?}"
        );
        assert_eq!(engine.paragraph(&spec, None).height, measured.height);
    }
    let mut spec = exact_linux::paint::text_spec(&style, "Child");
    let normal = engine.measure(&spec, AxisOffer::MaxContent).height;
    spec.runs[0].line_height = Some(10.0);
    assert_eq!(engine.measure(&spec, AxisOffer::MaxContent).height, normal);
}

#[test]
fn fixed_line_height_keeps_each_inline_fonts_shared_baseline_extents() {
    use exact_kernel::{AxisOffer, LineHeight, StyleProps};
    pin_font();
    let mut engine = exact_linux::text::TextEngine::new();
    let style = StyleProps {
        font_size: 20.0,
        line_height: LineHeight::Length(20.0),
        ..StyleProps::default()
    };
    let mut spec = exact_linux::paint::text_spec(&style, "Larger");
    spec.runs[0].size = 40.0;
    let measured = engine.measure(&spec, AxisOffer::MaxContent);
    // DejaVu's unrounded ascent/descent extrema. Chrome rounds this to 27;
    // baseline metric rounding is the separate LLP 1035.000 D6 investigation.
    assert!((measured.height - 26.923828).abs() < 0.02, "{measured:?}");
    assert_eq!(
        engine.paragraph(&spec, None).baselines()[0],
        measured.first_baseline.unwrap()
    );
}
