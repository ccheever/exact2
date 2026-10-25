//! A reader that takes a host that runs (its activity was made again), and
//! the size of one that tells no window.
//!
//! @ref LLP 1115.001 D15.1 (configuration changes are handled in place; an
//! activity made again adopts the running host)

use super::{colors, device, layer, stream, CanvasHost, Recorder};
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
        stream::reader_replaced();
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

#[cfg(target_os = "android")]
impl<D: DataSource + Default> CanvasHost<D> {
    /// A reader's `ANativeWindow` of `size` pixels for canvas `view`, alive
    /// until [`CanvasHost::detach_window`]; whether `view` is a GPU canvas.
    #[cfg(target_os = "android")]
    pub fn attach_window(&mut self, view: u32, window: usize, size: (u32, u32)) -> bool {
        self.surfaces = true;
        self.p.attach_window(view, window, size)
    }
    /// The window of canvas `view` is going.
    #[cfg(target_os = "android")]
    pub fn detach_window(&mut self, view: u32) {
        self.p.detach_window(view);
    }
}

/// `CLOCK_MONOTONIC` now, in nanoseconds: the clock `Instant` and the
/// reader's frame times both read.
pub(super) fn monotonic_ns() -> i64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid out pointer for the call's duration.
    #[allow(unsafe_code)]
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t);
    }
    t.tv_sec * 1_000_000_000 + t.tv_nsec
}

/// An atrace section for the rest of a scope.
pub(super) struct Section;
impl Section {
    pub(super) fn begin(name: &'static core::ffi::CStr) -> Section {
        crate::android::section_begin(name);
        Section
    }
}
impl Drop for Section {
    fn drop(&mut self) {
        crate::android::section_end();
    }
}
