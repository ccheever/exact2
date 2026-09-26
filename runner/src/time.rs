//! The date as a host fact: the Unix time at which the runner's clock read
//! zero, and the viewer's offset from UTC. `now()` stays elapsed time; the
//! date now is `epochAtZero + now()`.
//! @ref LLP 1027.000.000 (Draft; the Bluesky client is its consumer)

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactTime";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["epochAtZero", "utcOffset"];

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
