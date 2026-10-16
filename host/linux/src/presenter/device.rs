//! What the device's shell tells the presenter about the device and about
//! itself (LLP 1115.001 D3, D15): the Canvas host's reader on Android, and
//! an agent standing in for it.
use super::*;

/// The device's settings, whole (LLP 1115.001 D3.1): the shell tells them
/// once before the first frame and again, all of them, whenever one changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// Night mode: the system's scheme is dark.
    pub night: bool,
    /// The person's font scale: 1 unless they asked for larger or smaller text.
    pub font_scale: f32,
    /// The first of the device's locales, a BCP 47 tag (`en-GB`).
    pub locale: String,
    /// That locale is laid out right to left.
    pub rtl: bool,
    /// The person's clock is the 24-hour one.
    pub hour24: bool,
    /// "Remove animations" is on.
    pub reduced_motion: bool,
    /// Bold text is on.
    pub bold_text: bool,
    /// High contrast is on.
    pub high_contrast: bool,
    /// The screen is touched with a finger, which cannot hover.
    pub touch: bool,
    /// How far a finger moves before its contact is a drag and no longer a
    /// tap, in points (Android's scaled touch slop); 0 where the shell
    /// knows none, which keeps the shared one.
    pub touch_slop: f32,
    /// How the device writes a date and a time, in lines
    /// (`paint::dates::Dates::from_lines`); empty where the shell tells none.
    pub dates: String,
}

/// The root font size at a font scale of 1: CSS's `medium`.
const ROOT_FONT_SIZE: f64 = 16.0;

impl<D: DataSource> Presenter<D> {
    /// The device's settings, as its shell read them (LLP 1115.001 D3.1,
    /// D3.2). What has a door here goes through it, each one a commit only
    /// when its value changed:
    ///
    /// - night mode is the system's scheme (`setScheme("system")` and
    ///   `prefers-color-scheme` follow it);
    /// - the font scale is the root font size, 16 times it;
    /// - the locale is the viewer's place, which chooses the app's string
    ///   table and its dates;
    /// - "remove animations" is `prefers-reduced-motion`, high contrast is
    ///   `prefers-contrast: more`;
    /// - a touch screen is `pointer: coarse` and `hover: none`, and its
    ///   touch slop is how far a contact moves before it is a drag;
    /// - how the device writes dates is how a date control shows its value.
    ///
    /// The layout direction, the 12 or 24-hour clock and bold text are kept
    /// ([`Presenter::device`]) and change nothing yet: the runner has no
    /// door for them.
    pub fn set_device(&mut self, device: &Device) -> Option<String> {
        if self.device.as_ref() == Some(device) {
            return None;
        }
        self.device = Some(device.clone());
        self.slop = match device.touch && device.touch_slop.is_finite() && device.touch_slop > 0.0 {
            true => f64::from(device.touch_slop),
            false => exact_motion::gesture::SLOP,
        };
        self.set_system_scheme(device.night);
        let runner = self.host.runner();
        let mut preferences = runner.viewport().preferences;
        preferences.dark = device.night;
        preferences.reduced_motion = device.reduced_motion;
        preferences.contrast = match device.high_contrast {
            true => exact_runner::Contrast::More,
            false => exact_runner::Contrast::NoPreference,
        };
        (preferences.pointer, preferences.hover) = match device.touch {
            true => (exact_runner::Pointer::Coarse, exact_runner::Hover::None),
            false => (exact_runner::Pointer::Fine, exact_runner::Hover::Hover),
        };
        let place = exact_runner::time::Place {
            locale: device.locale.clone(),
            ..runner.place().clone()
        };
        let mut error = self.set_preferences(preferences);
        if device.font_scale.is_finite() && device.font_scale > 0.0 {
            let px = ROOT_FONT_SIZE * f64::from(device.font_scale);
            error = error.or(self.set_root_font_size(px));
            // A field the shell hosts starts at the scaled size too (D8).
            error = error.or(self.scale_field_font(device.font_scale));
        }
        // A tag the runner refuses (an empty one) leaves the place as it was.
        if !device.locale.is_empty() && place != *self.host.runner().place() {
            error = error.or(self.set_place(&place));
        }
        // A date control shows its value as the device writes it (D9).
        let dates = crate::paint::dates::Dates::from_lines(&device.dates);
        if self.brush.dates != dates {
            self.brush.dates = dates;
            self.dirty = true;
            error = error.or(self.after_commit());
        }
        error
    }

    /// What the device's shell last said ([`Presenter::set_device`]); `None`
    /// on a host whose shell reads no setting.
    pub fn device(&self) -> Option<&Device> {
        self.device.as_ref()
    }

    /// The backend's reader was made again (LLP 1115.001 D15.1: Android made
    /// the activity again while this host ran): it holds none of the rows,
    /// faces or pictures the last one was sent. `backend` records for it
    /// from nothing, and the next frame paints everything again; the app,
    /// its state and its scroll offsets are as they were.
    pub fn reader_replaced(&mut self, backend: Box<dyn Backend>) {
        self.brush.replace_backend(backend);
        self.brush.forget_rows();
        // Nor of what it was told of the scrollers: the new one asks again.
        self.reader = Default::default();
        // Nor of Back and the keyboard, which it is told anew (D5), nor of
        // the text fields it hosts (D8).
        self.requests_reset();
        self.hosted_reset();
        // Nor of the top layer's open surface, whose window it makes anew,
        // nor of its bars.
        self.surface_reset();
        self.bars_reset();
        self.dirty = true;
    }
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
