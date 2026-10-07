//! One authored transform owner; native tokens stay typed and bounded.
use super::*;
use exact_kernel::TransformDragBinding;
use exact_motion::{HoldEnd, TransformHold, Value};
use exact_plan::EventKind;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Bindings {
    handles: BTreeSet<NodeKey>,
    owner: Option<NodeKey>,
    active: Option<Active>,
}
#[derive(Clone, Copy)]
struct Active {
    binding: TransformDragBinding,
    held: TransformHold,
    released: bool,
}

fn valid_values(values: [Value; 2]) -> bool {
    values
        .into_iter()
        .all(|v| v.x.is_finite() && v.y.is_finite())
        && values[0].x.abs() <= f32::MAX as f64
        && values[0].y.abs() <= f32::MAX as f64
        && values[1].y == 0.
        && values[1].x > 0.
        && values[1].x <= f32::MAX as f64
        && (values[1].x as f32) > 0.
}
fn valid_velocities(values: [Value; 2]) -> bool {
    values
        .into_iter()
        .all(|v| v.x.is_finite() && v.y.is_finite())
        && values[1].y == 0.
}
impl<D: DataSource> Host<D> {
    pub(super) fn discover_transform_handles(&mut self) {
        self.transform_bindings.handles = self
            .runner
            .handlers()
            .into_iter()
            .filter(|(_, events)| {
                events.contains(&EventKind::Transformrelease)
                    && events.contains(&EventKind::Transformgeometry)
            })
            .filter_map(|(view, _)| self.kernel().node(view).map(|n| n.key))
            .collect();
    }
    pub(super) fn forget_transform_handle(&mut self, key: NodeKey) {
        self.transform_bindings.handles.remove(&key);
    }
    fn resolved_transform(&self, handle: NodeKey) -> Option<TransformDragBinding> {
        if self.roots().len() != 1 {
            return None;
        }
        let node = self.kernel().node_by_key(handle)?;
        let (hidden, inert) = self.route_visibility(node.id);
        if hidden || inert {
            return None;
        }
        self.kernel().transform_drag_binding(handle)
    }
    pub(super) fn reconcile_transform_bindings(&mut self) {
        let mut first = None;
        let mut referenced = false;
        for &handle in &self.transform_bindings.handles {
            if let Some(binding) = self.resolved_transform(handle) {
                first.get_or_insert(binding.target);
                referenced |= Some(binding.target) == self.transform_bindings.owner;
            }
        }
        if !referenced {
            self.transform_bindings.owner = first;
        }
    }
    /// Source-valid handles for the sole target; geometry remains Presenter policy.
    pub fn transform_bindings(&self) -> Vec<TransformDragBinding> {
        self.transform_bindings
            .handles
            .iter()
            .filter_map(|h| self.transform_drag_binding(*h))
            .collect()
    }
    /// Registered source binding; a second target cannot replace a live owner.
    pub fn transform_drag_binding(&self, handle: NodeKey) -> Option<TransformDragBinding> {
        if !self.transform_bindings.handles.contains(&handle) {
            return None;
        }
        let binding = self.resolved_transform(handle)?;
        (Some(binding.target) == self.transform_bindings.owner).then_some(binding)
    }
    /// Current non-target presentation must leave the parent coordinate system intact.
    pub fn transform_path_ready(&self, binding: TransformDragBinding) -> bool {
        if self.transform_drag_binding(binding.handle) != Some(binding) {
            return false;
        }
        let mut at = self.kernel().node_by_key(binding.handle).map(|n| n.id);
        while let Some(id) = at {
            let Some(n) = self.kernel().node(id) else {
                return false;
            };
            let p = self.presented(id);
            let node = motion_node(n.key);
            if p.rotate != 0. || self.engine.is_active(node, Property::Rotate) {
                return false;
            }
            if n.key != binding.target
                && ((p.translate != (0., 0.) || p.translate_percent != (0., 0.))
                    || p.scale != 1.
                    || self.engine.is_active(node, Property::Translate)
                    || self.engine.is_active(node, Property::Scale))
            {
                return false;
            }
            at = n.parent;
        }
        true
    }
    /// Whether both original tokens still own this exact live binding.
    pub fn transform_hold_live(&self, held: TransformHold, binding: TransformDragBinding) -> bool {
        let Some(active) = self.transform_bindings.active else {
            return false;
        };
        active.held == held
            && active.binding == binding
            && self.transform_path_ready(binding)
            && self.has_hold(held.translate().token)
            && self.has_hold(held.scale().token)
    }
    pub(super) fn retire_transform_binding(&mut self) {
        if let Some(active) = self.transform_bindings.active {
            if !self.transform_hold_live(active.held, active.binding) {
                self.transform_bindings.active = None;
                for start in [active.held.translate(), active.held.scale()] {
                    let _ = self
                        .engine
                        .end_hold(start.token, self.engine.now(), HoldEnd::Cancel);
                }
            }
        }
    }
    /// Catch published presentation atomically, after Presenter geometry admission.
    pub fn transform_drag_begin(
        &mut self,
        binding: TransformDragBinding,
        now_ms: f64,
    ) -> Result<Option<TransformHold>, String> {
        if !self.transform_path_ready(binding) {
            return Ok(None);
        }
        let node = motion_node(binding.target);
        if [Property::Translate, Property::Scale]
            .into_iter()
            .any(|p| self.engine.value(node, p).is_none())
        {
            return Ok(None);
        }
        let view = self
            .kernel()
            .node_by_key(binding.target)
            .expect("live target")
            .id;
        let p = self.presented(view);
        let values = [
            Value {
                x: p.translate.0 as f64,
                y: p.translate.1 as f64,
                ..Value::ZERO
            },
            Value::scalar(p.scale as f64),
        ];
        if !valid_values(values) || !now_ms.is_finite() || now_ms < self.now_ms {
            return Err("invalid transform presentation or clock".into());
        }
        let held = self
            .engine
            .begin_transform_hold(node, now_ms / 1000., Some(values))
            .map_err(|e| format!("transform begin: {e:?}"))?;
        if let Some(held) = held {
            self.transform_bindings.active = Some(Active {
                binding,
                held,
                released: false,
            });
            self.now_ms = now_ms;
            if let Err(error) = self.present_hold() {
                self.transform_bindings.active = None;
                for start in [held.translate(), held.scale()] {
                    let _ = self
                        .engine
                        .end_hold(start.token, self.engine.now(), HoldEnd::Cancel);
                }
                self.present();
                return Err(error);
            }
        }
        Ok(held)
    }
    /// Absolute parent-space presentation, preserving both authored targets.
    pub fn transform_drag_update(
        &mut self,
        held: TransformHold,
        binding: TransformDragBinding,
        values: [Value; 2],
        now_ms: f64,
    ) -> Result<bool, String> {
        if !self.transform_hold_live(held, binding) {
            return Ok(false);
        }
        if !valid_values(values) || !now_ms.is_finite() || now_ms < self.now_ms {
            return Err("invalid transform position or clock".into());
        }
        let accepted = self
            .engine
            .update_transform_hold(held, now_ms / 1000., values)
            .map_err(|e| format!("transform update: {e:?}"))?;
        if accepted {
            self.now_ms = now_ms;
            self.present_hold()?;
        }
        Ok(accepted)
    }
    /// Validate the complete final tuple, update both, dispatch while both held.
    pub fn dispatch_transform_held(
        &mut self,
        held: TransformHold,
        binding: TransformDragBinding,
        values: [Value; 2],
        velocities: [Value; 2],
        now_ms: f64,
    ) -> Result<bool, String> {
        if !self.transform_hold_live(held, binding)
            || self.transform_bindings.active.is_some_and(|a| a.released)
        {
            return Ok(false);
        }
        if !valid_values(values)
            || !valid_velocities(velocities)
            || !now_ms.is_finite()
            || now_ms < self.now_ms
        {
            return Err("invalid transform release tuple or clock".into());
        }
        if !self.transform_drag_update(held, binding, values, now_ms)? {
            return Ok(false);
        }
        self.transform_bindings
            .active
            .as_mut()
            .expect("live pair")
            .released = true;
        let view = self
            .kernel()
            .node_by_key(binding.handle)
            .expect("live handle")
            .id;
        self.dispatch_at(
            view,
            Event::TransformRelease {
                x: values[0].x,
                y: values[0].y,
                scale: values[1].x,
                vx: velocities[0].x,
                vy: velocities[0].y,
                vscale: velocities[1].x,
            },
            now_ms,
        )
        .map_or(Ok(true), Err)
    }
    /// End each surviving ORIGINAL token, never a successor of either property.
    pub fn transform_drag_end(
        &mut self,
        held: TransformHold,
        velocities: Option<[Value; 2]>,
        now_ms: f64,
    ) -> Result<(), String> {
        let starts = [held.translate(), held.scale()];
        if !starts.into_iter().any(|s| self.has_hold(s.token)) {
            return Ok(());
        }
        if velocities.is_some_and(|v| !valid_velocities(v))
            || !now_ms.is_finite()
            || now_ms < self.now_ms
            || now_ms / 1000. < self.engine.now()
        {
            return Err("invalid transform terminal velocity or clock".into());
        }
        for (i, start) in starts.into_iter().enumerate() {
            self.engine
                .end_hold(
                    start.token,
                    now_ms / 1000.,
                    velocities.map_or(HoldEnd::Cancel, |v| HoldEnd::Release { velocity: v[i] }),
                )
                .map_err(|e| format!("transform end: {e:?}"))?;
        }
        if self
            .transform_bindings
            .active
            .is_some_and(|a| a.held == held)
        {
            self.transform_bindings.active = None;
        }
        self.now_ms = now_ms;
        self.present_hold()
    }
}
