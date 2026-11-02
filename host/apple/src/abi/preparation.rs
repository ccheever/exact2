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
        let current = self.host.as_ref().map(Host::carry);
        let matches = match (&candidate.carried_from, &current) {
            (Some(before), Some(now)) => same_carried(before, now),
            (None, None) => true,
            _ => false,
        };
        if !matches {
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

/// Value's language equality merges signed zeros; a reload must preserve
/// every distinction expressions can observe, including nested values.
fn same_carried(a: &exact_runner::Carried, b: &exact_runner::Carried) -> bool {
    use exact_runner::{
        compare::{equivalent, equivalent_all},
        Carried,
    };
    let Carried {
        router,
        data_revision,
        keeps_answers,
        slots,
        panels,
        resources,
        store_readers,
        now_ms,
        store,
        forgot_waiting,
        faults,
        modes,
        uploads,
    } = a;
    router == &b.router
        && data_revision == &b.data_revision
        && keeps_answers == &b.keeps_answers
        && store_readers == &b.store_readers
        && now_ms.to_bits() == b.now_ms.to_bits()
        && store == &b.store
        && forgot_waiting == &b.forgot_waiting
        && faults == &b.faults
        && modes == &b.modes
        && slots.len() == b.slots.len()
        && slots
            .iter()
            .zip(&b.slots)
            .all(|((an, av), (bn, bv))| an == bn && equivalent(av, bv))
        && panels.len() == b.panels.len()
        && panels
            .iter()
            .zip(&b.panels)
            .all(|((ap, an, av), (bp, bn, bv))| ap == bp && an == bn && equivalent(av, bv))
        && resources.len() == b.resources.len()
        && resources
            .iter()
            .zip(&b.resources)
            .all(|((an, asrc, aa, av), (bn, bsrc, ba, bv))| {
                an == bn && asrc == bsrc && equivalent_all(aa, ba) && equivalent(av, bv)
            })
        && uploads.len() == b.uploads.len()
        && uploads.iter().zip(&b.uploads).all(|(a, b)| {
            use exact_runner::UploadEnd::{Answered, Failed, Sent};
            a.resource == b.resource
                && a.source == b.source
                && equivalent_all(&a.args, &b.args)
                && match (&a.end, &b.end) {
                    (Answered(av), Answered(bv)) => equivalent(av, bv),
                    (Sent(at), Sent(bt)) => at == bt,
                    (Failed(af), Failed(bf)) => af == bf,
                    _ => false,
                }
        })
}
