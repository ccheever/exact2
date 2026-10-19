use super::{backgrounds::InlineBackground, *};
use crate::paint::Shape;
use exact_kernel::{Dimension, Direction, Env, StyleProps, WhiteSpace};

const GRAY: [u8; 4] = [242, 242, 247, 255];
const ALL: [bool; 4] = [true; 4];
const LEFT: [bool; 4] = [true, false, false, true];
const RIGHT: [bool; 4] = [false, true, true, false];

fn engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(
        &[
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice(),
            include_bytes!("../../tests/fonts/NotoSansArabic.ttf").as_slice(),
        ],
        "DejaVu Sans",
    ))
}

fn spec(texts: &[&str]) -> Spec {
    let style = StyleProps {
        font_size: 16.0,
        ..Default::default()
    };
    let mut spec = crate::paint::text_spec(&style, "");
    spec.runs = texts
        .iter()
        .map(|t| Run::from_style(t, exact_kernel::TextStyle::from_style(&style)))
        .collect();
    spec
}

fn background(id: u32) -> Option<InlineBackground> {
    Some(InlineBackground {
        color: GRAY,
        id,
        radii: [Dimension::Points(3.0); 4],
    })
}

fn fills(p: &Paragraph, backgrounds: &[Option<InlineBackground>]) -> Vec<Shape> {
    p.inline_backgrounds(backgrounds, &Env::default())
        .into_iter()
        .map(|(s, c)| {
            assert_eq!(c, GRAY);
            s
        })
        .collect()
}

fn corners(shape: &Shape) -> [bool; 4] {
    shape.radii.map(|(x, y)| x > 0.0 && y > 0.0)
}

#[test]
fn inline_background_rounds_one_box_and_keeps_same_color_neighbors_separate() {
    let mut e = engine();
    let s = spec(&["left ", "marked", " right"]);
    let p = e.paragraph(&s, Some(400.0));
    let one = fills(&p, &[None, background(1), None]);
    assert_eq!(one.len(), 1);
    assert_eq!(corners(&one[0]), ALL);
    assert!(one[0].rect.0 > 0.0);
    let pair = e.paragraph(&spec(&["one", "two"]), None);
    let two = fills(&pair, &[background(1), background(2)]);
    assert_eq!(two.len(), 2);
    assert!(two.iter().all(|s| corners(s) == ALL));
    assert!((two[0].rect.0 + two[0].rect.2 - two[1].rect.0).abs() < 0.01);
    assert!(
        (two.iter().map(|s| s.rect.2).sum::<f32>() - pair.layout_runs().next().unwrap().line_w)
            .abs()
            < 0.01
    );
    let same = fills(&pair, &[background(1), background(1)]);
    assert_eq!(same.len(), 1, "canonical runs within one box join");
    assert_eq!(corners(&same[0]), ALL);
    assert!(
        Rc::ptr_eq(&p, &e.paragraph(&s, Some(400.0))),
        "paint must preserve the shape cache"
    );
}

#[test]
fn inline_background_slices_wrapped_and_hard_lines_in_css_direction() {
    let mut e = engine();
    for direction in [Direction::Ltr, Direction::Rtl] {
        for (text, width, white_space) in [
            ("one two three four", 90.0, WhiteSpace::Normal),
            ("one two\nthree four", 400.0, WhiteSpace::PreWrap),
        ] {
            let mut s = spec(&[text]);
            s.direction = direction;
            s.white_space = white_space;
            let p = e.paragraph(&s, Some(width));
            let pieces = fills(&p, &[background(1)]);
            assert_eq!(pieces.len(), 2, "{text:?} {direction:?}");
            let (first, last) = if direction == Direction::Ltr {
                (LEFT, RIGHT)
            } else {
                (RIGHT, LEFT)
            };
            assert_eq!(corners(&pieces[0]), first, "{text:?} {direction:?}");
            assert_eq!(corners(&pieces[1]), last, "{text:?} {direction:?}");
            assert!(pieces[1].rect.1 > pieces[0].rect.1);
        }
    }
}

