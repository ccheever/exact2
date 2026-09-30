//! Canvas 2D on Apple, the Rust half (LLP 1056 D4, D7): after each turn's
//! layout, every 2D canvas's geometry from the kernel, its due draws, and
//! their stamped lists onto the batch as `canvas2d` ops, which the Swift
//! presenter replays into Core Graphics off the main thread (§8.3).

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// The display's scale and physical memory, as the presenter last said:
/// process-wide, since every session shares the screen.
static SCALE: AtomicU64 = AtomicU64::new(0x4000_0000_0000_0000); // 2.0
static MEMORY: AtomicU64 = AtomicU64::new(8 << 30);

/// What the presenter knows of the display (`exact_canvas_display`).
pub fn set_display(scale: f64, memory: f64) {
    if scale.is_finite() && scale > 0.0 {
        SCALE.store(scale.to_bits(), Ordering::Relaxed);
    }
    if memory.is_finite() && memory > 0.0 {
        MEMORY.store(memory as u64, Ordering::Relaxed);
    }
}

fn scale() -> f64 {
    f64::from_bits(SCALE.load(Ordering::Relaxed))
}

impl<D: DataSource> Host<D> {
    /// The limits for this device: WebKit's iOS area on iOS, a quarter of
    /// physical memory everywhere (LLP 1056 D4, r3).
    pub(crate) fn canvas_limits(&mut self) {
        // Native hosts parse the wide colour forms (LLP 1056 §8.2).
        exact_runner::exact_canvas::color::link_wide();
        let memory = MEMORY.load(Ordering::Relaxed);
        self.runner.set_canvas_limits(exact_runner::Limits::native(
            memory,
            cfg!(target_os = "ios"),
        ));
    }

    /// This turn's canvas work, after layout: geometry, then due draws and
    /// their lists, unless draws are deferred (LLP 1072 §8.5): then the batch
    /// says whether a draw is owed, and `exact_canvas_draw` runs it.
    pub(crate) fn canvas_turn(&mut self, batch: &mut Batch) {
        if !self.canvas_deferred {
            self.canvas_draw_turn(batch);
            return;
        }
        if self.runner.plan().surfaces.is_empty() {
            return;
        }
        self.runner.layout_canvases(scale());
        let held = &self.canvas_held;
        batch.canvas_owed(self.runner.canvas_owed(&|v| !held.contains(&v)));
    }

    /// Where draws are deferred, a turn of canvas work alone: geometry, due
    /// draws, their lists (`exact_canvas_draw`, LLP 1072 §8.5). Main does not
    /// wait for it: the owner runs it after the turn that owed it.
    pub fn canvas_draw(&mut self) -> String {
        let mut batch = Batch::new();
        self.canvas_draw_turn(&mut batch);
        self.finish(batch, None)
    }

    /// Draw canvases in the turns main waits on (false), or only in their
    /// own turn and a frame's tick (true: LLP 1072 §8.5, iOS off the agent).
    pub fn set_canvas_deferred(&mut self, deferred: bool) {
        self.canvas_deferred = deferred;
    }

    /// Geometry, every due draw, and the lists onto the batch.
    pub(crate) fn canvas_draw_turn(&mut self, batch: &mut Batch) {
        if self.runner.plan().surfaces.is_empty() {
            return;
        }
        self.runner.layout_canvases(scale());
        let held = &self.canvas_held;
        self.runner.draw_canvases(&|v| !held.contains(&v));
        let lists = self.runner.take_canvas_lists();
        for c in &lists {
            let (content, radii) =
                self.runner
                    .kernel()
                    .node(c.view)
                    .map_or(((0.0, 0.0, 0.0, 0.0), [0.0; 4]), |n| {
                        let b = exact_kernel::svg::scene::content_box(&n);
                        let s = n.style;
                        // The content edge's curve: each radius less the inset
                        // (CSS Backgrounds 3 §5.2), clipping the bitmap as the
                        // web clips replaced content.
                        let inset = b.0.max(b.1);
                        let radii = [
                            s.border_radius_top_left,
                            s.border_radius_top_right,
                            s.border_radius_bottom_right,
                            s.border_radius_bottom_left,
                        ]
                        .map(|r| (r - inset).max(0.0));
                        (b, radii)
                    });
            batch.canvas2d(c, content, radii);
        }
        if !lists.is_empty() {
            // The last two batches' lists stay alive: the presenter's reader
            // copies them out of this memory (`Batch::canvas2d`).
            self.canvas_kept.swap(0, 1);
            self.canvas_kept[1] = lists;
        }
        batch.canvas_images(self.runner.take_canvas_image_requests());
    }

    /// The presenter's replay of `view` is (`held`) or is no longer behind
    /// (LLP 1056 D5, §8.3): while it is, the canvas's frame request waits,
    /// as a browser's animation frame waits for the last one to present, so
    /// frames drop rather than queue. Every other cause still draws.
    pub fn canvas_held(&mut self, view: ViewId, held: bool) {
        if held {
            self.canvas_held.insert(view);
        } else {
            self.canvas_held.remove(&view);
        }
    }

    /// The app's Core Text measurer for Canvas 2D (LLP 1056 D8).
    pub(crate) fn set_canvas_text(
        &mut self,
        f: crate::canvas_text::CanvasTextFn,
        ctx: *mut std::ffi::c_void,
    ) {
        self.runner
            .set_canvas_text(std::sync::Arc::new(crate::canvas_text::CallbackText::new(
                f, ctx,
            )));
    }

    /// The presenter decoded image handle `src` (`size` in pixels) or could
    /// not (`None`): the canvases that asked for it draw again (LLP 1056 D9).
    pub fn canvas_image(&mut self, src: &str, size: Option<(u32, u32)>) -> String {
        self.runner.canvas_image(
            src,
            size.ok_or_else(|| "the image could not be loaded or decoded".to_string()),
            &[],
        );
        let mut batch = Batch::new();
        self.canvas_turn(&mut batch);
        self.finish(batch, None)
    }

    /// The display changed: canvases redraw at the new scale (a new
    /// generation, LLP 1056 D4).
    pub fn canvas_display(&mut self) -> String {
        self.canvas_limits();
        let mut batch = Batch::new();
        self.canvas_turn(&mut batch);
        self.finish(batch, None)
    }
}
