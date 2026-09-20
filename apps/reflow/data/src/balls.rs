//! Three spheres crossing a paragraph, positioned by arithmetic on the clock.
//! @ref LLP 1043.000 §3 D8 — shapes move by commits, never transforms.
use crate::prose::BALLS;
use exact_plan::Value;

const BALLS_SPEC: [(f64, f64, f64, f64, f64, &str); 3] = [
    (
        58.0,
        0.061,
        0.043,
        40.0,
        30.0,
        "light-dark(#c2603f, #e39a77)",
    ),
    (
        40.0,
        -0.047,
        0.071,
        360.0,
        200.0,
        "light-dark(#4f7d6a, #93bda6)",
    ),
    (
        30.0,
        0.083,
        -0.052,
        200.0,
        120.0,
        "light-dark(#b89a48, #d9c27a)",
    ),
];

/// Bounce `p` between `lo` and `hi` without history: a triangle wave.
fn reflect(p: f64, lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return lo;
    }
    let span = hi - lo;
    let m = (p - lo).rem_euclid(2.0 * span);
    lo + if m > span { 2.0 * span - m } else { m }
}

/// `balls(elapsedMs, width, height)`.
pub fn balls(elapsed: f64, width: f64, height: f64) -> Value {
    let balls = BALLS_SPEC
        .iter()
        .enumerate()
        .map(|(i, &(r, vx, vy, x0, y0, color))| {
            let x = reflect(x0 + vx * elapsed, 0.0, (width - 2.0 * r).max(0.0));
            let y = reflect(y0 + vy * elapsed, 0.0, (height - 2.0 * r).max(0.0));
            Value::record(vec![
                Value::str(&format!("ball-{}", i + 1)),
                Value::Number(x.round()),
                Value::Number(y.round()),
                Value::Number(r),
                Value::str(color),
            ])
        })
        .collect();
    Value::record(vec![Value::str(BALLS), Value::list(balls)])
}
