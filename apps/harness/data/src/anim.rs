//! Animations: pure functions of (name, tick, cols, rows) to a frame of
//! exactly `rows` lines, each exactly `cols` cells wide. `tick` counts
//! frames at about 30 per second. Pixel effects draw two pixels per cell
//! with "▀": the foreground is the top pixel, the background the bottom.

use crate::art::{hex, hsv};
use crate::state::{Line, Run};

/// Every animation's name.
pub const NAMES: [&str; 11] = [
    "plasma",
    "fire",
    "matrix",
    "spinners",
    "bars",
    "donut",
    "knot",
    "tesseract",
    "warp",
    "tunnel",
    "wave",
];

/// The frame `tick` of `name`, at `cols`×`rows` cells.
pub fn frame(name: &str, tick: f64, cols: usize, rows: usize) -> (String, Vec<Line>) {
    let cols = cols.min(1000);
    let rows = rows.min(500);
    let t = if tick.is_finite() { tick.max(0.0) } else { 0.0 };
    let lines = match name {
        "plasma" => plasma(t, cols, rows),
        "fire" => fire(t as u64, cols, rows),
        "matrix" => matrix(t as u64, cols, rows),
        "spinners" => spinners(t as u64, cols, rows),
        "bars" => bars(t as u64, cols, rows),
        other => match crate::ascii_anim::frame(other, t, cols, rows) {
            Some(lines) => lines,
            None => text_frame(&[format!("no animation named {other:?}")], cols, rows),
        },
    };
    (name.to_string(), lines)
}

