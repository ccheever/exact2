//! Dropping across lists (LLP 1094 D5): the grouped session's calls, each
//! answered with the ordinary batch and its `reorder` op (`arrange_group.rs`).

use super::{not_booted, Bridge};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Lift a grouped grip; `ghost` nonzero when the host draws one.
    pub fn reorder_group_begin(
        &mut self,
        handle: u32,
        scroll_top: f64,
        ghost: u32,
        now: f64,
    ) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.reorder_group_begin(handle, scroll_top, ghost != 0, now)
        });
        self.emit(out)
    }
    /// The ghost's centre in `target`'s content.
    pub fn reorder_move_into(
        &mut self,
        token: u64,
        target: u32,
        content_y: f64,
        scroll_top: f64,
        inside: u32,
        now: f64,
    ) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.reorder_move_into(token, target, content_y, scroll_top, inside != 0, now)
        });
        self.emit(out)
    }
    /// A key's or custom action's step.
    pub fn reorder_group_step(&mut self, token: u64, step: u32, now: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.reorder_group_step(token, step, now));
        self.emit(out)
    }
    /// The contact ended: drop (nonzero) or cancel.
    pub fn reorder_group_end(&mut self, token: u64, drop: u32, now: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.reorder_group_end(token, drop != 0, now));
        self.emit(out)
    }
    /// The ghost landed or faded.
    pub fn reorder_group_finish(&mut self, token: u64, now: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.reorder_group_finish(token, now));
        self.emit(out)
    }
}
