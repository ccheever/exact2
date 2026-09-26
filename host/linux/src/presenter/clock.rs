//! The agent's clock (LLP 1012 `clock`): the runner's timers and the
//! motion engine's seek, moved to one instant.
use super::*;

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

    fn clocked(&mut self, e: Option<String>) -> (f64, Option<String>) {
        let landed = self.host.now();
        self.host.tick(landed);
        let after = self.after_commit();
        (landed, e.or(after))
    }
}
