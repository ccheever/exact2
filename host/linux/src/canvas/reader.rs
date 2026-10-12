//! A reader that takes a host that runs (its activity was made again), and
//! the size of one that tells no window.
//!
//! @ref LLP 1115.001 D15.1 (configuration changes are handled in place; an
//! activity made again adopts the running host)

use super::{colors, device, layer, pending, CanvasHost, Recorder};
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// The page fills `size` pixels at `scale` pixels a point: a reader
    /// that told no window ([`CanvasHost::set_window`]) and so no insets.
    fn resize(&mut self, size: (u32, u32), scale: f32) {
        if !scale.is_finite() || scale <= 0.0 {
            return;
        }
        let viewport = (size.0 as f32 / scale, size.1 as f32 / scale);
        if viewport == self.viewport && scale == self.scale {
            return;
        }
        let rescaled = scale != self.scale;
        self.scale = scale;
        self.viewport = viewport;
        crate::fields::rescale(scale);
        let error = if rescaled {
            self.p.resize_scaled(viewport.0, viewport.1, scale)
        } else {
            self.p.resize(viewport.0, viewport.1)
        };
        if let Some(e) = error {
            eprintln!("exact: resize: {e}");
        }
        self.force = true;
    }

    /// A new reader takes this host: Android made the activity again in the
    /// process (a configuration change it does not declare, or an activity
    /// destroyed in the background and shown again), and its view has none
    /// of what the last one was sent. Nothing boots. The recorder starts
    /// from nothing, so the next stream defines every face, picture, row
    /// and layer again, in the window the new reader told; the app's
    /// state, its routes and its scroll offsets are as they were.
    pub fn adopt(&mut self, size: (u32, u32), scale: f32) {
        self.p.reader_replaced(Box::new(Recorder::new()));
        // Pictures announced to the last reader and never fetched: the new
        // recorder numbers its own from one.
        pending().clear();
        layer::ALIVE.with(|a| a.borrow_mut().clear());
        self.tracks.clear();
        self.tracks_epoch = 0;
        self.clock_sent = false;
        // The last paint is the last reader's: nothing moves it, and the
        // next frame is a paint.
        self.painted = None;
        self.moved = 0;
        self.quiet = None;
        self.paint_by = None;
        self.travel = crate::travel::Travel::default();
        self.forget_chrome();
        // The device's colours as the new reader read them: a wallpaper
        // changed while the last one was stopped is a new palette (D4).
        if let Some(palette) = colors::palette() {
            self.p.set_palette(&palette);
        }
        match device::windowed(size, scale) {
            Some(told) => self.set_window(told),
            None => self.resize(size, scale),
        }
        self.force = true;
        eprintln!(
            "exact: canvas adopted by a new reader, {}x{} px, scale {scale}",
            size.0, size.1
        );
    }
}
