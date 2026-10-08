//! Motion starts at the first frame that shows it (LLP 1003.001): the
//! display's frames start what the engine holds pending, and a frame task's
//! target time never becomes the engine's input clock. Split from host.rs.

use super::*;

impl<D: DataSource> Host<D> {
    /// A motion frame: seek the engine to `now_ms` and report every
    /// presentation value that changed. Nothing else moves.
    pub fn tick(&mut self, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        let mut batch = Batch::new();
        let seek = self
            .engine
            .advance((now_ms / 1000.0).max(self.engine.now()));
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        // The frame is sampled before anything reads the engine (a reorder's
        // end, height layout); what the tick begins later starts at present.
        self.start_frame();
        if self.arrange_settled() {
            return self.arrange_settle();
        }
        let error = self.height_layout_if_needed(&mut batch).err();
        self.runner.canvas_frame();
        // A tick is never waited for where draws are deferred (LLP 1072
        // §9): it draws in its own turn.
        self.canvas_draw_turn(&mut batch);
        // Only suspended ancestor mappings need a settle recheck. Normal
        // photo Translate/Scale frames keep the existing cheap tick path.
        if self.transform_drags.mapping_pending {
            self.emit_transform_drags(&mut batch);
        }
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// [`Host::tick`] for the display frame presented at `frame_ms`
    /// (`targetTimestamp`, D5): every present until it returns starts what
    /// waits for that frame, whatever in the tick began it.
    pub fn tick_at(&mut self, now_ms: f64, frame_ms: f64) -> String {
        if !self.engine.starts_on_frame() {
            return self.tick(now_ms);
        }
        self.presence.frame = Some(frame_ms / 1000.0);
        let out = self.tick(now_ms);
        self.presence.frame = None;
        out
    }

    /// [`Host::frame`] at the display's target `frame_ms`, the wall at
    /// `wall_ms`: the runner and its frame tasks take the target (LLP 1073);
    /// the host's clock, which inputs and the engine's input clock follow,
    /// takes the wall (D5).
    pub fn frame_at(&mut self, frame_ms: f64, wall_ms: f64) -> String {
        if self.engine.starts_on_frame() {
            self.now_ms = wall_ms.max(self.now_ms);
        }
        self.frame(frame_ms)
    }

    /// Turn the first-frame rule on or off (D7). Off, at the agent's
    /// takeover, what waits starts at `at_ms`, sampled there, in the batch
    /// returned; no timer fires.
    pub fn start_on_frame(&mut self, on: bool, at_ms: f64) -> String {
        let error = self.set_start_on_frame(on, at_ms).err();
        let mut batch = Batch::new();
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// [`Host::start_on_frame`] with nothing presented: a prepared host's
    /// started plays reach the presenter in its first batch after commit.
    pub(crate) fn set_start_on_frame(&mut self, on: bool, at_ms: f64) -> Result<(), String> {
        let at = (at_ms / 1000.0).max(self.engine.now());
        let started = self
            .engine
            .set_start_on_frame(on, at)
            .map_err(|e| format!("start on frame: {e:?}"))?;
        self.lower_started(&started);
        Ok(())
    }

    /// The engine time a commit at `ms` is heard at: never behind the engine,
    /// and, with the rule on, never past the host's clock, which a runner a
    /// frame task put at its target may lead (D5).
    pub(super) fn engine_time(&self, ms: f64) -> f64 {
        let t = if self.engine.starts_on_frame() {
            ms.min(self.now_ms) / 1000.0
        } else {
            ms / 1000.0
        };
        t.max(self.engine.now())
    }

    /// While a motion tick's frame is set, start what waits for it, and send
    /// the started plays' lowered specs again (D4).
    pub(super) fn start_frame(&mut self) {
        let Some(frame) = self.presence.frame else {
            return;
        };
        match self.engine.present_frame(frame) {
            Ok(started) => self.lower_started(&started),
            Err(e) => debug_assert!(false, "a frame instant is finite: {e:?}"),
        }
    }

    fn lower_started(&mut self, nodes: &[u64]) {
        for node in nodes {
            if let Some(view) = self
                .keys
                .get(&exact_kernel::motion::node_key(*node))
                .copied()
            {
                self.svg.element(self.runner.kernel(), view);
            }
        }
    }
}
