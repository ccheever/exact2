use crate::support;
use exact_textflow::{Cursor, Options, OverflowWrap, Prepared};
use support::{advance, lines, prepare, CORPUS};

fn options(mode: OverflowWrap) -> Options {
    Options {
        white_space: exact_textflow::WhiteSpace::Normal,
        overflow_wrap: mode,
        hyphen_advance: 8.0,
    }
}
fn ranges(text: &str, width: f32, mode: OverflowWrap) -> Vec<(String, f32)> {
    lines(&prepare(text, options(mode)), width)
        .iter()
        .map(|l| (text[l.start.byte..l.end.byte].to_owned(), l.width))
        .collect()
}
#[test]
fn latin_hanging_and_multiple_spaces() {
    assert_eq!(
        ranges("one two three", 56.0, OverflowWrap::Normal),
        [("one two ".into(), 56.0), ("three".into(), 40.0)]
    );
    assert_eq!(
        ranges("one   two", 24.0, OverflowWrap::Normal),
        [("one   ".into(), 24.0), ("two".into(), 24.0)]
    );
    assert_eq!(
        ranges("one                  ", 24.0, OverflowWrap::Normal),
        [("one                  ".into(), 24.0)]
    );
    assert_eq!(
        ranges("  one   two  ", 56.0, OverflowWrap::Normal),
        [("  one   two  ".into(), 56.0)]
    );
}
#[test]
fn hard_breaks_blank_lines_and_terminal_newline() {
    let p = prepare(
        "one\ntwo\u{2028}\nthree\r\n",
        Options {
            white_space: exact_textflow::WhiteSpace::PreWrap,
            ..Options::default()
        },
    );
    let ls = lines(&p, 1000.0);
    assert_eq!(ls.len(), 4);
    assert!(ls.iter().all(|l| l.hard_break));
    assert_eq!(
        ls.iter().map(|l| l.width).collect::<Vec<_>>(),
        [24.0, 24.0, 0.0, 40.0]
    );
    assert_eq!(
        ls.last().unwrap().end.byte,
        "one\ntwo\u{2028}\nthree\r\n".len()
    );
    assert_eq!(
        prepare(
            "\n",
            Options {
                white_space: exact_textflow::WhiteSpace::PreWrap,
                ..Options::default()
            }
        )
        .line_stats(10.0),
        (1, 0.0)
    );
}
#[test]
fn soft_hyphen_only_costs_width_when_selected() {
    let text = "ab\u{ad}cdef";
    let p = prepare(text, options(OverflowWrap::Normal));
    let ls = lines(&p, 24.0);
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[0].width, 24.0);
    assert!(ls[0].hyphenated);
    assert_eq!(&text[..ls[0].end.byte], "ab\u{ad}");
    assert_eq!(ls[1].width, 32.0);
    let wide = lines(&p, 48.0);
    assert_eq!(wide.len(), 1);
    assert_eq!(wide[0].width, 48.0);
    assert!(!wide[0].hyphenated);
    let terminal = lines(&prepare("ab\u{ad}", options(OverflowWrap::Normal)), 16.0);
    assert_eq!(terminal[0].width, 16.0);
    assert!(!terminal[0].hyphenated);
    let retreat = lines(
        &prepare("x ab\u{ad}cdef", options(OverflowWrap::Normal)),
        32.0,
    );
    assert_eq!(retreat[0].end.byte, 2); // The visible hyphen would not fit after "x ab".
}
#[test]
fn all_overflow_wrap_modes_and_min_content() {
    for mode in [
        OverflowWrap::Normal,
        OverflowWrap::BreakWord,
        OverflowWrap::Anywhere,
    ] {
        let p = prepare("abcdefgh", options(mode));
        let ls = lines(&p, 24.0);
        if mode == OverflowWrap::Normal {
            assert_eq!(ls.len(), 1);
            assert_eq!(ls[0].width, 64.0);
        } else {
            assert_eq!(
                ls.iter()
                    .map(|l| (l.start.byte, l.end.byte, l.width))
                    .collect::<Vec<_>>(),
                [(0, 3, 24.0), (3, 6, 24.0), (6, 8, 16.0)]
            );
        }
        assert_eq!(p.natural_width(), 64.0);
        assert_eq!(
            p.min_content_width(),
            if mode == OverflowWrap::Anywhere {
                8.0
            } else {
                64.0
            }
        );
    }
    assert_eq!(
        ranges("a abcdef", 24.0, OverflowWrap::Anywhere),
        [
            ("a ".into(), 8.0),
            ("abc".into(), 24.0),
            ("def".into(), 24.0)
        ]
    );
}
#[test]
fn cjk_opener_closer_and_url_opportunities() {
    assert_eq!(
        ranges("中文日本", 32.0, OverflowWrap::Normal),
        [("中文".into(), 32.0), ("日本".into(), 32.0)]
    );
    assert_eq!(
        ranges("中「文」日", 32.0, OverflowWrap::Normal),
        [
            ("中".into(), 16.0),
            ("「文」".into(), 48.0),
            ("日".into(), 16.0)
        ]
    );
    // Chrome 154 keeps a URL's path whole and breaks only after `?`: its
    // Latin-1 pair table, not UAX #14's break after `/`.
    let url = "https://example.com/a/b?x=1&y=2";
    let p = prepare(url, Options::default());
    let ls = lines(&p, 80.0);
    assert!(ls.len() > 1);
    assert_eq!(&url[..ls[0].end.byte], "https://example.com/a/b?");
    assert_eq!(ls.last().unwrap().end.byte, url.len());
}
#[test]
fn logical_rtl_ranges_and_empty_inputs() {
    let text = "مرحبا بالعالم שלום עולם";
    let ls = lines(&prepare(text, Options::default()), 48.0);
    assert_eq!(ls.len(), 4);
    assert_eq!(&text[..ls[0].end.byte], "مرحبا ");
    for pair in ls.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    for empty in ["", " ", "\t  "] {
        assert!(lines(&prepare(empty, Options::default()), 0.0).is_empty());
    }
}
// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn cached_measurements_send_sync_and_invalid_advances() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Prepared>();
    let text = "first second third";
    let mut calls = Vec::new();
    let p = Prepared::new(text, Options::default(), &mut |r: std::ops::Range<
        usize,
    >| {
        calls.push(r.clone());
        advance(&text[r])
    });
    let before = calls.len();
    assert_eq!(before, 5);
    let unique: std::collections::HashSet<_> = calls.iter().map(|r| (r.start, r.end)).collect();
    assert_eq!(unique.len(), before);
    for width in [0.0, 24.0, 80.0, f32::INFINITY] {
        p.line_stats(width);
        p.natural_width();
        p.min_content_width();
    }
    assert_eq!(calls.len(), before);
    let p = Prepared::new(
        "a b",
        Options {
            hyphen_advance: f32::NAN,
            ..Options::default()
        },
        &mut |_| f32::NAN,
    );
    assert_eq!(p.line_stats(0.0), (1, 0.0));
    let mut bad = Cursor::default();
    bad.byte = 1;
    assert!(p.next_line(bad, 10.0).is_none());
}
#[test]
fn stats_and_coverage_for_the_corpus_at_many_widths() {
    for text in CORPUS {
        for mode in [
            OverflowWrap::Normal,
            OverflowWrap::BreakWord,
            OverflowWrap::Anywhere,
        ] {
            let p = prepare(text, options(mode));
            for width in [0.0, 8.0, 24.0, 64.0, 640.0, f32::INFINITY] {
                let ls = lines(&p, width);
                assert_eq!(
                    p.line_stats(width),
                    (ls.len(), ls.iter().map(|l| l.width).fold(0.0, f32::max))
                );
                let mut at = 0;
                for l in &ls {
                    assert_eq!(l.start.byte, at);
                    assert!(text.is_char_boundary(l.end.byte));
                    at = l.end.byte;
                }
                assert!(
                    text[at..].chars().all(char::is_whitespace),
                    "uncovered {text:?}"
                );
            }
        }
    }
    let p = prepare("one two\nthree", Options::default());
    assert_eq!(p.natural_width(), 104.0);
    assert_eq!(p.min_content_width(), 40.0);
    let p = prepare("ab\u{ad}cd", options(OverflowWrap::Normal));
    assert_eq!(p.min_content_width(), 24.0);
}

// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn pretext_emergency_wrapping_preserves_complete_graphemes() {
    for cluster in [
        "e\u{301}",
        "👩‍💻",
        "👍🏽",
        "क्ष",
        "❤️",
        "☀︎",
        "🇺🇸",
        "ب\u{650}",
        "का",
        "a\u{20dd}",
        "a\u{f7f}",
        "a\u{897}",
        "a\u{113b8}",
    ] {
        let text = format!("a{cluster}b");
        for mode in [OverflowWrap::BreakWord, OverflowWrap::Anywhere] {
            let ls = lines(&prepare(&text, options(mode)), 1.0);
            assert!(
                ls.iter()
                    .any(|l| &text[l.start.byte..l.end.byte] == cluster),
                "split {cluster:?}: {ls:?}"
            );
        }
    }
}
// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn pretext_empty_collapsed_whitespace_and_glue() {
    assert!(lines(&prepare("  \t  ", Options::default()), 200.0).is_empty());
    assert_eq!(
        prepare("  Hello\t   World  ", Options::default()).natural_width(),
        88.0
    );
    for text in [
        "Hello\u{a0}world",
        "10\u{202f}000",
        "a\u{2007}\u{2007}b",
        "foo\u{2060}bar",
    ] {
        assert_eq!(
            lines(&prepare(text, Options::default()), 1.0).len(),
            1,
            "{text}"
        );
    }
}
// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn pretext_zero_width_space_and_overflowing_first_glyph() {
    for text in ["\u{200b}", "\u{200b}\u{200b}"] {
        let ls = lines(&prepare(text, Options::default()), 0.0);
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0].width, 0.0);
        assert_eq!(ls[0].end.byte, text.len());
    }
    for separator in [" ", "\u{200b}"] {
        let text = format!("字{separator}字");
        let ls = lines(&prepare(&text, Options::default()), 5.0);
        assert_eq!(ls.len(), 2);
        assert_eq!(&text[..ls[0].end.byte], format!("字{separator}"));
    }
    assert_eq!(
        prepare("alpha\u{200b}beta", Options::default()).line_stats(40.0),
        (2, 40.0)
    );
}
// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn pretext_negative_width_matches_zero_and_selected_shy_threshold() {
    for tail in ["", " ab\u{ad}cd"] {
        let text = format!("\u{200b}\u{200b}b{tail}");
        let p = prepare(&text, options(OverflowWrap::Normal));
        assert_eq!(lines(&p, -5.0), lines(&p, 0.0));
    }
    let text = "foo trans\u{ad}atlantic said \"hello\" to 世界 and waved.";
    let p = prepare(text, options(OverflowWrap::BreakWord));
    let ls = lines(&p, 88.0);
    assert_eq!(&text[..ls[0].end.byte], "foo trans\u{ad}");
    assert!(ls[0].hyphenated);
    assert_eq!(ls[0].width, 80.0);
}
// Ported from Pretext (MIT, © Pretext contributors): src/layout.test.ts
#[test]
fn pretext_forward_combining_carry_and_run_of_openers() {
    for text in ["「tail", "「「tail", "「「「「字"] {
        assert_eq!(lines(&prepare(text, Options::default()), 1.0).len(), 1);
    }
    let text = "漢字\u{301}日本";
    let ls = lines(&prepare(text, Options::default()), 16.0);
    assert_eq!(&text[ls[1].start.byte..ls[1].end.byte], "字\u{301}");
}
#[test]
fn worst_case_long_word_emergency_work_is_linear_in_cached_clusters() {
    let text = "a".repeat(20_000);
    let mut calls = 0;
    let p = Prepared::new(
        &text,
        options(OverflowWrap::Anywhere),
        &mut |r: std::ops::Range<usize>| {
            calls += 1;
            r.len() as f32
        },
    );
    assert_eq!(calls, 20_001);
    assert_eq!(p.line_stats(1.0), (20_000, 1.0));
}

