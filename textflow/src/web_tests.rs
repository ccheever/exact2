//! @ref LLP 1043.000 §3 D6/D7 — native tests of the exported browser protocol.
use super::*;

fn segments(flow: &mut TextFlow, id: u32, text: &str, mode: u32) -> String {
    let mut input = mode.to_le_bytes().to_vec();
    input.extend(0u32.to_le_bytes());
    input.extend(text.as_bytes());
    flow.segments(id, &input).unwrap()
}
fn prepare(flow: &mut TextFlow, text: &str, mode: u32) {
    segments(flow, 1, text, mode);
    let mut input = 5f32.to_le_bytes().to_vec();
    for range in &flow.sources[&1].ranges {
        let width = exact_textflow::grapheme_ranges(&text[range.clone()]).count() as f32 * 5.0;
        input.extend(width.to_le_bytes());
    }
    assert_eq!(flow.prepare(1, &input), Ok("{\"prepared\":true}".into()));
}
fn options() -> FlowOptions {
    FlowOptions {
        direction: exact_textflow::Direction::Ltr,
        width: 100.0,
        line_height: 20.0,
        min_fragment: 15.0,
        max_lines: MAX_BANDS,
    }
}
fn exclusion(flow: &mut TextFlow, shape: &str) {
    let mut bytes = 5f32.to_le_bytes().to_vec();
    bytes.extend(shape.as_bytes());
    flow.exclusion(9, &bytes).unwrap();
}
fn geometry(x: f32, y: f32) -> Vec<u8> {
    let mut input = Vec::new();
    for v in [100f32, 20., 3.75] {
        input.extend(v.to_le_bytes());
    }
    input.extend(0u32.to_le_bytes());
    input.extend(200f32.to_le_bytes());
    input.extend(0u32.to_le_bytes());
    input.extend(9u32.to_le_bytes());
    for v in [20f32, 20., x, y] {
        input.extend(v.to_le_bytes());
    }
    input
}

#[test]
fn segment_requests_and_utf16_mapping_keep_emoji_marks_flags_and_runs_whole() {
    let text = "A😀e\u{301} 👩🏽‍🚀🇺🇸אב";
    let mut f = TextFlow::default();
    let json = segments(&mut f, 1, text, 2);
    let source = &f.sources[&1];
    for r in &source.ranges {
        assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
        assert_eq!(
            source.utf16[r.start],
            text[..r.start].encode_utf16().count()
        );
        assert_eq!(source.utf16[r.end], text[..r.end].encode_utf16().count());
    }
    let clusters: Vec<_> = exact_textflow::grapheme_ranges(text)
        .map(|r| &text[r])
        .collect();
    assert_eq!(clusters, ["A", "😀", "e\u{301}", " ", "👩🏽‍🚀", "🇺🇸", "א", "ב"]);
    assert!(json.contains("[1,5,1,3]"), "{json}");
    assert!(!source.ranges.is_empty()); // no-op segmentation cannot pass
    prepare(&mut f, text, 2);
    let reply = f.flow(1, &[], options()).unwrap();
    assert!(reply.contains("\"complete\":true"));
    assert_eq!(f.sources[&1].fragments.last().unwrap().end, text.len());
}

#[test]
fn geometry_resolves_circles_and_polygons_in_wasm_and_rejects_stale_nodes() {
    let mut f = TextFlow::default();
    prepare(&mut f, &"word ".repeat(80), 0);
    exclusion(&mut f, "circle()");
    let reply = f.layout(1, &geometry(30., 40.)).unwrap();
    assert!(
        reply.contains("\"kind\":\"Circle\",\"cx\":40,\"cy\":50,\"r\":15"),
        "{reply}"
    );
    exclusion(&mut f, "polygon(0% 0%, 100% 50%, 0% 100%)");
    let poly = f.layout(1, &geometry(30., 40.)).unwrap();
    assert!(poly.contains("\"kind\":\"Spans\""));
    exclusion(&mut f, "none");
    let clear = f.layout(1, &geometry(300., 400.)).unwrap();
    assert!(clear.contains("\"shapes\":[]"));
    f.request(5, 9, &[]);
    assert!(f.layout(1, &geometry(0., 0.)).is_err());
}

