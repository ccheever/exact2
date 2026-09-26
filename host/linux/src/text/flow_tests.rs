//! @ref LLP 1043.000 §3 D6/D7 — shape once, fragment order and bounded residency.
use super::*;
use exact_textflow::FlowShape;
fn spec(text: &str) -> Spec {
    crate::paint::text_spec(
        &exact_kernel::StyleProps {
            font_size: 16.0,
            line_height: exact_kernel::LineHeight::Number(1.5),
            ..Default::default()
        },
        text,
    )
}
fn circle(x: f32) -> [FlowShape; 1] {
    [FlowShape::Circle {
        cx: x,
        cy: 90.0,
        r: 48.0,
    }]
}
#[test]
fn moving_shape_retains_shaping_and_no_frame_history() {
    let mut engine = TextEngine::new();
    let spec = spec(&"A quiet river carries the light through the city. ".repeat(40));
    let before = shaping::shape_line_calls();
    let mut p = engine.paragraph_flow(&spec, 500.0, &circle(200.0), None);
    let initial = engine.residency();
    let prepared = p.source.flow.borrow().as_ref().unwrap().clone();
    let weak = Rc::downgrade(&p);
    for n in 0..1000 {
        p = engine.paragraph_flow(&spec, 500.0, &circle(140.0 + (n % 200) as f32), Some(&p));
        assert!(!p.fragments().is_empty());
    }
    assert!(weak.upgrade().is_none());
    assert!(Rc::ptr_eq(
        &prepared,
        p.source.flow.borrow().as_ref().unwrap()
    ));
    assert_eq!(shaping::shape_line_calls() - before, 1);
    assert_eq!(engine.residency().paragraphs, initial.paragraphs);
    assert_eq!(
        engine.residency().owned_capacity_bytes,
        initial.owned_capacity_bytes
    );
    let mut end = 0;
    for f in p.fragments() {
        assert_eq!(f.start, end);
        end = f.end;
    }
    assert_eq!(end, spec.runs[0].text.len());
}
#[test]
fn bilingual_fragments_reorder_clusters_but_keep_logical_fragment_order() {
    let mut engine = TextEngine::new();
    let text = "Before שלום עולם מילים רבות مرحبا بالعالم كتابة جميلة after. ".repeat(12);
    for direction in [exact_kernel::Direction::Ltr, exact_kernel::Direction::Rtl] {
        let mut s = spec(&text);
        s.direction = direction;
        let p = engine.paragraph_flow(&s, 390.0, &circle(195.0), None);
        let mut rtl_pairs = 0;
        let mut split_bands = 0;
        for pair in p.fragments().windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
            if pair[0].line == pair[1].line {
                assert_eq!(
                    pair[0].x > pair[1].x,
                    direction == exact_kernel::Direction::Rtl
                );
                split_bands += 1;
            }
        }
        for run in p.layout_runs() {
            for pair in run.glyphs.windows(2) {
                assert!(pair[0].x <= pair[1].x + 0.01);
                if pair[0].level.is_rtl()
                    && pair[0].level == pair[1].level
                    && pair[0].start != pair[1].start
                {
                    assert!(pair[0].start > pair[1].start);
                    rtl_pairs += 1;
                }
            }
        }
        assert!(rtl_pairs > 10 && split_bands > 0);
    }
}
#[test]
fn clamp_counts_bands_and_alignment_uses_each_interval() {
    let mut engine = TextEngine::new();
    let mut s = spec(&"One two three four five six seven eight nine. ".repeat(20));
    s.line_clamp = 5;
    let start = engine.paragraph_flow(&s, 400.0, &circle(200.0), None);
    assert!(start.fragments().iter().all(|f| f.line < 5));
    s.align = TextAlign::Center;
    let center = engine.paragraph_flow(&s, 400.0, &circle(200.0), None);
    assert_eq!(start.fragments().len(), center.fragments().len());
    assert!(start
        .fragments()
        .iter()
        .zip(center.fragments())
        .any(|(a, b)| b.x > a.x));
    assert_eq!(
        start.first_baseline,
        engine.paragraph(&s, Some(400.0)).first_baseline
    );
    assert_eq!(start.flow_line_height(), 24.0);
}
#[test]
fn worst_case_full_obstruction_is_bounded_and_empty_is_not_success() {
    let mut engine = TextEngine::new();
    let mut s = spec(&"W".repeat(20000));
    s.overflow_wrap = exact_kernel::OverflowWrap::Anywhere;
    let blocked = [FlowShape::RoundRect {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 1.0e9,
        radius: 0.0,
    }];
    s.line_clamp = 100;
    let p = engine.paragraph_flow(&s, 400.0, &blocked, None);
    assert!(p.fragments().is_empty());
    assert_eq!(p.height, 2400.0);
    s.line_clamp = 0;
    let p = engine.paragraph_flow(&s, 400.0, &circle(200.0), None);
    assert_eq!(p.fragments().last().unwrap().end, 20000);
}

