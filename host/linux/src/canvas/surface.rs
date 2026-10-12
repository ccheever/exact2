//! The top layer's windows, for the reader (LLP 1115.001 D10): an alert and
//! a menu are windows of Android's own (`Dialog`, `PopupWindow`), made by
//! the reader (`host/android/canvas`, `Surfaces.kt`), and what is in each is
//! painted by the presenter (`presenter/top.rs`). This is the Canvas host's
//! half: how a surface's picture is recorded, and the one call the reader
//! makes about a window.
//!
//! **A second stream, in a second view.** A surface is not part of the
//! page's stream. The presenter paints it through a recorder of its own
//! and the words go to the reader with the statement of the surface
//! (request kind 7), where the window's view replays them with a player of
//! its own. The stream is whole: it defines every face it draws with, so a
//! reader made again needs nothing the last one was sent, and it shares no
//! id with the page's stream (no row, group, layer or picture slot).

use super::{CanvasHost, Recorder};
use crate::paint::Backend;
use crate::presenter::top::SurfacePainter;
use exact_runner::DataSource;

/// Records a surface's picture for the reader's window.
pub(super) struct Windows;

impl SurfacePainter for Windows {
    fn record(
        &mut self,
        _size: (f32, f32),
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Backend),
    ) -> Vec<u32> {
        // A recorder made for this picture alone. It is never begun or
        // finished as a frame is: a frame's begin and finish keep the
        // page's own groups, layers and last stream for the thread, and a
        // surface has none of those.
        let mut recorder = Recorder::new();
        recorder.scale = scale;
        paint(&mut recorder);
        std::mem::take(&mut recorder.ops)
    }
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// What happened in the window the reader made for surface `serial`
    /// ([`crate::presenter::top::event`]): a touch (0 down, 1 up, 2 move,
    /// 3 cancel) at (`x`, `y`), pixels from the window's own top left; the
    /// system dismissed it (8); the reader could not make it (9); a key
    /// (16 and up).
    pub fn surface(&mut self, serial: u32, what: i32, x: f32, y: f32) {
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        self.p.surface_event(serial, what, x, y, now);
        // A row's press may have committed: the page paints with it.
        self.force = true;
    }
}
