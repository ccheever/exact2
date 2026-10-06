//! Bad font admission must not turn a finite CSS offer into infinite glyphs.
use super::*;

const GOOD: &[u8] = include_bytes!("../../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");

fn broken_bytes(missing: bool) -> Vec<u8> {
    let mut bytes = GOOD.to_vec();
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let entry = (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&i| &bytes[i..i + 4] == b"head")
        .unwrap();
    if missing {
        // Apple bitmap fonts can have bhed but no head. Fontdb admits these.
        bytes[entry..entry + 4].copy_from_slice(b"bhed");
    } else {
        let offset = u32::from_be_bytes(bytes[entry + 8..entry + 12].try_into().unwrap()) as usize;
        bytes[offset + 18..offset + 20].copy_from_slice(&0u16.to_be_bytes());
    }
    bytes
}

/// The good face and a damaged copy of it (same family name), as one
/// catalog's fonts: fontique refuses the copy at registration (LLP 1085.000
/// G2), so the family keeps one face and text never reaches the copy.
fn catalog_with_bad(missing: bool) -> catalog::Catalog {
    let bad = broken_bytes(missing);
    catalog::Catalog::from_bytes(&[GOOD, bad.as_slice()], "DejaVu Sans")
}

fn admitted(bytes: Vec<u8>) -> Option<fontique::FontInfo> {
    let source = fontique::SourceInfo::new(
        fontique::SourceId::new(),
        fontique::SourceKind::Memory(fontique::Blob::new(Arc::new(bytes))),
    );
    fontique::FontInfo::from_source(source, 0)
}

#[test]
fn missing_head_real_font_is_refused_at_admission() {
    assert!(admitted(broken_bytes(true)).is_none());
}

#[test]
fn zero_head_real_font_is_refused_and_good_face_stays_admitted() {
    assert!(admitted(broken_bytes(false)).is_none());
    assert!(admitted(GOOD.to_vec()).is_some());
    let font = swash::FontRef::from_index(GOOD, 0).unwrap();
    assert_eq!(font.metrics(&[]).units_per_em, 2048);
}

#[test]
fn failed_font_admission_continues_to_good_fallback() {
    for missing in [true, false] {
        let mut catalog = catalog_with_bad(missing);
        let family = catalog
            .fonts
            .collection
            .family_by_name("DejaVu Sans")
            .unwrap();
        assert_eq!(family.fonts().len(), 1, "the damaged copy was not admitted");
        catalog.families[0] = FamilyChoice::Declared("Broken Face".into());
        let mut engine = TextEngine::with_catalog(catalog);
        let mut spec = crate::paint::text_spec(&StyleProps::default(), "office café");
        spec.runs[0].size = 13.;
        let p = engine.paragraph(&spec, Some(572.));
        let glyphs: Vec<_> = p
            .layout_runs()
            .flat_map(|r| r.glyphs.iter().copied())
            .collect();
        assert!(!glyphs.is_empty());
        let good = swash::FontRef::from_index(GOOD, 0).unwrap();
        assert!(
            glyphs.iter().all(|g| g.glyph_id != 0
                && p.lines().faces[g.face as usize].font.data.data() == GOOD
                && good.charmap().map('o') != 0),
            "missing={missing} glyphs={glyphs:?}"
        );
        assert!(glyphs.iter().all(|g| g.x.is_finite() && g.w.is_finite()));
    }
}

fn code_spec(text: &str) -> Spec {
    // Code keeps its lines and spaces: `pre-wrap`, as a `<pre>` would.
    let style = StyleProps {
        font_family: 5,
        font_size: 13.,
        white_space: exact_kernel::WhiteSpace::PreWrap,
        ..StyleProps::default()
    };
    let mut spec = crate::paint::text_spec(&style, text);
    spec.strut.line_height = Some(20.);
    spec.runs[0].line_height = Some(20.);
    spec
}

fn code_text() -> String {
    let sentence = "A long paragraph keeps its complete text while the window changes width. Reading position, input and scroll responsiveness matter. Café 🦀 東京. ";
    format!("let section = 0;\nlet wrap = \"{}\";", sentence.repeat(2))
}

fn assert_finite(p: &Paragraph) {
    assert!(
        p.width.is_finite() && p.height.is_finite() && p.first_baseline.is_finite(),
        "metrics={:?}",
        paragraph_metrics(p)
    );
    assert!(p.layout_runs().all(|r| r.line_w.is_finite()
        && r.glyphs
            .iter()
            .all(|g| [g.x, g.y, g.w, g.font_size].iter().all(|v| v.is_finite()))));
}

