//! Authored handle ownership; only known handlers are revisited after receipts.
use super::{Batch, Host, Lowered};
use exact_kernel::{motion::motion_node, NodeKey, ViewId};
use exact_motion::{EngineError, HoldEnd, HoldStart, HoldToken, Property, Value};
use exact_runner::{DataSource, Event};
use std::collections::BTreeMap;

/// The live authored handle and its one admitted height target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightDragBinding {
    /// Handle generation, checked again on every delivery.
    pub handle: NodeKey,
    /// Resolved strict ancestor generation.
    pub target: NodeKey,
}

pub(super) struct Handle {
    pub key: NodeKey,
    // None has never been published; Some(None) explicitly has no binding.
    published: Option<Option<(NodeKey, ViewId)>>,
}

#[derive(Clone, Copy)]
struct Active {
    binding: HeightDragBinding,
    token: HoldToken,
    action_fired: bool,
}

#[derive(Default)]
pub(super) struct HeightDrags {
    handles: BTreeMap<ViewId, Handle>,
    automatic: bool,
    active: Option<Active>,
}

impl HeightDrags {
    pub fn insert(&mut self, view: ViewId, key: NodeKey) {
        self.handles.insert(
            view,
            Handle {
                key,
                published: None,
            },
        );
    }

    pub fn remove(&mut self, view: ViewId) {
        self.handles.remove(&view);
    }

    pub fn programmatic(&mut self) {
        self.automatic = false;
    }
}

impl<D: DataSource> Host<D> {
    /// Resolve a known authored handler against the currently admitted target.
    /// The kernel owns the IDREF/ancestry rules; the page never resolves them.
    pub fn height_drag_binding(&self, view: ViewId) -> Option<HeightDragBinding> {
        let handle = self.height_drags.handles.get(&view)?.key;
        let target = self.runner.kernel().height_drag_target(handle)?;
        (self.springs.height_owner().map(|(key, _)| key) == Some(target))
            .then_some(HeightDragBinding { handle, target })
    }

    /// Capture the computed, constrained browser height at recognition.
    /// Stale handle generations refuse before validating samples or the clock.
    pub fn begin_height_drag(
        &mut self,
        handle: NodeKey,
        presented: Value,
        now_ms: f64,
    ) -> Result<Option<(HoldStart, String)>, EngineError> {
        let Some(view) = self.runner.kernel().node_by_key(handle).map(|n| n.id) else {
            return Ok(None);
        };
        let Some(binding) = self
            .height_drag_binding(view)
            .filter(|b| b.handle == handle)
        else {
            return Ok(None);
        };
        let target = self
            .runner
            .kernel()
            .node_by_key(binding.target)
            .expect("resolved")
            .id;
        let Some((start, batch)) = self.begin_hold(target, Property::Height, presented, now_ms)?
        else {
            return Ok(None);
        };
        self.height_drags.active = Some(Active {
            binding,
            token: start.token,
            action_fired: false,
        });
        Ok(Some((start, batch)))
    }

    /// Apply the final absolute presentation, then dispatch on the handle while
    /// its target remains held. The caller separately ends once using op2/op3.
    /// No intermediate lowering may consume unrelated dirty animation frames.
    pub fn dispatch_height_held(
        &mut self,
        serial: u64,
        handle_view: ViewId,
        height: f64,
        velocity: f64,
        now_ms: f64,
    ) -> Result<Option<String>, EngineError> {
        self.validate_height_delivery(serial);
        let Some(active) = self.height_drags.active.filter(|a| {
            a.token.serial() == serial
                && !a.action_fired
                && self.height_drag_binding(handle_view) == Some(a.binding)
        }) else {
            return Ok(None);
        };
        // Validate both payload fields before the engine can advance. Its own
        // update validates the position range and time atomically.
        if !velocity.is_finite() {
            return Err(EngineError::NonFinite);
        }
        if !self
            .springs
            .update_hold(serial, Value::scalar(height), now_ms / 1000.0)?
        {
            return Ok(None);
        }
        self.height_drags.active = Some(Active {
            action_fired: true,
            ..active
        });
        Ok(Some(self.dispatch_at(
            handle_view,
            Event::HeightRelease { height, velocity },
            now_ms,
        )))
    }

    fn active_height_valid(&self, active: Active) -> bool {
        let Some(handle) = self.runner.kernel().node_by_key(active.binding.handle) else {
            return false;
        };
        active.token.property() == Property::Height
            && active.token.node() == motion_node(active.binding.target)
            && self.springs.token(active.token.serial()) == Some(active.token)
            && self.height_drag_binding(handle.id) == Some(active.binding)
    }

    pub(super) fn height_hold_valid(&self, serial: u64) -> bool {
        self.height_drags
            .active
            .filter(|a| a.token.serial() == serial)
            .is_none_or(|active| self.active_height_valid(active))
    }

