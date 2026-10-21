//! The Rust binding's fixture (LLP 1133.002 stage 3): `here(asked)` asks
//! `@exact/geolocation` for a position through the generated binding once
//! `asked` is above zero, and reads its reply: the coordinates, or the
//! module's error code.

#![deny(missing_docs)]

/// The binding `build.rs` generates from the app's plan.
pub mod modules {
    include!(concat!(env!("OUT_DIR"), "/modules.rs"));
}

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use modules::geolocation;

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Fixture;

/// `shape Spot`.
fn spot(text: String) -> Value {
    Value::record(vec![Value::str(&text)])
}

impl DataSource for Fixture {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "here" => Ok(spot("none".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let asked = args.first().and_then(Value::as_number).unwrap_or(0.0);
        if source != "here" || asked == 0.0 {
            return self.query(source, args).map(Answer::Now);
        }
        let options = geolocation::GetCurrentPositionOptions {
            maximum_age: Some(600000.0),
            android: Some(geolocation::GetCurrentPositionAndroid {
                quality: Some("highAccuracy".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        Ok(Answer::Later(geolocation::get_current_position(&options)?))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "here" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let text = match geolocation::get_current_position_reply(&outcome) {
            Ok(p) => format!("{},{}", p.coords.latitude, p.coords.longitude),
            Err(e) => format!("error {}", e.code),
        };
        Ok(Answer::Now(spot(text)))
    }

    fn grants(&self) -> &str {
        "device.geolocation locationPurpose"
    }

    fn app_id(&self) -> &str {
        "com.exact.modulefixture"
    }
}