#[test]
fn moving_one_pixel_touches_only_crossed_bands_when_breaks_stay_the_same() {
    let mut f = TextFlow::default();
    prepare(&mut f, &"abc def ghi jkl\n".repeat(12), 0);
    let shape = FlowShape::RoundRect {
        x: 35.,
        y: 60.,
        width: 20.,
        height: 40.,
        radius: 0.,
    };
    f.flow(1, std::slice::from_ref(&shape), options()).unwrap();
    let first = f.sources[&1].fragments.clone();
    f.flow(1, &[shape.translate(1., 0.)], options()).unwrap();
    let next = &f.sources[&1].fragments;
    assert_eq!(first.len(), next.len());
    let mut changed = 0;
    for (a, b) in first.iter().zip(next) {
        if a != b {
            changed += 1;
            assert!((60.0..100.0).contains(&a.y), "{a:?} -> {b:?}");
        }
    }
    assert!(
        changed > 0,
        "removing flow cannot satisfy this negative control"
    );
    assert!(changed < first.len());
}

#[test]
fn one_thousand_frames_keep_one_prepared_and_bounded_fragment_storage() {
    let mut f = TextFlow::default();
    prepare(&mut f, &"Measured once and flowed again. ".repeat(32), 0);
    let pointer = f.sources[&1].prepared.as_ref().unwrap() as *const Prepared;
    for frame in 0..1000 {
        let shape = FlowShape::Circle {
            cx: (frame % 100) as f32,
            cy: 60.,
            r: 14.,
        };
        f.flow(1, &[shape], options()).unwrap();
        assert_eq!(
            pointer,
            f.sources[&1].prepared.as_ref().unwrap() as *const Prepared
        );
        assert!(f.sources[&1].fragments.len() <= f.sources[&1].text.len());
    }
    assert_eq!(f.sources.len(), 1);
}

#[test]
fn worst_admitted_long_word_under_full_obstruction_stops_and_reports_incomplete() {
    let mut f = TextFlow::default();
    prepare(&mut f, &"x".repeat(MAX_TEXT), 2);
    let wall = FlowShape::RoundRect {
        x: 0.,
        y: 0.,
        width: 100.,
        height: 1e9,
        radius: 0.,
    };
    let reply = f.flow(1, &[wall], options()).unwrap();
    assert!(reply.contains("\"complete\":false"));
    assert!(f.sources[&1].fragments.is_empty());
    assert!(reply.contains("\"height\":81920"));
    let mut input = vec![0; 8];
    input.extend(vec![b'x'; MAX_TEXT + 1]);
    assert_eq!(
        f.segments(1, &input).unwrap_err(),
        "textflow exceeds 64 KiB source limit"
    );
}

#[test]
fn invalid_advances_and_store_bounds_refuse_without_replacing_preparation() {
    let mut f = TextFlow::default();
    prepare(&mut f, "hello", 0);
    assert!(f.prepare(1, &[]).is_err());
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let bytes: Vec<_> = [0f32, value]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        assert!(f.prepare(1, &bytes).is_err());
    }
    assert!(f.sources[&1].prepared.is_some());
    for id in 2..=MAX_PARAGRAPHS as u32 {
        segments(&mut f, id, "a", 0);
    }
    let mut input = vec![0; 8];
    input.push(b'a');
    assert_eq!(
        f.segments(999, &input).unwrap_err(),
        "textflow exceeds 64 live paragraphs"
    );
    assert_eq!(f.request(3, 1, &[]), "{}");
    assert_eq!(f.request(3, 1, &[]), "{}");
    assert!(f.request(1, 1, &[]).contains("absent"));
    let mut too_many = geometry(0., 0.);
    let record = too_many[24..].to_vec();
    for _ in 0..4096 {
        too_many.extend(&record);
    }
    assert!(f
        .layout(2, &too_many)
        .unwrap_err()
        .contains("4096 candidate exclusions"));
}

