//! The region's ordering: the merge sort holds to core's stable sort.

use super::stable_order;

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
