//! A request's life on the page: whether the runner still holds it, its
//! outcome from the page, and an app's abort heard between batches.
use super::Bridge;
use exact_runner::DataSource;

#[cfg(test)]
#[path = "abort_tests.rs"]
mod abort_tests;

impl<D: DataSource> Bridge<D> {
    /// Whether the current runner still holds a request ticket (LLP 1016 D5).
    pub fn request_active(&self, ticket: f64) -> u32 {
        u32::from(
            ticket.is_finite()
                && ticket >= 0.0
                && self
                    .host
                    .as_ref()
                    .is_some_and(|host| host.runner().holds(ticket as u64)),
        )
    }

    /// A request's outcome from the page (LLP 1016 D2): the input buffer
    /// holds `hlen` bytes of header lines, then `blen` bytes of body.
    pub fn fulfill(
        &mut self,
        ticket: f64,
        kind: u32,
        status: u32,
        hlen: usize,
        blen: usize,
        now_ms: f64,
    ) -> u32 {
        let n = self.input.len();
        let hlen = hlen.min(n);
        let blen = blen.min(n - hlen);
        let headers = String::from_utf8_lossy(&self.input[..hlen]).into_owned();
        let body = self.input[hlen..hlen + blen].to_vec();
        let out = match self.host.as_mut() {
            Some(h) => h.fulfill_at(ticket as u64, kind, status, &headers, body, now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// The aborts a realm heard between batches ([`Host::collect_aborts`]).
    pub fn collect_aborts(&mut self) -> u32 {
        let out = match self.host.as_mut() {
            Some(h) => h.collect_aborts(),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }
}
