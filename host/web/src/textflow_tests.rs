//! @ref LLP 1043.000 §3 D6/D7 — native tests of the exported browser protocol.
use super::*;
use exact_kernel::{NodeType, Op, StyleId, StyleProps, StyleValue};

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
fn kernel(shape: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut style = StyleProps::default();
    for (id, value) in [
        (StyleId::PositionType, "absolute"),
        (StyleId::WrapFlow, "both"),
        (StyleId::ShapeOutside, shape),
        (StyleId::ShapeMargin, "5px"),
    ] {
        style
            .set_dynamic(id, &StyleValue::Text(value.into()))
            .unwrap();
    }
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 9,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 9,
                patch: Box::new(style),
            },
        ],
    )
    .unwrap();
    k
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
    let reply = f
        .layout(1, &geometry(30., 40.), &kernel("circle()"))
        .unwrap();
    assert!(
        reply.contains("\"kind\":\"Circle\",\"cx\":40,\"cy\":50,\"r\":15"),
        "{reply}"
    );
    let poly = f
        .layout(
            1,
            &geometry(30., 40.),
            &kernel("polygon(0% 0%, 100% 50%, 0% 100%)"),
        )
        .unwrap();
    // M1's grown polygon is the shared conservative span envelope.
    assert!(poly.contains("\"kind\":\"Spans\""));
    let clear = f.layout(1, &geometry(300., 400.), &kernel("none")).unwrap();
    assert!(clear.contains("\"shapes\":[]"));
    assert!(f
        .layout(1, &geometry(0., 0.), &Kernel::with_monospace())
        .is_err());
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
    let k = Kernel::with_monospace();
    assert_eq!(f.request(3, 1, &[], &k), "{}");
    assert_eq!(f.request(3, 1, &[], &k), "{}");
    assert!(f.request(1, 1, &[], &k).contains("absent"));
    let mut too_many = geometry(0., 0.);
    let record = too_many[24..].to_vec();
    for _ in 0..4096 {
        too_many.extend(&record);
    }
    assert!(f
        .layout(2, &too_many, &kernel("circle()"))
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
fn exported_bridge_operations_run_natively_and_boot_retires_old_sources() {
    use crate::abi::Bridge;
    let plan = contract::compile("component Test\n  view\n    text \"hello\"\n")
        .unwrap()
        .encode();
    let mut b = Bridge::new();
    let n = b.boot(&plan, caltrain_data::Caltrain, 300., 200., "/");
    assert!(!String::from_utf8_lossy(b.output_bytes(n as usize)).contains("boot:"));
    let mut bytes = vec![0; 8];
    bytes.extend(b"hello");
    b.input_write(&bytes);
    let n = b.textflow(0, 1, bytes.len());
    assert_eq!(std::str::from_utf8(b.output_bytes(n as usize)).unwrap(), "{\"ranges\":[[0,5,0,5]],\"graphemes\":[[0,1,0,1],[1,2,1,2],[2,3,2,3],[3,4,3,4],[4,5,4,5]],\"utf16_len\":5}");
    let bytes: Vec<_> = [5f32, 25.].into_iter().flat_map(f32::to_le_bytes).collect();
    b.input_write(&bytes);
    let n = b.textflow(1, 1, bytes.len());
    assert!(std::str::from_utf8(b.output_bytes(n as usize))
        .unwrap()
        .contains("true"));
    let input = geometry(0., 0.)[..24].to_vec();
    b.input_write(&input);
    let n = b.textflow(2, 1, input.len());
    assert!(std::str::from_utf8(b.output_bytes(n as usize))
        .unwrap()
        .contains("\"utf16_end\":5"));
    b.boot(&plan, caltrain_data::Caltrain, 300., 200., "/");
    b.input_write(&input);
    let n = b.textflow(2, 1, input.len());
    assert!(std::str::from_utf8(b.output_bytes(n as usize))
        .unwrap()
        .contains("absent"));
}

#[test]
fn host_announces_contexts_inline_paragraphs_and_css_without_taffy_layout() {
    let plan = contract::compile(r#"component Test
  view
    box width=200 height=200
      text testId="para" height=180
        text "First "
        text "link" href="https://example.test"
      box position="absolute" width=20 height=20 wrap-flow="both" shape-outside="circle()" shape-margin=3
        text "inside"
      text "auto" testId="auto"
"#).unwrap();
    let (mut host, batch) = crate::Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"op\":\"textflow\""));
    assert_eq!(batch.matches("\"definite\":true").count(), 1);
    assert_eq!(batch.matches("\"definite\":false").count(), 1);
    assert!(batch.contains("shape-margin:3px;"));
    assert!(batch.contains("data-wrap-flow"));
    assert!(!host.advance(16.).contains("\"op\":\"textflow\""));
    let plain = contract::compile("component Test\n  view\n    text \"plain\"\n").unwrap();
    let (_, batch) = crate::Host::boot(
        &plain.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!batch.contains("textflow"));
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
    for (mode, bands, first_width) in [(0u32, 1, 25.0), (1, 2, 30.0)] {
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
        f.layout(1, &input, &kernel("none")).unwrap();
        let out = &f.sources[&1].fragments;
        assert_eq!(out[0].x, 265.); // kernel test fixture's five-point shape margin
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
    let reply = f.layout(1, &input, &kernel("circle()")).unwrap();
    assert!(reply.contains("\"limited\":true"));
    assert!(reply.contains("\"complete\":true"));
    assert!(!f.sources[&1].fragments.is_empty());
}

#[test]
fn polygon_fill_rule_survives_kernel_and_web_geometry() {
    let mut k = kernel("polygon(evenodd,0 0,100 0,100 100,0 100)");
    let mut style = StyleProps::default();
    style
        .set_dynamic(StyleId::ShapeMargin, &StyleValue::Number(0.))
        .unwrap();
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 9,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    let mut f = TextFlow::default();
    prepare(&mut f, "words around a polygon", 0);
    let json = f.layout(1, &geometry(0., 0.), &k).unwrap();
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
fn pan_abi_20_commits_deltas_and_refuses_nonfinite_payload_before_clock() {
    let plan = contract::compile(
        r#"component Test
  state x = 0
  action move(dx: number, dy: number) writes x
    x = x + dx + dy
  action playback(seconds: number) writes x
    x = seconds
  view
    box pan=move testId="pan"
      text `${x}`
      video timeupdate=playback testId="video"
"#,
    )
    .unwrap();
    let mut bridge = crate::abi::Bridge::new();
    let n = bridge.boot(&plan.encode(), caltrain_data::Caltrain, 300., 200., "/");
    let batch = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    let view_id = |name: &str| {
        let create = batch
            .split("\"op\":\"create\"")
            .find(|part| {
                part.split("\"op\"")
                    .next()
                    .unwrap()
                    .contains(&format!("\"data-testid\":\"{name}\""))
            })
            .unwrap();
        create
            .split("\"id\":")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let id = view_id("pan");
    let video = view_id("video");
    bridge.input_write(b"10,-3");
    let n = bridge.dispatch(id, 20, 5, 16.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"7\""), "{out}");
    bridge.input_write(b"NaN,1");
    let n = bridge.dispatch(id, 20, 5, f64::INFINITY);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid pan deltas"));
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 20, 3, 17.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"9\""), "{out}");
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 18, 3, 18.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid reorder event"));
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 19, 3, 18.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid media event"));
    bridge.input_write(b"timeupdate\n100");
    let n = bridge.dispatch(video, 19, 14, 19.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"100\""), "{out}");
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 20, 3, 20.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"102\""), "{out}");
}
