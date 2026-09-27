//! What the host tells the app about the device and its user, after boot:
//! the date (LLP 1027.000.000) and the display preferences (LLP 1061 D5).
//! This host reads no system setting; an agent sets them (`prefer`).
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Report the place and seed together, including on the first frame.
    pub fn set_place(&mut self, place: &exact_runner::time::Place) -> Option<String> {
        if let Some(error) = self.host.set_place(place) {
            return Some(error);
        }
        self.after_commit()
    }

    /// A candidate has a new runner, but belongs to the same launch.
    pub(super) fn restore_time(&self, host: &mut Host<D>) -> Result<(), HostError> {
        let time = self.host.runner().wall_time();
        if let Some(error) = host.set_place(self.host.runner().place()) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_time(time.epoch_at_zero, time.utc_offset) {
            return Err(HostError::Layout(error));
        }
        Ok(())
    }

    /// The date, as the clock `now()` reads: Unix ms at clock zero (the
    /// runner's clock starts at boot) and the local zone's offset, in
    /// minutes east of UTC, as Apple and the web read theirs (LLP 1054 R12).
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> Option<String> {
        let error = self.host.set_time(epoch_at_zero, utc_offset);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// `prefers-reduced-motion` and `prefers-reduced-transparency`, as
    /// `exactViewport()` answers them: re-answered in one commit.
    pub fn set_preferences(&mut self, preferences: exact_runner::Preferences) -> Option<String> {
        let error = self.host.set_preferences(preferences);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// The system's appearance, which `setScheme("system")` follows.
    pub fn set_system_scheme(&mut self, dark: bool) {
        self.scheme.1 = dark;
        self.apply_scheme();
    }

    pub(crate) fn app_scheme(&mut self, scheme: Option<bool>) {
        self.scheme.0 = scheme;
        self.apply_scheme();
    }

    /// What a `light-dark()` colour resolves to here (LLP 1034 D2).
    fn apply_scheme(&mut self) {
        let dark = self.scheme.0.unwrap_or(self.scheme.1);
        self.dirty |= self.brush.dark != dark;
        self.host.set_scheme(dark);
        self.brush.dark = dark;
        if let Some(error) = self.host.content_region_appearance(dark) {
            self.host.log(error);
        }
    }

    /// The scheme a `light-dark()` colour resolves to now.
    pub fn dark(&self) -> bool {
        self.brush.dark
    }
}
