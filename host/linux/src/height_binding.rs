//! Authored handle intent, bounded by mounted handles rather than numeric boxes.
use super::*;
use exact_motion::{HoldEnd, HoldStart, HoldToken, Value};
use exact_plan::EventKind;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Bindings {
    // Live release declarations, including temporarily absent/empty IDREFs.
    handles: BTreeSet<NodeKey>,
    #[cfg(test)]
    declaration_scans: usize,
    pub(super) automatic: bool,
    pub(super) active: Option<Active>,
}
#[derive(Clone, Copy)]
pub(super) struct Active {
    handle: NodeKey,
    target: NodeKey,
    token: HoldToken,
    released: bool,
}

impl<D: DataSource> Host<D> {
    // Listener declarations are immutable for a live view. Discover in one
    // existing Runner bulk-creation walk, never during moves/property updates.
    pub(super) fn discover_height_handles(&mut self) {
        #[cfg(test)]
        {
            self.height_bindings.declaration_scans += 1;
        }
        self.height_bindings.handles = self
            .runner
            .handlers()
            .into_iter()
            .filter(|(_, events)| events.contains(&EventKind::Heightrelease))
            .filter_map(|(view, _)| self.kernel().node(view).map(|n| n.key))
            .collect();
    }

    pub(super) fn forget_height_handle(&mut self, key: NodeKey) {
        self.height_bindings.handles.remove(&key);
    }

    fn resolved_height_target(&self, handle: NodeKey) -> Option<NodeKey> {
        if self.runner.roots().len() != 1 {
            return None;
        }
        let node = self.kernel().node_by_key(handle)?;
        let (hidden, inert) = self.route_visibility(node.id);
        if hidden || inert {
            return None;
        }
        self.kernel().height_drag_target(handle)
    }

    pub(super) fn reconcile_height_bindings(&mut self) {
        let mut first = None;
        let mut referenced = false;
        for handle in &self.height_bindings.handles {
            if let Some(target) = self.resolved_height_target(*handle) {
                first.get_or_insert(target);
                referenced |= Some(target) == self.height_owner;
            }
        }
        if self.height_bindings.automatic && !referenced {
            if let Some(old) = self.height_owner.take() {
                self.engine
                    .remove_property(motion_node(old), Property::Height);
            }
            self.height_bindings.automatic = false;
        }
        if self.height_owner.is_none() {
            self.height_owner = first;
            self.height_bindings.automatic = first.is_some();
        }
    }

    /// Read-only registered binding. Core resolves authored IDs and generations;
    /// handles for a second live target remain refused, never last-wins.
    pub fn height_drag_target(&self, handle: NodeKey) -> Option<NodeKey> {
        if !self.height_bindings.handles.contains(&handle) {
            return None;
        }
        let target = self.resolved_height_target(handle)?;
        (Some(target) == self.height_owner).then_some(target)
    }

    fn height_binding_live(&self, active: Active) -> bool {
        self.has_hold(active.token)
            && active.token.property() == Property::Height
            && active.token.node() == motion_node(active.target)
            && self.height_drag_target(active.handle) == Some(active.target)
    }

    pub(super) fn retire_height_binding(&mut self) {
        if let Some(active) = self.height_bindings.active {
            if !self.height_binding_live(active) {
                self.height_bindings.active = None;
                // Consume only this token; a newer takeover cannot be cancelled.
                let _ = self
                    .engine
                    .end_hold(active.token, self.engine.now(), HoldEnd::Cancel);
            }
        }
    }

    /// Catch a resolved handle/target pair; stale identity refuses before time.
    pub fn height_drag_begin(
        &mut self,
        handle: NodeKey,
        target: NodeKey,
        now_ms: f64,
    ) -> Result<Option<HoldStart>, String> {
        if self.height_drag_target(handle) != Some(target) {
            return Ok(None);
        }
        let view = self
            .kernel()
            .node_by_key(target)
            .expect("resolved live target")
            .id;
        let Some(start) = self.hold_begin(view, Property::Height, now_ms)? else {
            return Ok(None);
        };
        self.height_bindings.active = Some(Active {
            handle,
            target,
            token: start.token,
            released: false,
        });
        Ok(Some(start))
    }

