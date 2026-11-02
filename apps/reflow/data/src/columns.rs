//! Column cutting: the magazine (drag to resize; columns re-cut) and the
//! editorial spread (obstacles the columns flow around, on three hosts, from
//! the same walker the kernel runs).
use crate::font::{ITALIC, REGULAR};
use crate::prose::{ARTICLE, PULL_QUOTE};
use crate::typeset::{columns, Type};
use exact_plan::Value;
use exact_textflow::{FlowShape, ShapeOutside};

/// Body type for both scenes: regular 16 on 24.
pub const BODY: Type = Type {
    face: &REGULAR,
    size: 16.0,
    line_height: 24.0,
};
/// Gutter between columns, px.
pub const GAP: f32 = 28.0;

/// `magazine(width)`: as many columns as fit at 230 px or wider, each cut
/// at a line boundary.
pub fn magazine(width: f32) -> Value {
    let cols = (((width + GAP) / (230.0 + GAP)).floor() as usize).clamp(1, 5);
    let column = ((width - GAP * (cols - 1) as f32) / cols as f32)
        .floor()
        .max(120.0);
    let lines = if cols == 1 { 60 } else { 22 };
    let (chunks, leftover) = columns(&BODY, ARTICLE, column, lines, cols);
    Value::record(vec![
        Value::Number(cols as f64),
        Value::Number(column as f64),
        Value::Number((lines as f32 * BODY.line_height) as f64),
        Value::list(
            chunks
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    Value::record(vec![
                        Value::str(&format!("mag-{i}")),
                        Value::str(&c.text),
                        Value::Number(c.lines as f64),
                    ])
                })
                .collect(),
        ),
        Value::Number(leftover as f64),
    ])
}

/// An obstacle in the spread's wrapping box: a CSS shape the Contract sets
/// verbatim, and the same shape resolved for the walker.
struct Obstacle {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    css: &'static str,
    margin: f32,
}

impl Obstacle {
    /// The exclusion as the kernel resolves it: the CSS shape in the box,
    /// grown by `shape-margin`, in page coordinates.
    fn flow(&self) -> FlowShape {
        ShapeOutside::parse(self.css)
            .expect("a literal CSS shape")
            .resolve(self.w, self.h)
            .grow(self.margin)
            .translate(self.x, self.y)
    }
}

const CAP: &str = "inset(0)";
const QUOTE: &str = "inset(0 round 14px)";
const DISC: &str = "circle()";

