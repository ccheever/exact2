use crate::{GridTrack, GridTracks, LineHeight, StyleId, StyleProps, StyleValue};

/// xorshift64*, deterministic.
fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

#[test]
fn check_finite_names_the_row_the_row_by_row_check_names() {
    let numbers = [0.0, 1.5, -3.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut refused = 0;
    for _ in 0..4000 {
        let mut s = StyleProps::default();
        for _ in 0..(next(&mut state) % 12) {
            let id = StyleId::ALL[(next(&mut state) % StyleId::ALL.len() as u64) as usize];
            let n = numbers[(next(&mut state) % numbers.len() as u64) as usize];
            let m = numbers[(next(&mut state) % numbers.len() as u64) as usize];
            let value = match next(&mut state) % 3 {
                0 => StyleValue::Number(n),
                1 => StyleValue::Vec2(n as f32, m as f32),
                _ => StyleValue::Percent(n),
            };
            let _ = s.set_dynamic(id, &value);
        }
        // Rows set_dynamic refuses non-finite values for, written directly.
        if next(&mut state).is_multiple_of(4) {
            s.line_height = LineHeight::Length(f32::NAN);
            s.mask.set(StyleId::LineHeight);
        }
        if next(&mut state).is_multiple_of(4) {
            s.rare.grid_template_columns =
                GridTracks::from_tracks(vec![GridTrack::Fr(f32::INFINITY)]);
            if next(&mut state).is_multiple_of(2) {
                s.mask.set(StyleId::GridTemplateColumns);
            }
        }
        assert_eq!(s.check_finite(), s.check_finite_rows(), "{:?}", s.mask);
        refused += usize::from(s.check_finite().is_err());
    }
    assert!(refused > 500, "{refused} states with a non-finite row");
}

#[test]
fn line_height_css_is_the_text_format_wrote() {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut values = vec![0.0f32, 1.0, 1.5, 24.0, 0.1, 1e-7, 1e21, f32::MAX];
    values.extend((0..5000).map(|_| (next(&mut state) % 100_000) as f32 / 100.0));
    values.extend((0..5000).map(|_| f32::from_bits(next(&mut state) as u32 & 0x7fff_ffff)));
    for n in values.into_iter().filter(|n| n.is_finite()) {
        let shown = exact_num::Shortest32(n);
        assert_eq!(LineHeight::Number(n).css(), shown.to_string());
        assert_eq!(LineHeight::Length(n).css(), format!("{shown}px"));
    }
    assert_eq!(LineHeight::Normal.css(), "normal");
}
