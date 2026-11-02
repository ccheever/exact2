//! The region's keys and ordering: a key's text is what `format!` wrote,
//! and the merge sort holds to core's stable sort.

use super::{key_text, stable_order};
use exact_plan::Value;

#[test]
fn a_key_is_the_text_format_wrote() {
    for s in ["", "a", "é💬", "s:x"] {
        assert_eq!(key_text(&Value::str(s)), Some(format!("s:{s}")));
    }
    for n in [0.0, -0.0, 1.0, -1.5, 0.1 + 0.2, 1e21, 1e-7, 123_456_789.0] {
        let shown = if n == 0.0 { 0.0 } else { n };
        assert_eq!(key_text(&Value::Number(n)), Some(format!("n:{shown}")));
    }
    for b in [false, true] {
        assert_eq!(key_text(&Value::Bool(b)), Some(format!("b:{b}")));
    }
    assert_eq!(key_text(&Value::Number(f64::NAN)), None);
    assert_eq!(key_text(&Value::Unit), None);
}

/// xorshift64*, deterministic.
fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

#[test]
fn stable_order_is_the_order_a_stable_sort_gives() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for n in (0..70).chain([257, 1000]) {
        let keys: Vec<u64> = (0..n).map(|_| next(&mut state) % 7).collect();
        let mut expected: Vec<usize> = (0..n).collect();
        expected.sort_by_key(|&i| keys[i]);
        assert_eq!(
            stable_order(n, &|i, j| keys[i] < keys[j]),
            expected,
            "{keys:?}"
        );
    }
}
