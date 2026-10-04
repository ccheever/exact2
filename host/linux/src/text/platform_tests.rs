//! The platform text path over a stand-in shaper: every UTF-16 unit is half
//! an em wide but a mark, a joiner, a variation selector or a low surrogate
//! (zero, as Minikin gives a cluster's tail).
use super::platform::{Segment, Shaper, Style, ELLIPSIS};
use super::*;

struct Fake;

impl Shaper for Fake {
    fn declare(&self, _: u16, _: &[(String, u16, bool)]) -> bool {
        true
    }
    fn style(&self, style: &Style) -> u32 {
        style.weight as u32
    }
    fn measure(
        &self,
        chars: &[u16],
        segments: &[Segment],
        advances: &mut [f32],
        metrics: &mut [f32],
    ) {
        for (i, s) in segments.iter().enumerate() {
            for u in s.start..s.start + s.len {
                let c = chars[u as usize];
                let tail = (0xDC00..0xE000).contains(&c)
                    || (0x0300..0x0370).contains(&c)
                    || c == 0x200D
                    || c == 0xFE0F;
                advances[u as usize] = if tail { 0.0 } else { s.size * 0.5 };
            }
            metrics[3 * i..3 * i + 3].copy_from_slice(&[s.size * 0.9, s.size * 0.25, 0.0]);
        }
    }
    fn font_metrics(&self, _: u32, size: f32) -> [f32; 3] {
        [size * 0.9, size * 0.25, 0.0]
    }
}

fn engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::new_with(Some(&Fake)))
}

fn spec(text: &str, direction: exact_kernel::Direction) -> Spec {
    let style = exact_kernel::StyleProps {
        font_size: 10.0,
        direction,
        ..exact_kernel::StyleProps::default()
    };
    crate::paint::text_spec(&style, text)
}

#[test]
fn platform_widths_are_the_platform_advances() {
    let mut engine = engine();
    let p = engine.paragraph(&spec("abc de", exact_kernel::Direction::Ltr), None);
    assert_eq!(p.width, 30.0);
    assert_eq!(p.first_baseline, 9.0);
    let frags = engine
        .platform_fragments(
            &p,
            &[RunPaint {
                color: [0, 0, 0, 255],
                source: Default::default(),
            }],
        )
        .unwrap();
    assert_eq!(frags.len(), 1);
    assert_eq!(String::from_utf16_lossy(&frags[0].text), "abc de");
}

#[test]
fn platform_wraps_at_words_and_ellipsizes() {
    let mut engine = engine();
    let p = engine.paragraph(
        &spec("abc de fgh", exact_kernel::Direction::Ltr),
        Some(32.0),
    );
    assert_eq!(p.layout_runs().count(), 2);
    let mut nowrap = spec("abc de fgh", exact_kernel::Direction::Ltr);
    nowrap.white_space = exact_kernel::WhiteSpace::Nowrap;
    let p = engine.paragraph(&nowrap, None);
    let e = p.ellipsized(20.0).unwrap();
    let glyphs: Vec<_> = e.layout_runs().flat_map(|r| r.glyphs.to_vec()).collect();
    assert!(glyphs.iter().any(|g| g.glyph_id == ELLIPSIS));
}

#[test]
fn platform_shapes_mixed_scripts_marks_and_emoji() {
    let mut engine = engine();
    let palette = [RunPaint {
        color: [0, 0, 0, 255],
        source: Default::default(),
    }];
    for (text, dir) in [
        ("مرحبا بالعالم hello", exact_kernel::Direction::Ltr),
        ("שלום world עולם", exact_kernel::Direction::Rtl),
        ("नमस्ते दुनिया", exact_kernel::Direction::Ltr),
        (
            "family 👨\u{200D}👩\u{200D}👧 ★ é\u{301} ✌\u{FE0F}",
            exact_kernel::Direction::Ltr,
        ),
        ("", exact_kernel::Direction::Ltr),
        ("  lead and trail  ", exact_kernel::Direction::Ltr),
        ("tab\there", exact_kernel::Direction::Ltr),
    ] {
        let s = spec(text, dir);
        if s.is_empty() {
            continue;
        }
        for width in [None, Some(0.0), Some(25.0), Some(1000.0)] {
            let p = engine.paragraph(&s, width);
            let frags = engine.platform_fragments(&p, &palette).unwrap();
            let drawn: usize = frags.iter().map(|f| f.text.len()).sum();
            assert!(drawn > 0, "{text:?} at {width:?} drew nothing");
            let _ = p.ellipsized(10.0);
        }
    }
}

#[test]
fn platform_shapes_many_runs() {
    let mut engine = engine();
    let mut s = spec("Hello ", exact_kernel::Direction::Ltr);
    let run = |text: &str, weight: u16, size: f32| {
        let mut r = s.runs[0].clone();
        r.text = text.into();
        r.weight = weight;
        r.size = size;
        r
    };
    let runs = vec![
        run("Hello ", 400, 10.0),
        run("مرحبا", 700, 12.0),
        run(" 👍🏽 ", 400, 10.0),
        run("world", 600, 14.0),
        run("", 400, 10.0),
        run("\nnext line שלום", 400, 10.0),
    ];
    s.runs = runs;
    let palette = vec![
        RunPaint {
            color: [0, 0, 0, 255],
            source: Default::default()
        };
        s.runs.len()
    ];
    for width in [None, Some(0.0), Some(30.0), Some(1000.0)] {
        let p = engine.paragraph(&s, width);
        let frags = engine.platform_fragments(&p, &palette).unwrap();
        assert!(!frags.is_empty());
        let _ = p.ellipsized(10.0);
    }
}
