//! A shape's storage is bounded by its text (LLP 1085.000 §2 P3, the storage
//! spike: cosmic-text held 246–300 bytes per byte of text at one width,
//! Parley's shaping arrays 38–52). These replace the tests that pinned
//! cosmic-text's span reservation: the property is the same, bounded bytes
//! per paragraph, measured on Parley's structures.
use super::*;

const FONT: &[u8] = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");

fn engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(&[FONT], "DejaVu Sans"))
}

fn shaped(engine: &TextEngine, text: &str) -> Rc<ShapedSource> {
    let spec = crate::paint::text_spec(&exact_kernel::StyleProps::default(), text);
    Rc::new(ShapedSource::new(engine.catalog.clone(), Arc::new(spec)))
}

/// Bytes the shape keeps per byte of its text.
fn per_byte(source: &ShapedSource, text: &str) -> f64 {
    source.accessible_capacity_bytes as f64 / text.len().max(1) as f64
}

#[test]
fn a_long_paragraphs_shape_is_bounded_per_byte_of_text() {
    let engine = engine();
    for seed in ["office words 123 ", "café e\u{301} ", "a שלום b العربية "] {
        let text = seed.repeat(65536usize.div_ceil(seed.len()));
        assert!(text.len() >= 65536);
        let source = shaped(&engine, &text);
        let bytes = per_byte(&source, &text);
        eprintln!("{seed:?}: {bytes:.1} bytes per text byte");
        assert!(bytes <= 80.0, "{seed:?}: {bytes:.1} bytes per text byte");
    }
}

#[test]
fn widths_add_lines_not_a_second_shape() {
    let mut engine = engine();
    let spec = crate::paint::text_spec(
        &exact_kernel::StyleProps::default(),
        &"office words 123 ".repeat(400),
    );
    let a = engine.paragraph(&spec, Some(300.));
    let shape = a.source.accessible_capacity_bytes;
    let mut widths = vec![a];
    for width in [200., 500., 800.] {
        let p = engine.paragraph(&spec, Some(width));
        assert!(Rc::ptr_eq(&p.source, &widths[0].source));
        // Each width keeps its own lines and glyphs, far less than a shape.
        let own = p.owned_capacity_bytes() - shape;
        let glyphs = p.lines().glyphs.len();
        assert!(own <= glyphs * 48 + 4096, "{own} bytes for {glyphs} glyphs");
        widths.push(p);
    }
}

#[test]
fn empty_and_ordinary_lines_keep_little() {
    let engine = engine();
    for text in [
        "",
        "office 123",
        "café e\u{301}",
        "a\t b\0c",
        "שלום עולם",
        "a\u{2067}שלום\u{2069} z",
    ] {
        let source = shaped(&engine, text);
        assert!(
            source.accessible_capacity_bytes <= 16 * 1024,
            "{text:?}: {}",
            source.accessible_capacity_bytes
        );
    }
}

#[test]
fn a_giant_paragraph_gives_back_the_engines_scratch() {
    let mut engine = engine();
    let giant = "office words 123 ".repeat(20_000);
    let small = "after the giant";
    let before = engine.paragraph(
        &crate::paint::text_spec(&exact_kernel::StyleProps::default(), small),
        Some(200.),
    );
    let before: Vec<_> = before
        .lines()
        .glyphs
        .iter()
        .map(|g| (g.glyph_id, g.x.to_bits()))
        .collect();
    let _ = shaped(&engine, &giant);
    // The scratch was dropped and remade: shaping goes on unchanged.
    let fresh = Rc::new(ShapedSource::new(
        engine.catalog.clone(),
        Arc::new(crate::paint::text_spec(
            &exact_kernel::StyleProps::default(),
            small,
        )),
    ));
    let after: Vec<_> = fresh
        .layout(Some(200.))
        .lines()
        .glyphs
        .iter()
        .map(|g| (g.glyph_id, g.x.to_bits()))
        .collect();
    assert_eq!(before, after);
}

/// A width only measured keeps its scalars, not its lines (the storage
/// spike's arrangement (c)); its first paint breaks the shared shape again
/// and keeps the lines (b), the same ones the measure laid out.
#[test]
fn a_measured_width_keeps_no_lines_until_it_is_painted() {
    let mut engine = engine();
    let spec = crate::paint::text_spec(
        &exact_kernel::StyleProps::default(),
        &"office words 123 ".repeat(200),
    );
    let metrics = engine.measure(&spec, AxisOffer::Definite(300.));
    let p = engine.paragraph(&spec, Some(300.));
    assert_eq!(p.lines_capacity_bytes(), 0, "measured only: no lines kept");
    let before = engine.residency().owned_capacity_bytes;
    let painted = p.lines().glyphs.len();
    assert!(painted > 1000);
    let lines = p.lines_capacity_bytes();
    assert!(lines > 0);
    assert_eq!(engine.residency().owned_capacity_bytes, before + lines);
    // What was measured is what is painted.
    assert_eq!(paragraph_metrics(&p), metrics);
    let mut fresh = engine_fresh();
    let eager = fresh.layout(&spec, Some(300.));
    let glyphs = |p: &Paragraph| -> Vec<_> {
        p.lines()
            .glyphs
            .iter()
            .map(|g| (g.glyph_id, g.x.to_bits(), g.start))
            .collect()
    };
    assert_eq!(glyphs(&p), glyphs(&eager));
}

fn engine_fresh() -> TextEngine {
    engine()
}