#[test]
fn inline_background_shares_cross_box_ligatures_by_characters_in_glyph_direction() {
    let mut e = engine();
    for (parts, direction) in [(["f", "i"], Direction::Ltr), (["ل", "ا"], Direction::Rtl)] {
        let mut s = spec(&parts);
        s.direction = direction;
        let p = e.paragraph(&s, None);
        let line = p.layout_runs().next().unwrap();
        let advancing: Vec<_> = line.glyphs.iter().filter(|g| g.w > 0.0).collect();
        assert_eq!(
            advancing.len(),
            1,
            "fixture must shape {parts:?} as one ligature: {:?}",
            line.glyphs
        );
        let g = *advancing[0];
        assert_eq!(g.range(), 0..parts.concat().len());
        let pieces = fills(&p, &[background(1), background(2)]);
        assert_eq!(pieces.len(), 2);
        for piece in &pieces {
            assert_eq!(corners(piece), ALL);
            assert!((piece.rect.2 - g.w / 2.0).abs() < 0.01);
        }
        // Leave only the first logical character rounded to expose which half
        // of the ligature owns it; Arabic's first character is on the right.
        let single = fills(&p, &[background(1), None]);
        assert_eq!(single.len(), 1);
        let expected = g.x
            + if direction == Direction::Rtl {
                g.w / 2.0
            } else {
                0.0
            };
        assert!((single[0].rect.0 - expected).abs() < 0.01, "{parts:?}");
    }
}

#[test]
fn inline_background_rounds_visual_outer_edges_of_mixed_bidi_pieces() {
    let mut e = engine();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let mut s = spec(&["abc ", "ابج 12", "3 دهو xyz"]);
        s.direction = direction;
        let p = e.paragraph(&s, Some(400.0));
        assert!(p
            .layout_runs()
            .flat_map(|l| l.glyphs)
            .all(|g| g.glyph_id != 0));
        let pieces = fills(&p, &[None, background(1), None]);
        assert_eq!(
            pieces.len(),
            2,
            "fixture separates the box's bidi pieces: {direction:?} {:?}",
            p.layout_runs().next().unwrap().glyphs
        );
        assert_eq!(corners(&pieces[0]), LEFT);
        assert_eq!(corners(&pieces[1]), RIGHT);
        assert!(pieces[0].rect.0 + pieces[0].rect.2 < pieces[1].rect.0);
    }
}

#[test]
fn inline_background_counts_zero_width_and_trimmed_endpoints() {
    let mut e = engine();
    let p = e.paragraph(&spec(&["\u{200b}hello"]), None);
    assert_eq!(corners(&fills(&p, &[background(1)])[0]), ALL);
    let p = e.paragraph(&spec(&["abc ", "restless"]), Some(50.0));
    assert_eq!(p.layout_runs().count(), 2);
    let pieces = fills(&p, &[background(1), None]);
    assert_eq!(pieces.len(), 1);
    assert_eq!(corners(&pieces[0]), ALL);
    let visible = e.paragraph(&spec(&["abc"]), None);
    assert!(
        (pieces[0].rect.2 - visible.layout_runs().next().unwrap().line_w).abs() < 0.01,
        "the trimmed endpoint must not add space to the background"
    );
}

#[test]
fn inline_background_ellipsis_never_supplies_the_box_end() {
    let mut e = engine();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let mut s = spec(&["abcdefghijklmnopqrstuvwxyz"]);
        s.direction = direction;
        s.white_space = WhiteSpace::Nowrap;
        let p = e.paragraph(&s, Some(400.0));
        // A cut of one character still must not give the token that endpoint.
        for width in [80.0, p.width - 1.0] {
            let cut = p.ellipsized(width).unwrap();
            assert!(cut
                .layout_runs()
                .next()
                .unwrap()
                .glyphs
                .iter()
                .any(|g| g.range().is_empty()));
            let pieces = fills(&cut, &[background(1)]);
            assert_eq!(pieces.len(), 1);
            // Latin under RTL is truncated at the visual left, which removes
            // its first logical character. CSS direction puts the end there.
            assert_eq!(corners(&pieces[0]), LEFT);
        }
    }
    let mut s = spec(&["one two three four five"]);
    s.line_clamp = 1;
    let p = e.paragraph(&s, Some(90.0));
    assert_eq!(corners(&fills(&p, &[background(1)])[0]), LEFT);
    let flowed = e.paragraph_flow(
        &s,
        90.0,
        &[exact_textflow::FlowShape::Circle {
            cx: 80.0,
            cy: 10.0,
            r: 10.0,
        }],
        None,
    );
    assert!(!flowed.flow_incomplete());
    assert!(!flowed.fragments().is_empty());
    assert!(flowed
        .layout_runs()
        .next()
        .unwrap()
        .glyphs
        .last()
        .unwrap()
        .range()
        .is_empty());
    assert_eq!(corners(&fills(&flowed, &[background(1)])[0]), LEFT);
}

