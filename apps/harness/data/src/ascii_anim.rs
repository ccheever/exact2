//! Animated ASCII art: the demoscene in a terminal. Each is a pure function
//! of (tick, cols, rows) to exactly `rows` lines of exactly `cols` cells,
//! like the others in `anim` — a donut lit by a moving light, a torus knot
//! and a tesseract drawn in Braille (2×4 dots a cell), a warp starfield, a
//! texture tunnel, and a waving logo.

use crate::art::{hex, hsv};
use crate::state::{Line, Run};

/// The ASCII animations' names.
#[cfg(test)]
const NAMES: [&str; 6] = ["donut", "knot", "tesseract", "warp", "tunnel", "wave"];

/// The frame `tick` of `name`, or `None` when it isn't one of these.
pub fn frame(name: &str, t: f64, cols: usize, rows: usize) -> Option<Vec<Line>> {
    Some(match name {
        "donut" => donut(t, cols, rows),
        "knot" => knot(t, cols, rows),
        "tesseract" => tesseract(t, cols, rows),
        "warp" => warp(t, cols, rows),
        "tunnel" => tunnel(t, cols, rows),
        "wave" => wave(t, cols, rows),
        _ => return None,
    })
}

/// A cell: its character, colour, and weight.
#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    rgb: Option<(u8, u8, u8)>,
    bold: bool,
}

const BLANK: Cell = Cell {
    ch: ' ',
    rgb: None,
    bold: false,
};

/// A grid of cells to lines, runs merged where styles match.
fn lines(grid: &[Cell], cols: usize, rows: usize) -> Vec<Line> {
    (0..rows)
        .map(|r| {
            let mut line = Line::default();
            for cell in &grid[r * cols..(r + 1) * cols] {
                line.push(Run {
                    text: cell.ch.to_string(),
                    fg: cell.rgb.map_or(String::new(), |(a, b, c)| hex(a, b, c)),
                    bold: cell.bold,
                    ..Run::default()
                });
            }
            line
        })
        .collect()
}

fn scale((r, g, b): (u8, u8, u8), k: f64) -> (u8, u8, u8) {
    let k = k.clamp(0.0, 1.0);
    (
        (r as f64 * k) as u8,
        (g as f64 * k) as u8,
        (b as f64 * k) as u8,
    )
}

/// The spinning donut (after Andy Sloane's donut.c): a torus lit from
/// above and behind, shaded with `.,-~:;=!*#$@`, coloured by where on the
/// tube each point sits.
fn donut(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    const RAMP: &[u8] = b".,-~:;=!*#$@";
    let mut grid = vec![BLANK; cols * rows];
    let mut depth = vec![0.0f64; cols * rows];
    let (a, b) = (t * 0.07, t * 0.035);
    let (sa, ca, sb, cb) = (a.sin(), a.cos(), b.sin(), b.cos());
    let (r1, r2, k2) = (1.0, 2.0, 5.0);
    // Fit the torus to the smaller of the width and twice the height
    // (cells are twice as tall as they are wide).
    let size = (cols as f64).min(rows as f64 * 2.0);
    let k1 = size * k2 * 3.0 / (8.0 * (r1 + r2));
    let mut theta = 0.0f64;
    while theta < std::f64::consts::TAU {
        let (st, ct) = theta.sin_cos();
        let mut phi = 0.0f64;
        while phi < std::f64::consts::TAU {
            let (sp, cp) = phi.sin_cos();
            let (cx, cy) = (r2 + r1 * ct, r1 * st);
            let x = cx * (cb * cp + sa * sb * sp) - cy * ca * sb;
            let y = cx * (sb * cp - sa * cb * sp) + cy * ca * cb;
            let z = k2 + ca * cx * sp + cy * sa;
            let ooz = 1.0 / z;
            let xp = (cols as f64 / 2.0 + k1 * ooz * x) as isize;
            let yp = (rows as f64 / 2.0 - k1 * ooz * y * 0.5) as isize;
            let lum = cp * ct * sb - ca * ct * sp - sa * st + cb * (ca * st - ct * sa * sp);
            if xp >= 0 && yp >= 0 && (xp as usize) < cols && (yp as usize) < rows && lum > 0.0 {
                let i = yp as usize * cols + xp as usize;
                if ooz > depth[i] {
                    depth[i] = ooz;
                    let level = ((lum * 8.0) as usize).min(RAMP.len() - 1);
                    let hue = phi.to_degrees() + t * 4.0;
                    grid[i] = Cell {
                        ch: RAMP[level] as char,
                        rgb: Some(hsv(hue, 0.65, 0.35 + 0.65 * (lum / 1.42))),
                        bold: level > 8,
                    };
                }
            }
            phi += 0.02;
        }
        theta += 0.07;
    }
    lines(&grid, cols, rows)
}

