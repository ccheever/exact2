//! Cards with known heights: the masonry and the wall. Each card is measured
//! in the faces it is set in, placed in the shortest column, and only then
//! handed to the kernel — so the columns balance and the wall knows its own
//! height before a single card is a view.
use crate::font::{BOLD, REGULAR};
use crate::prose::{SENTENCES, TITLES};
use crate::typeset::Type;
use exact_plan::Value;
use exact_textflow::Prepared;

/// Gap between columns and between cards, px.
pub const GAP: f32 = 16.0;
/// Card padding, px.
pub const PAD: f32 = 16.0;
/// Narrowest column before the count drops, px.
pub const MIN_COLUMN: f32 = 240.0;
/// The title: bold 18 on 24 (the Contract sets the same).
pub const TITLE: Type = Type {
    face: &BOLD,
    size: 18.0,
    line_height: 24.0,
};
/// The body: regular 15 on 22.
pub const BODY: Type = Type {
    face: &REGULAR,
    size: 15.0,
    line_height: 22.0,
};
/// Space between title and body; between body and footer; the footer line.
const TITLE_GAP: f32 = 6.0;
const FOOT_GAP: f32 = 10.0;
const FOOT: f32 = 16.0;
/// The card's one-pixel border, top and bottom (`box-sizing: border-box`).
const BORDER: f32 = 2.0;
/// Cards on the masonry page and on the wall.
pub const MASONRY_COUNT: usize = 36;
/// Cards on the wall.
pub const WALL_COUNT: usize = 1200;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 33) as u32
}

/// A card's words, deterministic in `(seed, index)`.
pub fn card(seed: u64, index: usize) -> (String, String) {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (index as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03);
    let title = TITLES[lcg(&mut s) as usize % TITLES.len()].to_owned();
    let count = 1 + (lcg(&mut s) % 5) as usize;
    let mut body = String::new();
    for k in 0..count {
        if k > 0 {
            body.push(' ');
        }
        body.push_str(SENTENCES[lcg(&mut s) as usize % SENTENCES.len()]);
    }
    (title, body)
}

/// A card measured at a text width.
pub struct Measured {
    /// Index in the deck.
    pub index: usize,
    /// Title and body.
    pub title: String,
    /// The body.
    pub body: String,
    /// Title lines at the width.
    pub title_lines: usize,
    /// Body lines at the width.
    pub body_lines: usize,
    /// The card's outer height, px.
    pub height: f32,
}

/// Measure card `index` for a column `column_width` wide.
pub fn measure(seed: u64, index: usize, column_width: f32) -> Measured {
    let (title, body) = card(seed, index);
    let text_width = column_width - 2.0 * PAD;
    let title_lines = TITLE.lines(&title, text_width);
    let body_lines = BODY.lines(&body, text_width);
    let height = height(title_lines, body_lines);
    Measured {
        index,
        title,
        body,
        title_lines,
        body_lines,
        height,
    }
}

fn height(title_lines: usize, body_lines: usize) -> f32 {
    PAD + title_lines as f32 * TITLE.line_height
        + TITLE_GAP
        + body_lines as f32 * BODY.line_height
        + FOOT_GAP
        + FOOT
        + PAD
        + BORDER
}

/// Column count and integral column width for a page `width` wide.
pub fn grid(width: f32) -> (usize, f32) {
    let cols = (((width + GAP) / (MIN_COLUMN + GAP)).floor() as usize).clamp(1, 5);
    let column = ((width - GAP * (cols - 1) as f32) / cols as f32)
        .floor()
        .max(120.0);
    (cols, column)
}

fn shortest(heights: &[f32]) -> usize {
    let mut best = 0;
    for (i, &h) in heights.iter().enumerate() {
        if h < heights[best] - 0.01 {
            best = i;
        }
    }
    best
}