#[test]
fn internal_space_runs_collapse_even_after_an_opener() {
    assert_eq!(
        prepare("(   abc)", Options::default()).natural_width(),
        48.0
    );
    let text = "a \u{301}b";
    let p = prepare(text, options(OverflowWrap::Anywhere));
    for l in lines(&p, 8.0) {
        assert!(!text[l.start.byte..l.end.byte].starts_with('\u{301}'));
    }
}

#[test]
fn emergency_cuts_still_collapse_and_hang_internal_spaces() {
    assert_eq!(
        ranges("(   abc)", 8.0, OverflowWrap::Anywhere),
        [
            ("(   ".into(), 8.0),
            ("a".into(), 8.0),
            ("b".into(), 8.0),
            ("c".into(), 8.0),
            (")".into(), 8.0)
        ]
    );
    assert_eq!(
        ranges("(   abc)", 1.0, OverflowWrap::Anywhere),
        [
            ("(   ".into(), 8.0),
            ("a".into(), 8.0),
            ("b".into(), 8.0),
            ("c".into(), 8.0),
            (")".into(), 8.0)
        ]
    );
}

#[test]
fn a_space_break_after_soft_hyphen_does_not_paint_the_hyphen() {
    let text = "ab\u{ad}   cd";
    let p = prepare(text, options(OverflowWrap::Normal));
    let ls = lines(&p, 16.0);
    assert_eq!(ls.len(), 2);
    assert_eq!(&text[..ls[0].end.byte], "ab\u{ad}   ");
    assert_eq!(ls[0].width, 16.0);
    assert!(!ls[0].hyphenated);
    assert_eq!(p.min_content_width(), 16.0);
}

