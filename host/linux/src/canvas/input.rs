//! A driver's scroll steps and the reader's touches (LLP 1076 §3.3): a
//! driver's scroll moves the feed (a finger's is `scroll.rs`), and a touch
//! is the presenter's pointer at the view's pixels. Each touch says what touched (LLP 1115.001 D7.4): this entry is
//! where a finger is told from a mouse, which desktop Linux's own inputs
//! (evdev, VNC) and the agent remain.

use super::{CanvasHost, Section};
use crate::presenter::PointerKind;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// A driver from outside (a benchmark's, a test's) scrolls the feed by
    /// `dy` pixels: the scroller a finger last moved, else what is under
    /// the viewport's center.
    pub fn scroll(&mut self, dy: f32) {
        let (x, y) = (self.viewport.0 / 2.0, self.viewport.1 / 2.0);
        let _s = Section::begin(c"exact scroll");
        self.driven();
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

    /// A touch (0 down, 1 up, 2 move, 3 cancel) at pixels, by Android's
    /// tool type `tool`: a mouse (3) is a mouse, a stylus or its eraser (2,
    /// 4) a pen, and anything else a finger. Action 4 is no new touch: the
    /// one that is down has been held still for the system's long-press
    /// time, which the reader measures; what names a context menu there
    /// takes it ([`crate::presenter::Presenter::long_press`]), and the
    /// touch then presses nothing when it lifts.
    pub fn touch(&mut self, action: i32, x: f32, y: f32, tool: i32) {
        let _s = Section::begin(c"exact touch");
        self.p.set_pointer_kind(match tool {
            3 => PointerKind::Mouse,
            2 | 4 => PointerKind::Pen,
            _ => PointerKind::Touch,
        });
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
            4 => {
                self.p.long_press(x, y, now);
                Ok(())
            }
            _ => self.p.pointer_cancel(now),
        };
        self.touched(action, x, y, now, r);
    }
}
