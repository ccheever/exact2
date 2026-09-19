//! Public cosmic span storage, with optional immutable before/after evidence.
//! No system fonts, private scratch access, timing thresholds or allocator hooks.
use super::*;
use cosmic_text::{AttrsList, Cursor, Hinting, LayoutRun, ShapeBuffer, ShapeLine, ShapeSpan};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::Write as _;
use std::mem::size_of;

const FONT: &[u8] = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");

fn fonts() -> FontSystem {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(FONT.to_vec())));
    db.set_sans_serif_family("DejaVu Sans");
    FontSystem::new_with_locale_and_db("en-US".into(), db)
}

fn attrs() -> AttrsList {
    AttrsList::new(
        &Attrs::new()
            .family(Family::SansSerif)
            .metadata(7)
            .color(cosmic_text::Color::rgb(31, 73, 151))
            .metrics(Metrics::new(16., 22.)),
    )
}

fn shape(fonts: &mut FontSystem, text: &str) -> ShapeLine {
    ShapeLine::new(fonts, text, &attrs(), Shaping::Advanced, 8)
}

// Recreate oversized starting storage through the public API. This control
// changes allocation only; the pre-patch capture below is the independent
// before-data, since both in-process routes link the current cosmic source.
fn reserved_control(fonts: &mut FontSystem, text: &str) -> ShapeLine {
    let mut value = shape(fonts, "");
    value.spans.reserve(text.len() + 1);
    value.build(fonts, text, &attrs(), Shaping::Advanced, 8);
    value
}

fn metrics(value: Option<Metrics>) -> Option<[u32; 2]> {
    value.map(|m| [m.font_size.to_bits(), m.line_height.to_bits()])
}

// Record every public field, and float bits rather than rounded display values.
// Debug below is only for non-float metadata (IDs, flags, colors and ranges).
fn shaped(value: &ShapeLine) -> String {
    let mut out = format!(
        "rtl={} metrics={:?}\n",
        value.rtl,
        metrics(value.metrics_opt)
    );
    for span in &value.spans {
        writeln!(out, "level={}", span.level.number()).unwrap();
        for (range, data) in &span.decoration_spans {
            writeln!(out, "decoration={range:?}:{}", decoration(data)).unwrap();
        }
        for word in &span.words {
            writeln!(out, "blank={}", word.blank).unwrap();
            for g in &word.glyphs {
                writeln!(
                    out,
                    "{:?}:{:?}:{:?}:{:?}",
                    (
                        g.start,
                        g.end,
                        g.font_id,
                        g.font_weight,
                        g.glyph_id,
                        g.color_opt,
                        g.metadata,
                        g.cache_key_flags
                    ),
                    [
                        g.x_advance,
                        g.y_advance,
                        g.x_offset,
                        g.y_offset,
                        g.ascent,
                        g.descent
                    ]
                    .map(f32::to_bits),
                    g.font_monospace_em_width.map(f32::to_bits),
                    metrics(g.metrics_opt)
                )
                .unwrap();
            }
        }
    }
    out
}

fn decoration(data: &cosmic_text::GlyphDecorationData) -> String {
    format!(
        "{:?}:{:?}",
        data.text_decoration,
        [
            data.underline_metrics.offset,
            data.underline_metrics.thickness,
            data.strikethrough_metrics.offset,
            data.strikethrough_metrics.thickness,
            data.ascent
        ]
        .map(f32::to_bits)
    )
}

fn glyph(g: &LayoutGlyph) -> String {
    format!(
        "{:?}:{:?}:{:?}",
        (
            g.start,
            g.end,
            g.font_id,
            g.font_weight,
            g.glyph_id,
            g.level,
            g.color_opt,
            g.metadata,
            g.cache_key_flags
        ),
        [g.font_size, g.x, g.y, g.w, g.x_offset, g.y_offset].map(f32::to_bits),
        g.line_height_opt.map(f32::to_bits)
    )
}

fn layout(value: &ShapeLine, width: Option<f32>) -> String {
    let mut lines = Vec::new();
    value.layout_to_buffer(
        &mut ShapeBuffer::default(),
        16.,
        width,
        Wrap::WordOrGlyph,
        Ellipsize::None,
        None,
        &mut lines,
        None,
        Hinting::Disabled,
    );
    let mut out = String::new();
    for line in lines {
        writeln!(
            out,
            "line={:?}:{:?}",
            [line.w, line.max_ascent, line.max_descent].map(f32::to_bits),
            line.line_height_opt.map(f32::to_bits)
        )
        .unwrap();
        for g in &line.glyphs {
            writeln!(out, "{}", glyph(g)).unwrap();
        }
        for d in &line.decorations {
            writeln!(
                out,
                "decoration={:?}:{:?}:{}:{}",
                d.glyph_range,
                d.color_opt,
                d.font_size.to_bits(),
                decoration(&d.data)
            )
            .unwrap();
        }
    }
    out
}