#[test]
fn css_normal_transforms_segment_breaks_and_collapses_runs() {
    let text = "alpha\n \t\r\n\u{85}\u{2028}\u{2029}beta";
    let p = Prepared::new(text, Options::default(), &mut |r: std::ops::Range<
        usize,
    >| {
        if text[r.clone()].chars().all(char::is_whitespace) {
            8.0
        } else {
            advance(&text[r])
        }
    });
    let got = lines(&p, 1000.0);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].width, 80.0);
    assert!(!got[0].hard_break);
    assert_eq!(got[0].end.byte, text.len());
}

#[test]
fn pre_wrap_preserves_spaces_tabs_and_hangs_trailing_space() {
    let text = "  A    B\tC   \r\nD";
    let p = Prepared::new(
        text,
        Options {
            white_space: exact_textflow::WhiteSpace::PreWrap,
            ..Options::default()
        },
        &mut |r: std::ops::Range<usize>| {
            text[r]
                .chars()
                .map(|c| if c == '\t' { 32.0 } else { 8.0 })
                .sum()
        },
    );
    let got = lines(&p, 1000.0);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].width, 104.0);
    assert!(got[0].hard_break);
    assert_eq!(got[1].width, 8.0);
    assert_eq!(p.paint_range(text, 0..got[0].end.byte), 0..text.len() - 6);
    let hanging = prepare(
        "A    B",
        Options {
            white_space: exact_textflow::WhiteSpace::PreWrap,
            ..Options::default()
        },
    );
    let split = lines(&hanging, 8.0);
    assert_eq!(split.len(), 2);
    assert_eq!(split[0].width, 8.0);
    assert_eq!(split[0].end.byte, 5);
}

