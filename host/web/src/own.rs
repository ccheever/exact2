//! The platform's own navigation, under a navigation root with no `navigate`
//! handler: the runner writes the router itself, as one commit, and the page
//! gets its batch. Back is LLP 1115 D5's; the browser's Forward and a link to
//! a declared route are LLP 1115.000 D4.3's.
use super::Host;
use exact_kernel::CommitReceipt;
use exact_runner::{DataSource, RunnerError, Timed};

impl<D: DataSource> Host<D> {
    /// The platform's own Back from visit `id`, at `now_ms`.
    pub fn host_back(&mut self, id: u64, now_ms: f64) -> String {
        self.own(now_ms, |host| host.runner.host_back(id))
    }

    /// The platform's own navigation to `location`, at `now_ms`.
    pub fn host_go(&mut self, location: &str, now_ms: f64) -> String {
        self.own(now_ms, |host| host.runner.host_go(location))
    }

    fn own(
        &mut self,
        now_ms: f64,
        commit: impl FnOnce(&mut Self) -> Result<Option<CommitReceipt>, RunnerError>,
    ) -> String {
        let mut a = self.runner.advance_timed(now_ms.max(self.now_ms));
        self.now_ms = a.now_ms.max(self.now_ms);
        let at_ms = self.now_ms;
        match a.error.is_none().then(|| commit(self)) {
            Some(Ok(r)) => a.receipts.extend(r.map(|receipt| Timed { at_ms, receipt })),
            Some(Err(e)) => a.error = Some(e),
            None => {}
        }
        self.batch_for(&a.receipts, a.error.map(|e| format!("{e:?}")).as_deref())
    }

    /// The motion engine's clock, seconds (tests)
    #[cfg(test)]
    pub(crate) fn motion_now(&self) -> f64 {
        self.springs.now()
    }
}
