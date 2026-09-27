//! Primary pan + authored zoom controls; no pinch or wheel-zoom inference:
//! Linux declares pinch absent (LLP 1057.001 §4; evdev tracks no multitouch).
use super::contact::{Candidate, HeldKind, Hold};
use super::*;
use exact_kernel::{NodeKey, TransformDragBinding};
use exact_motion::{TransformHold, Value, VelocityTracker};

impl<D: DataSource> Presenter<D> {
    pub(super) fn transform_candidate(&self, hit: NodeKey) -> Option<Candidate> {
        if !self.input_live(hit) {
            return None;
        }
        let mut at = self.host.kernel().node_by_key(hit).map(|n| n.id);
        while let Some(id) = at {
            let node = self.host.kernel().node(id)?;
            if node.props.str(PropId::TransformDragFor).is_some() {
                let binding = self.host.transform_drag_binding(node.key)?;
                self.transform_revision(binding)?;
                return Some(Candidate::Transform(binding));
            }
            if node.node_type == NodeType::TextInput
                || self
                    .host
                    .runner()
                    .handlers_of(id)
                    .contains(&EventKind::Press)
            {
                return None;
            }
            at = node.parent;
        }
        None
    }
    pub(super) fn begin_transform_drag(
        &mut self,
        binding: TransformDragBinding,
        now_ms: f64,
    ) -> Result<Option<Hold>, String> {
        let Some(revision) = self.transform_revision(binding) else {
            return Ok(None);
        };
        let Some(pair) = self.host.transform_drag_begin(binding, now_ms)? else {
            return Ok(None);
        };
        let primary = pair.translate();
        let mut velocity = VelocityTracker::new();
        velocity.push(now_ms / 1000., primary.value);
        let view = self
            .host
            .kernel()
            .node_by_key(binding.handle)
            .expect("live handle")
            .id;
        self.set_collection_interaction(Some(view));
        self.dirty = true;
        Ok(Some(Hold {
            primary,
            velocity,
            kind: HeldKind::Transform {
                binding,
                pair,
                revision,
            },
        }))
    }
    pub(super) fn move_transform_drag(
        &mut self,
        held: &mut Hold,
        delta: Value,
        now_ms: f64,
    ) -> Result<bool, String> {
        let HeldKind::Transform {
            binding,
            pair,
            revision,
        } = held.kind
        else {
            unreachable!()
        };
        if self.transform_revision(binding) != Some(revision) {
            return Ok(false);
        }
        let value = Value {
            x: pair.translate().value.x + delta.x,
            y: pair.translate().value.y + delta.y,
            ..Value::ZERO
        };
        let accepted =
            self.host
                .transform_drag_update(pair, binding, [value, pair.scale().value], now_ms)?;
        if accepted {
            held.velocity.push(now_ms / 1000., value);
            self.dirty = true;
        }
        Ok(accepted)
    }
    pub(super) fn finish_transform_drag(
        &mut self,
        held: &Hold,
        now_ms: f64,
    ) -> Result<bool, String> {
        let HeldKind::Transform {
            binding,
            pair,
            revision,
        } = held.kind
        else {
            unreachable!()
        };
        if self.transform_revision(binding) != Some(revision)
            || !self.host.transform_hold_live(pair, binding)
        {
            self.end_transform_drag(pair, None, now_ms)?;
            return Ok(false);
        }
        let values = [
            self.host
                .engine()
                .value(
                    pair.translate().token.node(),
                    exact_motion::Property::Translate,
                )
                .expect("live translate"),
            self.host
                .engine()
                .value(pair.scale().token.node(), exact_motion::Property::Scale)
                .expect("live scale"),
        ];
        let velocities = [held.velocity.estimate(now_ms / 1000.), Value::ZERO];
        let dispatched = self
            .host
            .dispatch_transform_held(pair, binding, values, velocities, now_ms);
        let after = self.after_commit();
        let end = self.end_transform_drag(
            pair,
            matches!(dispatched, Ok(true)).then_some(velocities),
            now_ms,
        );
        match dispatched {
            Err(e) => Err(e),
            Ok(accepted) => after.or(end.err()).map_or(Ok(accepted), Err),
        }
    }
    pub(super) fn end_transform_drag(
        &mut self,
        pair: TransformHold,
        velocities: Option<[Value; 2]>,
        now_ms: f64,
    ) -> Result<(), String> {
        let result = self.host.transform_drag_end(pair, velocities, now_ms);
        self.dirty = true;
        result
    }
}
