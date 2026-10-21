//! The shared-elements example's data (LLP 1013.000): a dozen photos for the
//! grid, two hundred for the virtualized list, three cards and five tasks.
//! Deterministic, so every host shows the same thing.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Shared;

const ASSETS: [&str; 6] = [
    "assets/north-shore.jpg",
    "assets/ochre-dunes.jpg",
    "assets/alpine-water.jpg",
    "assets/winter-ridge.jpg",
    "assets/glasshouse.jpg",
    "assets/last-light.jpg",
];
const PLACES: [&str; 6] = [
    "North Shore",
    "Ochre Dunes",
    "Alpine Water",
    "Winter Ridge",
    "Glasshouse",
    "Last Light",
];
const TONES: [&str; 6] = [
    "#2f5d62", "#b0703c", "#3d6b8c", "#7b8794", "#4f6f52", "#8c4a3d",
];

/// Photo `n` (1-based), as `shape Photo` declares its fields: id, n, asset,
/// title, tone.
fn photo(n: usize) -> Value {
    let i = (n - 1) % ASSETS.len();
    Value::record(vec![
        Value::str(&format!("p{n}")),
        Value::Number(n as f64),
        Value::str(ASSETS[i]),
        Value::str(&format!("{} {}", PLACES[i], n.div_ceil(ASSETS.len()))),
        Value::str(TONES[i]),
    ])
}

/// A card, as `shape Card` declares it: id, title, body, color.
fn card(id: &str, title: &str, body: &str, color: &str) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::str(title),
        Value::str(body),
        Value::str(color),
    ])
}

/// A task, as `shape Task` declares it: id, bit, label.
fn task(n: usize, label: &str) -> Value {
    Value::record(vec![
        Value::str(&format!("t{n}")),
        Value::Number((1u32 << (n - 1)) as f64),
        Value::str(label),
    ])
}

impl DataSource for Shared {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "photos" => Ok(Value::list((1..=12).map(photo).collect())),
            "rows" => Ok(Value::list((1..=200).map(photo).collect())),
            "cards" => Ok(Value::list(vec![
                card(
                    "c1",
                    "Morning",
                    "Coffee, then the long walk along the shore while the light is still low.",
                    "#e8d5b7",
                ),
                card(
                    "c2",
                    "Afternoon",
                    "The glasshouse in the heat of the day, and a nap in the shade after.",
                    "#cfe3d4",
                ),
                card(
                    "c3",
                    "Evening",
                    "Last light over the ridge; dinner when the stars come out.",
                    "#d6d3ec",
                ),
            ])),
            "tasks" => Ok(Value::list(vec![
                task(1, "Pack the camera"),
                task(2, "Charge batteries"),
                task(3, "Print the map"),
                task(4, "Book the ferry"),
                task(5, "Water the plants"),
            ])),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.sharedelements"
    }
}