    /// Pointer-only release seam. Generic typed event synthesis is separate.
    /// Final sample and action happen while live and held; the caller ends once.
    pub fn dispatch_height_held(
        &mut self,
        token: HoldToken,
        handle: NodeKey,
        height: f64,
        velocity: f64,
        now_ms: f64,
    ) -> Result<bool, String> {
        let Some(active) = self.height_bindings.active else {
            return Ok(false);
        };
        if active.released
            || active.token != token
            || active.handle != handle
            || !self.height_binding_live(active)
        {
            return Ok(false);
        }
        if !height.is_finite()
            || height < 0.
            || height > f32::MAX as f64
            || !velocity.is_finite()
            || !now_ms.is_finite()
            || now_ms < self.now_ms
        {
            return Err("invalid height release position, velocity, or clock".into());
        }
        if !self.hold_update(token, Value::scalar(height), now_ms)?
            || !self.height_binding_live(active)
        {
            return Ok(false);
        }
        self.height_bindings
            .active
            .as_mut()
            .expect("live binding")
            .released = true;
        let view = self
            .kernel()
            .node_by_key(handle)
            .expect("validated handle")
            .id;
        let used = self
            .kernel()
            .node_by_key(active.target)
            .expect("validated target")
            .frame
            .height as f64;
        self.dispatch_at(
            view,
            Event::HeightRelease {
                height: used,
                velocity,
            },
            now_ms,
        )
        .map_or(Ok(true), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{MonospaceMeasurer, Op, PropId, PropValue};
    use exact_runner::{DataError, Value as DataValue};
    struct Empty;
    impl DataSource for Empty {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }
    #[test]
    fn release_declaration_survives_idref_absent_set_clear_set() {
        let plan = contract::compile(
            r#"component App
  state count = 0
  action release(height: number, velocity: number)
    count = count + 1
  view
    box width=400 height=500
      box id="panel" testId="panel" height=180 box-sizing="border-box"
        box testId="handle" heightrelease=release height=32
"#,
        )
        .unwrap();
        let (mut h, error) = Host::boot(
            &plan.encode(),
            Empty,
            Box::new(MonospaceMeasurer::default()),
            400.,
            500.,
        )
        .unwrap();
        assert!(error.is_none());
        let target = h.kernel().find_by_test_id("panel")[0];
        let key = h.kernel().find_by_test_id("handle")[0];
        let view = h.kernel().node_by_key(key).unwrap().id;
        assert!(h.height_owner().is_none());
        assert!(h.height_drag_target(key).is_none());
        assert_eq!(h.height_bindings.declaration_scans, 1);
        let mut previous = None;
        for value in [Some("panel"), None, Some("panel")] {
            let op = match value {
                Some(value) => Op::SetProp {
                    id: view,
                    prop: PropId::HeightDragFor,
                    value: PropValue::Str(value.into()),
                },
                None => Op::ClearProp {
                    id: view,
                    prop: PropId::HeightDragFor,
                },
            };
            let root = h.roots()[0];
            let receipt = h.runner.kernel_mut().apply(root, 0, &[op]).unwrap();
            assert!(h.commit(&[Timed { at_ms: 0., receipt }], None).is_none());
            assert_eq!(
                h.height_bindings.declaration_scans, 1,
                "IDREF updates must not rescan listener declarations"
            );
            if value.is_some() {
                assert_eq!(h.height_drag_target(key), Some(target));
                previous = Some(h.height_drag_begin(key, target, 0.).unwrap().unwrap().token);
            } else {
                assert!(h.height_owner().is_none());
                assert!(!h.has_hold(previous.take().unwrap()));
            }
        }
        h.hold_end(previous.unwrap(), HoldEnd::Cancel, 0.).unwrap();
    }
}
