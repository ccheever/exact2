//! @exact/photos's fixture (LLP 1133.019 stage 3): a data source with no
//! sources of its own, which declares both photo lines and the temporary
//! directory a copy is written to and added back from.

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
        "device.photos recentsPurpose\ndevice.photos-add savePurpose\nfs.read app:/tmp\nfs.write app:/tmp"
    }

    fn app_id(&self) -> &str {
        "com.exact.photosfixture"
    }
}
