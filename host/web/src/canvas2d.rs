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
    /// The lists the last two batches with lists named by address: the glue
    /// reads them in place while it applies the batch (a batch applied
    /// inside another's is the second).
    kept: [Vec<exact_runner::CanvasList>; 2],
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
        let lists = self.runner.take_canvas_lists();
        for c in &lists {
            batch.canvas2d(c);
        }
        if !lists.is_empty() {
            self.canvas2d.kept.swap(0, 1);
            self.canvas2d.kept[1] = lists;
        }
        // Handles a Rust draw asked for: the page loads them (LLP 1056 D9).
        let images = self.runner.take_canvas_image_requests();
        if !images.is_empty() {
            batch.canvas2d_images(&images);
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

impl<D: DataSource> Host<D> {
    /// The page decoded (or failed) an image handle (LLP 1056 D9): `payload`
    /// is `src\0width\0height\0ok\0lifetimes`, the lifetimes a JSON array of
    /// the canvases whose TypeScript draw asked for it on the page. The
    /// canvases that asked draw again, cause `"image"`.
    pub fn canvas_image(&mut self, payload: &str) -> String {
        let mut parts = payload.split('\0');
        let src = parts.next().unwrap_or("");
        let num = |p: Option<&str>| p.and_then(|v| exact_num::parse_f64(v).ok()).unwrap_or(0.0);
        let (w, h) = (num(parts.next()), num(parts.next()));
        let ok = parts.next() == Some("1");
        let lifetimes: Vec<u64> = parts
            .next()
            .unwrap_or("")
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        let result = if ok {
            Ok((w as u32, h as u32))
        } else {
            Err("the image did not load".to_string())
        };
        self.runner.canvas_image(src, result, &lifetimes);
        let mut batch = Batch::new();
        self.canvas_turn(&mut batch);
        batch.finish(self.runner.timer_due_ms(), self.runner.now_ms(), None)
    }

    /// A font finished loading on the page (LLP 1056 D8): canvases that drew
    /// text draw again, cause `"font"`.
    pub fn canvas_fonts(&mut self) -> String {
        self.runner.canvas_fonts_loaded();
        let mut batch = Batch::new();
        self.canvas_turn(&mut batch);
        batch.finish(self.runner.timer_due_ms(), self.runner.now_ms(), None)
    }
}
