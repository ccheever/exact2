//! Timestamp arithmetic, independent of GPU or host clock access.
pub fn milliseconds(start: u64, end: u64, period: f32) -> Result<f64, String> {
    let ticks = end.wrapping_sub(start);
    if (start == 0 && end == 0) || ticks > u64::MAX / 2 || !period.is_finite() || period <= 0.0 {
        return Err(format!(
            "invalid timestamp pair: start={start} end={end} period={period}"
        ));
    }
    Ok(ticks as f64 * period as f64 / 1e6)
}
#[cfg(test)]
mod tests {
    #[test]
    fn timestamp_wrap_and_invalid_counters() {
        let cases = [
            (100, 200, 1.0, Some(0.0001)),
            (u64::MAX - 3, 2, 1.0, Some(0.000006)),
            (100, 0, 1.0, None),
            (0, 0, 1.0, None),
            (100, 200, f32::NAN, None),
        ];
        let mut failures = Vec::new();
        for (a, b, period, expected) in cases {
            let actual = super::milliseconds(a, b, period);
            let ok = match (expected, &actual) {
                (Some(v), Ok(x)) => (*x - v).abs() < 1e-12,
                (None, Err(_)) => true,
                _ => false,
            };
            println!(
                "timestamp start={a} end={b} period={period} actual={actual:?} expected={expected:?}"
            );
            if !ok {
                failures.push((a, b));
            }
        }
        println!("timestamp_cases=5 failures={failures:?}");
        assert!(failures.is_empty());
    }
}
