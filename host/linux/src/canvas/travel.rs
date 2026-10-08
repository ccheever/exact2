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

/// The share of its viewport the feed travels before a pass is worth
/// running. Below 12,000 dp/s a pass each third step mounted a row a commit
/// (and, below 3,000 dp/s, most found nothing to mount): the list's layout,
/// its publication, the paint order and the stream were walked for each row.
/// The window leads the view by a viewport, so what a pass this late mounts
/// is still three quarters of one away.
const BATCH: f32 = 0.25;

/// Logical px a second from which the feed's window leads its travel, and
/// below which a leading one stops. Two thresholds, and a lead of all the
/// runner allows rather than one that follows the speed: a window whose far
/// edge moved with each frame's jitter mounted and retired rows there.
const LEAD_ON: f32 = 8000.0;
const LEAD_OFF: f32 = 4000.0;
/// The velocity a leading window is reported with, logical px/s: past any
/// viewport's cap (the runner leads by a quarter second of travel, two
/// viewports at most).
const LEAD_VELOCITY: f64 = 1e6;
/// Viewports a leading window reaches past the view: its own, and the two
/// of [`LEAD_VELOCITY`].
const LED_REACH: f32 = 3.0;
/// Frames of travel a leading window's pass leaves before the rows it
/// mounts can show, and the viewports of rows one pass may wait for.
const LED_FRAMES: f32 = 5.0;
const LED_BATCH: f32 = 1.75;
/// Scroll steps between a reader's asks for a pass.
const ASK_STEPS: f32 = 3.0;
/// Scroll steps the feed's speed is taken over: all of them, so a step's
/// worth of travel more or less (steps are whole frames' travel, taken when
/// this thread is free) is a seventh of it. Over four, 6,000 dp/s read as
/// 9,000 after a turn and led for the two seconds it then travelled.
const SPEED_STEPS: usize = 8;
/// ms without a scroll step after which travel has stopped: a leading
/// window goes back to a viewport each side.
pub(crate) const SETTLE_MS: f64 = 100.0;

/// The feed's travel, logical px, and what a row costs a pass to build.
#[derive(Default)]
pub(crate) struct Travel {
    /// Per scroll step, smoothed (a step a frame while the view moves).
    step: f32,
    /// The last steps of this travel, oldest first: when (ms) and how far.
    /// A turn or a pause starts them again.
    recent: [(f64, f32); SPEED_STEPS],
    steps: usize,
    /// The side the window leads toward (1: the end, -1: the start), or 0.
    leading: f32,
    /// A step came since the reader last asked for a pass.
    stepped: bool,
    /// Since the last collection pass.
    since_pass: f32,
    /// A pass's ms per row it built, smoothed (0: none measured).
    row_ms: f32,
}

impl Travel {
    /// A scroll step of `dy` logical px, at `at` ms.
    pub(crate) fn scrolled(&mut self, dy: f32, at: f64) {
        let d = dy.abs();
        self.step = if self.step == 0.0 {
            d
        } else {
            0.5 * (self.step + d)
        };
        self.since_pass += d;
        self.stepped = true;
        let toward = dy.signum();
        let last = self.steps.checked_sub(1).map(|i| self.recent[i]);
        if last.is_some_and(|(t, was)| at - t > SETTLE_MS || was.signum() != toward) {
            self.steps = 0;
        }
        if self.steps == SPEED_STEPS {
            self.recent.rotate_left(1);
            self.steps -= 1;
        }
        self.recent[self.steps] = (at, dy);
        self.steps += 1;
        let speed = self.speed();
        let on = if self.leading == toward {
            speed >= LEAD_OFF
        } else {
            speed >= LEAD_ON
        };
        self.leading = if on { toward } else { 0.0 };
    }

