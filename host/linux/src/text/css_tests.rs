//! @ref LLP 1053 §0 G4, G5 — CSS white space, `nowrap`, `pre-line`, `text-overflow`,
//! tabular figures and inline backgrounds, with the browser as the oracle.
use super::*;
use exact_kernel::{StyleProps, WhiteSpace};

fn engine(font: &'static [u8], family: &str) -> TextEngine {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(font.to_vec())));
    db.set_sans_serif_family(family);
    TextEngine::with_catalog(catalog::Catalog::with_fonts(
        FontSystem::new_with_locale_and_db("en-US".into(), db),
    ))
}
const INTER: &[u8] = include_bytes!("../../../../vendor/cosmic-text/fonts/Inter-Regular.ttf");
const DEJAVU: &[u8] = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");

fn spec(text: &str, white_space: WhiteSpace) -> Spec {
    crate::paint::text_spec(
        &StyleProps {
            font_size: 16.0,
            white_space,
            ..Default::default()
        },
        text,
    )
}

#[test]
fn normal_and_nowrap_collapse_as_the_browser_renders() {
    let mut e = engine(INTER, "Inter");
    let clean = e.measure(&spec("a b c", WhiteSpace::Normal), AxisOffer::MaxContent);
    for source in ["  a \n\t b\r\nc  ", "a  b   c", "a\tb\nc"] {
        for ws in [WhiteSpace::Normal, WhiteSpace::Nowrap] {
            let s = spec(source, ws);
            assert_eq!(s.runs[0].text, "a b c", "{source:?}");
            assert_eq!(
                e.measure(&s, AxisOffer::MaxContent),
                clean,
                "{source:?} {ws:?}"
            );
        }
    }
    // pre-wrap keeps every character; NBSP is never collapsible.
    assert_eq!(spec("  a \n", WhiteSpace::PreWrap).runs[0].text, "  a \n");
    assert_eq!(
        spec("a\u{a0}\u{a0}b", WhiteSpace::Normal).runs[0].text,
        "a\u{a0}\u{a0}b"
    );
    // Only white space is empty: no line box, as a `<div> </div>`.
    assert_eq!(
        e.measure(&spec(" \n\t ", WhiteSpace::Normal), AxisOffer::MaxContent),
        TextMetrics::default()
    );
    // Across runs the first space is kept in its own run.
    let mut s = spec("", WhiteSpace::Normal);
    s.runs = ["a ", " b", "  c"]
        .iter()
        .map(|t| {
            Run::from_style(
                t,
                exact_kernel::TextStyle::from_style(&StyleProps::default()),
            )
        })
        .collect();
    let s = s.collapse_white_space();
    assert_eq!(
        s.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a ", "b", " c"]
    );
}

#[test]
fn nowrap_min_content_is_max_content_and_no_soft_break_is_taken() {
    let mut e = engine(INTER, "Inter");
    let text = "a nowrap label that is much wider than its box";
    let nowrap = spec(text, WhiteSpace::Nowrap);
    let min = e.measure(&nowrap, AxisOffer::MinContent);
    let max = e.measure(&nowrap, AxisOffer::MaxContent);
    assert_eq!(min, max);
    let narrow = e.measure(&nowrap, AxisOffer::Definite(60.0));
    assert_eq!(narrow.height, max.height, "one line at any width");
    assert!(narrow.width > 60.0, "the line overflows");
    let normal = spec(text, WhiteSpace::Normal);
    assert!(e.measure(&normal, AxisOffer::MinContent).width < min.width);
    assert!(e.measure(&normal, AxisOffer::Definite(60.0)).height > max.height);
}

#[test]
fn pre_line_keeps_line_feeds_wraps_and_sizes_by_forced_lines() {
    // Chrome 154's innerText for each source under `white-space: pre-line`.
    for (source, rendered) in [
        ("a    b", "a b"),
        ("a  \n  b", "a\nb"),
        ("a\n\nb", "a\n\nb"),
        ("\nfirst", "\nfirst"),
        ("  lead\n  mid  \ntrail  ", "lead\nmid\ntrail"),
        ("a\r\nb", "a\nb"),
        ("a\tb\t\nc", "a b\nc"),
    ] {
        assert_eq!(spec(source, WhiteSpace::PreLine).runs[0].text, rendered);
    }
    let mut e = engine(INTER, "Inter");
    let one = |e: &mut TextEngine, t: &str| {
        e.measure(&spec(t, WhiteSpace::Normal), AxisOffer::MaxContent)
    };
    let line = one(&mut e, "one two three");
    let s = spec("one two three  \n  four", WhiteSpace::PreLine);
    let max = e.measure(&s, AxisOffer::MaxContent);
    assert_eq!(
        max.width, line.width,
        "max-content is the longest forced line"
    );
    // Lines as the collapsed text's forced breaks, preserved, give them.
    let pre = |e: &mut TextEngine, t: &str, w| e.measure(&spec(t, WhiteSpace::PreWrap), w);
    assert_eq!(
        max,
        pre(&mut e, "one two three\nfour", AxisOffer::MaxContent)
    );
    let min = e.measure(&s, AxisOffer::MinContent);
    assert_eq!(
        min.width,
        one(&mut e, "three").width,
        "min-content the longest word"
    );
    let offer = one(&mut e, "one two").width + 1.0;
    let narrow = e.measure(&s, AxisOffer::Definite(offer));
    assert_eq!(
        narrow,
        pre(&mut e, "one two\nthree\nfour", AxisOffer::MaxContent),
        "and it wraps"
    );
    // A blank line keeps its line box, as in the browser.
    let blank = e.measure(&spec("a\n\nb", WhiteSpace::PreLine), AxisOffer::MaxContent);
    assert_eq!(blank, pre(&mut e, "a\n\nb", AxisOffer::MaxContent));
    assert!(blank.height > 2.5 * line.height);
}