/// A dot's colour and how near it is.
type Lit = ((u8, u8, u8), f64);

/// A Braille canvas: 2×4 dots a cell, each cell coloured by its nearest dot.
struct Braille {
    cols: usize,
    rows: usize,
    bits: Vec<u8>,
    colour: Vec<Option<Lit>>,
}

impl Braille {
    fn new(cols: usize, rows: usize) -> Braille {
        Braille {
            cols,
            rows,
            bits: vec![0; cols * rows],
            colour: vec![None; cols * rows],
        }
    }

    fn dot(&mut self, x: isize, y: isize, rgb: (u8, u8, u8), near: f64) {
        if x < 0 || y < 0 || x as usize >= self.cols * 2 || y as usize >= self.rows * 4 {
            return;
        }
        let (cx, cy) = (x as usize / 2, y as usize / 4);
        let (dx, dy) = (x as usize % 2, y as usize % 4);
        const BIT: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
        let i = cy * self.cols + cx;
        self.bits[i] |= BIT[dx][dy];
        if self.colour[i].is_none_or(|(_, n)| near > n) {
            self.colour[i] = Some((rgb, near));
        }
    }

    /// A line between two dots (Bresenham), its colour and nearness
    /// interpolated from the ends.
    fn line(
        &mut self,
        (x0, y0, n0): (f64, f64, f64),
        (x1, y1, n1): (f64, f64, f64),
        rgb: impl Fn(f64) -> (u8, u8, u8),
    ) {
        let (mut x, mut y) = (x0.round() as isize, y0.round() as isize);
        let (xe, ye) = (x1.round() as isize, y1.round() as isize);
        let (dx, dy) = ((xe - x).abs(), -(ye - y).abs());
        let (sx, sy) = (if x < xe { 1 } else { -1 }, if y < ye { 1 } else { -1 });
        let steps = dx.max(-dy).max(1) as f64;
        let mut err = dx + dy;
        let mut k = 0.0;
        loop {
            let f = k / steps;
            let near = n0 + (n1 - n0) * f;
            self.dot(x, y, rgb(near), near);
            if x == xe && y == ye {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
            k += 1.0;
        }
    }

    fn lines(&self) -> Vec<Line> {
        let grid: Vec<Cell> = self
            .bits
            .iter()
            .zip(&self.colour)
            .map(|(bits, colour)| match (bits, colour) {
                (0, _) => BLANK,
                (b, c) => Cell {
                    ch: char::from_u32(0x2800 + *b as u32).unwrap_or(' '),
                    rgb: c.map(|(rgb, _)| rgb),
                    bold: false,
                },
            })
            .collect();
        lines(&grid, self.cols, self.rows)
    }
}

/// Project a point in camera space to Braille dots, with its nearness.
fn project(p: [f64; 3], cols: usize, rows: usize, zoom: f64) -> (f64, f64, f64) {
    let (w, h) = (cols as f64 * 2.0, rows as f64 * 4.0);
    let d = 4.0 / (4.0 + p[2]);
    let s = w.min(h) * zoom * d;
    (w / 2.0 + p[0] * s, h / 2.0 - p[1] * s, d)
}

fn rotate(p: [f64; 3], ax: f64, ay: f64, az: f64) -> [f64; 3] {
    let [x, y, z] = p;
    let (s, c) = ax.sin_cos();
    let (y, z) = (y * c - z * s, y * s + z * c);
    let (s, c) = ay.sin_cos();
    let (x, z) = (x * c + z * s, -x * s + z * c);
    let (s, c) = az.sin_cos();
    let (x, y) = (x * c - y * s, x * s + y * c);
    [x, y, z]
}

/// A (3, 5) torus knot turning in Braille, its colour running along it.
fn knot(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    let mut canvas = Braille::new(cols, rows);
    let (p, q) = (3.0, 5.0);
    let n = 900;
    let point = |i: usize| {
        let u = i as f64 / n as f64 * std::f64::consts::TAU;
        let r = (q * u).cos() * 0.45 + 1.0;
        let pt = [r * (p * u).cos(), r * (p * u).sin(), (q * u).sin() * 0.45];
        let pt = rotate(pt, t * 0.031, t * 0.023, t * 0.011);
        (project(pt, cols, rows, 0.24), u)
    };
    let mut prev = point(0);
    for i in 1..=n {
        let next = point(i);
        let hue = prev.1.to_degrees() + t * 3.0;
        canvas.line(prev.0, next.0, |near| {
            scale(hsv(hue, 0.75, 1.0), 0.35 + (near - 0.6) * 1.6)
        });
        prev = next;
    }
    canvas.lines()
}

/// A tesseract turning in two 4D planes at once, projected to 3D and then
/// to the screen; nearer edges brighter.
fn tesseract(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    let mut canvas = Braille::new(cols, rows);
    let verts: Vec<[f64; 4]> = (0..16)
        .map(|i| std::array::from_fn(|k| if i >> k & 1 == 1 { 1.0 } else { -1.0 }))
        .collect();
    let (a, b) = (t * 0.03, t * 0.021);
    let project4 = |v: [f64; 4]| {
        // Rotate in the XW and YZ planes.
        let (s, c) = a.sin_cos();
        let (x, w) = (v[0] * c - v[3] * s, v[0] * s + v[3] * c);
        let (s, c) = b.sin_cos();
        let (y, z) = (v[1] * c - v[2] * s, v[1] * s + v[2] * c);
        let d = 2.5 / (2.5 - w);
        let p = rotate([x * d, y * d, z * d], 0.45, t * 0.012, 0.0);
        (project(p, cols, rows, 0.17), w)
    };
    let projected: Vec<_> = verts.iter().map(|v| project4(*v)).collect();
    for i in 0..16 {
        for k in 0..4 {
            let j = i ^ (1 << k);
            if j > i {
                let ((pa, wa), (pb, wb)) = (projected[i], projected[j]);
                let hue = 180.0 + (wa + wb) * 50.0 + t;
                canvas.line(pa, pb, |near| {
                    scale(hsv(hue, 0.6, 1.0), 0.45 + (near - 0.6) * 1.4)
                });
            }
        }
    }
    canvas.lines()
}

fn hash(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

fn unit(seed: u64) -> f64 {
    (hash(seed) >> 11) as f64 / (1u64 << 53) as f64
}

/// Flying through a starfield: each star rushes outward and brightens as
/// it nears, leaving a short trail.
fn warp(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    let mut grid = vec![BLANK; cols * rows];
    let (cx, cy) = (cols as f64 / 2.0, rows as f64 / 2.0);
    let put = |grid: &mut Vec<Cell>, x: f64, y: f64, cell: Cell| {
        if x >= 0.0 && y >= 0.0 && (x as usize) < cols && (y as usize) < rows {
            let i = y as usize * cols + x as usize;
            if grid[i].ch == ' ' || cell.bold {
                grid[i] = cell;
            }
        }
    };
    for s in 0..420u64 {
        let (x0, y0) = (unit(s * 3) * 2.0 - 1.0, unit(s * 3 + 1) * 2.0 - 1.0);
        let z = (unit(s * 3 + 2) - t * 0.012).rem_euclid(1.0).max(0.02);
        let at = |z: f64| (cx + x0 / z * cx * 0.5, cy + y0 / z * cy * 0.5);
        let near = 1.0 - z;
        let tint = if s % 7 == 0 {
            (255, 210, 160)
        } else {
            (190, 210, 255)
        };
        // The trail first, dim, then the star.
        let (tx, ty) = at((z + 0.06).min(1.0));
        put(
            &mut grid,
            tx,
            ty,
            Cell {
                ch: '·',
                rgb: Some(scale(tint, near * 0.5)),
                bold: false,
            },
        );
        let (x, y) = at(z);
        let ch = match near {
            n if n > 0.9 => '✦',
            n if n > 0.75 => '*',
            n if n > 0.5 => '+',
            n if n > 0.25 => '·',
            _ => '.',
        };
        put(
            &mut grid,
            x,
            y,
            Cell {
                ch,
                rgb: Some(scale(tint, 0.3 + near)),
                bold: near > 0.75,
            },
        );
    }
    lines(&grid, cols, rows)
}

/// The demoscene tunnel: a checkered texture mapped by angle and inverse
/// distance, rushing towards the viewer, fogged at the far end.
fn tunnel(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let mut grid = vec![BLANK; cols * rows];
    let (cx, cy) = (
        cols as f64 / 2.0 + (t * 0.03).sin() * cols as f64 * 0.12,
        rows as f64 / 2.0,
    );
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (
                (c as f64 - cx) / (cols as f64 / 2.0),
                (r as f64 - cy) / (rows as f64 / 2.0) * 0.6,
            );
            let dist = (x * x + y * y).sqrt().max(1e-3);
            let u = y.atan2(x) / std::f64::consts::PI * 8.0 + t * 0.02;
            let v = 0.6 / dist + t * 0.08;
            let check = ((u.floor() as i64) ^ (v.floor() as i64)) & 1 == 1;
            let fog = (dist * 1.6).min(1.0);
            let level =
                ((if check { 0.95 } else { 0.45 }) * fog * (RAMP.len() - 1) as f64) as usize;
            grid[r * cols + c] = Cell {
                ch: RAMP[level.min(RAMP.len() - 1)] as char,
                rgb: Some(scale(hsv(v * 40.0 + 200.0, 0.7, 1.0), 0.25 + fog * 0.75)),
                bold: check && fog > 0.8,
            };
        }
    }
    lines(&grid, cols, rows)
}