    /// The feed's speed, logical px/s, over its last steps: their travel
    /// by the time they took, so steps a busy thread took late, or several
    /// at once, count as the travel they were (0: too few to tell).
    fn speed(&self) -> f32 {
        if self.steps < SPEED_STEPS {
            return 0.0;
        }
        let steps = &self.recent[..self.steps];
        let ms = (steps[self.steps - 1].0 - steps[0].0) as f32;
        let px: f32 = steps[1..].iter().map(|s| s.1.abs()).sum();
        if ms > 0.0 {
            px * 1000.0 / ms
        } else {
            0.0
        }
    }

    /// The velocity the feed's window leads by, logical px/s, positive
    /// toward the end: [`LEAD_VELOCITY`] while the feed travels fast one
    /// way (from [`LEAD_ON`], until it slows below [`LEAD_OFF`], turns or
    /// pauses), else `None`.
    pub(crate) fn lead(&self) -> Option<f64> {
        (self.leading != 0.0).then(|| f64::from(self.leading) * LEAD_VELOCITY)
    }

    /// The reader asks for a pass: whether a step came since it last did
    /// (not its timer's ask once the steps pause, nor its ask for what a
    /// slice left).
    pub(crate) fn asked(&mut self) -> bool {
        std::mem::take(&mut self.stepped)
    }

    /// The feed stopped (a touch took it): nothing leads.
    pub(crate) fn stopped(&mut self) {
        self.steps = 0;
        self.leading = 0.0;
    }

    /// Whether a pass of a leading window can wait: at the reader's next
    /// ask ([`ASK_STEPS`] steps on) the nearest row it would mount is still
    /// [`LED_FRAMES`] frames of travel and three quarters of the `viewport`
    /// past the view, and no more than [`LED_BATCH`] viewports of rows have
    /// come into the window. A pass each third step at 24,000 dp/s mounted
    /// its rows a frame and a half before they showed, a row or two a
    /// commit; one each sixth step under a lead mounts them seven frames
    /// before, eight or nine a pass.
    pub(crate) fn waits_led(&self, viewport: f32) -> bool {
        if self.step <= 0.0 {
            return false;
        }
        let keep = (0.75 * viewport).max(LED_FRAMES * self.step);
        let may = (LED_REACH * viewport - keep)
            .min(LED_BATCH * viewport)
            .max(BATCH * viewport);
        self.since_pass + ASK_STEPS * self.step <= may
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
        (self.fast() && self.row_ms > 0.0).then(|| ((PASS_MS / self.row_ms) as u32).max(1))
    }

    /// Whether the feed travels fast: a pass's rows are limited, and their
    /// cost is worth measuring.
    pub(crate) fn fast(&self) -> bool {
        self.step >= FAST_STEP
    }

