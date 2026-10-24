//! The photo editor's data source: none. The photo, its gestures and the
//! edit state live in the app's native module (LLP 1024); the camera is the
//! standard package's (LLP 1133.008 §10); the rest is Contract.

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

    /// The camera, for a photo to edit (LLP 1133.008 §10).
    fn grants(&self) -> &str {
        "device.camera cameraPurpose"
    }
}
