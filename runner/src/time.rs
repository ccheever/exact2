//! The date as a host fact: the Unix time at which the runner's clock read
//! zero, and the viewer's offset from UTC. `now()` stays elapsed time; the
//! date now is `epochAtZero + now()`. Beside it, where the viewer is: their
//! locale and IANA time zone, so a source formats a date the way the device
//! would (`new Intl.DateTimeFormat(time.locale, { timeZone: time.timeZone })`)
//! without reading either from ambient state.
//! @ref LLP 1027.000.000 (Draft; the Bluesky client is its consumer)

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactTime";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["epochAtZero", "utcOffset", "locale", "timeZone"];

/// What the host said about the date. Zero until it says: the bake, and a
/// host that has not supplied it, answer an unknown date as `0`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WallTime {
    /// Unix milliseconds at which `now()` read zero.
    pub epoch_at_zero: f64,
    /// Minutes east of UTC in the viewer's zone (UTC+2 is `120`).
    pub utc_offset: f64,
}

impl WallTime {
    /// Refuse a non-finite or implausible fact before changing any state.
    pub fn validate(self) -> Result<(), crate::RunnerError> {
        if self.epoch_at_zero.is_finite()
            && self.epoch_at_zero >= 0.0
            && self.utc_offset.is_finite()
            && self.utc_offset.abs() <= 18.0 * 60.0
        {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidTime)
        }
    }

    /// Fill a declared field; unknown names are refused.
    pub fn field(self, name: &str) -> Option<Value> {
        match name {
            "epochAtZero" => Some(Value::Number(self.epoch_at_zero)),
            "utcOffset" => Some(Value::Number(self.utc_offset)),
            _ => None,
        }
    }
}

/// Where the viewer is: a BCP 47 locale (`en-GB`) and an IANA zone
/// (`Europe/London`). Empty until the host says, as the date is zero.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Place {
    /// BCP 47, the device's first preferred language.
    pub locale: String,
    /// IANA, the device's current zone.
    pub time_zone: String,
}

impl Place {
    /// Refuse what no host would report: a fact that is empty, long, or has
    /// characters neither form uses. The engine's `Intl` judges the rest.
    pub fn validate(&self) -> Result<(), crate::RunnerError> {
        let fine = |s: &str, extra: &[char]| {
            !s.is_empty()
                && s.len() <= 64
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || extra.contains(&c))
        };
        if fine(&self.locale, &[]) && fine(&self.time_zone, &['/', '_', '+']) {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidPlace)
        }
    }

    /// Fill a declared field; the date's fields are [`WallTime`]'s.
    pub fn field(&self, name: &str) -> Option<Value> {
        match name {
            "locale" => Some(Value::Str(self.locale.as_str().into())),
            "timeZone" => Some(Value::Str(self.time_zone.as_str().into())),
            _ => None,
        }
    }
}
