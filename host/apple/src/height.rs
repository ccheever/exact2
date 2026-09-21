//! Explicit draggable height plus opted-in content-sized section transitions.
use super::*;
use exact_kernel::{BoxSizing, Dimension};
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
            self.height_targets_dirty = true;
            // A different accepted owner (or cleanup) is explicit intent. The
            // same-live-owner fast path above deliberately preserves provenance.
            self.height_auto_owned = false;
        }
        let mut batch = Batch::new();
        // Publish retirement/rebinding now, without auto-selecting over explicit
        // None. Ordinary future receipts may admit authored handles again.
        self.reconcile_height_handles(&mut batch, false);
        self.cancel_invalid_height_drag();
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

    /// Track declarations during the existing receipt walk, never scan all
    /// mounted nodes on a motion frame. Keep ineligible declarations so an
    /// inherited opt-in or ancestor visibility change can admit them later.
    pub(super) fn track_height_transition(&mut self, view: ViewId) {
        let Some(node) = self.runner.kernel().node(view) else {
            return;
        };
        let key = node.key;
        if node.style.transition.matching(Property::Height).is_some() {
            self.height_transitions.entry(key).or_insert(None);
        } else if self.height_transitions.remove(&key).is_some() && self.height_owner != Some(key) {
            self.engine
                .remove_property(motion_node(key), Property::Height);
        }
    }

    /// Target measurement is per authored/viewport/intrinsic change, not tick.
    /// Admission and first layout snap to the target. Subsequent height changes
    /// use the shared engine's interruption and CSS reversal rules.
    pub(super) fn sync_height_transitions(&mut self) -> Result<(), String> {
        let epoch = self.runner.kernel().epoch();
        if !self.height_targets_dirty && self.height_transition_epoch == Some(epoch) {
            return Ok(());
        }
        let kernel = self.runner.kernel();
        let roots = self.runner.roots();
        let mut eligible = BTreeMap::new();
        let mut auto = Vec::new();
        if roots.len() == 1 && self.content_region.is_none() {
            for (key, previous) in &self.height_transitions {
                if self.height_owner == Some(*key) {
                    continue;
                }
                let Some((height, allowed)) = kernel.height_transition_target(*key) else {
                    continue;
                };
                // CSS uses the opt-in at transition start. Changing that flag
                // alone does not cancel an already running transition.
                if !allowed
                    && !(previous.as_ref() == Some(&height)
                        && self.engine.is_active(motion_node(*key), Property::Height))
                {
                    continue;
                }
                if height == Dimension::Auto {
                    auto.push(*key);
                }
                let transitions = kernel
                    .node_by_key(*key)
                    .expect("eligible height")
                    .style
                    .transition
                    .clone();
                eligible.insert(*key, (height, transitions));
            }
        }
        let measured = if auto.is_empty() {
            Vec::new()
        } else {
            #[cfg(test)]
            {
                self.height_target_passes += 1;
            }
            self.runner
                .kernel_mut()
                .measure_height_targets(
                    roots[0],
                    Offer::definite(self.viewport.0, self.viewport.1),
                    &auto,
                )
                .map_err(|error| format!("height target: {error:?}"))?
        };
        // No engine mutation precedes successful measurement of the whole set.
        for (key, previous) in &mut self.height_transitions {
            let id = motion_node(*key);
            let Some((height, transitions)) = eligible.get(key) else {
                if self.height_owner != Some(*key) {
                    self.engine.remove_property(id, Property::Height);
                }
                *previous = None;
                continue;
            };
            let px = match height {
                Dimension::Points(px) => *px,
                Dimension::Auto => {
                    measured
                        .iter()
                        .find(|p| p.node == *key)
                        .expect("measured auto target")
                        .px
                }
                _ => unreachable!("eligible target"),
            };
            let value = Value::scalar(px as f64);
            let active = self.engine.is_active(id, Property::Height);
            // Content changes under a settled auto height are ordinary layout,
            // not a new transition. During motion, retarget without a jump.
            if previous.is_none()
                || (!active
                    && previous.as_ref() == Some(height)
                    && self.engine.target(id, Property::Height) != Some(value))
            {
                self.engine.remove_property(id, Property::Height);
            }
            self.engine
                .set_transitions(id, transitions.clone())
                .map_err(|e| format!("height transition: {e:?}"))?;
            self.engine
                .observe(Change {
                    node: id,
                    property: Property::Height,
                    value,
                    velocity: None,
                })
                .map_err(|e| format!("height transition: {e:?}"))?;
            *previous = Some(*height);
        }
        self.height_transitions
            .retain(|key, _| self.runner.kernel().node_by_key(*key).is_some());
        self.height_transition_epoch = Some(epoch);
        self.height_targets_dirty = false;
        Ok(())
    }

    pub(super) fn collect_height_samples(&mut self) -> Result<(), String> {
        self.height_sampling.clear();
        let owners = self.height_owner.into_iter().chain(
            self.height_transitions
                .iter()
                .filter(|(key, previous)| {
                    previous.is_some()
                        && self.height_owner != Some(**key)
                        && self.engine.is_active(motion_node(**key), Property::Height)
                })
                .map(|(key, _)| *key),
        );
        for owner in owners {
            let Some(value) = self.engine.value(motion_node(owner), Property::Height) else {
                continue;
            };
            if !value.x.is_finite() || value.x > f32::MAX as f64 {
                return Err("height sample outside layout range".into());
            }
            // Preserve spring velocity/undershoot in the engine, clamp only
            // the CSS length. Reuse the sample storage across motion frames.
            self.height_sampling.push((owner, value.x.max(0.0) as f32));
        }
        Ok(())
    }

    /// Motion-only entry: unchanged held values and compositor-only frames do
    /// not traverse layout. Settled automatic owners return to authored layout.
    pub(super) fn height_layout_if_needed(&mut self, batch: &mut Batch) -> Result<(), String> {
        self.sync_height_owner()?;
        self.sync_height_transitions()?;
        self.collect_height_samples()?;
        if self.height_sampling != self.height_projection {
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
}
