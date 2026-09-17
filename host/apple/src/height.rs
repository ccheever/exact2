//! One explicitly registered numeric border-box panel; no implicit adoption.
use super::*;
use exact_kernel::{BoxSizing, PresentedHeight};
use exact_motion::Value;

/// Atomic admission refusal; existing ownership and clock are unchanged.
#[derive(Debug, PartialEq, Eq)]
pub enum HeightOwnerError {
    /// No live node has this view id.
    UnknownView(ViewId),
    /// This trial cannot project across multiple layout roots.
    RequiresSingleRoot {
        /// Current number of attached roots.
        roots: usize,
    },
    /// The node is not an attached, visible numeric-height box.
    Ineligible(NodeKey),
    /// Content-box catch needs resolved padding, not authored dimensions.
    UnsupportedBoxSizing(NodeKey),
}

/// The ownership change admitted before producing the returned batch.
#[derive(Debug, PartialEq, Eq)]
pub enum HeightOwnerDisposition {
    /// Same owner, including no owner.
    Unchanged,
    /// First owner adopted.
    Registered {
        /// Newly registered generational key.
        node: NodeKey,
    },
    /// Previous Height retired; its other properties survive.
    Replaced {
        /// Retired owner.
        previous: NodeKey,
        /// Newly registered owner.
        node: NodeKey,
    },
    /// Explicit cleanup.
    Cleared {
        /// Retired owner.
        previous: NodeKey,
    },
}

/// Accepted ownership plus ordinary host presentation/error status.
#[derive(Debug)]
pub struct HeightOwnerChange {
    /// Accepted registration change (independent of subsequent layout failure).
    pub disposition: HeightOwnerDisposition,
    /// Apply through the ordinary presenter. Layout failures preserve frames.
    pub batch: String,
}

impl<D: DataSource> Host<D> {
    /// The sole trial owner; replacement Hosts begin without registration.
    pub fn height_owner(&self) -> Option<NodeKey> {
        self.height_owner
    }

    /// Explicitly register, replace or clear the numeric border-box trial.
    /// Invalid candidates cannot retire a previous hold or move the clock.
    /// `None` always allows cleanup, including after unsupported authoring.
    pub fn set_height_owner(
        &mut self,
        view: Option<ViewId>,
    ) -> Result<HeightOwnerChange, HeightOwnerError> {
        let next = view
            .map(|view| {
                let kernel = self.runner.kernel();
                let node = kernel
                    .node(view)
                    .ok_or(HeightOwnerError::UnknownView(view))?;
                if self.height_owner == Some(node.key) {
                    return Ok(node.key);
                }
                let roots = self.runner.kernel().roots().len();
                if roots != 1 {
                    return Err(HeightOwnerError::RequiresSingleRoot { roots });
                }
                if kernel.height_target(node.key).is_none() {
                    return Err(HeightOwnerError::Ineligible(node.key));
                }
                if node.style.box_sizing != BoxSizing::BorderBox {
                    return Err(HeightOwnerError::UnsupportedBoxSizing(node.key));
                }
                Ok(node.key)
            })
            .transpose()?;
        let previous = self.height_owner;
        if previous == next {
            return Ok(HeightOwnerChange {
                disposition: HeightOwnerDisposition::Unchanged,
                batch: self.finish(Batch::new(), None),
            });
        }
        let disposition = match (previous, next) {
            (a, b) if a == b => HeightOwnerDisposition::Unchanged,
            (None, Some(node)) => HeightOwnerDisposition::Registered { node },
            (Some(previous), Some(node)) => HeightOwnerDisposition::Replaced { previous, node },
            (Some(previous), None) => HeightOwnerDisposition::Cleared { previous },
            (None, None) => HeightOwnerDisposition::Unchanged,
        };
        if previous != next {
            if let Some(old) = previous {
                self.engine
                    .remove_property(motion_node(old), Property::Height);
            }
            self.height_owner = next;
        }
        let mut batch = Batch::new();
        let error = self.height_layout_if_needed(&mut batch).err();
        self.present(&mut batch, false);
        Ok(HeightOwnerChange {
            disposition,
            batch: self.finish(batch, error),
        })
    }

    /// Revalidate on every receipt/layout even when the owner was untouched.
    /// Consume the retirement before deriving another presentation sample.
    pub(super) fn sync_height_owner(&mut self) -> Result<(), String> {
        let Some(owner) = self.height_owner else {
            return Ok(());
        };
        let kernel = self.runner.kernel();
        let sync = kernel.height_motion_sync(owner);
        let policy_ok = self.runner.kernel().roots().len() == 1
            && kernel
                .node_by_key(owner)
                .is_some_and(|node| node.style.box_sizing == BoxSizing::BorderBox);
        if !policy_ok || !sync.retired.is_empty() {
            // Registration is generational intent, not active presentation.
            // Hidden/unsupported owners may readopt; destroyed keys cannot.
            if kernel.node_by_key(owner).is_none() {
                self.height_owner = None;
            }
            self.engine
                .remove_property(motion_node(owner), Property::Height);
        }
        // A host-policy refusal must not readopt an otherwise kernel-eligible
        // content-box target. Kernel retirement contains only this Height.
        if policy_ok || !sync.retired.is_empty() {
            sync.apply(&mut self.engine)
                .map_err(|e| format!("height sync: {e:?}"))?;
        }
        Ok(())
    }

    pub(super) fn height_sample(&self) -> Result<Option<(NodeKey, f32)>, String> {
        let Some(owner) = self.height_owner else {
            return Ok(None);
        };
        let Some(value) = self.engine.value(motion_node(owner), Property::Height) else {
            return Ok(None);
        };
        let value = value.x;
        if !value.is_finite() || value > f32::MAX as f64 {
            return Err("height sample outside layout range".into());
        }
        // CSS heights cannot be negative; spring undershoot clips at zero.
        // The sampled curve/velocity and newest authored target remain intact.
        Ok(Some((owner, value.max(0.0) as f32)))
    }

    /// Motion-only entry: unchanged held values and compositor-only frames do
    /// not traverse layout. Final samples/retirement still publish once.
    pub(super) fn height_layout_if_needed(&mut self, batch: &mut Batch) -> Result<(), String> {
        self.sync_height_owner()?;
        if self.height_sample()? != self.height_projection {
            self.layout(batch)?;
        }
        Ok(())
    }

    /// Catch the last published constrained CSS height, never authored target.
    /// Trial admission makes border-box size and used CSS height equivalent.
    pub(super) fn height_catch(&self, key: NodeKey) -> Option<Value> {
        if self.height_owner != Some(key) {
            return None;
        }
        self.engine.value(motion_node(key), Property::Height)?;
        let id = self.keys.get(&key)?;
        let px = self.mirror.get(id)?.frame?.3;
        (px.is_finite() && px >= 0.).then(|| Value::scalar(px as f64))
    }

    pub(super) fn presented_height(
        &self,
        sample: Option<(NodeKey, f32)>,
    ) -> Option<PresentedHeight> {
        sample.map(|(node, px)| PresentedHeight {
            node,
            px,
            epoch: self.runner.kernel().epoch(),
        })
    }
}