#[test]
fn pre_line_keeps_line_feeds_and_collapses_the_rest_as_chrome_renders() {
    // @ref LLP 1053 §0 G5 — each source's lines as Chrome 154 renders them
    // under `white-space: pre-line` (a monospace face; `None` is max-content,
    // a width is in characters), whitespace runs shown collapsed.
    let pre_line = Options {
        white_space: exact_textflow::WhiteSpace::PreLine,
        ..options(OverflowWrap::Normal)
    };
    for (text, width, chrome) in [
        ("a    b", None, &["a b"][..]),
        ("a\nb", None, &["a", "b"]),
        ("a\n\nb", None, &["a", "", "b"]),
        ("a  \n  b", None, &["a", "b"]),
        ("  lead\n  mid  \ntrail  ", None, &["lead", "mid", "trail"]),
        (
            "one two three four five six",
            Some(10),
            &["one two", "three four", "five six"],
        ),
        ("\nfirst", None, &["", "first"]),
        ("last\n", None, &["last"]),
        ("a\rb", None, &["a b"]),
        ("a\r\nb", None, &["a", "b"]),
        ("a\tb\t\nc", None, &["a b", "c"]),
    ] {
        let p = prepare(text, pre_line);
        let got: Vec<(String, f32)> = lines(&p, width.map_or(f32::INFINITY, |w| w as f32 * 8.0))
            .iter()
            .map(|l| {
                let painted = &text[p.paint_range(text, l.start.byte..l.end.byte)];
                let shown = painted.split([' ', '\t', '\r']).filter(|w| !w.is_empty());
                (shown.collect::<Vec<_>>().join(" "), l.width)
            })
            .collect();
        let want: Vec<(String, f32)> = chrome
            .iter()
            .map(|l| (l.to_string(), l.chars().count() as f32 * 8.0))
            .collect();
        assert_eq!(got, want, "{text:?}");
    }
    // min-content is the longest word; max-content the longest forced line.
    let p = prepare("one two\nthree", pre_line);
    assert_eq!(p.min_content_width(), 5.0 * 8.0);
    assert_eq!(p.natural_width(), 7.0 * 8.0);
    assert_eq!(p.line_stats(f32::INFINITY), (2, 7.0 * 8.0));
}

#[test]
fn nowrap_collapses_whitespace_and_never_soft_wraps() {
    let text = "one   two\nthree four";
    let p = prepare(
        text,
        Options {
            white_space: exact_textflow::WhiteSpace::Nowrap,
            overflow_wrap: OverflowWrap::Anywhere,
            hyphen_advance: 8.0,
        },
    );
    let got = lines(&p, 24.0);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].end.byte, text.len());
    assert!(!got[0].hard_break);
    // "one two three four": whitespace runs collapse to one space each.
    assert_eq!(got[0].width, 144.0);
    assert_eq!(p.line_stats(24.0), (1, 144.0));
    assert_eq!(p.min_content_width(), 144.0);
    assert_eq!(p.natural_width(), 144.0);
    assert_eq!(p.paint_range(text, 0..text.len()), 0..text.len());
}

#[test]
fn pre_preserves_and_breaks_only_where_forced() {
    // @ref LLP 1053 G5 — `pre` is preserve × nowrap: spaces and tabs are
    // text, a line feed ends a line, no width ends one, and a line's
    // trailing spaces are kept rather than hung.
    let text = "  one   two  \nthree four";
    let p = prepare(
        text,
        Options {
            white_space: exact_textflow::WhiteSpace::Pre,
            overflow_wrap: OverflowWrap::Anywhere,
            hyphen_advance: 8.0,
        },
    );
    let got = lines(&p, 24.0);
    assert_eq!(got.len(), 2);
    assert!(got[0].hard_break);
    assert_eq!(
        got[0].width,
        13.0 * 8.0,
        "\"  one   two  \" keeps every space"
    );
    assert_eq!(got[1].width, 10.0 * 8.0);
    assert_eq!(p.line_stats(24.0), (2, 13.0 * 8.0));
    assert_eq!(p.min_content_width(), 13.0 * 8.0);
    assert_eq!(p.natural_width(), 13.0 * 8.0);
    // Spaces carry no ink wherever they are kept.
    assert_eq!(p.paint_range(text, 0..14), 0..11);
}

