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

    pub(super) fn give_clock(&mut self) {
        let wall = self.live_clock.clone();
        let sample = wall.map(|wall| Box::new(move || wall()) as Box<dyn FnMut() -> f64>);
        self.host.runner_mut().set_live_clock(sample);
    }

    fn clocked(&mut self, e: Option<String>) -> (f64, Option<String>) {
        let landed = self.host.now();
        self.host.tick(landed);
        // A seek can move motion with no commit (a press's scale, a
        // transition), and `after_commit` repaints only for a new epoch.
        self.dirty = true;
        let after = self.after_commit();
        (landed, e.or(after))
    }
}

#[cfg(test)]
#[path = "live_clock_tests.rs"]
mod live_clock_tests;
