//! The demo content the slash commands show: the banner, ASCII and ANSI
//! art, a generated image, a Unicode width tour, a coloured diff, and the
//! stress transcript. Pure functions of their inputs, except
//! [`write_png`], which writes the file it names.

use crate::markdown;
use crate::state::{Block, Entry, Line, Run};
use crate::tools;

/// `#rrggbb`.
pub fn hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// HSV (h in degrees, s and v 0–1) to RGB.
pub fn hsv(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to = |f: f64| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

fn hsv_hex(h: f64, s: f64, v: f64) -> String {
    let (r, g, b) = hsv(h, s, v);
    hex(r, g, b)
}

/// The banner: a gradient logo, the model and directory, and tips.
pub fn banner(model: &str, cwd: &str, branch: &str) -> Vec<Block> {
    const LOGO: [&str; 2] = ["█▀▀ ▀▄▀ ▄▀█ █▀▀ ▀█▀", "██▄ █ █ █▀█ █▄▄  █ "];
    let width = LOGO[0].chars().count() as f64;
    let mut lines: Vec<Line> = LOGO
        .iter()
        .enumerate()
        .map(|(n, row)| {
            let mut line = Line::default();
            for (i, c) in row.chars().enumerate() {
                line.push(Run {
                    text: c.to_string(),
                    fg: hsv_hex(190.0 + 130.0 * i as f64 / width, 0.65, 1.0),
                    bold: true,
                    ..Run::default()
                });
            }
            if n == 0 {
                line.push(Run::dim("  harness"));
            }
            line
        })
        .collect();
    lines.push(Line::default());
    let mut place = vec![Run::fg(model, "#61afef"), Run::dim(" · "), Run::plain(cwd)];
    if !branch.is_empty() {
        place.push(Run::dim(" · "));
        place.push(Run::fg(branch, "#c678dd"));
    }
    vec![
        Block::lines(lines),
        Block::p(place),
        Block::p(vec![Run::dim(
            "/help for commands · esc to interrupt · ctrl+c to quit",
        )]),
    ]
}

/// `/help`.
pub const HELP: &str = "**Commands**\n\n\
- `/help` — this list\n\
- `/clear` — clear the transcript\n\
- `/model <id>` · `/models` — choose the model, list them\n\
- `/tools` — the tools the agent can call\n\
- `/stress [n]` — n entries of varied content (default 200)\n\
- `/ascii` · `/ansi` · `/image [path]` · `/unicode` · `/diff` — rendering demos\n\n\
Anything else is a prompt. With the **mock** model, try prompts containing *long*, *edit* or *error*.";

/// The Mandelbrot escape at (x, y): a smooth iteration count, `None`
/// inside the set.
pub fn mandel(x: f64, y: f64, max: u32) -> Option<f64> {
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    for i in 0..max {
        let (r2, i2) = (zr * zr, zi * zi);
        if r2 + i2 > 16.0 {
            let log_zn = (r2 + i2).ln() / 2.0;
            let nu = (log_zn / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2;
            return Some(i as f64 + 1.0 - nu);
        }
        zi = 2.0 * zr * zi + y;
        zr = r2 - i2 + x;
    }
    None
}

/// The image's colour at (u, v) in 0–1: the Mandelbrot set, coloured.
pub fn picture(u: f64, v: f64) -> (u8, u8, u8) {
    let x = -2.3 + u * 3.2;
    let y = -1.0 + v * 2.0;
    match mandel(x, y, 96) {
        None => (12, 10, 30),
        Some(n) => hsv(220.0 + n * 9.0, 0.75, (0.35 + n / 30.0).min(1.0)),
    }
}

/// Write the generated picture as a `w`×`h` PNG.
pub fn write_png(path: &std::path::Path, w: u32, h: u32) -> Result<(), String> {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = picture(x as f64 / w as f64, y as f64 / h as f64);
            data.extend([r, g, b]);
        }
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&data).map_err(|e| e.to_string())
}

