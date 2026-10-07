//! Immutable rows for the Android core example; no I/O, storage or timers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// A data source whose only answer is compiled into the first frame.
#[derive(Default, Clone, Copy)]
pub struct Core;

impl DataSource for Core {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("rows", []) => Ok(Value::List(
                (0..1000).map(|n| Value::Number(n as f64)).collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}
