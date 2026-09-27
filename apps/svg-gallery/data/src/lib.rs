//! The SVG gallery's data source (LLP 1055.000): none. Every fixture is
//! literal Contract, so each host draws exactly the same pictures and Chrome
//! is the reference.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: answers nothing.
#[derive(Default, Clone, Copy)]
pub struct Gallery;

impl DataSource for Gallery {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.svggallery"
    }
}
