//! The geofencing fixture's data source (LLP 1133.021 §13 drives 10 to 16):
//! no sources; the grant lines background location needs.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source: answers nothing.
#[derive(Default, Clone, Copy)]
pub struct Fences;

impl DataSource for Fences {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.geofencefixture"
    }

    fn grants(&self) -> &str {
        "device.geolocation locationPurpose\ndevice.geolocation-background arrivalPurpose\ndevice.notifications noticePurpose"
    }
}