#[test]
fn giant_and_multibyte_sources_refuse_without_replacing_accepted_geometry() {
    // LLP 1044.001 §7.4: the exclusions walker is not a giant-text fallback.
    // In particular, a UTF-16 count below the ceiling does not admit UTF-8
    // above it, and a refused candidate must not evict the accepted source.
    let mut flow = TextFlow::default();
    prepare(&mut flow, "accepted source", 0);
    let accepted = flow.flow(1, &[], options()).unwrap();
    for text in [
        "😀".repeat(MAX_TEXT / 4 + 1),
        "x".repeat(1024 * 1024),
        "x".repeat(4 * 1024 * 1024),
    ] {
        let mut input = vec![0; 8];
        input.extend(text.as_bytes());
        for id in [1, 2] {
            assert_eq!(
                flow.segments(id, &input).unwrap_err(),
                "textflow exceeds 64 KiB source limit"
            );
            assert_eq!(flow.sources.len(), 1);
            assert_eq!(flow.sources[&1].text, "accepted source");
            assert_eq!(flow.flow(1, &[], options()).unwrap(), accepted);
        }
    }
}

#[test]
fn trailing_collapsed_source_after_a_hard_break_stays_owned_by_the_dom() {
    let mut f = TextFlow::default();
    let text = "hello\n  \t";
    prepare(&mut f, text, 0);
    let reply = f.flow(1, &[], options()).unwrap();
    assert!(reply.contains("\"complete\":true"));
    assert_eq!(f.sources[&1].fragments.len(), 1);
    assert_eq!(f.sources[&1].fragments[0].end, text.len());
    assert!(reply.contains("\"paint_end\":5"));
}

#[test]
fn white_space_protocol_preserves_spaces_and_segment_breaks() {
    let text = "A    B\nC";
    for (mode, bands, first_width) in [(0u32, 1, 25.0), (1, 2, 30.0), (2, 1, 25.0)] {
        let mut f = TextFlow::new();
        let mut input = 0u32.to_le_bytes().to_vec();
        input.extend(mode.to_le_bytes());
        input.extend(text.as_bytes());
        f.segments(1, &input).unwrap();
        let mut advances = 5f32.to_le_bytes().to_vec();
        for r in &f.sources[&1].ranges {
            let s = &text[r.clone()];
            let width = if mode == 0 && s.chars().all(char::is_whitespace) {
                5.0
            } else {
                s.chars().count() as f32 * 5.0
            };
            advances.extend(width.to_le_bytes());
        }
        f.prepare(1, &advances).unwrap();
        f.flow(1, &[], options()).unwrap();
        let out = &f.sources[&1].fragments;
        assert_eq!(out.len(), bands);
        assert_eq!(out[0].width, first_width);
        assert_eq!(out.last().unwrap().end, text.len());
    }
}

#[test]
fn white_space_nowrap_protocol_keeps_one_band_wider_than_the_flow() {
    let text = "one two three four five six seven eight";
    let mut f = TextFlow::new();
    let mut input = 0u32.to_le_bytes().to_vec();
    input.extend(2u32.to_le_bytes());
    input.extend(text.as_bytes());
    f.segments(1, &input).unwrap();
    let mut advances = 5f32.to_le_bytes().to_vec();
    for r in &f.sources[&1].ranges {
        advances.extend((text[r.clone()].chars().count() as f32 * 5.0).to_le_bytes());
    }
    f.prepare(1, &advances).unwrap();
    f.flow(1, &[], options()).unwrap();
    let out = &f.sources[&1].fragments;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].end, text.len());
    assert!(out[0].width > options().width);
}

