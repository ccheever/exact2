//! The orientation fixture's data source (LLP 1133.029 §9.1): none. Every
//! case is literal Contract over state.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: answers nothing.
#[derive(Default, Clone, Copy)]
pub struct Fixture;

impl DataSource for Fixture {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.orientationfixture"
    }
}
