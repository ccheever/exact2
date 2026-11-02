// CSS `text-box` (Inline Layout 3 §4.2): the trim is cut to the root inline
// box's font, the one CSS font matching picks, even when it has no Latin `x`.
use super::*;
use exact_kernel::{AxisOffer, StyleProps, TextBoxEdge, TextBoxTrim};

/// Noto Sans Arabic as the paragraph's font, with DejaVu Sans installed to
/// fall back to: the Arabic face has no `x`, and its own cap height is 416 of
/// its 1000 units (OS/2), where DejaVu's is 1493 of 2048.
#[test]
fn a_face_without_an_x_is_still_its_own_root() {
    let mut engine = TextEngine::with_catalog(catalog::Catalog::from_bytes(
        &[
            include_bytes!("../../tests/fonts/NotoSansArabic.ttf").as_slice(),
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice(),
        ],
        "Noto Sans Arabic",
    ));
    let mut spec = crate::paint::text_spec(&StyleProps::default(), "بيت");
    spec.text_box_trim = TextBoxTrim::TrimBoth;
    spec.text_box_edge = TextBoxEdge::CapAlphabetic;
    let size = spec.strut.size;
    let trimmed = engine.measure(&spec, AxisOffer::Definite(200.0));
    let baseline = trimmed.first_baseline.unwrap();
    assert!(
        (baseline - size * 0.416).abs() < 0.05,
        "the Arabic face's own cap height, not DejaVu's ({}): {trimmed:?}",
        size * 1493.0 / 2048.0
    );
}

/// A line shorter than its glyphs is cut by negative amounts: the paragraph
/// grows, and its last line ends at the grown height, as a column of it is
/// that tall.
#[test]
fn the_last_line_ends_at_the_trimmed_height() {
    let (mut height, mut baselines, mut bottoms) = (20.0, vec![8.0, 18.0], vec![10.0, 20.0]);
    shaping::trim_lines((-5.0, -5.0), &mut height, &mut baselines, &mut bottoms);
    assert_eq!(height, 30.0);
    assert_eq!(baselines, [13.0, 23.0]);
    assert_eq!(bottoms, [15.0, 30.0]);
}
