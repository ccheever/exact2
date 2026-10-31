//! The agent's clock (LLP 1012 `clock`): the runner's timers and the
//! motion engine's seek, moved to one instant; and the display's live clock
//! while it owns time (the timers RFC, D1), given to every host booted.
use super::*;

/// A live clock: milliseconds since the loop's start, the domain of the
/// times the loop passes the host.
pub type Wall = std::rc::Rc<dyn Fn() -> f64>;

/// The live clock of a loop that started at `started`.
pub fn wall_since(started: std::time::Instant) -> Wall {
    std::rc::Rc::new(move || started.elapsed().as_secs_f64() * 1000.0)
}

impl<D: DataSource> Presenter<D> {
    /// Android's painted loop: defer an ordinary timer behind a waiting paint,
    /// but bring a due motion sample's clock forward before testing frame demand.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn android_clock(
        &mut self,
        now: f64,
        vsync: bool,
        last_tick: &mut f64,
        frame_ms: f64,
    ) -> (bool, Option<String>) {
        let motion = self.host.motion_due_ms().is_some_and(|due| due <= now);
        if vsync && self.host.wants_frames() {
            *last_tick = now;
            (false, self.animation_frame(now))
        } else if self.wake_due_ms().is_some_and(|due| due <= now)
            && (motion || now - *last_tick >= frame_ms * 0.5)
        {
            *last_tick = now;
            if vsync && self.dirty() && !motion {
                (true, None)
            } else {
                (false, self.advance(now))
            }
        } else {
            (false, None)
        }
    }

    /// The Canvas reader's frame turn, also driven headlessly in tests.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn reader_clock(&mut self, now: f64, live: bool) -> Option<String> {
        let error = if self.wants_frames() && live {
            self.animation_frame(now)
        } else if self.wake_due_ms().is_some_and(|due| due <= now) {
            self.advance(now)
        } else {
            None
        };
        if self.needs_animation_frame() {
            self.tick(now);
        }
        error
    }

    /// The runner's clock (timers), from the presenter's loop.
    pub fn advance(&mut self, now_ms: f64) -> Option<String> {
        let (e, paint) = self.host.advance_effects(now_ms);
        self.dirty |= paint;
        let after = self.finish_commit();
        self.hatch_timers(now_ms);
        let ends = self.deliver_scroll_ends();
        e.or(after)
            .or(ends)
            .or(self.admit_timer_waits(now_ms, false))
    }

    /// A display frame for the frame tasks (LLP 1073 D5): timers due by
    /// `now_ms`, then every frame task once at it.
    pub fn animation_frame(&mut self, now_ms: f64) -> Option<String> {
        let admission = self.admit_timer_waits(now_ms, true);
        let (e, paint) = if self.live_clock.is_some()
            && self
                .host
                .runner()
                .plan()
                .timers
                .iter()
                .any(|row| row.wait != 0)
        {
            let a = self.host.runner_mut().complete_frame_tasks(now_ms);
            self.host.advanced(a)
        } else {
            self.host.frame(now_ms)
        };
        self.dirty |= paint;
        let after = self.finish_commit();
        self.hatch_presented(now_ms);
        let ends = self.deliver_scroll_ends();
        admission.or(e).or(after).or(ends)
    }

    /// Move the clock: timers fire, motion is seeked to where the clock
    /// landed. Returns the landing time and the error.
    pub fn clock(&mut self, to_ms: f64) -> (f64, Option<String>) {
        let e = self.host.advance(to_ms);
        self.clocked(e)
    }

    /// [`Presenter::clock`], stopping after a timer that sends as well: an
    /// agent's jump ([`exact_runner::Runner::advance_until_request`]).
    pub fn clock_until_request(&mut self, to_ms: f64) -> (f64, Option<String>) {
        let e = self.host.advance_until_request(to_ms);
        self.clocked(e)
    }

    /// The `then`s an agent's input settled, the clock unmoved (LLP 1012 §2).
    pub fn land_then(&mut self) -> (f64, Option<String>) {
        let e = self.host.land_then();
        self.clocked(e)
    }

    /// The display's clock owns time (`Some`), or a driver's seekable one
    /// does (`None`): the running runner and every one booted later take it.
    pub fn set_live_clock(&mut self, wall: Option<Wall>) {
        self.live_clock = wall;
        self.give_clock();
    }

    /// Delegate ordinary live timers to per-task OS timers.
    pub fn use_native_timers(&mut self, enabled: bool) {
        if self.native_timers != enabled {
            self.native_timers = enabled;
            self.give_clock();
        }
    }

    /// Identity of the live clock and runner, including reload and takeover.
    pub fn native_timer_generation(&self) -> u64 {
        self.clock_generation
    }

    /// Deliver only the task whose native callback ran, preserving normal commit effects.
    pub fn fire_native_timer(
        &mut self,
        generation: u64,
        timer: &exact_runner::NativeTimer,
        now_ms: f64,
    ) -> Option<String> {
        if generation != self.clock_generation {
            return None;
        }
        let a = self
            .host
            .runner_mut()
            .fire_native_timer(timer.index, timer.generation, now_ms);
        let (e, paint) = self.host.advanced(a);
        self.dirty |= paint;
        let after = self.finish_commit();
        e.or(after)
    }

    pub(super) fn give_clock(&mut self) {
        self.clock_generation = self.clock_generation.wrapping_add(1);
        self.host.runner_mut().use_native_timers(self.native_timers);
        let wall = self.live_clock.clone();
        let sample = wall.map(|wall| Box::new(move || wall()) as Box<dyn FnMut() -> f64>);
        self.host
            .runner_mut()
            .hold_waits(exact_runner::WaitKinds::ALL);
        self.host
            .runner_mut()
            .present_frames(self.live_clock.is_some());
        self.host.runner_mut().set_live_clock(sample);
    }

    /// Whether the carrier reports that this session presents no frames.
    pub fn timer_visibility(&mut self, hidden: bool) {
        self.timers_hidden = hidden;
    }

    /// Latest reader report: scrolling, animation sampling, and unapplied publications.
    pub fn timer_observation(&mut self, scrolling: bool, sampling: bool, applying: bool) {
        self.timer_observations = Some((scrolling, sampling, applying));
    }

    pub(crate) fn admit_timer_waits(&mut self, cutoff: f64, frame: bool) -> Option<String> {
        if self.live_clock.is_none()
            || !self
                .host
                .runner()
                .plan()
                .timers
                .iter()
                .any(|row| row.wait != 0)
        {
            return None;
        }
        let mut error = None;
        loop {
            let scrolling = !self.timers_hidden && self.scroll_activity();
            let demand = !self.timers_hidden
                && (self
                    .host
                    .motion_due_ms()
                    .is_some_and(|due| due <= self.host.now())
                    || self.host.canvas_wants_frame()
                    || self.wants_frames()
                    || scrolling
                    || self
                        .timer_observations
                        .is_some_and(|(_, sampling, _)| sampling));
            let mut kinds = exact_runner::WaitKinds::NONE;
            if frame || self.timers_hidden {
                kinds = kinds.union(exact_runner::WaitKinds::FRAME);
            }
            if frame || !demand {
                kinds = kinds.union(exact_runner::WaitKinds::UPDATE);
            }
            if !scrolling {
                kinds = kinds.union(exact_runner::WaitKinds::SCROLLEND);
            }
            let batch = self.host.runner_mut().release_waits(kinds, cutoff);
            let continuing = batch.continuing;
            let (e, paint) = self.host.advanced(batch.advanced);
            self.dirty |= paint;
            error = error.or(e).or(self.finish_commit());
            if !continuing || self.host.runner().is_poisoned() {
                break;
            }
        }
        error
    }

    /// Sample idle after applying ordinary and admitted batches, before the loop sleeps.
    pub fn admit_idle_timer(&mut self, now: f64) -> Option<String> {
        self.idle_timer_retry = false;
        self.live_clock.as_ref()?;
        let pending = self.host.runner().waits();
        if pending.frame.held || pending.update.held || pending.scrollend.held {
            self.admit_timer_waits(now, false);
        }
        let waits = self.host.runner().waits();
        if !waits.idle.held {
            return None;
        }
        let idle = exact_runner::IdleObservation {
            scrolling: !self.timers_hidden && self.scroll_activity(),
            demand: !self.timers_hidden
                && (self
                    .host
                    .motion_due_ms()
                    .is_some_and(|due| due <= self.host.now())
                    || self.host.canvas_wants_frame()
                    || self.wants_frames()
                    || self
                        .timer_observations
                        .is_some_and(|(_, sampling, _)| sampling)),
            applying: self
                .timer_observations
                .is_some_and(|(_, _, applying)| applying),
            ordinary: waits.frame.held || waits.update.held || waits.scrollend.held,
            next_ms: self
                .host
                .runner()
                .timer_due_ms()
                .into_iter()
                .chain(self.host.motion_due_ms())
                .chain(
                    self.host
                        .runner()
                        .native_timers()
                        .iter()
                        .map(|timer| timer.due_ms),
                )
                .reduce(f64::min),
        };
        if !idle.allows(now) {
            return None;
        }
        let batch = self
            .host
            .runner_mut()
            .release_waits(exact_runner::WaitKinds::IDLE, now);
        let (e, paint) = self.host.advanced(batch.advanced);
        self.dirty |= paint;
        let result = e.or(self.finish_commit());
        self.idle_timer_retry = result.is_none() && self.host.runner().waits().idle.held;
        result
    }

    fn clocked(&mut self, e: Option<String>) -> (f64, Option<String>) {
        let landed = self.host.now();
        self.host.tick(landed);
        // A seek can move motion with no commit (a press's scale, a
        // transition), and `after_commit` repaints only for a new epoch.
        self.dirty = true;
        let after = self.after_commit();
        let ends = self.deliver_scroll_ends();
        (landed, e.or(after).or(ends))
    }
}

#[cfg(test)]
#[path = "live_clock_tests.rs"]
mod live_clock_tests;
