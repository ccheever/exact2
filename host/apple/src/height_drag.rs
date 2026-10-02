//! Authored headers resolve through the kernel; one target owns presentation.
use super::*;
use exact_motion::{HoldEnd, Value};

#[derive(Clone, Copy)]
pub(super) struct HeightHandle {
    id: ViewId,
    target: Option<NodeKey>,
    sent: bool,
}
#[derive(Clone, Copy)]
pub(super) struct HeightDrag {
    handle: NodeKey,
    target: NodeKey,
    token: HoldToken,
    released: bool,
}

impl<D: DataSource> Host<D> {
    pub(super) fn track_height_handle(&mut self, id: ViewId) {
        let Some(node) = self.runner.kernel().node(id) else {
            return;
        };
        // Called from creation's existing handler traversal. Keep known release
        // handlers even while their IDREF is absent, so later prop updates work.
        self.height_handles.entry(node.key).or_insert(HeightHandle {
            id,
            target: None,
            sent: false,
        });
    }

    // Called only after boot/tree commits, never by ordinary compositor ticks.
    pub(super) fn reconcile_height_handles(&mut self, batch: &mut Batch, auto_register: bool) {
        let kernel = self.runner.kernel();
        let single_root = kernel.roots().len() == 1;
        let candidates: Vec<_> = self
            .height_handles
            .iter()
            .map(|(&key, entry)| {
                let target = single_root
                    .then(|| kernel.height_drag_target(key))
                    .flatten();
                (key, entry.id, target)
            })
            .collect();
        if self.height_auto_owned
            && !candidates
                .iter()
                .any(|(_, _, target)| target.is_some() && *target == self.height_owner)
        {
            if let Some(old) = self.height_owner.take() {
                self.engine
                    .remove_property(motion_node(old), Property::Height);
            }
            self.height_auto_owned = false;
        }
        if auto_register && self.height_owner.is_none() {
            if let Some(target) = candidates.iter().find_map(|(_, _, target)| *target) {
                self.height_owner = Some(target);
                self.height_auto_owned = true;
            }
        }
        for (handle, id, resolved) in candidates {
            let target = resolved.filter(|target| Some(*target) == self.height_owner);
            let entry = self
                .height_handles
                .get_mut(&handle)
                .expect("tracked handle");
            if !entry.sent || entry.target != target {
                let target_id =
                    target.and_then(|key| self.runner.kernel().node_by_key(key).map(|n| n.id));
                batch.height_drag(
                    id,
                    motion_node(handle),
                    target_id.zip(target.map(motion_node)),
                );
                entry.target = target;
                entry.sent = true;
            }
        }
        self.height_handles
            .retain(|key, _| self.runner.kernel().node_by_key(*key).is_some());
    }

    fn height_binding_matches(&self, handle: NodeKey, target: NodeKey) -> bool {
        self.height_owner == Some(target)
            && self.runner.kernel().roots().len() == 1
            && self
                .height_handles
                .get(&handle)
                .is_some_and(|entry| entry.target == Some(target))
            && self.runner.kernel().height_drag_target(handle) == Some(target)
    }
    fn live_height_drag(&self, serial: u64) -> Option<HeightDrag> {
        self.height_drag.filter(|drag| {
            drag.token.serial() == serial
                && self.engine.has_hold(drag.token)
                && self.height_binding_matches(drag.handle, drag.target)
        })
    }
    pub(super) fn cancel_invalid_height_drag(&mut self) {
        let Some(drag) = self.height_drag else { return };
        if self.live_height_drag(drag.token.serial()).is_none() {
            self.height_drag = None;
            // A hidden handle may leave its otherwise eligible target alive.
            // Retire ownership separately from input eligibility.
            let _ = self
                .engine
                .end_hold(drag.token, self.engine.now(), HoldEnd::Cancel);
        }
    }

