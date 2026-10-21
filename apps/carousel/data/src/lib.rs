//! Carousel's data (LLP 1070): `cards(count)` is `count` cards whose widths
//! vary (so a strip's estimate is replaced by real measurements), in one of
//! six colours, the same on every host. It ignores a second argument, which
//! the evaluation budget's conformance plan passes (LLP 1090 D7), as it does
//! `long(n, c)`: `c` repeated `n` times.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Cards;

const COLORS: [&str; 6] = [
    "#b45309", "#4338ca", "#047857", "#b91c1c", "#0369a1", "#7e22ce",
];
const WIDTHS: [f64; 5] = [96.0, 120.0, 160.0, 120.0, 200.0];

fn card(n: u64) -> Value {
    // Fields in the order `shape Card` declares them.
    Value::record(vec![
        Value::str(&format!("c{n}")),
        Value::Number(n as f64),
        Value::str(&format!("Card {n}")),
        Value::Number(WIDTHS[(n % 5) as usize]),
        Value::str(COLORS[(n % 6) as usize]),
    ])
}

/// Feed row `n`: every fifth a strip of 200 cards, every seventh an inbox
/// of 120 messages, the rest text. Fields in `shape Post`'s order.
fn post(n: u64) -> Value {
    let (kind, items) = if n.is_multiple_of(5) {
        ("strip", 200)
    } else if n.is_multiple_of(7) {
        ("inbox", 120)
    } else {
        ("text", 0)
    };
    Value::record(vec![
        Value::str(&format!("p{n}")),
        Value::Number(n as f64),
        Value::str(kind),
        Value::list((0..items).map(card).collect()),
    ])
}

impl DataSource for Cards {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "cards" => {
                let count = match args.first() {
                    Some(Value::Number(n)) if *n >= 0.0 => (*n as u64).min(100_000),
                    _ => 0,
                };
                Ok(Value::list((0..count).map(card).collect()))
            }
            "long" => Ok(match args {
                [Value::Number(n), c] if *n >= 0.0 => {
                    let c = c.as_str().unwrap_or_default();
                    Value::str(&c.repeat((*n as usize).min(1 << 27)))
                }
                _ => Value::str(""),
            }),
            "posts" => {
                let count = match args.first() {
                    Some(Value::Number(n)) if *n >= 0.0 => (*n as u64).min(10_000),
                    _ => 0,
                };
                Ok(Value::list((0..count).map(post).collect()))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.carousel"
    }
}