/// An image file's pixel size, from its header (PNG, GIF, JPEG).
pub fn image_size(path: &std::path::Path) -> Option<(u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    let be16 = |i: usize| Some(u16::from_be_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]) as u32);
    if bytes.starts_with(b"\x89PNG") && bytes.len() >= 24 {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if bytes.starts_with(b"GIF") && bytes.len() >= 10 {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return Some((w, h));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xff {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + be16(i + 2)? as usize;
        }
    }
    None
}

/// The cell box for an image `w`×`h` pixels, `cols` wide, cells 1:2.
pub fn cell_box(w: u32, h: u32, cols: u32) -> (u32, u32) {
    let rows = (cols as f64 * h as f64 / w.max(1) as f64 / 2.0).round();
    (cols, (rows as u32).clamp(1, 60))
}

const BIG: [(char, [&str; 5]); 5] = [
    ('E', ["█████", "█    ", "████ ", "█    ", "█████"]),
    ('X', ["█   █", " █ █ ", "  █  ", " █ █ ", "█   █"]),
    ('A', [" ███ ", "█   █", "█████", "█   █", "█   █"]),
    ('C', [" ████", "█    ", "█    ", "█    ", " ████"]),
    ('T', ["█████", "  █  ", "  █  ", "  █  ", "  █  "]),
];

/// `/ascii`: a Mandelbrot in a 10-step ramp, and "EXACT" in a block font.
pub fn ascii() -> Vec<Block> {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let (w, h) = (76, 24);
    let mut mandel_lines = Vec::new();
    for y in 0..h {
        let mut row = String::with_capacity(w);
        for x in 0..w {
            let cx = -2.2 + 3.0 * x as f64 / w as f64;
            let cy = -1.2 + 2.4 * y as f64 / h as f64;
            let c = match mandel(cx, cy, 200) {
                None => b'@',
                Some(n) => RAMP[((n.ln().max(0.0) * 2.4) as usize).min(RAMP.len() - 2)],
            };
            row.push(c as char);
        }
        mandel_lines.push(Line::plain(row.trim_end().to_string()));
    }
    let mut big = Vec::new();
    for r in 0..5 {
        let mut line = Line::default();
        for (i, (_, rows)) in BIG.iter().enumerate() {
            line.push(Run {
                text: format!("{}  ", rows[r]),
                fg: hsv_hex(i as f64 * 60.0, 0.7, 1.0),
                bold: true,
                ..Run::default()
            });
        }
        big.push(line);
    }
    vec![
        Block::p(vec![Run::dim(
            "ASCII Mandelbrot, 76×24, ramp \" .:-=+*#%@\"",
        )]),
        Block::lines(mandel_lines),
        Block::lines(big),
    ]
}

/// The picture as half-block lines `w` cells wide, `h` cells tall.
pub fn halfblocks(w: usize, h: usize) -> Vec<Line> {
    (0..h)
        .map(|row| {
            let mut line = Line::default();
            for col in 0..w {
                let u = col as f64 / w as f64;
                let (r, g, b) = picture(u, (2 * row) as f64 / (2 * h) as f64);
                let (r2, g2, b2) = picture(u, (2 * row + 1) as f64 / (2 * h) as f64);
                line.push(Run {
                    text: "▀".into(),
                    fg: hex(r, g, b),
                    bg: hex(r2, g2, b2),
                    ..Run::default()
                });
            }
            line
        })
        .collect()
}

const XTERM16: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// The 256-colour palette entry `i`.
pub fn xterm(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => XTERM16[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            (level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            (g, g, g)
        }
    }
}

fn swatch(i: u8, width: usize) -> Run {
    let (r, g, b) = xterm(i);
    Run {
        text: " ".repeat(width),
        bg: hex(r, g, b),
        ..Run::default()
    }
}

