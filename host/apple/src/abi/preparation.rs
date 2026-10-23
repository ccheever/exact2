//! A candidate may commit only while its carried source is still current.
use super::*;
impl<D: DataSource> Bridge<D> {
    pub(super) fn preparation_refusal(&mut self) -> Option<u32> {
        let candidate = self.prepared.as_ref()?;
        if candidate.module
            && !candidate.let_go
            && self.host.as_ref().is_some_and(|h| h.runner().has_pending())
        {
            return Some(self.refuse_pending());
        }
        if candidate.carried_from != self.host.as_ref().map(Host::carry) {
            self.discard_plan();
            return Some(self.emit("{\"ops\":[],\"pending\":true}".into()));
        }
        None
    }

    /// Check all sessions in one owner job before the durable composition commit.
    pub fn check_prepared(&mut self) -> u32 {
        if self.prepared.is_none() {
            return self.prepare_error("no prepared plan");
        }
        self.preparation_refusal()
            .unwrap_or_else(|| self.emit("{\"ops\":[]}".into()))
    }
}
