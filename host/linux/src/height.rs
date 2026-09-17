//! One explicitly registered numeric border-box panel. No gesture recognition.
use super::*;
use exact_kernel::{BoxSizing, Offer, PresentedHeight};

impl<D: DataSource> Host<D> {
    /// Register one panel at the current clock. Invalid replacement is atomic.
    /// This trial requires one root and explicit border-box authoring: a frame's
    /// height is then the actual used CSS height, including min/max constraints.
    pub fn set_height_owner(&mut self, view: Option<ViewId>) -> Result<(), String> {
        let next = view
            .map(|view| {
                let key = self.kernel().node(view).ok_or("unknown height owner")?.key;
                Ok::<_, String>(key)
            })
            .transpose()?;
        if next == self.height_owner {
            return Ok(());
        }
        if let Some(key) = next {
            self.validate_height_owner(key)?;
        }
        if let Some(old) = self.height_owner {
            self.engine
                .remove_property(motion_node(old), Property::Height);
        }
        self.height_owner = next;
        self.height_bindings.automatic = false;
        self.retire_height_binding();
        self.layout()?;
        self.present();
        Ok(())
    }

    /// Generation-bound registration. Temporary ineligibility retires playback,
    /// but keeps registration; destruction requires explicit registration again.
    pub fn height_owner(&self) -> Option<NodeKey> {
        self.height_owner
    }

    pub(super) fn validate_height_owner(&self, key: NodeKey) -> Result<(), String> {
        if self.runner.roots().len() != 1 {
            return Err("height requires a single root".into());
        }
        let node = self
            .kernel()
            .node_by_key(key)
            .ok_or("unknown height owner")?;
        if node.style.box_sizing != BoxSizing::BorderBox {
            return Err("height trial requires box-sizing: border-box".into());
        }
        let (hidden, inert) = self.route_visibility(node.id);
        if hidden || inert || self.kernel().height_target(key).is_none() {
            return Err("height owner is hidden, detached, or not numeric px".into());
        }
        Ok(())
    }

    pub(super) fn sync_height_owner(&mut self) -> Result<(), String> {
        let Some(key) = self.height_owner else {
            return Ok(());
        };
        let mut sync = self.kernel().height_motion_sync(key);
        if self.validate_height_owner(key).is_err() {
            sync = MotionSync {
                retired: vec![(motion_node(key), Property::Height)],
                ..Default::default()
            };
        }
        if self.kernel().node_by_key(key).is_none() {
            self.height_owner = None;
        }
        sync.apply(&mut self.engine)
            .map_err(|e| format!("height sync: {e:?}"))
    }

    pub(super) fn height_active(&self) -> bool {
        self.height_owner
            .is_some_and(|key| self.engine.is_active(motion_node(key), Property::Height))
    }

    fn current_height_projection(&self) -> Result<Option<PresentedHeight>, String> {
        self.height_owner
            .filter(|_| self.height_active())
            .map(|node| {
                let value = self
                    .engine
                    .value(motion_node(node), Property::Height)
                    .expect("active height slot");
                Ok(PresentedHeight {
                    node,
                    epoch: self.kernel().epoch(),
                    px: height_px(value.x)?,
                })
            })
            .transpose()
    }

    // Mandatory layout entries always reconcile eligibility and use fresh epochs.
    // Failed layout invalidates this cache; unchanged samples must retry recovery.
    pub(super) fn layout(&mut self) -> Result<bool, String> {
        #[cfg(test)]
        {
            self.layout_calls += 1;
        }
        self.height_layout_valid = false;
        self.sync_height_owner()?;
        let projection = self.current_height_projection()?;
        let (w, h) = self.viewport;
        let mut changed = false;
        for root in self.runner.roots() {
            let receipt = self
                .runner
                .kernel_mut()
                .compute_layout_presented(root, Offer::definite(w, h), projection)
                .map_err(|e| format!("layout: {e:?}"))?;
            changed |= !receipt.changed.is_empty();
        }
        self.height_projection = projection;
        self.height_layout_valid = true;
        Ok(changed)
    }

    // Compare projected CSS samples, not is_active: a held panel is active but
    // quiescent. Translate ticks and identical held updates must not call layout.
    pub(super) fn layout_motion(&mut self) -> Result<bool, String> {
        if self.height_layout_valid && self.current_height_projection()? == self.height_projection {
            Ok(false)
        } else {
            self.layout()
        }
    }

    pub(super) fn present_hold(&mut self) -> Result<(), String> {
        let result = self.layout_motion();
        // Drain every property dirtied by advance, not just the held property.
        self.present();
        result.map(|_| ())
    }
}

#[cfg(test)]
#[path = "height_tests.rs"]
mod tests;

fn height_px(raw: f64) -> Result<f32, String> {
    if !raw.is_finite() || raw > f32::MAX as f64 {
        return Err("height presentation exceeds finite layout range".into());
    }
    Ok(raw.max(0.) as f32)
}
