//! Canvas 2D on the web, the wasm half (LLP 1056 D4, D7): the browser lays
//! a canvas out, so `canvas2d-glue.js` watches each 2D canvas with a
//! `ResizeObserver` and reports its content box and device scale
//! (`exact_canvas_geometry`); the runner draws, and the stamped lists go to
//! the glue as `canvas2d` ops, replayed into the canvas element's real
//! `CanvasRenderingContext2D`. Frames are the page's: while a canvas wants
//! one, the glue advances the clock each animation frame, and the runner
//! draws the canvases that asked at the landed time.

use super::*;
use exact_runner::Geometry;

/// What the page has been told.
#[derive(Default)]
pub(crate) struct Watch {
    watched: SortedSet<ViewId>,
    frames: bool,
}

impl<D: DataSource> Host<D> {
    /// This turn's canvas work: new canvases to watch, due draws and their
    /// lists, and whether frames are wanted.
    pub(crate) fn canvas_turn(&mut self, batch: &mut Batch) {
        if self.runner.plan().surfaces.is_empty() {
            return;
        }
        let live = self.runner.canvas_views();
        for view in &live {
            if self.canvas2d.watched.insert(*view) {
                batch.canvas2d_watch(*view);
            }
        }
        let gone: Vec<ViewId> = self
            .canvas2d
            .watched
            .iter()
            .copied()
            .filter(|v| !live.contains(v))
            .collect();
        for v in gone {
            self.canvas2d.watched.remove(&v);
        }
        self.runner.draw_canvases(&|_| true);
        for c in self.runner.take_canvas_lists() {
            batch.canvas2d(&c);
        }
        let frames = self.runner.canvas_wants_frame();
        if frames != self.canvas2d.frames {
            self.canvas2d.frames = frames;
            batch.canvas2d_frames(frames);
        }
    }

    /// A 2D canvas's content box (CSS px) and device scale, from the page's
    /// `ResizeObserver`; the batch with any draw it caused.
    pub fn canvas_geometry(&mut self, view: ViewId, width: f64, height: f64, scale: f64) -> String {
        let bitmap = self.runner.canvas_bitmap(view);
        self.runner.set_canvas_geometry(
            view,
            Geometry {
                width,
                height,
                scale,
                bitmap,
            },
        );
        let mut batch = Batch::new();
        self.canvas_turn(&mut batch);
        batch.finish(self.runner.timer_due_ms(), self.runner.now_ms(), None)
    }
}
