//! The device's colours, from the reader (LLP 1115.001 D4). The reader
//! resolves Material 3's colour roles for the device and tells them whole,
//! before the host boots and again whenever they change (`Native.colors`).
//! What it said last is kept for the process, as the device's settings are
//! (`device.rs`): a host that boots after the call resolves every role by
//! it from its first layout, and one that a new reader adopts is told it
//! again. Which of Exact's roles is which of Material's is the presenter's
//! to say (`presenter/palette.rs`).

use super::CanvasHost;
use crate::presenter::Palette;
use exact_runner::DataSource;

/// What the reader last said of the device's colours, for a host that
/// boots after it said so.
static PALETTE: std::sync::Mutex<Option<Palette>> = std::sync::Mutex::new(None);

/// Keep `palette` for the host that boots next (the reader's `colors`, from
/// any thread; a running host is told on its own thread, by
/// [`CanvasHost::set_colors`]).
pub fn colors(palette: Palette) {
    *PALETTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(palette);
}

/// What the reader last said of the device's colours, if anything.
pub(super) fn palette() -> Option<Palette> {
    *PALETTE.lock().unwrap_or_else(|e| e.into_inner())
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// The device's palette changed, or is told to a running host for the
    /// first time: every colour role resolves to it, and the next frame is
    /// a paint.
    pub fn set_colors(&mut self, palette: &Palette) {
        self.p.set_palette(palette);
        self.force = true;
    }
}
