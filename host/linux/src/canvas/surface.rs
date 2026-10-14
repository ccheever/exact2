//! The top layer's windows, for the reader (LLP 1115.001 D10): an alert and
//! a menu are windows of Android's own (`Dialog`, `PopupWindow`), made by
//! the reader (`host/android/canvas`, `Surfaces.kt`), and what is in each is
//! painted by the presenter (`presenter/top.rs`). This is the Canvas host's
//! half: how a surface's picture is recorded, and the one call the reader
//! makes about a window.
//!
//! The app's bars are shown the same way (D11; `presenter/bars.rs`,
//! `Bars.kt`): Material's top app bar and its navigation bar, each a picture
//! of its own in a view of its own over the page. Their statement is taken
//! with each stream and not on the channel, so a bar and the page under it
//! change in one frame.
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
use crate::presenter::Presenter;
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

/// The reader makes this presenter's top layer as windows and shows its
/// bars, each from a recording of its own.
pub(super) fn serve<D: DataSource>(p: &mut Presenter<D>) {
    p.serve_windows(Box::new(Windows));
    if let Some(e) = p.serve_bars(Box::new(Windows)) {
        eprintln!("exact: bars: {e}");
    }
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// What the reader is told of the app's bars, when that is not what it
    /// was last told ([`crate::presenter::Presenter::reader_bars`] has the
    /// words): taken with each stream.
    pub fn bars(&mut self) -> Option<Vec<i32>> {
        self.p.reader_bars(self.scale)
    }

    /// What happened in bar `which` (1 the top app bar, 2 the navigation
    /// bar): a touch (0 down, 1 up, 2 move, 3 cancel) at (`x`, `y`), pixels
    /// from the bar's own top left. What it pressed commits, and the page
    /// paints with it.
    pub fn bar(&mut self, which: i32, what: i32, x: f32, y: f32) {
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        self.p.bar_event(which, what, x, y, now);
    }

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