fn card_value(m: &Measured) -> Value {
    Value::record(vec![
        Value::str(&format!("card-{}", m.index)),
        Value::Number(m.index as f64 + 1.0),
        Value::str(&m.title),
        Value::str(&m.body),
        Value::Number(m.title_lines as f64),
        Value::Number(m.body_lines as f64),
        Value::Number(m.height as f64),
    ])
}

/// `masonry(width, seed)`: the columns, each card in the shortest one.
pub fn masonry(width: f32, seed: u64) -> Value {
    let (cols, column) = grid(width);
    let mut heights = vec![0f32; cols];
    let mut columns: Vec<Vec<Value>> = vec![Vec::new(); cols];
    for index in 0..MASONRY_COUNT {
        let m = measure(seed, index, column);
        let c = shortest(&heights);
        heights[c] += m.height + GAP;
        columns[c].push(card_value(&m));
    }
    let tallest = heights.iter().cloned().fold(0f32, f32::max) - GAP;
    Value::record(vec![
        Value::Number(cols as f64),
        Value::Number(column as f64),
        Value::Number(tallest.max(0.0) as f64),
        Value::list(
            columns
                .into_iter()
                .enumerate()
                .map(|(i, cards)| {
                    Value::record(vec![Value::str(&format!("col-{i}")), Value::list(cards)])
                })
                .collect(),
        ),
    ])
}

/// A wall card's place, kept between scrolls.
pub struct Placed {
    m: Measured,
    title: Prepared,
    body: Prepared,
    x: f32,
    y: f32,
}

/// One seed's prepared text and placement at the most recent column geometry.
pub struct Wall {
    seed: u64,
    cols: usize,
    column: f32,
    total: f32,
    placed: Vec<Placed>,
}

impl Wall {
    fn build(width: f32, seed: u64) -> Self {
        let placed = (0..WALL_COUNT)
            .map(|index| {
                let (title, body) = card(seed, index);
                Placed {
                    title: TITLE.prepare(&title),
                    body: BODY.prepare(&body),
                    m: Measured {
                        index,
                        title,
                        body,
                        title_lines: 0,
                        body_lines: 0,
                        height: 0.0,
                    },
                    x: 0.0,
                    y: 0.0,
                }
            })
            .collect();
        let mut wall = Self {
            seed,
            cols: 0,
            column: 0.0,
            total: 0.0,
            placed,
        };
        wall.reflow(width);
        wall
    }

    fn reflow(&mut self, width: f32) {
        let (cols, column) = grid(width);
        if (self.cols, self.column) == (cols, column) {
            return;
        }
        let text_width = column - 2.0 * PAD;
        let mut heights = [0f32; 5];
        for p in &mut self.placed {
            p.m.title_lines = p.title.count_lines(text_width);
            p.m.body_lines = p.body.count_lines(text_width);
            p.m.height = height(p.m.title_lines, p.m.body_lines);
            let c = shortest(&heights[..cols]);
            p.x = c as f32 * (column + GAP);
            p.y = heights[c];
            heights[c] += p.m.height + GAP;
        }
        self.total = (heights[..cols].iter().copied().fold(0f32, f32::max) - GAP).max(0.0);
        self.cols = cols;
        self.column = column;
    }
}