#[test]
fn measure_request_and_fragment_paint_have_identical_metrics() {
    let shared = TextEngine::shared();
    let style = exact_kernel::TextStyle::from_style(&exact_kernel::StyleProps::default());
    let runs = [exact_kernel::TextRun {
        text: "Measure what we paint. ",
        style,
    }];
    let exclusions = circle(120.);
    let request = TextMeasureRequest {
        runs: &runs,
        paragraph: exact_kernel::text::Paragraph::from_style(&exact_kernel::StyleProps::default()),
        width: AxisOffer::Definite(250.),
        height: AxisOffer::Definite(300.),
        exclusions: &exclusions,
    };
    let answer = Measurer(shared.clone()).measure(&request);
    let p =
        shared
            .borrow_mut()
            .paragraph_flow(&Spec::from_request(&request), 250., &exclusions, None);
    assert_eq!(answer, paragraph_metrics(&p));
    assert!(!p.fragments().is_empty());
}

#[test]
fn fragment_byte_coverage_survives_clusters_and_hard_breaks() {
    let mut engine = TextEngine::new();
    let source = "office affinity e\u{301} 👩‍👩‍👧‍👦   word\n\nשלום עולם\r\nNext\t\tend".repeat(30);
    let mut s = spec(&source);
    s.overflow_wrap = exact_kernel::OverflowWrap::Anywhere;
    // Fragments address the spec's text, collapsed per CSS (LLP 1053 G5).
    let text = s.runs[0].text.clone();
    assert!(text.len() < source.len());
    let p = engine.paragraph_flow(&s, 190., &circle(90.), None);
    let mut at = 0;
    for f in p.fragments() {
        assert_eq!(at, f.start);
        assert!(text.is_char_boundary(f.end));
        at = f.end;
    }
    assert_eq!(at, text.len());
    for (f, line) in p.fragments().iter().zip(p.layout_runs()) {
        let right = line.glyphs.iter().map(|g| g.x + g.w).fold(f.x, f32::max);
        assert!(
            right <= f.x + f.width + 0.1,
            "cluster extends beyond measured fragment: {right} {f:?}"
        );
    }
}

#[test]
fn unbreakable_normal_word_reports_its_actual_overflow() {
    let mut engine = TextEngine::new();
    let s = spec(&"unbreakable".repeat(100));
    let p = engine.paragraph_flow(&s, 200., &circle(100.), None);
    assert!(p.width > 2000.);
    assert_eq!(p.fragments().last().unwrap().end, s.runs[0].text.len());
    let ordinary = engine.paragraph(&s, None);
    assert!((ordinary.width - p.width).abs() < 1.);
}

#[test]
fn trailing_blank_lines_keep_source_glyphs_in_their_own_bands() {
    let mut engine = TextEngine::new();
    let text = "First source line.\nSecond source line.\n\n\n";
    // Hard breaks are preserved segment breaks: `pre-wrap` (CSS; LLP 1053 G5).
    let mut pre = spec("");
    pre.white_space = exact_kernel::WhiteSpace::PreWrap;
    pre.runs[0].text = text.into();
    let p = engine.paragraph_flow(&pre, 400., &circle(200.), None);
    assert_eq!(p.fragments().last().unwrap().end, text.len());
    assert_eq!(p.fragments().len(), p.layout_runs().count());
    for (f, run) in p.fragments().iter().zip(p.layout_runs()) {
        assert_eq!(
            text[f.start..f.end].trim().is_empty(),
            run.glyphs.is_empty()
        );
    }
}

#[test]
fn retired_font_catalog_cannot_seed_a_new_flow_layout() {
    let mut engine = TextEngine::new();
    let spec = spec("The same text in a replacement font catalog.");
    let old = engine.paragraph_flow(&spec, 400., &circle(200.), None);
    // A plan/catalog replacement swaps the entire engine while the accepted
    // frame may still own its previous paragraph until publication succeeds.
    engine = TextEngine::new();
    let before = shaping::shape_line_calls();
    let new = engine.paragraph_flow(&spec, 400., &circle(200.), Some(&old));
    assert_eq!(shaping::shape_line_calls() - before, 1);
    assert!(!Rc::ptr_eq(&old, &new));
    assert!(Rc::ptr_eq(&engine.catalog, &new.source.catalog));
}

#[test]
fn css_white_space_changes_flow_width_and_breaks() {
    let mut engine = TextEngine::new();
    let mut s = spec("A    B\nC");
    let normal = engine.paragraph_flow(&s, 500.0, &circle(900.0), None);
    assert_eq!(normal.fragments().len(), 1);
    let normalized = engine.paragraph_flow(&spec("A B C"), 500.0, &circle(900.0), None);
    assert!((normal.fragments()[0].width - normalized.fragments()[0].width).abs() < 0.01);
    let normal_ink: Vec<_> = normal
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.x))
        .collect();
    let normalized_ink: Vec<_> = normalized
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.x))
        .collect();
    assert_eq!(normal_ink, normalized_ink);
    // The spec collapsed its source (LLP 1053 G5); preserve the raw source.
    s.white_space = exact_kernel::WhiteSpace::PreWrap;
    s.runs[0].text = "A    B\nC".into();
    let preserved = engine.paragraph_flow(&s, 500.0, &circle(900.0), None);
    assert_eq!(preserved.fragments().len(), 2);
    let pair = engine.paragraph_flow(&spec("A B"), 500.0, &circle(900.0), None);
    assert!(preserved.fragments()[0].width > pair.fragments()[0].width);
    s.white_space = exact_kernel::WhiteSpace::Nowrap;
    let unwrapped = engine.paragraph_flow(&s, 500.0, &circle(900.0), None);
    assert_eq!(unwrapped.fragments().len(), 1);
    assert!((unwrapped.fragments()[0].width - normalized.fragments()[0].width).abs() < 0.01);
    let mut long = spec("one two three four five six seven eight nine ten");
    long.white_space = exact_kernel::WhiteSpace::Nowrap;
    let one = engine.paragraph_flow(&long, 40.0, &circle(900.0), None);
    assert_eq!(one.fragments().len(), 1);
    assert!(one.fragments()[0].width > 40.0);
}

