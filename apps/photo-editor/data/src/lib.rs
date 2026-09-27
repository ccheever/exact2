//! The photo editor's data source: none. The photo, its gestures and the
//! edit state live in the app's native module (LLP 1024); the rest is Contract.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: answers nothing.
#[derive(Default, Clone, Copy)]
pub struct Photo;

impl DataSource for Photo {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.photoeditor"
    }
}
