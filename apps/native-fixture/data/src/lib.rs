//! The native-module fixture's data source (LLP 1024 D8): one list, the
//! rows of the hooked list (LLP 1075.003.000); everything else the smoke
//! drives is literal Contract over the module seam.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: `rows()`, a `shape Rows` of two hundred `shape Row`.
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
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.nativefixture"
    }
}
