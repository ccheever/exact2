//! The display preferences (LLP 1061 D4; LLP 1069.000 D1), told to the
//! running host and kept for the next boot's first frame; the date (LLP
//! 1027.000.000) and where the viewer is, told to the running host, and
//! before each boot for its first settle (`set_launch_time`, `set_launch_place`);
//! and the host's live clock while it owns time (the timers RFC, D1), told to the
//! running host and kept for every boot (`set_live_clock`).

use super::{not_booted, Bridge};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// The running session's role overrides (LLP 1121 stage 5), bound
    /// around each call to it (`registry::with_entry`).
    pub fn roles(&self) -> Option<std::rc::Rc<exact_kernel::style::roles::Roles>> {
        self.host.as_ref().map(|h| h.runner().roles().clone())
    }

    /// App modes, `axis=value` pins comma-separated in the input buffer
    /// (LLP 1121 D5), set in one commit; after a `?`, only checked.
    pub fn set_mode(&mut self, len: usize) -> u32 {
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())]).unwrap_or("");
        // `?` first: only ask whether they would be taken.
        let (check, text) = match text.strip_prefix('?') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let pins: Option<Vec<(String, String)>> = text
            .split(',')
            .filter(|pin| !pin.is_empty())
            .map(|pin| {
                pin.split_once('=')
                    .map(|(a, v)| (a.to_owned(), v.to_owned()))
            })
            .collect();
        let Some(pins) = pins else {
            return self.refuse("mode: expected axis=value pins");
        };
        let pins: Vec<(&str, &str)> = pins.iter().map(|(a, v)| (a.as_str(), v.as_str())).collect();
        let out = self.host.as_mut().map_or_else(not_booted, |h| match check {
            true => h.check_modes(&pins),
            false => h.set_modes(&pins),
        });
        self.emit(out)
    }

    /// A live design batch or snapshot frame from the dev stream in the
    /// input buffer (LLP 1121 stage 4), applied in place; returns the batch.
    pub fn design_rows(&mut self, len: usize) -> u32 {
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())])
            .unwrap_or("")
            .to_owned();
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.apply_live(&text));
        self.emit(out)
    }

    /// The binding core's answer (LLP 1121 D12, stage 5) to the request in
    /// the input buffer ([`Host::tamber_json`]), UTF-8 in the output buffer.
    /// Not a batch: nothing changes.
    pub fn tamber(&mut self, len: usize) -> u32 {
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())])
            .unwrap_or("")
            .to_owned();
        self.output = match self.host.as_mut() {
            Some(h) => h.tamber_json(&text),
            None => exact_runner::agent::error("tamber: not booted"),
        }
        .into_bytes();
        self.output.len() as u32
    }

    /// The design digest the running app runs (its plan's, or the live
    /// table's applied since), UTF-8 in the output buffer; empty before
    /// boot. Not a batch: nothing changes.
    pub fn design_digest(&mut self) -> u32 {
        self.output = self
            .host
            .as_ref()
            .map(|h| h.runner().design())
            .unwrap_or_default()
            .into_bytes();
        self.output.len() as u32
    }

    /// The running plan's digest, UTF-8 in the output buffer; empty before boot.
    pub fn plan_digest(&mut self) -> u32 {
        self.output = self
            .host
            .as_ref()
            .map(|h| h.runner().inspection_digest().as_bytes().to_vec())
            .unwrap_or_default();
        self.output.len() as u32
    }

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

    /// The presenter's clock owns time (`Some`), or the agent's seekable one
    /// does (`None`): the running runner and every one booted later take it.
    pub fn set_live_clock(&mut self, wall: Option<LiveClockFn>) {
        self.live_clock = wall;
        if let Some(h) = self.host.as_mut() {
            h.runner_mut().set_live_clock(wall.map(sampler));
        }
    }
}

/// The presenter's clock in the runner's milliseconds (`ExactEnv.wall`).
pub type LiveClockFn = extern "C" fn() -> f64;

/// The runner's sampler for `wall`.
pub(super) fn sampler(wall: LiveClockFn) -> Box<dyn FnMut() -> f64> {
    Box::new(move || wall())
}

#[cfg(test)]
#[path = "live_clock_tests.rs"]
mod live_clock_tests;