/// `/ansi`: the picture in half blocks, the 256-colour palette, a hue ramp.
pub fn ansi() -> Vec<Block> {
    let mut palette = Vec::new();
    for half in 0..2u8 {
        let mut line = Line::default();
        for i in 0..8 {
            line.push(swatch(half * 8 + i, 4));
        }
        palette.push(line);
    }
    for row in 0..6u8 {
        let mut line = Line::default();
        for i in 0..36u8 {
            line.push(swatch(16 + row * 36 + i, 2));
        }
        palette.push(line);
    }
    let mut grays = Line::default();
    for i in 232..=255u8 {
        grays.push(swatch(i, 3));
    }
    palette.push(grays);
    let mut ramp = Vec::new();
    for v in [1.0, 0.55] {
        let mut line = Line::default();
        for i in 0..72 {
            let h = i as f64 * 5.0;
            line.push(Run {
                text: "▀".into(),
                fg: hsv_hex(h, 1.0, v),
                bg: hsv_hex(h, 0.6, v * 0.8),
                ..Run::default()
            });
        }
        ramp.push(line);
    }
    vec![
        Block::p(vec![Run::dim(
            "Half-block truecolour, 64×16 cells (64×32 pixels)",
        )]),
        Block::lines(halfblocks(64, 16)),
        Block::p(vec![Run::dim("The 256-colour palette")]),
        Block::lines(palette),
        Block::p(vec![Run::dim("A truecolour hue ramp")]),
        Block::lines(ramp),
    ]
}

/// `/unicode`: lines whose widths are easy to get wrong.
pub fn unicode() -> Vec<Block> {
    let rows = [
        ("CJK", "漢字かなカナ한국어 — two cells each"),
        ("emoji", "🎉 👩‍💻 🏳️‍🌈 👍🏽 ❤️ 🇯🇵"),
        (
            "combining",
            "é (precomposed) · e\u{301} (e + U+0301) · n\u{303} · a\u{30a}",
        ),
        ("box", "┌─┬─┐ ╭─╮ ╔═╗ ┃ ┣━┫ ░▒▓█"),
        ("braille", "⠁⠃⠉⠙⠑⠋⠛⠓⠊⠚ ⣿⣷⣯⣟⡿⢿⣻⣽⣾"),
        ("math", "∑ ∫ √ ∞ ≈ ≠ ≤ ≥ ∂ ∇ ∀ ∃ ∈ ⊂ ℝ ℕ"),
        ("fullwidth", "ＡＢＣ１２３！"),
        ("zwj", "👨‍👩‍👧‍👦 = 👨 + ZWJ + 👩 + ZWJ + 👧 + ZWJ + 👦"),
        ("tab", "a\tb\tc (tabs at stops of 8)"),
        ("rtl", "שלום עולם · مرحبا"),
        ("arrows", "← ↑ → ↓ ↔ ⇐ ⇒ ⟶ ↺"),
    ];
    let lines = rows
        .iter()
        .map(|(label, text)| {
            let mut l = Line::default();
            l.push(Run::dim(format!("{label:<10} ")));
            l.push(Run::plain(tools::clean_line(text)));
            l
        })
        .collect();
    vec![Block::lines(lines)]
}

/// `/diff`: a demo edit as a coloured unified diff.
pub fn diff_demo() -> Vec<Block> {
    let before = "use std::fmt;\n\npub struct Greeting {\n    name: String,\n}\n\nimpl Greeting {\n    pub fn new(name: &str) -> Self {\n        Greeting { name: name.to_string() }\n    }\n\n    pub fn say(&self) -> String {\n        format!(\"Hello, {}\", self.name)\n    }\n}\n";
    let after = "use std::fmt;\n\npub struct Greeting {\n    name: String,\n    excited: bool,\n}\n\nimpl Greeting {\n    pub fn new(name: &str) -> Self {\n        Greeting { name: name.to_string(), excited: false }\n    }\n\n    pub fn say(&self) -> String {\n        let end = if self.excited { \"!\" } else { \".\" };\n        format!(\"Hello, {}{end}\", self.name)\n    }\n}\n";
    vec![
        Block::p(vec![
            Run::plain("Demo edit to "),
            Run::fg("src/greeting.rs", markdown::CODE),
        ]),
        Block::lines(tools::diff(before, after)),
    ]
}