/// A 7-row block font for the logo.
fn glyph(c: char) -> [&'static str; 7] {
    match c {
        'E' => [
            "██████",
            "██    ",
            "██    ",
            "█████ ",
            "██    ",
            "██    ",
            "██████",
        ],
        'X' => [
            "██  ██",
            "██  ██",
            " ████ ",
            "  ██  ",
            " ████ ",
            "██  ██",
            "██  ██",
        ],
        'A' => [
            " ████ ",
            "██  ██",
            "██  ██",
            "██████",
            "██  ██",
            "██  ██",
            "██  ██",
        ],
        'C' => [
            " █████",
            "██    ",
            "██    ",
            "██    ",
            "██    ",
            "██    ",
            " █████",
        ],
        'T' => [
            "██████",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "  ██  ",
        ],
        _ => ["      "; 7],
    }
}

/// "EXACT" in big letters riding a sine wave, a rainbow sweeping across
/// and a shadow below.
fn wave(t: f64, cols: usize, rows: usize) -> Vec<Line> {
    let mut grid = vec![BLANK; cols * rows];
    let word: Vec<char> = "EXACT".chars().collect();
    let width = word.len() * 8 - 2;
    let left = cols.saturating_sub(width) / 2;
    let top = rows.saturating_sub(7) as f64 / 2.0;
    for (k, ch) in word.iter().enumerate() {
        let g = glyph(*ch);
        for gx in 0..6 {
            let x = left + k * 8 + gx;
            if x >= cols {
                continue;
            }
            let lift =
                ((x as f64 * 0.22) - t * 0.18).sin() * (rows as f64 / 2.0 - 4.0).clamp(0.0, 3.0);
            let hue = x as f64 * 6.0 + t * 5.0;
            for (gy, row) in g.iter().enumerate() {
                if row.chars().nth(gx) != Some('█') {
                    continue;
                }
                let y = (top + gy as f64 + lift).round() as isize;
                // The shadow, one down and one right, then the letter.
                let (sy, sx) = (y + 1, x + 1);
                if sy >= 0
                    && (sy as usize) < rows
                    && sx < cols
                    && grid[sy as usize * cols + sx].ch == ' '
                {
                    grid[sy as usize * cols + sx] = Cell {
                        ch: '░',
                        rgb: Some((70, 70, 90)),
                        bold: false,
                    };
                }
                if y >= 0 && (y as usize) < rows {
                    grid[y as usize * cols + x] = Cell {
                        ch: '█',
                        rgb: Some(hsv(hue, 0.8, 1.0)),
                        bold: true,
                    };
                }
            }
        }
    }
    // A line of sparkles drifting along the bottom.
    if rows > 0 {
        for c in 0..cols {
            if hash(c as u64 * 31 + (t as u64 / 3)).is_multiple_of(9) {
                grid[(rows - 1) * cols + c] = Cell {
                    ch: '✧',
                    rgb: Some(hsv(c as f64 * 12.0 + t * 6.0, 0.5, 0.9)),
                    bold: false,
                };
            }
        }
    }
    lines(&grid, cols, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn every_ascii_animation_fills_its_frame_exactly() {
        for name in NAMES {
            for (cols, rows) in [(72, 16), (40, 10), (1, 1), (120, 30)] {
                for tick in [0.0, 1.0, 37.0, 500.0] {
                    let lines = frame(name, tick, cols, rows).expect("an animation");
                    assert_eq!(lines.len(), rows, "{name}");
                    for line in &lines {
                        let width: usize = line.runs.iter().map(|r| r.text.width()).sum();
                        assert_eq!(width, cols, "{name} at {cols}×{rows}, tick {tick}");
                    }
                }
            }
        }
        assert!(frame("nope", 0.0, 10, 10).is_none());
    }
}
