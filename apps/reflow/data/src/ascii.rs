//! ASCII art in a proportional face: each row advances by the real width of
//! the glyph just chosen, so the picture lines up although no two characters
//! are the same width. The naive mode samples on a uniform grid instead, and
//! the same rows come out warped — the difference measuring makes.
use crate::font::REGULAR;
use exact_plan::Value;

/// Font size and line height of a row (the Contract sets the same).
pub const SIZE: f32 = 13.0;
/// Row height, px.
pub const ROW: f32 = 13.0;
/// Rows of the picture.
pub const ROWS: usize = 32;
/// Lightest to darkest.
const RAMP: [char; 13] = [
    ' ', '.', '·', ':', '-', '~', '=', '+', '*', 'x', '#', '%', '@',
];

/// Ink darkness at a pixel of a `width` × `height` picture: a lit sphere
/// inside a tilted ring, the light circling with the clock.
fn darkness(x: f32, y: f32, width: f32, height: f32, elapsed: f32) -> f32 {
    let (cx, cy) = (width * 0.5, height * 0.47);
    let r = height * 0.34;
    let theta = elapsed / 1400.0;
    let light = {
        let (lx, ly, lz) = (theta.cos(), -0.55f32, 0.7f32);
        let n = (lx * lx + ly * ly + lz * lz).sqrt();
        (lx / n, ly / n, lz / n)
    };
    let (dx, dy) = (x - cx, y - cy);
    // The ring: an ellipse band, in front of the sphere below its centre.
    let (a, b) = (width * 0.43, height * 0.115);
    let e = (dx * dx) / (a * a) + (dy * dy) / (b * b);
    let on_ring = (0.66..=1.0).contains(&e);
    let inside_sphere = dx * dx + dy * dy < r * r;
    if on_ring && (dy > 0.0 || !inside_sphere) {
        let angle = dy.atan2(dx);
        return 0.42 + 0.22 * angle.sin() + 0.12 * (e - 0.83).abs() * 4.0;
    }
    if inside_sphere {
        let nz = (r * r - dx * dx - dy * dy).sqrt() / r;
        let (nx, ny) = (dx / r, dy / r);
        let lit = (nx * light.0 + ny * light.1 + nz * light.2).max(0.0);
        let rim = (1.0 - nz).powi(3) * 0.25;
        return (1.0 - (0.06 + 0.94 * lit) + rim).clamp(0.0, 1.0);
    }
    0.0
}

fn pick(dark: f32) -> char {
    let i = (dark.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32).round() as usize;
    RAMP[i]
}

/// `ascii(elapsedMs, measured, width)`: the rows of the picture.
pub fn ascii(elapsed: f32, measured: bool, width: f32) -> Value {
    let height = ROWS as f32 * ROW;
    let uniform = RAMP
        .iter()
        .map(|&c| REGULAR.width(&c.to_string(), SIZE))
        .sum::<f32>()
        / RAMP.len() as f32;
    let rows = (0..ROWS)
        .map(|row| {
            let y = row as f32 * ROW + ROW * 0.5;
            let mut text = String::new();
            let mut x = 0.0f32;
            let mut index = 0usize;
            while x < width {
                let sample_x = if measured { x } else { index as f32 * uniform };
                let c = pick(darkness(sample_x + 2.0, y, width, height, elapsed));
                let before = text.len();
                text.push(c);
                let advance = if measured {
                    // Advance by what the host will draw: the glyph, kerned to its neighbour.
                    let tail = &text[text.char_indices().nth_back(1).map_or(before, |(i, _)| i)..];
                    let pair = REGULAR.width(tail, SIZE);
                    let prev = REGULAR.width(&tail[..tail.len() - c.len_utf8()], SIZE);
                    pair - prev
                } else {
                    uniform
                };
                x += advance.max(0.5);
                index += 1;
                if index > 400 {
                    break;
                }
            }
            Value::record(vec![
                Value::str(&format!("row-{row}")),
                Value::str(text.trim_end()),
            ])
        })
        .collect();
    Value::record(vec![
        Value::Number(width as f64),
        Value::Number(height as f64),
        Value::list(rows),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_rows_meet_the_width_and_naive_rows_do_not() {
        let m = ascii(0.0, true, 400.0);
        let n = ascii(0.0, false, 400.0);
        let rows = |v: &Value| -> Vec<String> {
            let Value::Record(f) = v else { panic!() };
            let Value::List(rows) = &f[2] else { panic!() };
            rows.iter()
                .map(|r| {
                    let Value::Record(f) = r else { panic!() };
                    f[1].as_str().unwrap().to_owned()
                })
                .collect()
        };
        let (m, n) = (rows(&m), rows(&n));
        assert_eq!(m.len(), ROWS);
        let widest = m
            .iter()
            .map(|r| REGULAR.width(r, SIZE))
            .fold(0f32, f32::max);
        assert!(widest <= 400.0 + 12.0, "{widest}");
        assert!(m.iter().any(|r| r.contains('@')), "a shadow side exists");
        // The naive rows drift: at least one row measures far from any measured row.
        let drift = n
            .iter()
            .map(|r| REGULAR.width(r, SIZE))
            .fold(0f32, |a, w| a.max((w - 400.0).abs()));
        assert!(drift > 20.0, "{drift}");
    }
}