const STRESS_MD: [&str; 6] = [
    "Here is a paragraph with **bold**, *italic* and `code`, long enough to wrap across \
     several lines in a narrow terminal so that the layout has some real work to do on every \
     entry it measures.",
    "### A heading\n\n- a list item\n- another, with `code`\n  - nested deeper\n- and a third\n\n1. one\n2. two",
    "```rust\nfn fib(n: u64) -> u64 {\n\tmatch n {\n\t\t0 | 1 => n,\n\t\t_ => fib(n - 1) + fib(n - 2),\n\t}\n}\n```",
    "Wide text: 漢字とかな、한국어, emoji 🎉👩‍💻🏳️‍🌈 and fullwidth ＡＢＣ — each of these takes two cells.",
    "> A quote, which wraps like a paragraph but sits behind a bar.\n\n---\n\nAfter the rule.",
    "",
];

/// `/stress`: `n` settled entries of varied content.
pub fn stress(n: usize) -> Vec<Entry> {
    let token: String = "x".repeat(40) + &"0123456789abcdef".repeat(20);
    (0..n)
        .map(|i| {
            if i % 9 == 4 {
                let lines: Vec<Line> = (0..(5 + i % 23))
                    .map(|k| {
                        Line::plain(tools::clean_line(&format!(
                            "{k:>4}\tout\tline of output {i}"
                        )))
                    })
                    .collect();
                let more = lines.len().saturating_sub(tools::SHOWN);
                return Entry {
                    kind: "tool".into(),
                    title: format!("Bash(seq {i})"),
                    status: if i % 2 == 0 { "ok" } else { "error" }.into(),
                    blocks: vec![Block::lines(lines.into_iter().take(tools::SHOWN).collect())],
                    more: more as f64,
                    ..Entry::default()
                };
            }
            if i % 11 == 0 {
                return Entry {
                    kind: "user".into(),
                    blocks: vec![Block::lines(vec![Line::plain(format!(
                        "Prompt number {i}"
                    ))])],
                    ..Entry::default()
                };
            }
            let pick = |k: usize| match STRESS_MD[k % STRESS_MD.len()] {
                "" => format!("A very long unbroken token #{i}: {token}"),
                s => s.to_string(),
            };
            let md = format!("**#{i}** {}\n\n{}\n\n{}", pick(i), pick(i + 2), pick(i + 3));
            Entry {
                kind: "assistant".into(),
                blocks: markdown::blocks(&md),
                ..Entry::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_png_is_written_and_its_size_read_back() {
        let path =
            std::env::temp_dir().join(format!("exact-harness-test-{}.png", std::process::id()));
        write_png(&path, 32, 20).unwrap();
        assert_eq!(image_size(&path), Some((32, 20)));
        assert_eq!(cell_box(320, 200, 48), (48, 15));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_palette_is_xterms() {
        assert_eq!(xterm(16), (0, 0, 0));
        assert_eq!(xterm(231), (255, 255, 255));
        assert_eq!(xterm(196), (255, 0, 0));
        assert_eq!(xterm(232), (8, 8, 8));
    }

    #[test]
    fn stress_varies() {
        let entries = stress(200);
        assert_eq!(entries.len(), 200);
        for kind in ["user", "assistant", "tool"] {
            assert!(entries.iter().any(|e| e.kind == kind));
        }
        // Lines on an 80-column screen, roughly: a paragraph wraps.
        let lines: usize = entries
            .iter()
            .flat_map(|e| &e.blocks)
            .map(|b| {
                let prose: usize = b.runs.iter().map(|r| r.text.chars().count()).sum();
                b.lines.len().max(prose / 80 + 1)
            })
            .sum();
        assert!(lines > 2000, "{lines}");
    }
}
