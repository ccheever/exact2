//! Header-only vertical policy. The inner list's scrolling is unchanged.
use super::contact::{Candidate, HeldKind, Hold};
use super::*;
use exact_kernel::NodeKey;
use exact_motion::{HoldEnd, Value, VelocityTracker};
impl<D: DataSource> Presenter<D> {
    pub(super) fn height_candidate(&self, hit: NodeKey) -> Option<Candidate> {
        if !self.input_live(hit) {
            return None;
        }
        let mut at = self.host.kernel().node_by_key(hit).map(|n| n.id);
        while let Some(id) = at {
            let node = self.host.kernel().node(id)?;
            if node.props.str(PropId::HeightDragFor).is_some() {
                return self
                    .host
                    .height_drag_target(node.key)
                    .map(|target| Candidate::Height {
                        handle: node.key,
                        target,
                    });
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
    pub(super) fn begin_height_drag(
        &mut self,
        handle: NodeKey,
        target: NodeKey,
        now_ms: f64,
    ) -> Result<Option<Hold>, String> {
        let Some(primary) = self.host.height_drag_begin(handle, target, now_ms)? else {
            return Ok(None);
        };
        let mut velocity = VelocityTracker::new();
        velocity.push(now_ms / 1000., primary.value);
        let view = self
            .host
            .kernel()
            .node_by_key(handle)
            .expect("live handle")
            .id;
        self.set_collection_interaction(Some(view));
        self.dirty = true;
        Ok(Some(Hold {
            primary,
            velocity,
            kind: HeldKind::Height { handle, target },
        }))
    }
    pub(super) fn move_height_drag(
        &mut self,
        held: &mut Hold,
        delta: f64,
        now_ms: f64,
    ) -> Result<bool, String> {
        let HeldKind::Height { target, .. } = held.kind else {
            unreachable!()
        };
        let px = (held.primary.value.x + delta).clamp(0., f32::MAX as f64);
        if !self.height_update(held.primary.token, px, now_ms)? {
            return Ok(false);
        }
        let Some(node) = self.host.kernel().node_by_key(target) else {
            return Ok(false);
        };
        // Record the actual constrained presentation, not finger displacement.
        held.velocity
            .push(now_ms / 1000., Value::scalar(node.frame.height as f64));
        Ok(true)
    }
    pub(super) fn finish_height_drag(&mut self, held: &Hold, now_ms: f64) -> Result<bool, String> {
        let HeldKind::Height { handle, target } = held.kind else {
            unreachable!()
        };
        let velocity = held.velocity.estimate(now_ms / 1000.);
        let height = self
            .host
            .kernel()
            .node_by_key(target)
            .map_or(0., |n| n.frame.height as f64);
        let dispatched =
            self.host
                .dispatch_height_held(held.primary.token, handle, height, velocity.x, now_ms);
        let after = self.after_commit();
        let end = if matches!(dispatched, Ok(true)) {
            HoldEnd::Release { velocity }
        } else {
            HoldEnd::Cancel
        };
        let ended = self.height_end(held.primary.token, end, now_ms);
        match dispatched {
            Err(e) => Err(e),
            Ok(accepted) => after.or(ended.err()).map_or(Ok(accepted), Err),
        }
    }
}