    /// Whether a pass can wait for more travel: the feed moves and has come
    /// less than [`BATCH`] of its `viewport` since the last pass. (From
    /// 12,000 dp/s three steps are more than that, and no pass waits.)
    pub(crate) fn waits(&self, viewport: f32) -> bool {
        self.step > 0.0 && self.since_pass < BATCH * viewport
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

    /// A 120 Hz frame, ms.
    const FRAME: f64 = 1000.0 / 120.0;

    fn moving(step: f32, steps: u32) -> Travel {
        let mut t = Travel::default();
        for i in 0..steps {
            t.scrolled(step, f64::from(i) * FRAME);
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
    fn slow_travel_waits_a_quarter_viewport_for_its_pass() {
        // 3,000 dp/s at two steps a frame: 12.5 dp a step.
        assert!(moving(12.5, 3).waits(858.0));
        assert!(!moving(12.5, 18).waits(858.0), "225 dp since the last pass");
        let mut t = moving(12.5, 18);
        t.passed(858.0, 6);
        assert!(t.waits(858.0), "counted from the pass");
        assert!(!moving(100.0, 3).waits(858.0), "12,000 dp/s: three steps");
        assert!(moving(50.0, 3).waits(858.0), "6,000 dp/s: six");
        assert!(!moving(50.0, 6).waits(858.0));
        assert!(!Travel::default().waits(858.0), "nothing moved");
    }

    #[test]
    fn a_view_that_never_moved_owes_nothing() {
        assert_eq!(Travel::default().passed(858.0, 6), None);
    }

    #[test]
    fn travel_counts_from_the_last_pass() {
        let mut t = moving(100.0, 3);
        t.passed(858.0, 6);
        t.scrolled(100.0, 3.0 * FRAME);
        assert_eq!(t.passed(858.0, 6), Some(5));
    }

    #[test]
    fn fast_travel_leads_and_slow_travel_does_not() {
        // 12,000 and 24,000 dp/s lead; 6,000 does not.
        assert_eq!(moving(100.0, 8).lead(), Some(1e6));
        assert_eq!(moving(-200.0, 8).lead(), Some(-1e6));
        assert_eq!(moving(50.0, 30).lead(), None);
        assert_eq!(moving(100.0, 7).lead(), None, "too few steps to tell");
    }

    #[test]
    fn a_lead_holds_until_the_feed_slows_turns_or_pauses() {
        let mut t = moving(100.0, 8);
        // 6,000 dp/s: under the speed that starts a lead, over the one that ends it.
        for i in 8..24 {
            t.scrolled(50.0, f64::from(i) * FRAME);
        }
        assert_eq!(t.lead(), Some(1e6));
        for i in 24..40 {
            t.scrolled(25.0, f64::from(i) * FRAME);
        }
        assert_eq!(t.lead(), None, "3,000 dp/s");
        let mut t = moving(100.0, 8);
        t.scrolled(-100.0, 8.0 * FRAME);
        assert_eq!(t.lead(), None, "a turn");
        for i in 9..16 {
            t.scrolled(-100.0, f64::from(i) * FRAME);
        }
        assert_eq!(t.lead(), Some(-1e6));
        t.scrolled(-100.0, 16.0 * FRAME + 150.0);
        assert_eq!(t.lead(), None, "a pause");
        let mut t = moving(100.0, 8);
        t.stopped();
        assert_eq!(t.lead(), None);
    }

    #[test]
    fn steps_taken_late_are_the_travel_they_were() {
        // 6,000 dp/s; a busy thread takes three frames' steps as one, then
        // the next a millisecond after: no step of 150 px a frame.
        let mut t = moving(50.0, 8);
        t.scrolled(150.0, 10.0 * FRAME);
        t.scrolled(50.0, 10.0 * FRAME + 1.0);
        assert_eq!(t.lead(), None);
        for i in 12..20 {
            t.scrolled(50.0, f64::from(i) * FRAME);
        }
        assert_eq!(t.lead(), None);
    }

    #[test]
    fn a_turn_at_slow_travel_does_not_lead() {
        // 6,000 dp/s turned back: its first steps, one of them two frames'
        // travel taken late and the next a millisecond after.
        let mut t = moving(50.0, 8);
        t.scrolled(-50.0, 8.0 * FRAME);
        t.scrolled(-100.0, 10.0 * FRAME + 7.0);
        t.scrolled(-50.0, 11.0 * FRAME);
        t.scrolled(-50.0, 12.0 * FRAME);
        assert_eq!(t.lead(), None);
        for i in 13..30 {
            t.scrolled(-50.0, f64::from(i) * FRAME);
            assert_eq!(t.lead(), None, "step {i}");
        }
    }

    #[test]
    fn an_ask_knows_whether_the_feed_stepped() {
        let mut t = moving(100.0, 3);
        assert!(t.asked());
        assert!(!t.asked(), "no step since");
        t.scrolled(100.0, 4.0 * FRAME);
        assert!(t.asked());
    }

    #[test]
    fn a_leading_window_waits_longer_for_its_pass() {
        // 24,000 dp/s: a pass each sixth step.
        assert!(moving(200.0, 3).waits_led(858.0));
        assert!(!moving(200.0, 6).waits_led(858.0));
        // 12,000 dp/s: each fifteenth.
        assert!(moving(100.0, 12).waits_led(858.0));
        assert!(!moving(100.0, 15).waits_led(858.0));
        assert!(!Travel::default().waits_led(858.0), "nothing moved");
    }
}