    /// Begin only the exported generational binding, before advancing any clock.
    pub fn height_drag_begin(&mut self, handle: NodeKey, target: NodeKey, now_ms: f64) -> String {
        if !self.height_binding_matches(handle, target) {
            return self.finish(Batch::new(), None);
        }
        let Some(value) = self.height_catch(target) else {
            return self.finish(Batch::new(), None);
        };
        let mut batch = Batch::new();
        let error = match self.engine.begin_hold(
            motion_node(target),
            Property::Height,
            now_ms / 1000.,
            Some(value),
        ) {
            Ok(Some(start)) => {
                self.now_ms = now_ms;
                self.holds.insert(start.token.serial(), start.token);
                self.height_drag = Some(HeightDrag {
                    handle,
                    target,
                    token: start.token,
                    released: false,
                });
                batch.hold(start.token.serial(), start.value.x, start.value.y);
                self.height_layout_if_needed(&mut batch).err()
            }
            Ok(None) => None,
            Err(error) => Some(format!("height drag: {error:?}")),
        };
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// Update only a live header/target/token triple; stale values are ignored.
    pub fn height_drag_update(&mut self, serial: u64, height: f64, now_ms: f64) -> String {
        let Some(drag) = self.live_height_drag(serial).filter(|drag| !drag.released) else {
            return self.finish(Batch::new(), None);
        };
        let out = self.hold_update(serial, Value::scalar(height), now_ms);
        // An accepted sample: record the constrained height shown, which the
        // release velocity follows (LLP 1057.001 §3).
        if self.engine.now() == now_ms / 1000. {
            if let Some(shown) = self.height_catch(drag.target) {
                self.engine.track_hold(drag.token, now_ms / 1000., shown);
            }
        }
        out
    }

    /// Apply the final sample then the typed action while the token still owns Height.
    /// The caller ends once afterward, even if the action destroyed its target.
    pub fn dispatch_height_held(
        &mut self,
        serial: u64,
        height: f64,
        velocity: f64,
        now_ms: f64,
    ) -> String {
        self.dispatch_height(serial, height, Some(velocity), now_ms)
    }

    /// The same, at the velocity the engine measured over the heights shown
    /// (LLP 1057.001 §3): what the native bridge sends.
    pub fn dispatch_height_measured(&mut self, serial: u64, height: f64, now_ms: f64) -> String {
        self.dispatch_height(serial, height, None, now_ms)
    }

    fn dispatch_height(
        &mut self,
        serial: u64,
        height: f64,
        velocity: Option<f64>,
        now_ms: f64,
    ) -> String {
        let Some(drag) = self.live_height_drag(serial).filter(|drag| !drag.released) else {
            return self.finish(Batch::new(), None);
        };
        if !height.is_finite()
            || !(0. ..=f32::MAX as f64).contains(&height)
            || velocity.is_some_and(|v| !v.is_finite())
        {
            return self.hold_refusal("invalid height release coordinates");
        }
        // Do not dispatch after a rejected clock/sample or failed layout.
        // Compose the final sample and action in one native batch.
        let mut batch = Batch::new();
        if let Err(error) =
            self.engine
                .update_hold(drag.token, now_ms / 1000., Value::scalar(height))
        {
            return self.hold_refusal(&format!("height drag: {error:?}"));
        }
        self.now_ms = now_ms;
        if let Err(error) = self.height_layout_if_needed(&mut batch) {
            return self.finish(batch, Some(error));
        }
        // Action receives actual constrained CSS height, not a min/max-clipped input.
        let Some(height) = self.height_catch(drag.target).map(|v| v.x) else {
            return self.finish(batch, None);
        };
        // The shown (constrained) height is what the release velocity follows.
        self.engine
            .track_hold(drag.token, now_ms / 1000., Value::scalar(height));
        let velocity = velocity.unwrap_or_else(|| {
            self.engine
                .hold_velocity(drag.token, now_ms / 1000.)
                .map_or(0., |v| v.x)
        });
        let Some(view) = self
            .runner
            .kernel()
            .node_by_key(drag.handle)
            .map(|node| node.id)
        else {
            return self.finish(batch, None);
        };
        if let Some(active) = self.height_drag.as_mut() {
            active.released = true;
        }
        // Keep the pre-action frame operations: the commit mirror has already
        // observed them, so they cannot be reconstructed from the action batch.
        match self
            .runner
            .dispatch(view, Event::HeightRelease { height, velocity })
        {
            Ok(receipt) => self.commit_into(
                &[Timed {
                    at_ms: now_ms,
                    receipt,
                }],
                None,
                batch,
            ),
            Err(error) => self.commit_into(&[], Some(format!("{error:?}")), batch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{MonospaceMeasurer, Op};
    use exact_runner::{DataError, Value as DataValue};

    struct Empty;
    impl DataSource for Empty {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn known_release_handler_survives_absent_set_clear_set_prop_receipts() {
        let plan = contract::compile(
            r#"component App
  state count = 0
  action release(height: number, velocity: number)
    count = count + 1
  view
    box id="sheet" testId="sheet" height=200 box-sizing="border-box"
      box testId="header" heightrelease=release
"#,
        )
        .unwrap()
        .encode();
        let (mut h, _) = Host::boot(
            &plan,
            Empty,
            Box::new(MonospaceMeasurer::default()),
            800.,
            900.,
        )
        .unwrap();
        let header = h.runner.kernel().find_by_test_id("header")[0];
        let target = h.runner.kernel().find_by_test_id("sheet")[0];
        let id = h.runner.kernel().node_by_key(header).unwrap().id;
        assert_eq!(h.height_owner, None);
        assert_eq!(h.height_handles.len(), 1);
        for enabled in [true, false, true] {
            let op = if enabled {
                Op::SetProp {
                    id,
                    prop: PropId::HeightDragFor,
                    value: PropValue::Str("sheet".into()),
                }
            } else {
                Op::ClearProp {
                    id,
                    prop: PropId::HeightDragFor,
                }
            };
            let receipt = h.runner.kernel_mut().apply(0, 99, &[op]).unwrap();
            let batch = h.commit(&[Timed { at_ms: 0., receipt }], None);
            assert!(batch.contains("\"error\":null"), "{batch}");
            assert_eq!(h.height_owner, enabled.then_some(target));
            assert_eq!(h.height_handles.len(), 1);
            let start = h.height_drag_begin(header, target, 0.);
            assert_eq!(start.contains("\"token\""), enabled, "{start}");
        }
    }
}