#[test]
fn ellipsis_ends_an_over_wide_nowrap_line_in_paint_only() {
    let mut e = engine(INTER, "Inter");
    let s = spec(
        "a nowrap label that is much wider than its box",
        WhiteSpace::Nowrap,
    );
    let p = e.paragraph(&s, Some(100.0));
    assert!(p.width > 100.0);
    let shown = p.ellipsized(100.0).expect("over-wide");
    assert!(shown.width <= 100.0 + 0.5, "{}", shown.width);
    assert_eq!(shown.height, p.height, "paint keeps the measured line box");
    assert!(
        Rc::ptr_eq(&shown, &p.ellipsized(100.0).unwrap()),
        "kept per width"
    );
    // A line that fits is painted as measured.
    assert!(e
        .paragraph(&spec("fits", WhiteSpace::Nowrap), Some(100.0))
        .ellipsized(100.0)
        .is_none());
    let visible: String = shown
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
        .map(|g| g.to_string())
        .collect();
    let plain: String = p
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
        .map(|g| g.to_string())
        .collect();
    assert_ne!(visible, plain);
}

fn digits(e: &mut TextEngine, text: &str, numeric: u8) -> f32 {
    let mut s = spec(text, WhiteSpace::Normal);
    s.runs[0].font_variant_numeric = numeric;
    e.measure(&s, AxisOffer::MaxContent).width
}

#[test]
fn tabular_nums_is_the_faces_tnum_feature() {
    // Inter: proportional figures by default, `tnum` present.
    let mut e = engine(INTER, "Inter");
    assert!(digits(&mut e, "111", 0) < digits(&mut e, "888", 0));
    assert_eq!(digits(&mut e, "111", 1), digits(&mut e, "888", 1));
    // DejaVu Sans: already tabular figures and no `tnum`: nothing changes.
    let mut e = engine(DEJAVU, "DejaVu Sans");
    assert_eq!(digits(&mut e, "111", 0), digits(&mut e, "111", 1));
    assert_eq!(digits(&mut e, "111", 1), digits(&mut e, "888", 1));
}

#[test]
fn tabular_nums_is_a_distinct_shape_identity() {
    let mut e = engine(INTER, "Inter");
    let a = spec("1111", WhiteSpace::Normal);
    let mut b = a.clone();
    b.runs[0].font_variant_numeric = 1;
    let pa = e.paragraph(&a, None);
    let pb = e.paragraph(&b, None);
    assert!(!Rc::ptr_eq(&pa, &pb));
    assert_ne!(pa.width, pb.width);
}

#[test]
fn an_inline_background_covers_each_line_fragment_of_its_run() {
    let mut e = engine(INTER, "Inter");
    let mut s = spec("", WhiteSpace::Normal);
    let style = exact_kernel::TextStyle::from_style(&StyleProps::default());
    s.runs = ["plain ", "list_viewport code run", " after"]
        .iter()
        .map(|t| Run::from_style(t, style))
        .collect();
    let p = e.paragraph(&s, Some(150.0));
    assert!(p.layout_runs().count() > 1, "the code run wraps");
    let gray = [242, 242, 247, 255];
    let rects = p.run_backgrounds(&[None, Some(gray), None]);
    assert_eq!(rects.len(), 2, "one rectangle per line fragment: {rects:?}");
    let (ascent, descent, _) = p.source.data.run_metrics[1];
    for ((x, y, w, h), color) in &rects {
        assert_eq!(*color, gray);
        assert!(*w > 0.0 && *x >= 0.0 && x + w <= 150.5);
        assert!(
            (h - (ascent + descent)).abs() < 0.01,
            "the run font's content area"
        );
        assert!(*y >= -0.01);
    }
    assert!(
        rects[1].0 .1 > rects[0].0 .1,
        "second fragment on the next line"
    );
    assert!(rects[0].0 .0 > 1.0, "starts after the plain run");
    assert!(p.run_backgrounds(&[None, None, None]).is_empty());
}

#[test]
fn document_language_replaces_the_shaping_catalog_and_cached_paragraphs() {
    use exact_kernel::TextMeasurer;
    let shared = TextEngine::shared();
    let mut measurer = super::Measurer(shared.clone());
    let old = shared.borrow().catalog.clone();
    measurer.set_language("ar");
    let engine = shared.borrow();
    assert_eq!(engine.catalog.borrow().fonts.locale(), "ar");
    assert!(!std::rc::Rc::ptr_eq(&old, &engine.catalog));
    drop(engine);
    measurer.set_language("en");
    assert_eq!(shared.borrow().catalog.borrow().fonts.locale(), "en");
}