/// `spread(width, quote_size)`: one, two or three columns around a drop cap, a pull
/// quote and a round picture, each column's text cut where its flow stops.
pub fn spread(width: f32, quote_size: f32) -> Value {
    let cols = if width >= 880.0 {
        3
    } else if width >= 580.0 {
        2
    } else {
        1
    };
    let column = ((width - GAP * (cols - 1) as f32) / cols as f32)
        .floor()
        .max(120.0);
    let cap = Obstacle {
        x: 0.0,
        y: 0.0,
        w: 58.0,
        h: 64.0,
        css: CAP,
        margin: 8.0,
    };
    let (mut quote, mut disc) = match cols {
        3 => (
            Obstacle {
                x: (column * 0.55).round(),
                y: 250.0,
                w: (column * 1.15).round(),
                h: 0.0,
                css: QUOTE,
                margin: 16.0,
            },
            Obstacle {
                x: (2.0 * column + 1.5 * GAP - 96.0).round(),
                y: 540.0,
                w: 192.0,
                h: 192.0,
                css: DISC,
                margin: 16.0,
            },
        ),
        2 => (
            Obstacle {
                x: (column * 0.62).round(),
                y: 260.0,
                w: (column * 0.92).round(),
                h: 0.0,
                css: QUOTE,
                margin: 16.0,
            },
            Obstacle {
                x: (column + GAP / 2.0 - 90.0).round(),
                y: 640.0,
                w: 180.0,
                h: 180.0,
                css: DISC,
                margin: 16.0,
            },
        ),
        _ => (
            Obstacle {
                x: (width * 0.44).round(),
                y: 300.0,
                w: (width * 0.56).round(),
                h: 0.0,
                css: QUOTE,
                margin: 14.0,
            },
            Obstacle {
                x: (width * 0.05).round(),
                y: 700.0,
                w: 168.0,
                h: 168.0,
                css: DISC,
                margin: 14.0,
            },
        ),
    };
    let quote_padding = 20.0;
    let quote_type = Type {
        face: &ITALIC,
        size: quote_size,
        line_height: quote_size * 1.28,
    };
    // Measure before column cutting, so the painted box and the exclusion
    // reserve the same space. As for cards, leave 2 px of width for host
    // shaping differences; never clip text to hide an underestimated height.
    let quote_lines = quote_type.lines(PULL_QUOTE, quote.w - 2.0 * quote_padding - 2.0);
    quote.h = (quote_lines as f32 * quote_type.line_height + 2.0 * quote_padding).ceil();
    disc.y = disc
        .y
        .max(quote.y + quote.h + quote.margin + disc.margin + BODY.line_height);
    let shapes = [cap.flow(), quote.flow(), disc.flow()];
    let bands: u32 = match cols {
        3 => 34,
        2 => 46,
        _ => 0,
    };
    let text = &ARTICLE[1..];
    let mut rest = text;
    let mut chunks = Vec::new();
    let mut height = bands as f32 * BODY.line_height;
    for i in 0..cols {
        if rest.is_empty() {
            break;
        }
        let x = i as f32 * (column + GAP);
        let local: Vec<FlowShape> = shapes.iter().map(|s| s.translate(-x, 0.0)).collect();
        let (end, used, _) = BODY.take_flow(rest, column, &local, bands);
        if bands == 0 {
            height = (used / BODY.line_height).ceil() * BODY.line_height;
        }
        chunks.push(Value::record(vec![
            Value::str(&format!("spread-{i}")),
            Value::Number(x as f64),
            Value::str(rest[..end].trim()),
        ]));
        rest = rest[end..].trim_start();
    }
    let leftover = rest.split_whitespace().count();
    let num = |v: f32| Value::Number(v as f64);
    Value::record(vec![
        Value::Number(cols as f64),
        num(column),
        num(height),
        Value::list(chunks),
        num(cap.w),
        num(cap.h),
        Value::str(CAP),
        num(cap.margin),
        num(quote.x),
        num(quote.y),
        num(quote.w),
        num(quote.h),
        Value::str(QUOTE),
        num(quote.margin),
        num(disc.x),
        num(disc.y),
        num(disc.w),
        Value::str(DISC),
        num(disc.margin),
        Value::Number(leftover as f64),
        Value::str(PULL_QUOTE),
        num(quote_type.size),
        num(quote_type.line_height),
        num(quote_padding),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(v: &Value) -> &[Value] {
        match v {
            Value::Record(f) => f,
            _ => panic!("record"),
        }
    }
    #[test]
    fn narrower_pages_cut_more_columns_and_keep_every_word() {
        let (wide, narrow) = (magazine(1100.0), magazine(500.0));
        let (wide, narrow) = (record(&wide), record(&narrow));
        assert_eq!(wide[0].as_number(), Some(4.0));
        assert_eq!(narrow[0].as_number(), Some(2.0));
        let words = |v: &Value| -> usize {
            let Value::List(items) = v else { panic!() };
            items
                .iter()
                .map(|c| record(c)[1].as_str().unwrap().split_whitespace().count())
                .sum()
        };
        let total = ARTICLE.split_whitespace().count();
        assert_eq!(
            words(&wide[3]) + wide[4].as_number().unwrap() as usize,
            total
        );
        assert_eq!(
            words(&narrow[3]) + narrow[4].as_number().unwrap() as usize,
            total
        );
    }
    #[test]
    fn the_spread_flows_around_its_obstacles_on_every_column_count() {
        for (width, cols) in [(1000.0, 3.0), (700.0, 2.0), (390.0, 1.0)] {
            let s = spread(width, if width < 580.0 { 17.0 } else { 21.0 });
            let f = record(&s);
            assert_eq!(f[0].as_number(), Some(cols), "{width}");
            let Value::List(chunks) = &f[3] else { panic!() };
            assert_eq!(chunks.len(), cols as usize);
            assert!(f[2].as_number().unwrap() > 400.0);
            // A one-column page has an uncapped height and keeps every word.
            if cols == 1.0 {
                assert_eq!(f[19].as_number(), Some(0.0));
            }
        }
    }
}