/// `wall(width, scrollTop, viewportHeight, seed)`: the cards that meet the
/// viewport (one screen of overscan each way) and the wall's total height.
pub fn wall(
    cache: &mut Option<Wall>,
    width: f32,
    scroll_top: f32,
    viewport: f32,
    seed: u64,
) -> Value {
    if cache.as_ref().map(|w| w.seed) != Some(seed) {
        *cache = Some(Wall::build(width, seed));
    }
    let wall = cache.as_mut().unwrap();
    wall.reflow(width);
    let top = scroll_top - viewport;
    let bottom = scroll_top + 2.0 * viewport;
    let visible: Vec<Value> = wall
        .placed
        .iter()
        .filter(|p| p.y < bottom && p.y + p.m.height > top)
        .map(|p| {
            Value::record(vec![
                Value::str(&format!("wall-{}", p.m.index)),
                Value::Number(p.m.index as f64 + 1.0),
                Value::Number(p.x as f64),
                Value::Number(p.y as f64),
                Value::Number(p.m.height as f64),
                Value::str(&p.m.title),
                Value::str(&p.m.body),
                Value::Number(p.m.body_lines as f64),
            ])
        })
        .collect();
    Value::record(vec![
        Value::Number(wall.cols as f64),
        Value::Number(wall.column as f64),
        Value::Number(wall.total as f64),
        Value::Number(WALL_COUNT as f64),
        Value::Number(visible.len() as f64),
        Value::list(visible),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn columns_balance_and_the_wall_windows() {
        let mut cache = None;
        let v = wall(&mut cache, 1000.0, 0.0, 800.0, 1);
        let Value::Record(fields) = &v else { panic!() };
        assert_eq!(fields[0].as_number(), Some(3.0));
        let visible = fields[4].as_number().unwrap();
        assert!(visible > 6.0 && visible < 120.0, "{visible}");
        let total = fields[2].as_number().unwrap();
        assert!(total > 20_000.0, "{total}");
        let far = wall(&mut cache, 1000.0, (total / 2.0) as f32, 800.0, 1);
        let Value::Record(far) = &far else { panic!() };
        assert!(far[4].as_number().unwrap() > 6.0);
        // Same width, same seed: the second query reused the placement.
        assert_eq!(cache.as_ref().unwrap().seed, 1);
        assert_eq!(cache.as_ref().unwrap().column, grid(1000.0).1);
        let w = Wall::build(1000.0, 1);
        let mut per_column = vec![0f32; w.cols];
        for p in &w.placed {
            let c = (p.x / (w.column + GAP)).round() as usize;
            per_column[c] = per_column[c].max(p.y + p.m.height);
        }
        let (min, max) = per_column
            .iter()
            .fold((f32::MAX, 0f32), |(a, b), &h| (a.min(h), b.max(h)));
        assert!(max - min < 400.0, "unbalanced: {per_column:?}");
    }

    #[test]
    fn resized_wall_matches_fresh_measurements_across_columns_and_seeds() {
        let mut cache = None;
        // Both sides of column-count boundaries, widths sharing a rounded
        // column, narrower/wider text, and a return to the original geometry.
        let widths = [
            1000.0, 1001.0, 1002.0, 495.5, 496.0, 751.5, 752.0, 1007.5, 1008.0, 1263.5, 1264.0,
            120.0, 2000.0, 1000.0,
        ];
        for seed in [1, 9, u64::MAX, 1] {
            for width in widths {
                wall(&mut cache, width, 0.0, 800.0, seed);
                let w = cache.as_ref().unwrap();
                let (cols, column) = grid(width);
                let mut heights = vec![0.0; cols];
                assert_eq!(w.placed.len(), WALL_COUNT);
                assert_eq!((w.cols, w.column, w.seed), (cols, column, seed));
                for (index, placed) in w.placed.iter().enumerate() {
                    let fresh = measure(seed, index, column);
                    let c = shortest(&heights);
                    assert_eq!(placed.m.index, index);
                    assert_eq!(placed.m.title, fresh.title);
                    assert_eq!(placed.m.body, fresh.body);
                    assert_eq!(placed.m.title_lines, fresh.title_lines);
                    assert_eq!(placed.m.body_lines, fresh.body_lines);
                    assert_eq!(placed.m.height, fresh.height);
                    assert_eq!(placed.x, c as f32 * (column + GAP));
                    assert_eq!(placed.y, heights[c]);
                    heights[c] += fresh.height + GAP;
                }
                assert_eq!(w.total, heights.into_iter().fold(0.0, f32::max) - GAP);
            }
            // Reused preparations and placements must not change windowing.
            for scroll in [0.0, 1234.0, 18000.0, 50000.0] {
                let actual = wall(&mut cache, 1000.0, scroll, 800.0, seed);
                let fresh = wall(&mut None, 1000.0, scroll, 800.0, seed);
                assert_eq!(actual, fresh);
            }
        }
    }
}