#[test]
fn inline_background_resolves_percentage_and_calc_radii_against_each_piece() {
    let mut e = engine();
    let p = e.paragraph(&spec(&["one two three four"]), Some(90.0));
    for radius in [
        Dimension::Percent(25.0),
        Dimension::Calc(10.0, 2.0),
        Dimension::parse_calc("calc(25% + 1px)").unwrap(),
    ] {
        let mut bg = background(1).unwrap();
        bg.radii = [radius; 4];
        let pieces = fills(&p, &[Some(bg)]);
        assert_eq!(pieces.len(), 2);
        for (piece, corner) in [(&pieces[0], 0), (&pieces[1], 1)] {
            let expected = (
                radius.at_basis(&Env::default(), piece.rect.2),
                radius.at_basis(&Env::default(), piece.rect.3),
            );
            assert!((piece.radii[corner].0 - expected.0).abs() < 0.01);
            assert!((piece.radii[corner].1 - expected.1).abs() < 0.01);
        }
    }
    let mut bg = background(1).unwrap();
    bg.radii = [Dimension::Percent(100.0); 4];
    let p = e.paragraph(&spec(&["round"]), None);
    let piece = fills(&p, &[Some(bg)])[0];
    assert_eq!(
        piece.radii,
        [(piece.rect.2 / 2.0, piece.rect.3 / 2.0); 4],
        "CSS reduces all radii by one factor"
    );
}

#[test]
fn inline_background_zero_radius_keeps_the_square_path() {
    let mut e = engine();
    let p = e.paragraph(&spec(&["one", "two"]), None);
    let backgrounds = [1, 2].map(|id| {
        Some(InlineBackground {
            radii: [Dimension::Points(0.0); 4],
            ..background(id).unwrap()
        })
    });
    let actual = p.inline_backgrounds(&backgrounds, &Env::default());
    let expected: Vec<_> = p
        .run_backgrounds(&[Some(GRAY), Some(GRAY)])
        .into_iter()
        .map(|(r, c)| (Shape::rect(r), c))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 1);
}

fn exclusion() -> [exact_textflow::FlowShape; 1] {
    [exact_textflow::FlowShape::RoundRect {
        x: 64.0,
        y: 0.0,
        width: 160.0,
        height: 100.0,
        radius: 0.0,
    }]
}

#[test]
fn flowed_background_generated_hyphen_has_source_owner() {
    let mut e = engine();
    let p = e.paragraph_flow(&spec(&["é", "ab\u{ad}cdefghij"]), 400.0, &exclusion(), None);
    assert!(!p.flow_incomplete());
    assert!(p.fragments()[0].hyphenated, "{:?}", p.fragments());
    let pieces = fills(&p, &[None, background(1)]);
    assert_eq!(pieces.len(), 2);
    assert_eq!(corners(&pieces[0]), LEFT);
    assert_eq!(corners(&pieces[1]), RIGHT);
    let line = p.layout_runs().next().unwrap();
    let hyphen = line.glyphs.last().unwrap();
    assert_eq!(hyphen.run(), 1);
    assert!((pieces[0].rect.0 + pieces[0].rect.2 - hyphen.x - hyphen.w).abs() < 0.01);
}

#[test]
fn flowed_background_generated_spaces_keep_breaks_open() {
    let mut e = engine();
    for text in ["Eab cd ef\ngh ij kl", "Éab cd ef\ngh ij kl"] {
        let p = e.paragraph_flow(&spec(&[text]), 300.0, &exclusion(), None);
        assert!(!p.flow_incomplete());
        assert!(p.fragments().len() > 1);
        let pieces = fills(&p, &[background(1)]);
        assert_eq!(corners(&pieces[0]), LEFT, "{text:?}");
        assert_eq!(corners(pieces.last().unwrap()), RIGHT, "{text:?}");
        for piece in &pieces[1..pieces.len() - 1] {
            assert_eq!(corners(piece), [false; 4], "{text:?}");
        }
    }
}

