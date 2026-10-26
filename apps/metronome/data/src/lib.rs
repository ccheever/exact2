//! Metronome's data source: it answers nothing. What plays is the app's
//! `sequence`, which the runner expands (LLP 1125 D3); it grants MIDI and
//! alarms.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Metronome;

impl DataSource for Metronome {
    /// MIDI out (LLP 1125 D6): the notes and clock go to an output the
    /// person picks. Alarms (LLP 1133.030 D17): the practice timer.
    fn grants(&self) -> &str {
        "device.midi purpose.midi\ndevice.alarms practice-timer-purpose"
    }

    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
