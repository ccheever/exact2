//! Carousel's data (LLP 1070): `cards(count)` is `count` cards whose widths
//! vary (so a strip's estimate is replaced by real measurements), in one of
//! six colours, the same on every host.

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
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.carousel"
    }
}