#[test]
fn flowed_background_counts_consumed_zero_width_endpoint() {
    let mut e = engine();
    let p = e.paragraph_flow(&spec(&["\u{200b}hello"]), 400.0, &exclusion(), None);
    assert!(!p.flow_incomplete());
    let pieces = fills(&p, &[background(1)]);
    assert_eq!(pieces.len(), 1);
    assert_eq!(corners(&pieces[0]), ALL);
}

#[test]
fn flowed_background_counts_consumed_space_endpoint() {
    let mut e = engine();
    let p = e.paragraph_flow(&spec(&["abc ", "restless"]), 400.0, &exclusion(), None);
    assert!(!p.flow_incomplete());
    assert_eq!(p.fragments().len(), 2);
    let pieces = fills(&p, &[background(1), None]);
    assert_eq!(pieces.len(), 1);
    assert_eq!(corners(&pieces[0]), ALL);
    let visible = e.paragraph(&spec(&["abc"]), None);
    assert!((pieces[0].rect.2 - visible.layout_runs().next().unwrap().line_w).abs() < 0.01);
}

#[test]
fn inline_background_many_runs() {
    let mut e = engine();
    for count in [2_500, 6_000] {
        let p = e.paragraph(&spec(&vec!["abcdefghi "; count]), Some(1000.0));
        for all in [false, true] {
            let backgrounds: Vec<_> = (0..count)
                .map(|i| {
                    if all || i + 1 == count {
                        background(i as u32)
                    } else {
                        None
                    }
                })
                .collect();
            let start = std::time::Instant::now();
            let pieces = fills(&p, &backgrounds);
            eprintln!(
                "backgrounds: {count} runs, all={all}: {:?}",
                start.elapsed()
            );
            assert_eq!(pieces.len(), if all { count } else { 1 });
            assert!(pieces.iter().all(|p| corners(p) == ALL));
        }
    }
}

#[test]
fn flowed_background_clamp_excludes_consumed_but_truncated_endpoint() {
    let mut e = engine();
    let mut s = spec(&["one two ", "three four"]);
    s.line_clamp = 1;
    let p = e.paragraph_flow(
        &s,
        90.0,
        &[exact_textflow::FlowShape::Circle {
            cx: 80.0,
            cy: 10.0,
            r: 10.0,
        }],
        None,
    );
    assert_eq!(p.fragments().len(), 1);
    assert_eq!(p.fragments()[0].end, s.runs[0].text.len());
    assert!(
        p.layout_runs()
            .flat_map(|l| l.glyphs)
            .map(|g| g.end as usize)
            .max()
            .unwrap()
            < s.runs[0].text.len()
    );
    let pieces = fills(&p, &[background(1), None]);
    assert_eq!(pieces.len(), 1);
    assert_eq!(corners(&pieces[0]), LEFT);
}

#[test]
fn flowed_background_spaces_and_later_hard_lines_keep_run_ownership() {
    let mut e = engine();
    let s = spec(&["Éab", " cd\nef", " gh ij kl"]);
    let p = e.paragraph_flow(&s, 300.0, &exclusion(), None);
    assert!(!p.flow_incomplete());
    let mut ends = Vec::new();
    let mut end = 0;
    for run in &s.runs {
        end += run.text.len();
        ends.push(end);
    }
    for line in p.layout_runs() {
        for g in line.glyphs {
            assert!(!line.text[g.range()].is_empty());
            assert_eq!(
                g.run(),
                ends.partition_point(|end| *end <= g.start as usize)
            );
        }
    }
    let pieces = fills(&p, &[None, background(1), None]);
    assert_eq!(pieces.len(), 2);
    assert_eq!(corners(&pieces[0]), LEFT);
    assert_eq!(corners(&pieces[1]), RIGHT);
}

#[test]
fn flowed_background_clamp_keeps_consumed_space_before_cut() {
    let mut e = engine();
    let mut s = spec(&["one ", "twothreefour"]);
    s.line_clamp = 1;
    let p = e.paragraph_flow(
        &s,
        90.0,
        &[exact_textflow::FlowShape::Circle {
            cx: 80.0,
            cy: 10.0,
            r: 10.0,
        }],
        None,
    );
    assert_eq!(p.fragments().len(), 1);
    assert_eq!(p.fragments()[0].end, s.runs[0].text.len());
    let pieces = fills(&p, &[background(1), None]);
    assert_eq!(pieces.len(), 1);
    assert_eq!(corners(&pieces[0]), ALL);
}
