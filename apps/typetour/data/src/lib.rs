//! Type Tour has no data: every screen is authored in `app.contract`, so the
//! source answers nothing and the first frame needs no query (LLP 1004 D4).

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source, which declares no sources.
#[derive(Default, Clone, Copy)]
pub struct Tour;

impl DataSource for Tour {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.typetour"
    }
}