    pub(super) fn cancel_invalid_height_drag(&mut self) {
        if let Some(active) = self.height_drags.active {
            if !self.active_height_valid(active) {
                // Accepted receipts already synchronized their clamped clock
                // and newest declaration while held. A stale delivery never
                // synchronizes its incoming clock. Stale Engine tokens cannot
                // end a newer generic/handle hold on this same property.
                let _ = self.springs.end_hold(
                    active.token.serial(),
                    HoldEnd::Cancel,
                    self.springs.now(),
                );
                self.height_drags.active = None;
            }
        }
    }

    pub(super) fn validate_height_delivery(&mut self, serial: u64) {
        if self
            .height_drags
            .active
            .is_some_and(|a| a.token.serial() == serial)
        {
            self.cancel_invalid_height_drag();
        }
    }

    pub(super) fn reconcile_height_drags(&mut self, batch: &mut Batch) {
        self.cancel_invalid_height_drag();
        let kernel = self.runner.kernel();
        self.height_drags
            .handles
            .retain(|_, handle| kernel.node_by_key(handle.key).is_some());
        let current = self
            .springs
            .height_owner()
            .filter(|(key, _)| kernel.node_by_key(*key).is_some());
        let valid: Vec<_> = self
            .height_drags
            .handles
            .values()
            .filter_map(|handle| kernel.height_drag_target(handle.key))
            .collect();
        // Programmatic registration has its own lifetime. Auto registration
        // survives any one handle's removal, but never steals a second target.
        let next = if current.is_some() && !self.height_drags.automatic {
            current.map(|(key, _)| key)
        } else {
            current
                .map(|(key, _)| key)
                .filter(|key| valid.contains(key))
                .or_else(|| valid.first().copied())
        };
        if current.map(|(key, _)| key) != next || current != self.springs.height_owner() {
            let view = next.and_then(|key| kernel.node_by_key(key).map(|n| n.id));
            for item in self
                .springs
                .set_height_owner(kernel, view)
                .expect("resolved owner")
            {
                if let Lowered::Retire { view, property } = item {
                    batch.retire_motion(view, property.name());
                }
            }
            self.height_drags.automatic = next.is_some();
        }
        self.cancel_invalid_height_drag();
    }

    pub(super) fn emit_height_drags(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let owner = self.springs.height_owner().map(|(key, _)| key);
        for (&view, handle) in &mut self.height_drags.handles {
            let target = kernel
                .height_drag_target(handle.key)
                .filter(|key| Some(*key) == owner)
                .and_then(|key| kernel.node_by_key(key).map(|n| (key, n.id)));
            if handle.published != Some(target) {
                batch.height_drag(view, handle.key, target);
                handle.published = Some(target);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use exact_runner::{DataError, Value as DataValue};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn final_height_action_must_not_discard_unrelated_pending_lowering() {
        let source = r#"component App
  state count = 0
  action snap(height: number, velocity: number) writes count
    count = count + 1
  view
    column id="sheet" height=640 box-sizing="border-box"
      column testId="handle" heightDragFor="sheet" heightrelease=snap
      text `${count}` testId="other" transition="scale spring(180, 12, 1)"
"#;
        crate::link::link_for_tests();
        let (mut host, _) = Host::boot(
            &contract::compile(source).unwrap().encode(),
            NoData,
            Default::default(),
            "/",
        )
        .unwrap();
        let handle = host.runner.kernel().find_by_test_id("handle")[0];
        let view = host.runner.kernel().node_by_key(handle).unwrap().id;
        let other = host
            .runner
            .kernel()
            .node_by_key(host.runner.kernel().find_by_test_id("other")[0])
            .unwrap()
            .id;
        let (height, _) = host
            .begin_height_drag(handle, Value::scalar(400.0), 0.0)
            .unwrap()
            .unwrap();
        let (scale, _) = host
            .begin_hold(other, Property::Scale, Value::scalar(2.0), 0.0)
            .unwrap()
            .unwrap();
        // Manufacture a pending release at the Springs seam, before lowering.
        // Calling Host::update_hold here and throwing away its batch would
        // consume this Scale start, permanently hiding it from the page.
        assert!(host
            .springs
            .end_hold(scale.token.serial(), HoldEnd::Cancel, 0.001)
            .unwrap());
        let batch = host
            .dispatch_height_held(height.token.serial(), view, 350.0, 0.0, 2.0)
            .unwrap()
            .unwrap();
        assert!(batch.contains("\"property\":\"scale\""), "{batch}");
        assert!(batch.contains("\"values\":[2,"), "{batch}");
        assert!(host.has_hold(height.token.serial()));
    }
}
