mod support;
use exact_textflow::{flow, FlowOptions, FlowShape, Options, OverflowWrap};
use support::{lines, prepare, CORPUS};
fn opts(width: f32) -> FlowOptions {
    FlowOptions {
        direction: exact_textflow::Direction::Ltr,
        width,
        line_height: 22.0,
        min_fragment: 16.0,
        max_lines: 0,
    }
}
fn rect(x: f32, y: f32, width: f32, height: f32) -> FlowShape {
    FlowShape::RoundRect {
        x,
        y,
        width,
        height,
        radius: 0.0,
    }
}
#[test]
fn no_shapes_equals_plain_wrapping_for_the_whole_corpus() {
    for text in CORPUS {
        for mode in [
            OverflowWrap::Normal,
            OverflowWrap::BreakWord,
            OverflowWrap::Anywhere,
        ] {
            let p = prepare(
                text,
                Options {
                    white_space: exact_textflow::WhiteSpace::Normal,
                    overflow_wrap: mode,
                    hyphen_advance: 8.0,
                },
            );
            for width in [8.0, 32.0, 80.0, 640.0] {
                let plain = lines(&p, width);
                let mut out = Vec::new();
                for shapes in [&[][..], &[rect(-100.0, -100.0, 1.0, 1.0)][..]] {
                    let result = flow(&p, shapes, &opts(width), &mut out);
                    assert!(result.complete);
                    let height = result.height;
                    assert_eq!(out.len(), plain.len(), "{text:?}, {width}");
                    assert_eq!(height, plain.len() as f32 * 22.0);
                    for (i, (f, l)) in out.iter().zip(&plain).enumerate() {
                        assert_eq!(
                            (f.start, f.end, f.width, f.x, f.y, f.line),
                            (
                                l.start.byte,
                                l.end.byte,
                                l.width,
                                0.0,
                                i as f32 * 22.0,
                                i as u32
                            )
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn circle_fragments_read_in_order_cover_bytes_and_avoid_ink_overlap() {
    let text = "  one   two three four five six seven eight nine ten\n".repeat(30);
    let p = prepare(&text, Options::default());
    let circle = FlowShape::Circle {
        cx: 320.0,
        cy: 110.0,
        r: 90.0,
    };
    let mut out = Vec::new();
    flow(&p, &[circle], &opts(640.0), &mut out);
    // Negative control: returning the full band (or no bands) fails this assertion.
    assert!(out
        .windows(2)
        .any(|p| p[0].line == p[1].line && p[0].x < p[1].x));
    let mut at = 0;
    let mut previous = (0, 0.0);
    for f in &out {
        assert_eq!(f.start, at);
        at = f.end;
        assert!(f.line > previous.0 || f.x >= previous.1);
        previous = (f.line, f.x);
        let near = 110.0_f32.clamp(f.y, f.y + 22.0);
        let dy = (near - 110.0).abs();
        if dy < 90.0 {
            let half = (90.0_f32.powi(2) - dy.powi(2)).sqrt();
            assert!(
                f.x + f.width <= 320.0 - half + 0.0001 || f.x >= 320.0 + half - 0.0001,
                "{f:?}"
            );
        }
    }
    assert_eq!(at, text.len());
    assert!(out.iter().any(|f| text[f.start..f.end].ends_with(' '))); // Spaces remain source-owned, not counted in width.
}
#[test]
fn left_edge_full_width_obstruction_and_hard_break_ends_band() {
    let p = prepare(
        "a\nb c d e f g h",
        Options {
            white_space: exact_textflow::WhiteSpace::PreWrap,
            ..Options::default()
        },
    );
    let mut out = Vec::new();
    flow(&p, &[rect(32.0, 0.0, 32.0, 100.0)], &opts(96.0), &mut out);
    assert_eq!(out[0].line, 0);
    assert_eq!(out[0].end, 2);
    assert_eq!(out[1].line, 1);
    assert_eq!(out[1].x, 0.0);
    flow(&p, &[rect(0.0, 0.0, 32.0, 44.0)], &opts(96.0), &mut out);
    assert_eq!(out[0].x, 32.0);
    flow(&p, &[rect(0.0, 0.0, 96.0, 2200.0)], &opts(96.0), &mut out);
    assert_eq!(out[0].y, 2200.0);
    assert_eq!(out[0].line, 100);
}
#[test]
fn skip_too_narrow_slots_then_overflow_only_when_clear() {
    let p = prepare("abcdefgh tail", Options::default());
    let mut out = Vec::new();
    flow(&p, &[rect(30.0, 0.0, 40.0, 44.0)], &opts(100.0), &mut out);
    assert_eq!(out[0].line, 2);
    assert_eq!(out[0].x, 0.0);
    let p = prepare(
        "abcdefgh tail",
        Options {
            white_space: exact_textflow::WhiteSpace::Normal,
            overflow_wrap: OverflowWrap::Anywhere,
            ..Options::default()
        },
    );
    flow(&p, &[rect(30.0, 0.0, 40.0, 44.0)], &opts(100.0), &mut out);
    assert_eq!(out[0].line, 0);
    assert_eq!(out[0].width, 24.0);
    assert_eq!(out[1].line, 0);
    assert_eq!(out[1].x, 70.0);
}
#[test]
fn max_lines_includes_skipped_bands_and_invalid_height_is_empty() {
    let p = prepare("a b c d e", Options::default());
    let mut out = Vec::new();
    let mut options = opts(16.0);
    options.max_lines = 2;
    assert_eq!(flow(&p, &[], &options, &mut out).height, 44.0);
    assert_eq!(out.len(), 2);
    assert_eq!(
        flow(&p, &[rect(0.0, 0.0, 16.0, 44.0)], &options, &mut out).height,
        44.0
    );
    assert!(out.is_empty());
    for h in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        options.line_height = h;
        assert_eq!(flow(&p, &[], &options, &mut out).height, 0.0);
        assert!(out.is_empty());
    }
}
#[test]
fn guard_counterexample_and_narrow_column_progress_are_defined() {
    let p = prepare("a", Options::default());
    let mut out = Vec::new();
    // A full-width wall must be skipped without consuming the band guard.
    flow(
        &p,
        &[rect(0.0, 0.0, 100.0, 30_000.0)],
        &opts(100.0),
        &mut out,
    );
    assert_eq!(out.len(), 1);
    assert!(out[0].y >= 30_000.0);
    assert_eq!(out[0].end, 1);
    let text = "aa bb cc";
    let p = prepare(text, Options::default());
    flow(&p, &[rect(0.0, 0.0, 8.0, 100.0)], &opts(8.0), &mut out);
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|f| f.y >= 100.0));
    assert_eq!(out.last().unwrap().end, text.len());
    assert!(!flow(&p, &[], &opts(0.0), &mut out).complete);
    assert!(out.is_empty());
    for height in [1.0, 22.0, 220.0, 22_000.0] {
        flow(&p, &[rect(0.0, 0.0, 100.0, height)], &opts(100.0), &mut out);
        assert_eq!(out.last().unwrap().end, text.len());
    }
}
#[test]
fn long_emergency_word_completes_and_reuses_output() {
    let text = "a".repeat(20_000);
    let p = prepare(
        &text,
        Options {
            white_space: exact_textflow::WhiteSpace::Normal,
            overflow_wrap: OverflowWrap::Anywhere,
            ..Options::default()
        },
    );
    let mut out = Vec::new();
    assert_eq!(flow(&p, &[], &opts(8.0), &mut out).height, 20_000.0 * 22.0);
    assert_eq!(out.len(), 20_000);
    assert_eq!(out.last().unwrap().end, text.len());
    let capacity = out.capacity();
    let ptr = out.as_ptr();
    flow(&p, &[], &opts(8.0), &mut out);
    assert_eq!(out.capacity(), capacity);
    assert_eq!(out.as_ptr(), ptr);
}

#[test]
fn rtl_hebrew_and_arabic_start_in_the_right_interval() {
    for text in [
        "שלום עולם מילים רבות בעברית ".repeat(8),
        "مرحبا بالعالم هذه كلمات عربية ".repeat(8),
    ] {
        let p = prepare(&text, Options::default());
        let mut options = opts(400.0);
        options.direction = exact_textflow::Direction::Rtl;
        let mut out = Vec::new();
        flow(&p, &[rect(140.0, 0.0, 120.0, 100.0)], &options, &mut out);
        assert_eq!(out[0].x, 260.0);
        assert_eq!(out[0].start, 0);
        assert!(out
            .windows(2)
            .any(|p| p[0].line == p[1].line && p[0].x > p[1].x));
        for pair in out.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert_eq!(out.last().unwrap().end, text.len());
    }
}