#[test]
fn rtl_hebrew_and_arabic_take_right_interval_first() {
    let mut engine = TextEngine::new();
    let hole = [FlowShape::RoundRect {
        x: 140.,
        y: 0.,
        width: 120.,
        height: 100.,
        radius: 0.,
    }];
    for text in [
        "שלום עולם מילים רבות בעברית ".repeat(8),
        "مرحبا بالعالم هذه كلمات عربية ".repeat(8),
    ] {
        for direction in [exact_kernel::Direction::Ltr, exact_kernel::Direction::Rtl] {
            let mut s = spec(&text);
            s.direction = direction;
            let p = engine.paragraph_flow(&s, 400., &hole, None);
            assert_eq!(p.fragments()[0].start, 0);
            assert_eq!(
                p.fragments()[0].x,
                if direction == exact_kernel::Direction::Rtl {
                    260.
                } else {
                    0.
                }
            );
            let mut split = 0;
            for pair in p.fragments().windows(2) {
                assert_eq!(pair[0].end, pair[1].start);
                if pair[0].line == pair[1].line {
                    assert_eq!(
                        pair[0].x > pair[1].x,
                        direction == exact_kernel::Direction::Rtl
                    );
                    split += 1;
                }
            }
            assert!(split > 0);
            assert_eq!(p.fragments().last().unwrap().end, s.runs[0].text.len());
        }
    }
}

#[test]
fn tall_wall_jumps_and_unrepresentable_flow_falls_back_to_visible_text() {
    let mut engine = TextEngine::new();
    let s = spec("word");
    let mut wall = [FlowShape::RoundRect {
        x: 0.,
        y: 0.,
        width: 400.,
        height: 30_000.,
        radius: 0.,
    }];
    let p = engine.paragraph_flow(&s, 400., &wall, None);
    assert_eq!(p.fragments().len(), 1);
    assert!(p.fragments()[0].y >= 30_000.);
    if let FlowShape::RoundRect { height, .. } = &mut wall[0] {
        *height = f32::MAX;
    }
    let p = engine.paragraph_flow(&s, 400., &wall, None);
    assert!(p.flow_incomplete());
    assert!(p.fragments().is_empty());
    assert!(p.layout_runs().any(|r| !r.glyphs.is_empty()));
    assert!(p.height < 100.);
}

#[test]
fn clamp_paints_measured_ellipsis_inside_last_interval() {
    let mut engine = TextEngine::new();
    let ellipsis = engine.paragraph(&spec("…"), None);
    let ink = ellipsis
        .layout_runs()
        .flat_map(|r| r.glyphs)
        .next()
        .unwrap();
    let mut s = spec(&"One two three four five six seven eight nine ten. ".repeat(20));
    s.line_clamp = 2;
    let p = engine.paragraph_flow(&s, 400., &circle(200.), None);
    let last = p.layout_runs().last().unwrap();
    let glyph = last.glyphs.last().unwrap();
    assert_eq!((glyph.font_id, glyph.glyph_id), (ink.font_id, ink.glyph_id));
    let f = p.fragments().last().unwrap();
    assert!(glyph.x + glyph.w <= 400. + 0.01);
    assert!(glyph.x + glyph.w <= f.x + f.width + 0.01);
}

#[test]
fn soft_hyphen_paints_real_dash_beside_hole() {
    let mut engine = TextEngine::new();
    let dash = engine.paragraph(&spec("-"), None);
    let glyph = dash.layout_runs().next().unwrap().glyphs[0].clone();
    let p = engine.paragraph_flow(
        &spec("ab\u{ad}cdefghij"),
        400.,
        &[FlowShape::RoundRect {
            x: 64.,
            y: 0.,
            width: 160.,
            height: 100.,
            radius: 0.,
        }],
        None,
    );
    assert_eq!(p.fragments()[0].end, 4);
    let first = p.layout_runs().next().unwrap();
    let ink = first.glyphs.last().unwrap();
    assert_eq!((ink.font_id, ink.glyph_id), (glyph.font_id, glyph.glyph_id));
    assert!((first.line_w - first.glyphs.iter().map(|g| g.w).sum::<f32>()).abs() < 0.01);
}
