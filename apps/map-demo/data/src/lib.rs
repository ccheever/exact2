//! The map demo's data source: none. The pins are literal Contract; the map
//! is the app's native module (LLP 1024).

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: answers nothing.
#[derive(Default, Clone, Copy)]
pub struct Map;

impl DataSource for Map {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.mapdemo"
    }
}
