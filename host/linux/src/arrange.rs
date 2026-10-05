//! Arrange receipts use the ordinary authoring, motion and projected-layout path.
use super::*;
use exact_kernel::CommitReceipt;
use exact_motion::{HoldEnd, HoldStart, HoldToken, Value};
use exact_runner::{ReorderBinding, ReorderGeometry, ReorderProgress, ReorderToken};

impl<D: DataSource> Host<D> {
    fn arrange_receipt(&mut self, receipt: Option<CommitReceipt>) -> Result<(), String> {
        let receipts: Vec<_> = receipt
            .into_iter()
            .map(|receipt| Timed {
                at_ms: self.now_ms,
                receipt,
            })
            .collect();
        self.commit(&receipts, None).map_or(Ok(()), Err)
    }
    pub(crate) fn arrange_begin(
        &mut self,
        binding: ReorderBinding,
        geometry: ReorderGeometry,
        now: f64,
    ) -> Result<Option<(ReorderToken, HoldStart)>, String> {
        if self.runner.reorder_binding(binding.handle) != Some(binding)
            || self.runner.reorder_geometry(binding.list).as_ref() != Some(&geometry)
        {
            return Ok(None);
        }
        self.arrange_clock(now)?;
        let Some(start) = self
            .runner
            .begin_reorder(binding, geometry)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Ok(None);
        };
        self.now_ms = now;
        if let Err(error) = self.arrange_receipt(start.receipt) {
            let _ = self.arrange_cancel(start.token);
            let _ = self.arrange_finish(start.token);
            return Err(error);
        }
        let view = self.kernel().node_by_key(binding.wrapper).map(|n| n.id);
        let hold = match view {
            Some(id) => self.hold_begin(id, Property::Translate, now),
            None => Ok(None),
        };
        if let Ok(Some(hold)) = hold {
            return Ok(Some((start.token, hold)));
        }
        self.arrange_cancel(start.token)?;
        self.arrange_finish(start.token)?;
        hold.map(|_| None)
    }
    /// A grouped session (LLP 1094 D4): no hold, the runner hides the row
    /// when a ghost stands for it.
    pub(crate) fn group_begin(
        &mut self,
        binding: ReorderBinding,
        geometry: ReorderGeometry,
        ghost: bool,
        now: f64,
    ) -> Result<Option<ReorderToken>, String> {
        self.arrange_clock(now)?;
        let Some(start) = self
            .runner
            .begin_group_reorder(binding, geometry, ghost)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Ok(None);
        };
        self.now_ms = now;
        self.arrange_receipt(start.receipt)?;
        Ok(Some(start.token))
    }
    /// The gap in `target` at content `y` (LLP 1094 D5).
    pub(crate) fn group_into(
        &mut self,
        token: ReorderToken,
        target: NodeKey,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<bool, String> {
        match self
            .runner
            .preview_reorder_into(token, target, geometry, y)
            .map_err(|e| format!("{e:?}"))?
        {
            ReorderProgress::Accepted { receipt } => {
                self.arrange_receipt(receipt)?;
                Ok(true)
            }
            ReorderProgress::Stale | ReorderProgress::NeedsMeasurement => Ok(false),
        }
    }
    /// A key's step (LLP 1094 D9).
    pub(crate) fn group_step(
        &mut self,
        token: ReorderToken,
        step: exact_runner::ReorderStep,
    ) -> Result<(), String> {
        let receipts = self
            .runner
            .reorder_step(token, step)
            .map_err(|e| format!("{e:?}"))?;
        for receipt in receipts {
            self.arrange_receipt(Some(receipt))?;
        }
        Ok(())
    }
    /// A grouped drop on its target, which may hold (LLP 1094 D8).
    pub(crate) fn group_drop(
        &mut self,
        token: ReorderToken,
        geometry: ReorderGeometry,
    ) -> Result<bool, String> {
        let receipt = self
            .runner
            .drop_reorder(token, geometry)
            .map_err(|e| format!("{e:?}"))?;
        let accepted = receipt.is_some();
        self.arrange_receipt(receipt)?;
        Ok(accepted)
    }
    pub(crate) fn arrange_clock(&self, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < self.now_ms || now / 1000. < self.engine.now() {
            Err("invalid Arrange clock".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn arrange_preview(
        &mut self,
        token: ReorderToken,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<bool, String> {
        match self
            .runner
            .preview_reorder(token, geometry, y)
            .map_err(|e| format!("{e:?}"))?
        {
            ReorderProgress::Accepted { receipt } => {
                self.arrange_receipt(receipt)?;
                Ok(true)
            }
            ReorderProgress::Stale | ReorderProgress::NeedsMeasurement => Ok(false),
        }
    }
    pub(crate) fn arrange_drop(
        &mut self,
        token: ReorderToken,
        hold: HoldToken,
        geometry: ReorderGeometry,
    ) -> Result<bool, String> {
        if !self.engine.has_hold(hold) || !self.runner.has_reorder(token) {
            return Ok(false);
        }
        let receipt = self
            .runner
            .drop_reorder(token, geometry)
            .map_err(|e| format!("{e:?}"))?;
        let accepted = receipt.is_some();
        self.arrange_receipt(receipt)?;
        Ok(accepted)
    }
    pub(crate) fn arrange_cancel(&mut self, token: ReorderToken) -> Result<(), String> {
        let receipt = self
            .runner
            .cancel_reorder(token)
            .map_err(|e| format!("{e:?}"))?;
        self.arrange_receipt(receipt)
    }
    pub(crate) fn arrange_finish(&mut self, token: ReorderToken) -> Result<(), String> {
        let receipt = self
            .runner
            .finish_reorder(token)
            .map_err(|e| format!("{e:?}"))?;
        self.arrange_receipt(receipt)
    }
    /// C0 only. A new hold retires no caller-owned token: neighbors are admitted
    /// only when not held, and the source uses its existing token separately.
    pub(crate) fn arrange_rebase_neighbor(
        &mut self,
        key: NodeKey,
        value: Value,
        now: f64,
    ) -> Result<(), String> {
        if self.kernel().node_by_key(key).is_none()
            || self.engine.is_held(motion_node(key), Property::Translate)
        {
            return Ok(());
        }
        if let Some(start) = self
            .engine
            .begin_hold(
                motion_node(key),
                Property::Translate,
                now / 1000.,
                Some(value),
            )
            .map_err(|e| format!("{e:?}"))?
        {
            self.engine
                .end_hold(
                    start.token,
                    now / 1000.,
                    HoldEnd::Release {
                        velocity: Value::ZERO,
                    },
                )
                .map_err(|e| format!("{e:?}"))?;
        }
        self.present_hold()
    }
}