#[test]
fn rtl_hebrew_and_arabic_wasm_geometry_take_right_interval_first() {
    for text in [
        "שלום עולם מילים רבות בעברית ".repeat(8),
        "مرحبا بالعالم هذه كلمات عربية ".repeat(8),
    ] {
        let mut f = TextFlow::new();
        prepare(&mut f, &text, 0);
        let mut input = geometry(140., 0.);
        for (offset, value) in [(0, 400f32), (8, 16.), (28, 120.), (32, 100.)] {
            input[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        input[20..24].copy_from_slice(&1u32.to_le_bytes());
        exclusion(&mut f, "none");
        f.layout(1, &input).unwrap();
        let out = &f.sources[&1].fragments;
        assert_eq!(out[0].x, 265.); // registered five-point shape margin
        assert_eq!(out[0].start, 0);
        assert!(out
            .windows(2)
            .any(|p| p[0].line == p[1].line && p[0].x > p[1].x));
        assert_eq!(out.last().unwrap().end, text.len());
    }
}

#[test]
fn soft_hyphen_flag_is_walkers_decision_beside_hole() {
    let mut f = TextFlow::default();
    prepare(&mut f, "ab\u{ad}cdefghij", 0);
    let shapes = [FlowShape::RoundRect {
        x: 15.,
        y: 0.,
        width: 40.,
        height: 100.,
        radius: 0.,
    }];
    let reply = f.flow(1, &shapes, options()).unwrap();
    assert!(reply.contains("\"hyphenated\":true"), "{reply}");
    assert_eq!(f.sources[&1].fragments[0].end, 4);
    let clear = f.flow(1, &[], options()).unwrap();
    assert!(!clear.contains("\"hyphenated\":true"));
}

#[test]
fn excess_shapes_keep_flow_with_a_conservative_bounded_envelope() {
    let mut f = TextFlow::default();
    prepare(&mut f, &"word ".repeat(30), 0);
    let mut input = geometry(30., 40.);
    let record = input[24..].to_vec();
    for _ in 1..65 {
        input.extend(&record);
    }
    exclusion(&mut f, "circle()");
    let reply = f.layout(1, &input).unwrap();
    assert!(reply.contains("\"limited\":true"));
    assert!(reply.contains("\"complete\":true"));
    assert!(!f.sources[&1].fragments.is_empty());
}

#[test]
fn polygon_fill_rule_survives_registered_web_geometry() {
    let mut f = TextFlow::default();
    prepare(&mut f, "words around a polygon", 0);
    let mut bytes = 0f32.to_le_bytes().to_vec();
    bytes.extend(b"polygon(evenodd,0 0,100 0,100 100,0 100)");
    f.exclusion(9, &bytes).unwrap();
    let json = f.layout(1, &geometry(0., 0.)).unwrap();
    assert!(json.contains("\"fill_rule\":\"evenodd\""), "{json}");
}

#[test]
fn short_source_headers_return_errors_instead_of_panicking() {
    for n in 0..8 {
        let mut f = TextFlow::default();
        let answer =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.segments(1, &vec![0; n])));
        assert!(answer.is_ok(), "short source header {n} panicked");
        assert!(answer.unwrap().is_err());
        assert_eq!(f.sources.len(), 0);
    }
}

// @ref LLP 1043.000 §3 D8 — Pan, media and reorder share a dispatcher, never an ID.

#[test]
fn exclusion_admission_is_bounded_transactional_and_reset_releases_everything() {
    let mut f = TextFlow::new();
    prepare(&mut f, "words around an exclusion", 0);
    exclusion(&mut f, "circle()");
    let accepted = f.layout(1, &geometry(30., 40.)).unwrap();
    for bytes in [
        vec![],
        vec![0; 3],
        vec![0; 16 * 1024 + 1],
        b"\0\0\0\0invalid".to_vec(),
        [f32::NAN.to_le_bytes().as_slice(), b"none"].concat(),
        [(-1f32).to_le_bytes().as_slice(), b"none"].concat(),
    ] {
        assert!(f.request(4, 9, &bytes).contains("error"));
        assert_eq!(f.layout(1, &geometry(30., 40.)).unwrap(), accepted);
    }
    let bytes = b"\0\0\0\0none";
    for id in 0..MAX_GEOMETRY_SHAPES as u32 {
        f.exclusion(id, bytes).unwrap();
    }
    assert!(f
        .request(4, MAX_GEOMETRY_SHAPES as u32, bytes)
        .contains("4096"));
    assert_eq!(f.request(5, 9, &[]), "{}");
    assert_eq!(f.request(4, MAX_GEOMETRY_SHAPES as u32, bytes), "{}");
    assert_eq!(f.request(6, 0, &[]), "{}");
    assert_eq!(f.sources.len(), 0);
    assert!(f.shapes.is_empty());
}