fn assert_semantics(actual: &ShapeLine, control: &ShapeLine) {
    assert_eq!(shaped(actual), shaped(control));
    for width in [Some(0.), Some(97.125), Some(600.), None] {
        assert_eq!(
            layout(actual, width),
            layout(control, width),
            "width={width:?}"
        );
    }
}

// Run once with EXACT_SPAN_CAPTURE_DIR against unchanged production, then with
// EXACT_SPAN_COMPARE_DIR against the treatment. No automatic expected updates.
// Paths are caller-owned ignored evidence, never a dependency or source fixture.
fn artifact(name: &str, bytes: &[u8]) {
    let capture = std::env::var_os("EXACT_SPAN_CAPTURE_DIR");
    let compare = std::env::var_os("EXACT_SPAN_COMPARE_DIR");
    assert!(capture.is_none() || compare.is_none());
    if let Some(root) = capture {
        std::fs::create_dir_all(&root).unwrap();
        let path = Path::new(&root).join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
    }
    if let Some(root) = compare {
        let expected = std::fs::read(Path::new(&root).join(name)).unwrap();
        assert_eq!(expected.len(), bytes.len(), "baseline size: {name}");
        assert!(expected == bytes, "baseline bytes: {name}");
    }
}

fn run_data(run: LayoutRun<'_>) -> String {
    let mut out = format!(
        "{}:{}:{:?}\n",
        run.line_i,
        run.rtl,
        [run.line_y, run.line_top, run.line_height, run.line_w].map(f32::to_bits)
    );
    for g in run.glyphs {
        writeln!(out, "{}", glyph(g)).unwrap();
    }
    // Every boundary for the small bidi/control cases; representative boundaries
    // per visual line for large capacity REDs avoid quadratic diagnostic work.
    let offsets: Vec<_> = if run.text.len() <= 1024 {
        run.text
            .char_indices()
            .map(|(i, _)| i)
            .chain([run.text.len()])
            .collect()
    } else {
        [
            run.glyphs.first(),
            run.glyphs.get(run.glyphs.len() / 2),
            run.glyphs.last(),
        ]
        .into_iter()
        .flatten()
        .flat_map(|g| [g.start, g.end])
        .collect()
    };
    for i in offsets {
        let cursor = Cursor::new(run.line_i, i);
        writeln!(
            out,
            "cursor={i}:{:?}:{:?}",
            run.cursor_position(&cursor).map(f32::to_bits),
            run.cursor_glyph(&cursor)
        )
        .unwrap();
    }
    let spans: Vec<_> = run
        .highlight(
            Cursor::new(run.line_i, 0),
            Cursor::new(run.line_i, run.text.len()),
        )
        .map(|(x, w)| (x.to_bits(), w.to_bits()))
        .collect();
    writeln!(out, "selection={spans:?}").unwrap();
    for d in run.decorations {
        writeln!(
            out,
            "decoration={:?}:{:?}:{}:{}",
            d.glyph_range,
            d.color_opt,
            d.font_size.to_bits(),
            decoration(&d.data)
        )
        .unwrap();
    }
    out
}

fn evidence(name: &str, text: &str, value: &ShapeLine) {
    if std::env::var_os("EXACT_SPAN_CAPTURE_DIR").is_none()
        && std::env::var_os("EXACT_SPAN_COMPARE_DIR").is_none()
    {
        return;
    }
    artifact(&format!("{name}-source.txt"), text.as_bytes());
    artifact(&format!("{name}-shape.txt"), shaped(value).as_bytes());
    artifact(
        &format!("{name}-font.sha256"),
        format!("{:x}", Sha256::digest(FONT)).as_bytes(),
    );
    let mut engine = TextEngine::with_catalog(catalog::Catalog::with_fonts(fonts()));
    let style = exact_kernel::StyleProps::default();
    let spec = crate::paint::text_spec(&style, text);
    let palette = [RunPaint {
        color: [31, 73, 151, 255],
        source: 7,
    }];
    for width in [600., 984.] {
        artifact(
            &format!("{name}-{width}-public-layout.txt"),
            layout(value, Some(width)).as_bytes(),
        );
        let p = engine.paragraph(&spec, Some(width));
        let mut numeric = format!(
            "{:?}:{:?}\n",
            [p.width, p.height, p.first_baseline].map(f32::to_bits),
            p.baselines
                .iter()
                .copied()
                .map(f32::to_bits)
                .collect::<Vec<_>>()
        );
        for run in p.layout_runs() {
            numeric.push_str(&run_data(run));
        }
        artifact(
            &format!("{name}-{width}-host-layout.txt"),
            numeric.as_bytes(),
        );
        for (sample, y) in [0., -p.height / 2., -(p.height - 100.).max(0.)]
            .into_iter()
            .enumerate()
        {
            let mut image = Pixmap::new(400, 180).unwrap();
            engine.paint(
                &mut image,
                &p,
                &palette,
                (0., y),
                1.,
                Transform::identity(),
                None,
            );
            artifact(&format!("{name}-{width}-{sample}.rgba"), image.data());
        }
        let mut gpu = String::new();
        for batch in engine.glyph_runs(&p, &palette) {
            writeln!(
                gpu,
                "{:?}:{:?}:{:?}:{}:{:x}",
                batch.font.index,
                (
                    batch.size.to_bits(),
                    batch.run_index,
                    batch.synthetic_italic
                ),
                batch.paint,
                batch.glyphs.len(),
                Sha256::digest(batch.font.data.data())
            )
            .unwrap();
            for (g, x, y) in batch.glyphs {
                writeln!(gpu, "{g}:{}:{}", x.to_bits(), y.to_bits()).unwrap();
            }
        }
        artifact(&format!("{name}-{width}-gpu.txt"), gpu.as_bytes());
    }
}

