//! The native-module fixture's data source (LLP 1024 D8): two lists, the
//! rows of the hooked list (LLP 1075.003.000) and the map providers an
//! "Open in…" chooser offers (LLP 1021); everything else the smoke drives
//! is literal Contract over the module seam.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: `rows()`, a `shape Rows` of two hundred `shape Row`,
/// and `providers()`, the `shape Providers` installed — what an app asks its
/// own native module at run time, fixed here.
#[derive(Default, Clone, Copy)]
pub struct Fixture;

impl DataSource for Fixture {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "rows" => Ok(Value::record(vec![Value::list(
                (1..=200)
                    .map(|n| Value::record(vec![Value::Number(n as f64)]))
                    .collect(),
            )])),
            "providers" => Ok(Value::record(vec![Value::list(
                [
                    ("apple", "Apple Maps"),
                    ("google", "Google Maps"),
                    ("waze", "Waze"),
                ]
                .into_iter()
                .map(|(id, name)| {
                    Value::record(vec![
                        Value::str(id),
                        Value::str(name),
                        Value::str(&format!("assets/maps/{id}.png")),
                    ])
                })
                .collect(),
            )])),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.nativefixture"
    }
}
