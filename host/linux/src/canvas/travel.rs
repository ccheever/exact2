//! How soon rows a collection pass mounted out of view can scroll in.
//!
//! A pass that only mounts rows past the view leaves the last paint standing
//! (`quiet`); frames move it until the next paint, which shows the new rows.
//! The window leads the view by a viewport, and the reader runs a pass every
//! few scroll steps, so the nearest row a pass mounts lies a viewport less
//! the travel since the last pass beyond the view. Fast enough (crypto at
//! 24k dp/s: 200 dp a frame, a pass every three), that is under two frames
//! of travel, and the paint that would show it came up to four frames later:
//! the list ended at the last drawn row until it did (20-24% of the view
//! blank). The paint now comes by the frame the rows can first show.

/// Frames between a paint and the frame drawing it: the reader takes the
/// stream before its next frame, which shows it the frame after.
const HANDOFF_FRAMES: f32 = 2.0;

/// The share of a frame one pass may build rows for while the feed travels
/// fast, ms: what it builds shows when it ends, so a pass of 35 ms (Extra
/// Heavy at 24k dp/s: refine, paint and pictures) left the view past its last
/// drawn row for four frames.
const PASS_MS: f32 = 8.0;
/// Logical px a scroll step from which the feed travels fast (50 a 120 Hz
/// frame: 6,000 dp/s).
const FAST_STEP: f32 = 50.0;

/// The feed's travel, logical px, and what a row costs a pass to build.
#[derive(Default)]
pub(crate) struct Travel {
    /// Per scroll step, smoothed (a step a frame while the view moves).
    step: f32,
    /// Since the last collection pass.
    since_pass: f32,
    /// A pass's ms per row it built, smoothed (0: none measured).
    row_ms: f32,
}

impl Travel {
    /// A scroll step of `dy` logical px.
    pub(crate) fn scrolled(&mut self, dy: f32) {
        let d = dy.abs();
        self.step = if self.step == 0.0 {
            d
        } else {
            0.5 * (self.step + d)
        };
        self.since_pass += d;
    }

    /// A collection pass ran: what it mounted reaches `lead` px past the view.
    /// Frames the scroll may move the last paint before one must paint to
    /// show it in time (0: paint now); `None` when that is not soon.
    pub(crate) fn passed(&mut self, lead: f32, soon: u32) -> Option<u32> {
        let ahead = (lead - std::mem::take(&mut self.since_pass)).max(0.0);
        if self.step <= 0.0 {
            return None;
        }
        let frames = (ahead / self.step - HANDOFF_FRAMES).max(0.0) as u32;
        (frames < soon).then_some(frames)
    }

    /// The rows past what shows a pass may build: while the feed travels
    /// fast, those its measured cost fits in [`PASS_MS`] (at least one);
    /// otherwise the whole window (`None`).
    pub(crate) fn limit(&self) -> Option<u32> {
        (self.step >= FAST_STEP && self.row_ms > 0.0)
            .then(|| ((PASS_MS / self.row_ms) as u32).max(1))
    }

    /// A pass built `rows` in `ms`.
    pub(crate) fn built(&mut self, rows: usize, ms: f32) {
        if rows == 0 {
            return;
        }
        let per = ms / rows as f32;
        self.row_ms = if self.row_ms == 0.0 {
            per
        } else {
            0.75 * self.row_ms + 0.25 * per
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Travel;

    fn moving(step: f32, steps: u32) -> Travel {
        let mut t = Travel::default();
        for _ in 0..steps {
            t.scrolled(step);
        }
        t
    }

    #[test]
    fn fast_travel_paints_at_once() {
        // 24k dp/s at 120 Hz, a pass every third step, a 858 dp view.
        assert_eq!(moving(200.0, 3).passed(858.0, 6), Some(0));
    }

    #[test]
    fn slower_travel_leaves_the_paint_where_it_was() {
        assert_eq!(moving(50.0, 3).passed(858.0, 6), None);
        assert_eq!(moving(100.0, 3).passed(858.0, 6), Some(3));
    }

    #[test]
    fn fast_travel_builds_what_a_pass_fits() {
        let mut t = moving(200.0, 3);
        assert_eq!(t.limit(), None, "no cost measured yet");
        t.built(2, 14.0);
        assert_eq!(t.limit(), Some(1));
        let mut cheap = moving(200.0, 3);
        cheap.built(8, 7.0);
        assert_eq!(cheap.limit(), Some(9));
        let mut slow = moving(20.0, 3);
        slow.built(2, 14.0);
        assert_eq!(slow.limit(), None, "slow travel builds whole windows");
    }

    #[test]
    fn a_view_that_never_moved_owes_nothing() {
        assert_eq!(Travel::default().passed(858.0, 6), None);
    }

    #[test]
    fn travel_counts_from_the_last_pass() {
        let mut t = moving(100.0, 3);
        t.passed(858.0, 6);
        t.scrolled(100.0);
        assert_eq!(t.passed(858.0, 6), Some(5));
    }
}