fn capacity(name: &str, text: &str, value: &ShapeLine) {
    eprintln!(
        "{name} bytes={} span_size={} layout_glyph_size={} layout_line_size={} len={} capacity={} outer_bytes={}",
        text.len(),
        size_of::<ShapeSpan>(),
        size_of::<LayoutGlyph>(),
        size_of::<cosmic_text::LayoutLine>(),
        value.spans.len(),
        value.spans.capacity(),
        value.spans.capacity() * size_of::<ShapeSpan>()
    );
}

#[test]
fn fresh_ascii_one_span_does_not_reserve_by_utf8_bytes() {
    let seed = "office words 123 ";
    let text = seed.repeat(65536usize.div_ceil(seed.len()));
    assert!(text.len() >= 65536);
    let value = shape(&mut fonts(), &text);
    capacity("ascii", &text, &value);
    evidence("ascii", &text, &value); // Preserve before-data even when RED panics.
    assert_eq!(value.spans.len(), 1);
    assert!(
        value.spans.capacity() <= 64,
        "one span reserved {} headers",
        value.spans.capacity()
    );
}

#[test]
fn fresh_multibyte_one_span_does_not_reserve_by_utf8_bytes() {
    let seed = "café e\u{301} ";
    let text = seed.repeat(65536usize.div_ceil(seed.len()));
    assert!(text.len() >= 65536 && text.len() > text.chars().count());
    let value = shape(&mut fonts(), &text);
    capacity("multibyte", &text, &value);
    evidence("multibyte", &text, &value);
    assert_eq!(value.spans.len(), 1);
    assert!(
        value.spans.capacity() <= 64,
        "one span reserved {} headers",
        value.spans.capacity()
    );
}

#[test]
fn warm_one_span_growth_does_not_reserve_for_longer_source() {
    let mut fonts = fonts();
    let mut value = shape(&mut fonts, "a");
    let old_capacity = value.spans.capacity();
    let old_pointer = value.spans.as_ptr();
    let text = "plain text ".repeat(1024);
    value.build(&mut fonts, &text, &attrs(), Shaping::Advanced, 8);
    capacity("warm", &text, &value);
    evidence("warm", &text, &value);
    assert_eq!(value.spans.len(), 1);
    assert_eq!(
        value.spans.capacity(),
        old_capacity,
        "only source bytes grew"
    );
    assert_eq!(value.spans.as_ptr(), old_pointer);
}

#[test]
fn ordinary_empty_multibyte_and_bidi_match_reserved_control() {
    let mut fonts = fonts();
    for (i, text) in [
        "",
        "office 123",
        "café e\u{301}",
        "a\t b\0c",
        "שלום עולם",
        "left שלום 123 العربية right",
        "a\u{2067}שלום\u{2069} z",
    ]
    .into_iter()
    .enumerate()
    {
        let value = shape(&mut fonts, text);
        let control = reserved_control(&mut fonts, text);
        assert_semantics(&value, &control);
        capacity(&format!("ordinary-{i}"), text, &value);
        evidence(&format!("ordinary-{i}"), text, &value);
        if text.is_empty() {
            assert!(value.spans.is_empty());
            assert_eq!(value.spans.capacity(), 0);
        }
    }
}

#[test]
fn reused_bidi_line_keeps_high_water_and_matches_fresh_semantics() {
    let mut fonts = fonts();
    let alternating = "a שלום b العربية ".repeat(32);
    let mut value = shape(&mut fonts, &alternating);
    assert!(
        value.spans.len() > 32,
        "fixture must exercise many real bidi spans"
    );
    let old_capacity = value.spans.capacity();
    let old_pointer = value.spans.as_ptr();
    let original = shaped(&value);
    for (i, text) in [
        alternating.as_str(),
        "",
        "x",
        "café e\u{301}",
        alternating.as_str(),
    ]
    .into_iter()
    .enumerate()
    {
        value.build(&mut fonts, text, &attrs(), Shaping::Advanced, 8);
        assert_semantics(&value, &reserved_control(&mut fonts, text));
        assert_eq!(
            value.spans.capacity(),
            old_capacity,
            "no old-capacity reclaim is promised"
        );
        assert_eq!(value.spans.as_ptr(), old_pointer);
        capacity(&format!("reused-{i}"), text, &value);
        evidence(&format!("reused-{i}"), text, &value);
    }
    assert_eq!(shaped(&value), original);
}
