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

/// The feed's travel, logical px.
#[derive(Default)]
pub(crate) struct Travel {
    /// Per scroll step, smoothed (a step a frame while the view moves).
    step: f32,
    /// Since the last collection pass.
    since_pass: f32,
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
