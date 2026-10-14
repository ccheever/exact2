//! The display preferences (LLP 1061 D4; LLP 1069.000 D1), told to the
//! running host and kept for the next boot's first frame; the date (LLP
//! 1027.000.000) and where the viewer is, told to the running host, and
//! before each boot for its first settle (`set_launch_time`, `set_launch_place`).

use super::{not_booted, Bridge};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// The user's display preferences changed or became known (LLP 1061
    /// D4; LLP 1069.000 D1): bit 0 reduced motion, bit 1 reduced
    /// transparency, bit 2 contrast more, bit 3 contrast less, bit 4 a dark
    /// system.
    pub fn set_preferences(&mut self, bits: u32) -> u32 {
        let preferences = exact_runner::Preferences::from_bits(bits);
        self.preferences = preferences;
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_preferences(preferences));
        self.emit(out)
    }

    /// The date changed (LLP 1027.000.000), told to the running host.
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_time(epoch_at_zero, utc_offset));
        self.emit(out)
    }

    /// The date the next boot's first settle reads
    /// (`exact_runner::time::launching`), told before each boot or
    /// candidate; the running host is not told. A date that does not
    /// validate leaves the last one.
    pub fn set_launch_time(&mut self, epoch_at_zero: f64, utc_offset: f64) {
        let time = exact_runner::time::WallTime {
            epoch_at_zero,
            utc_offset,
        };
        if time.validate().is_ok() {
            self.launch_facts.time = Some(time);
        }
    }

    /// Where the viewer is, for the next boot's first settle, as
    /// `set_place` reads it from the input buffer (`locale NUL timeZone NUL
    /// seed`); the running host is not told. A fact that does not validate
    /// (or a missing seed) leaves the last one.
    pub fn set_launch_place(&mut self, len: usize) {
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())]).unwrap_or("");
        if let Some(place) = exact_runner::time::Place::launched(text) {
            self.launch_facts.place = Some(place);
        }
    }

    /// A boot's first viewport: the size, and the preferences last told.
    pub(super) fn boot_viewport(&self, width: f32, height: f32) -> exact_runner::Viewport {
        exact_runner::Viewport {
            preferences: self.preferences,
            ..exact_runner::Viewport::sized(width as f64, height as f64)
        }
    }
}