#[test]
fn sub_pixel_fit_tolerance_matches_engine_rounding() {
    // Advances summing to within 0.005 past the offer still fit, as with
    // Pretext's lineFitEpsilon; anything further past still breaks.
    let text = "aa bb";
    let widths = |scale: f32| {
        Prepared::new(text, Options::default(), &mut |r: std::ops::Range<
            usize,
        >| {
            text[r].chars().count() as f32 * scale
        })
    };
    assert_eq!(widths(2.5).line_stats(12.499), (1, 12.5));
    assert_eq!(widths(2.5).line_stats(12.494), (2, 5.0));
    assert_eq!(widths(2.5).count_lines(12.499), 1);
}

#[test]
fn fast_path_agrees_with_streaming_walker_on_fuzz() {
    // Deterministic fuzz over scripts, glue, breaks and fractional widths:
    // line_stats (fast under overflow-wrap: normal) must equal the
    // next_line loop exactly, in count and in largest advance.
    let pieces = [
        "word",
        "a",
        " ",
        "  ",
        "\t",
        "\n",
        "\u{ad}",
        "\u{200b}",
        " ",
        "中文",
        "日本語",
        "👩\u{200d}💻",
        "e\u{301}",
        "مرحبا",
        "https://x.io/a?b=1",
        "“quoted”",
        "well-known",
        "3.14",
        ".",
        ",",
        "(",
        ")",
        "-",
    ];
    let mut seed: u64 = 0x12345678;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    let widths = [
        0.0,
        1.0,
        7.5,
        8.0,
        8.5,
        23.999,
        24.0,
        24.001,
        40.0,
        100.0,
        640.0,
        f32::INFINITY,
        f32::NAN,
    ];
    for round in 0..300 {
        let n = 1 + next() % 12;
        let text: String = (0..n)
            .map(|_| pieces[next() % pieces.len()])
            .collect::<Vec<_>>()
            .join("");
        for ws in [
            exact_textflow::WhiteSpace::Normal,
            exact_textflow::WhiteSpace::PreWrap,
            exact_textflow::WhiteSpace::Nowrap,
            exact_textflow::WhiteSpace::PreLine,
            exact_textflow::WhiteSpace::Pre,
        ] {
            for mode in [
                OverflowWrap::Normal,
                OverflowWrap::BreakWord,
                OverflowWrap::Anywhere,
            ] {
                let p = Prepared::new(
                    &text,
                    Options {
                        white_space: ws,
                        overflow_wrap: mode,
                        hyphen_advance: 8.0,
                    },
                    &mut |r: std::ops::Range<usize>| advance(&text[r]),
                );
                for &w in &widths {
                    let slow = lines(&p, w);
                    let (count, max) = p.line_stats(w);
                    assert_eq!(count, slow.len(), "count {text:?} {ws:?} {mode:?} {w}");
                    let slow_max = slow.iter().map(|l| l.width).fold(0.0, f32::max);
                    assert_eq!(max, slow_max, "max {text:?} {ws:?} {mode:?} {w}");
                    assert_eq!(p.count_lines(w), count, "count_lines {round}");
                }
            }
        }
    }
}

/// A no-break space after an ordinary one may start a line, as UAX #14 LB12a
/// and Chrome allow; it still joins a word it follows directly.
#[test]
fn a_break_before_no_break_glue_after_a_space_stays() {
    let text = "hello \u{a0}world";
    assert_eq!(exact_textflow::line_breaks(text, &[]), [6, text.len()]);
    assert_eq!(exact_textflow::line_breaks("a\u{a0}b c", &[]), [5, 6]);
}