#[test]
fn ordinary_code_finite_offer_matches_independent_hard_lines() {
    let mut engine = TextEngine::new();
    let text = code_text();
    for width in [105., 572., 632.] {
        let p = engine.paragraph(&code_spec(&text), Some(width));
        assert_finite(&p);
        let mut w = 0f32;
        let mut h = 0f32;
        for line in text.split('\n') {
            let single = engine.paragraph(&code_spec(line), Some(width));
            assert_finite(&single);
            w = w.max(single.width);
            h += single.height;
        }
        assert_eq!(p.width, w);
        assert_eq!(p.height, h);
        eprintln!("code width offer={width}: {:?}", paragraph_metrics(&p));
    }
}

#[test]
fn min_max_and_overwide_words_keep_real_advance_not_offer_clamps() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let spec = code_spec(&"W".repeat(80));
    let minimum = engine.measure(&spec, AxisOffer::MinContent);
    let maximum = engine.measure(&spec, AxisOffer::MaxContent);
    let narrow = engine.measure(&spec, AxisOffer::Definite(80.));
    assert!(minimum.width > 80. && minimum.width.is_finite());
    assert_eq!(minimum, maximum);
    assert_eq!(narrow, maximum);
}

#[test]
fn finite_font_glyphs_geometry_and_pixels_have_stock_oracle() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let text = "office ffi café e\u{301} العربية שלום\nsecond hard line";
    let spec = code_spec(text);
    let mut signature = String::new();
    let mut rgba = Vec::new();
    for width in [105., 572.] {
        let p = engine.paragraph(&spec, Some(width));
        assert_finite(&p);
        signature.push_str(&format!(
            "{:?}\n{:?}\n{:?}\n",
            paragraph_metrics(&p),
            p.baselines,
            glyphs(&p)
        ));
        rgba.extend(pixels(&mut engine, &p));
    }
    // Optional evidence output, never read as a self-updating test expectation.
    // The run harness compares stock and treatment bytes independently.
    if let Some(dir) = std::env::var_os("EXACT_ADMISSION_ORACLE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("finite-glyphs.txt"), signature).unwrap();
        std::fs::write(dir.join("finite.rgba"), rgba).unwrap();
    }
}

#[test]
fn real_thread_code_transfer_matches_ordinary_geometry_glyphs_and_pixels() {
    // Only two owned font sources; this tests the real transfer path without
    // making a full system-font capture prerequisite for a small regression.
    let mut catalog = catalog_with_bad(true);
    catalog.families[5] = FamilyChoice::Declared("Broken Face".into());
    let mut engine = TextEngine::with_catalog(catalog);
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, _) = request(recipe.catalog_label(), 572.);
    let mut style = StyleProps {
        font_family: 5,
        font_size: 13.,
        ..StyleProps::default()
    };
    style.mask.set(StyleId::FontFamily);
    style.mask.set(StyleId::FontSize);
    style.mask.set(StyleId::WhiteSpace);
    style.white_space = WhiteSpace::PreWrap;
    k.apply(
        0,
        0,
        &[
            Op::SetStyle {
                id: 4,
                patch: Box::new(style),
            },
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: code_text().into(),
            },
        ],
    )
    .unwrap();
    let q = next_request(&mut k, recipe.catalog_label(), 572.);
    let spec = q.with_request(Spec::from_request);
    assert!(spec.runs.iter().any(|r| r.text.contains('\n')));
    let width = match q.offer().width {
        AxisOffer::Definite(w) => w,
        _ => panic!("final definite offer"),
    };
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(recipe).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    assert!(
        output.metrics().width.is_finite(),
        "worker completion={:?}",
        output.metrics()
    );
    let expected = engine.paragraph(&spec, Some(width));
    assert_finite(&expected);
    let p = adopt(output, &input, &raster).unwrap();
    let actual = p.paragraph().unwrap();
    assert_finite(actual);
    assert_eq!(p.metrics(), paragraph_metrics(&expected));
    assert_eq!(actual.baselines, expected.baselines);
    assert_eq!(glyphs(actual), glyphs(&expected));
    assert_eq!(pixels(&mut engine, actual), pixels(&mut engine, &expected));
}
