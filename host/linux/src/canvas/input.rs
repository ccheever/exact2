//! The reader's scroll steps and touches (LLP 1076 §3.3): a driver's scroll
//! moves the feed, and a touch is the presenter's pointer at the view's
//! pixels.

use super::{CanvasHost, Section};
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// Scroll what is under the viewport's center by `dy` pixels.
    pub fn scroll(&mut self, dy: f32) {
        let (x, y) = (self.viewport.0 / 2.0, self.viewport.1 / 2.0);
        let _s = Section::begin(c"exact scroll");
        self.scrolled = self.prefetching;
        self.scrolled_at = self.now();
        self.travel.scrolled(dy / self.scale, self.scrolled_at);
        self.p.hold_collections(self.prefetching);
        // The feed: the scroller the first wheel at the centre took, moved
        // directly after (a nested list under the centre would take it).
        let moved = self
            .feed
            .is_some_and(|id| self.p.scroll_by(id, dy / self.scale));
        if !moved && self.feed.is_none() {
            self.p.wheel_at(x, y, 0.0, dy / self.scale);
            self.feed = self.p.last_wheel();
        }
        self.p.hold_collections(false);
    }

    /// A touch (0 down, 1 up, 2 move, 3 cancel) at pixels.
    pub fn touch(&mut self, action: i32, x: f32, y: f32) {
        let _s = Section::begin(c"exact touch");
        self.force |= action != 2;
        if action != 2 && self.painted_once {
            self.p.slice_collections(Some(0), 0.0);
            self.responding = true;
            self.lead = false;
            self.travel.stopped();
        }
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        let r = match action {
            0 => self.p.pointer_down(x, y, now).map(|_| ()),
            1 => self.p.pointer_up(x, y, now).map(|_| ()),
            2 => self.p.pointer_move(x, y, now).map(|_| ()),
            _ => self.p.pointer_cancel(now),
        };
        self.touched(action, x, y, now, r);
    }
}
