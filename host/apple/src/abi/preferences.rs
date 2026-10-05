//! The display preferences (LLP 1061 D4; LLP 1069.000 D1): told to the
//! running host, and kept for the next boot's first viewport.

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

    /// A boot's first viewport: the size, and the preferences last told.
    pub(super) fn boot_viewport(&self, width: f32, height: f32) -> exact_runner::Viewport {
        exact_runner::Viewport {
            preferences: self.preferences,
            ..exact_runner::Viewport::sized(width as f64, height as f64)
        }
    }
}
