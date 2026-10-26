//! @exact/contacts' and @exact/calendar's fixture (LLP 1133.018 stage 3): a
//! data source with no sources of its own, which declares the contacts line
//! and the full calendar line.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Fixture;

impl DataSource for Fixture {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn grants(&self) -> &str {
        "device.contacts contactsPurpose\ndevice.calendar calendarPurpose"
    }

    fn app_id(&self) -> &str {
        "com.exact.contactscalendarfixture"
    }
}