fn hash(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

/// Rows of pixel colours to half-block lines.
fn pixels(cols: usize, rows: usize, px: impl Fn(usize, usize) -> (u8, u8, u8)) -> Vec<Line> {
    (0..rows)
        .map(|r| {
            let mut line = Line::default();
            for c in 0..cols {
                let (a, b, cc) = px(c, 2 * r);
                let (d, e, f) = px(c, 2 * r + 1);
                line.push(Run {
                    text: "▀".into(),
                    fg: hex(a, b, cc),
                    bg: hex(d, e, f),
                    ..Run::default()
                });
            }
            line
        })
        .collect()
}

fn plasma(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    let time = t * 0.08;
    let (w, h) = (cols.max(1) as f64, (2 * rows).max(1) as f64);
    pixels(cols, rows, |x, y| {
        let (u, v) = (x as f64 / w * 8.0, y as f64 / h * 8.0 * h / w * 2.0);
        let a = (u + time).sin()
            + ((v + time * 0.7) * 0.9).sin()
            + ((u + v + time * 1.3) * 0.5).sin()
            + (((u - 4.0).powi(2) + (v - 4.0).powi(2)).sqrt() * 1.2 - time * 1.5).sin();
        let k = a / 4.0;
        let r = 128.0 + 127.0 * (std::f64::consts::PI * k).sin();
        let g = 128.0 + 127.0 * (std::f64::consts::PI * k + 2.094).sin();
        let b = 128.0 + 127.0 * (std::f64::consts::PI * k + 4.188).sin();
        (r as u8, g as u8, b as u8)
    })
}

const FIRE: [(u8, u8, u8); 37] = [
    (7, 7, 7),
    (31, 7, 7),
    (47, 15, 7),
    (71, 15, 7),
    (87, 23, 7),
    (103, 31, 7),
    (119, 31, 7),
    (143, 39, 7),
    (159, 47, 7),
    (175, 63, 7),
    (191, 71, 7),
    (199, 71, 7),
    (223, 79, 7),
    (223, 87, 7),
    (223, 87, 7),
    (215, 95, 7),
    (215, 95, 7),
    (215, 103, 15),
    (207, 111, 15),
    (207, 119, 15),
    (207, 127, 15),
    (207, 135, 23),
    (199, 135, 23),
    (199, 143, 23),
    (199, 151, 31),
    (191, 159, 31),
    (191, 159, 31),
    (191, 167, 39),
    (191, 167, 39),
    (191, 175, 47),
    (183, 175, 47),
    (183, 183, 47),
    (183, 183, 55),
    (207, 207, 111),
    (223, 223, 159),
    (239, 239, 199),
    (255, 255, 255),
];

/// Doom's fire. A pixel `k` rows up came from the bottom row `k` steps
/// ago, so simulating the last `height` steps from black is the same
/// picture an endless simulation shows: a pure function of `tick`.
fn fire(tick: u64, cols: usize, rows: usize) -> Vec<Line> {
    let (w, h) = (cols, 2 * rows);
    if w == 0 || h == 0 {
        return vec![Line::default(); rows];
    }
    let mut p = vec![0u8; w * h];
    let top = (FIRE.len() - 1) as u8;
    for x in 0..w {
        p[(h - 1) * w + x] = top;
    }
    let first = tick.saturating_sub(h as u64 + 4);
    for step in first..=tick {
        for x in 0..w {
            for y in 1..h {
                let src = y * w + x;
                let r = hash(step.wrapping_mul(0x9e37) ^ ((src as u64) << 20)) & 3;
                let decay = (r & 1) as u8;
                let dx = (x + 3 * w + 1 - r as usize) % w;
                p[(y - 1) * w + dx] = p[src].saturating_sub(decay);
            }
        }
    }
    pixels(cols, rows, |x, y| FIRE[p[y * w + x] as usize])
}

const RAIN: &str = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜｦﾝ0123456789Z:.=*+<>";

fn matrix(tick: u64, cols: usize, rows: usize) -> Vec<Line> {
    let glyphs: Vec<char> = RAIN.chars().collect();
    let rows_i = rows as i64;
    (0..rows)
        .map(|r| {
            let mut line = Line::default();
            for c in 0..cols {
                let seed = hash(c as u64 + 1);
                let speed = 1 + seed % 3;
                let trail = 6 + (seed >> 8) % (rows as u64 / 2 + 6);
                let period = rows as u64 + trail + (seed >> 16) % 12;
                let head = ((tick / speed + (seed >> 24)) % period) as i64;
                let behind = head - r as i64;
                let lit = behind >= 0 && (behind as u64) < trail && head - behind < rows_i + 1;
                if !lit {
                    line.push(Run::plain(" "));
                    continue;
                }
                let g = glyphs[(hash(seed ^ ((r as u64) << 7) ^ ((tick / 5) * 31))
                    % glyphs.len() as u64) as usize];
                let fade = 1.0 - behind as f64 / trail as f64;
                let fg = if behind == 0 {
                    "#e8ffe8".to_string()
                } else {
                    let (r, g, b) = hsv(130.0, 0.9, 0.25 + 0.75 * fade);
                    hex(r, g, b)
                };
                line.push(Run {
                    text: g.to_string(),
                    fg,
                    bold: behind == 0,
                    ..Run::default()
                });
            }
            line
        })
        .collect()
}

/// Truncate or pad a line to exactly `cols` cells.
pub fn fit(line: Line, cols: usize) -> Line {
    let mut out = Line::default();
    let mut used = 0;
    for run in line.runs {
        let mut text = String::new();
        for g in run.text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(g).unwrap_or(0);
            if used + w > cols {
                break;
            }
            used += w;
            text.push(g);
        }
        out.push(Run { text, ..run });
        if used >= cols {
            break;
        }
    }
    if used < cols {
        out.push(Run::plain(" ".repeat(cols - used)));
    }
    out
}

fn text_frame(text: &[String], cols: usize, rows: usize) -> Vec<Line> {
    (0..rows)
        .map(|r| fit(Line::plain(text.get(r).cloned().unwrap_or_default()), cols))
        .collect()
}

