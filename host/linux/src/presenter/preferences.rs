//! What the host tells the app about the device and its user, after boot:
//! the date (LLP 1027.000.000), the display preferences (LLP 1061 D5; LLP
//! 1069.000 D1) and the page's facts (LLP 1069.000 D2). This host reads no
//! system setting: it reports no preference, a light system, a visible page,
//! online, with focus (the app is the whole display: no other window can take
//! it), and no share sheet; an agent sets them (`prefer`).
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
        host.set_scheme(self.dark());
        // The same launch keeps what the device said about itself.
        let runner = self.host.runner();
        let (preferences, page) = (runner.viewport().preferences, runner.page());
        if let Some(error) = host.set_preferences(preferences) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_page(page) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_root_font_size(runner.host_root_font_size()) {
            return Err(HostError::Layout(error));
        }
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

    /// The local zone's offset now, told when it is not the runner's: the
    /// display loop asks before each advance, so a DST change or a new zone
    /// reaches the timer that fires after it (habits F6). An agent drive's
    /// offset is the drive's (`agent.rs` `retell_offset`), never this.
    pub fn follow_local_offset(&mut self) -> Option<String> {
        let time = self.host.runner().wall_time();
        let offset = crate::zone::local_offset_minutes();
        if time.epoch_at_zero <= 0.0 || offset == time.utc_offset {
            return None;
        }
        self.set_time(time.epoch_at_zero, offset)
    }

    /// `prefers-reduced-motion`, `-reduced-transparency`, `-contrast` and
    /// `-color-scheme`, as `exactViewport()` answers them: re-answered in
    /// one commit.
    pub fn set_preferences(&mut self, preferences: exact_runner::Preferences) -> Option<String> {
        let error = self.host.set_preferences(preferences);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// `visibilityState`, `onLine`, `canShare`, `canOpenFiles` and
    /// `hasFocus`, as `exactPage()` answers them: re-answered in one commit.
    pub fn set_page(&mut self, page: exact_runner::Page) -> Option<String> {
        let error = self.host.set_page(page);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// The root font size `rem` lengths follow (LLP 1069.000 D3): 16 here,
    /// as a browser's `medium`, unless an agent says otherwise.
    pub fn set_root_font_size(&mut self, px: f64) -> Option<String> {
        let error = self.host.set_root_font_size(px);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// `devicePosture` and the viewport segments (LLP 1078 D7): what the
    /// agent's `prefer posture` and `prefer segments` set on this host,
    /// which has no fold of its own; kept for `layout.env`.
    pub fn set_segments(
        &mut self,
        posture: exact_runner::Posture,
        cols: u32,
        rows: u32,
        rects: Vec<exact_kernel::Rect>,
    ) -> Option<String> {
        let error = self.host.set_segments(posture, cols, rows, rects.clone());
        if error.is_some() {
            return error;
        }
        self.segments = rects;
        self.dirty = true;
        self.after_commit()
    }

    /// The system's appearance, which `setScheme("system")` follows.
    pub fn set_system_scheme(&mut self, dark: bool) {
        self.scheme.1 = dark;
        self.apply_scheme();
    }

    /// Report an individual view's appearance (LLP 1062 D4).
    pub fn set_view_scheme(&mut self, view: ViewId, dark: bool) {
        self.host.set_view_scheme(view, dark);
        self.dirty = true;
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
