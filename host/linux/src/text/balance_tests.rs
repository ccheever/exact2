// CSS `text-wrap-style: balance` (LLP 1001 §6) against Chrome: the issue's
// sentence in 16px DejaVu Sans (the fixture face, loaded in Chrome as a web
// font) in a 240px box breaks greedily as [208.61, 230.39, 74.8] and
// balanced as [174.33, 170.72, 168.75]; centred, the balanced lines centre
// in the box (the first starts at 32.83), and justified all but the last
// fill it ([240, 240, 168.75]).
use super::*;
use exact_kernel::{StyleProps, TextAlign, TextWrapStyle};

const SENTENCE: &str = "Settle every thread in this section and keep the newest one open";

fn engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(
        &[include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice()],
        "DejaVu Sans",
    ))
}

fn spec(text: &str, balance: bool, align: TextAlign) -> Spec {
    let mut spec = crate::paint::text_spec(&StyleProps::default(), text);
    assert_eq!(spec.strut.size, 16.0);
    spec.align = align;
    if balance {
        spec.text_wrap_style = TextWrapStyle::Balance;
    }
    spec
}

fn widths(engine: &mut TextEngine, spec: &Spec, width: f32) -> (Vec<f32>, Vec<f32>, f32) {
    let p = engine.paragraph(spec, Some(width));
    let lines = p.lines();
    let starts = lines
        .lines
        .iter()
        .map(|l| lines.glyphs_of(l)[0].x)
        .collect();
    (lines.lines.iter().map(|l| l.w).collect(), starts, p.width)
}

fn near(actual: &[f32], chrome: &[f32]) -> bool {
    actual.len() == chrome.len() && actual.iter().zip(chrome).all(|(a, c)| (a - c).abs() < 0.5)
}

#[test]
fn the_issues_sentence_balances_as_chromes_does() {
    let mut engine = engine();
    let (greedy, _, measured) = widths(&mut engine, &spec(SENTENCE, false, TextAlign::Left), 240.0);
    assert!(near(&greedy, &[208.61, 230.39, 74.8]), "greedy {greedy:?}");
    let (balanced, _, width) = widths(&mut engine, &spec(SENTENCE, true, TextAlign::Left), 240.0);
    assert!(
        near(&balanced, &[174.33, 170.72, 168.75]),
        "balanced {balanced:?}"
    );
    assert_eq!(width, measured, "balancing moves breaks, never the box");
    let (_, starts, _) = widths(&mut engine, &spec(SENTENCE, true, TextAlign::Center), 240.0);
    assert!(
        near(&starts, &[32.83, 34.64, 35.63]),
        "centred in the box: {starts:?}"
    );
    let (justified, _, _) = widths(
        &mut engine,
        &spec(SENTENCE, true, TextAlign::Justify),
        240.0,
    );
    assert!(
        near(&justified, &[240.0, 240.0, 168.75]),
        "justified, the balanced lines fill the box: {justified:?}"
    );
}

#[test]
fn seven_lines_and_a_clamp_stay_greedy() {
    let mut engine = engine();
    let seven = ["aa"; 20].join(" ");
    let (greedy, ..) = widths(&mut engine, &spec(&seven, false, TextAlign::Left), 60.0);
    assert!(greedy.len() > 6, "{greedy:?}");
    let (balanced, ..) = widths(&mut engine, &spec(&seven, true, TextAlign::Left), 60.0);
    assert_eq!(balanced, greedy);
    let mut clamped = spec(SENTENCE, true, TextAlign::Left);
    clamped.line_clamp = 3;
    let (lines, ..) = widths(&mut engine, &clamped, 240.0);
    assert!(near(&lines, &[208.61, 230.39, 74.8]), "clamped {lines:?}");
}