const SPINNERS: [(&str, &[&str], u64); 10] = [
    (
        "braille",
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        2,
    ),
    ("dots", &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"], 3),
    ("line", &["|", "/", "-", "\\"], 3),
    ("arc", &["◜", "◠", "◝", "◞", "◡", "◟"], 3),
    (
        "ball",
        &[
            "( ●    )",
            "(  ●   )",
            "(   ●  )",
            "(    ● )",
            "(     ●)",
            "(    ● )",
            "(   ●  )",
            "(  ●   )",
            "( ●    )",
            "(●     )",
        ],
        3,
    ),
    ("moon", &["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"], 4),
    (
        "clock",
        &[
            "🕛", "🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚",
        ],
        3,
    ),
    (
        "grow",
        &["▁", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃"],
        2,
    ),
    ("arrows", &["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"], 3),
    ("toggle", &["⊶", "⊷"], 8),
];

fn spinners(tick: u64, cols: usize, rows: usize) -> Vec<Line> {
    (0..rows)
        .map(|r| {
            let mut line = Line::default();
            if let Some((name, frames, every)) = SPINNERS.get(r) {
                let glyph = frames[((tick / every) % frames.len() as u64) as usize];
                line.push(Run::dim(format!("{name:<9} ")));
                let (cr, cg, cb) = hsv(r as f64 * 36.0, 0.6, 1.0);
                line.push(Run::fg(glyph, &hex(cr, cg, cb)));
            }
            fit(line, cols)
        })
        .collect()
}

const EIGHTHS: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

fn bar(label: &str, frac: f64, cols: usize, hue: f64) -> Line {
    let mut line = Line::default();
    line.push(Run::dim(format!("{label:<8} ")));
    let width = cols.saturating_sub(9 + 6);
    let eighths = (frac.clamp(0.0, 1.0) * (width * 8) as f64).round() as usize;
    let (full, part) = (eighths / 8, eighths % 8);
    let (r, g, b) = hsv(hue, 0.7, 1.0);
    let fg = hex(r, g, b);
    let mut fill = "█".repeat(full);
    if full < width {
        fill.push_str(EIGHTHS[part]);
        fill.push_str(&" ".repeat(width - full - 1));
    }
    line.push(Run {
        text: fill,
        fg,
        bg: "#2c313a".into(),
        ..Run::default()
    });
    line.push(Run::plain(format!(
        " {:>3}%",
        (frac * 100.0).floor() as u32
    )));
    fit(line, cols)
}

fn bars(tick: u64, cols: usize, rows: usize) -> Vec<Line> {
    let speeds = [
        ("fast", 90u64, 120.0),
        ("medium", 240, 200.0),
        ("slow", 600, 30.0),
        ("steady", 1500, 280.0),
    ];
    let mut lines: Vec<Line> = speeds
        .iter()
        .map(|(label, period, hue)| bar(label, (tick % period) as f64 / *period as f64, cols, *hue))
        .collect();
    // An indeterminate bar: a block bouncing end to end.
    let width = cols.saturating_sub(9 + 6);
    let block = 6.min(width);
    let span = (width - block).max(1) as u64;
    let pos = tick % (2 * span);
    let at = if pos < span { pos } else { 2 * span - pos } as usize;
    let mut bounce = Line::default();
    bounce.push(Run::dim("working  "));
    bounce.push(Run {
        text: " ".repeat(at),
        bg: "#2c313a".into(),
        ..Run::default()
    });
    bounce.push(Run {
        text: "█".repeat(block),
        fg: "#61afef".into(),
        bg: "#2c313a".into(),
        ..Run::default()
    });
    bounce.push(Run {
        text: " ".repeat(width.saturating_sub(at + block)),
        bg: "#2c313a".into(),
        ..Run::default()
    });
    lines.push(fit(bounce, cols));
    (0..rows)
        .map(|r| match lines.get(r) {
            Some(l) => l.clone(),
            None => fit(Line::default(), cols),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn every_animation_fills_its_box_exactly() {
        for name in NAMES.iter().chain(&["nope"]) {
            for (cols, rows) in [(80, 24), (13, 5), (1, 1), (120, 40), (0, 3)] {
                for tick in [0.0, 1.0, 17.0, 1234.0, 100_000.0] {
                    let (_, lines) = frame(name, tick, cols, rows);
                    assert_eq!(lines.len(), rows, "{name} {cols}x{rows}");
                    for l in &lines {
                        assert_eq!(
                            l.text().width(),
                            cols,
                            "{name} {cols}x{rows} t{tick}: {:?}",
                            l.text()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn animations_move_and_are_fast() {
        for name in NAMES {
            assert_ne!(
                frame(name, 3.0, 80, 24).1,
                frame(name, 40.0, 80, 24).1,
                "{name}"
            );
            assert_eq!(
                frame(name, 7.0, 80, 24).1,
                frame(name, 7.0, 80, 24).1,
                "{name}"
            );
        }
        let start = std::time::Instant::now();
        for t in 0..30 {
            for name in NAMES {
                frame(name, t as f64, 80, 24);
            }
        }
        // 30 frames of each in debug well under a second each.
        assert!(start.elapsed().as_secs_f64() < 5.0, "{:?}", start.elapsed());
    }
}
