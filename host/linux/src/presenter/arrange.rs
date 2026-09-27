//! One primary Arrange contact and its terminal settling lease; no item graph.
use super::arrange_geometry::{finite, Mapping};
use super::contact::{Candidate, HeldKind, Hold};
use super::*;
use exact_kernel::{motion::motion_node, NodeKey};
use exact_motion::{HoldEnd, HoldToken, Property, Value, VelocityTracker};
use exact_runner::{ReorderBinding, ReorderToken};

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Active,
    Committing,
    Settling,
}

pub(super) struct State {
    binding: ReorderBinding,
    token: ReorderToken,
    hold: HoldToken,
    pin: ViewId,
    pin_owned: bool,
    mapping: Mapping,
    scroll: f32,
    phase: Phase,
    // Last actual pointer point, not an app resource or a destination pin.
    point: (f32, f32),
    edge_clock: f64,
}
impl<D: DataSource> Presenter<D> {
    pub(super) fn arrange_candidate(&self, hit: NodeKey) -> Option<Candidate> {
        if !self.input_live(hit) {
            return None;
        }
        let mut at = Some(hit);
        while let Some(key) = at {
            let node = self.host.kernel().node_by_key(key)?;
            if node.props.str(PropId::ReorderFor).is_some() {
                let binding = self.host.runner().reorder_binding(key)?;
                self.arrange_mapping(binding)?;
                return Some(Candidate::Arrange(binding));
            }
            if node.node_type == NodeType::TextInput
                || self
                    .host
                    .runner()
                    .handlers_of(node.id)
                    .contains(&EventKind::Press)
            {
                return None;
            }
            at = node
                .parent
                .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
        }
        None
    }
    pub(super) fn arrange_live(&self, token: ReorderToken) -> bool {
        self.arrange.as_ref().is_some_and(|s| {
            s.token == token
                && s.phase == Phase::Active
                && self.host.has_hold(s.hold)
                && self.host.runner().has_reorder(token)
                && self
                    .host
                    .runner()
                    .reorder_geometry(s.binding.list)
                    .is_some_and(|g| self.reorder_facts_current(&g))
                && self.host.runner().reorder_binding(s.binding.handle) == Some(s.binding)
                && self.input_live(s.binding.handle)
                && self.arrange_mapping(s.binding).as_ref() == Some(&s.mapping)
        })
    }
    // Feedback must retain the common source-key lease even when its old grip died.
    pub(super) fn arrange_pin(&self) -> Option<(ViewId, ViewId)> {
        let s = self.arrange.as_ref()?;
        if !s.pin_owned {
            return None;
        }
        self.host.runner().reorder_frame(s.token)?;
        self.host.kernel().node_by_key(s.binding.wrapper)?;
        Some((s.pin, self.host.kernel().node_by_key(s.binding.list)?.id))
    }
    pub(super) fn arrange_pin_transfer(&mut self, view: Option<ViewId>) {
        if let Some(s) = self.arrange.as_mut() {
            if view != Some(s.pin) {
                s.pin_owned = false;
            }
        }
    }
    pub(super) fn prepare_arrange_down(
        &mut self,
        candidate: Option<Candidate>,
    ) -> Result<(), String> {
        let Some(old) = self.arrange.as_ref() else {
            return Ok(());
        };
        // Transfer the SAME interaction slot before finish: an offscreen source
        // must not unmount between old terminal cleanup and its new catch.
        let keep =
            matches!(candidate,Some(Candidate::Arrange(b)) if b.wrapper == old.binding.wrapper);
        if keep {
            if let Some(id) = self
                .host
                .kernel()
                .node_by_key(old.binding.wrapper)
                .map(|n| n.id)
            {
                self.set_collection_interaction(Some(id));
                if let Some(e) = self.refine_collections() {
                    return Err(e);
                }
            }
        }
        self.finish_arrange_lease(keep)
    }
    pub(super) fn begin_arrange(
        &mut self,
        binding: ReorderBinding,
        now: f64,
    ) -> Result<Option<Hold>, String> {
        if self.arrange.is_some() {
            return Ok(None);
        }
        let Some(pin) = self.host.kernel().node_by_key(binding.handle).map(|n| n.id) else {
            return Ok(None);
        };
        self.set_collection_interaction(Some(pin));
        if let Some(e) = self.refine_collections() {
            return Err(e);
        }
        let Some(mapping) = self.arrange_mapping(binding) else {
            return Ok(None);
        };
        let Some(geometry) = self.host.runner().reorder_geometry(binding.list) else {
            return Ok(None);
        };
        if !self.reorder_facts_current(&geometry) {
            return Ok(None);
        }
        let Some((token, primary)) = self.host.arrange_begin(binding, geometry, now)? else {
            return Ok(None);
        };
        let list = self.host.kernel().node_by_key(binding.list).unwrap().id;
        self.arrange = Some(State {
            binding,
            token,
            hold: primary.token,
            pin,
            pin_owned: true,
            mapping,
            scroll: self.scroll_of(list).1,
            phase: Phase::Active,
            point: (0., 0.),
            edge_clock: now,
        });
        self.brush.arrange_lift = Some((binding.list, binding.wrapper));
        let mut velocity = VelocityTracker::new();
        let base = self.arrange_base(binding.wrapper).unwrap();
        velocity.push(
            now / 1000.,
            Value {
                x: base.x + primary.value.x,
                y: base.y + primary.value.y,
                ..Value::ZERO
            },
        );
        self.dirty = true;
        Ok(Some(Hold {
            primary,
            velocity,
            kind: HeldKind::Arrange(token),
        }))
    }
    pub(super) fn move_arrange(
        &mut self,
        held: &mut Hold,
        dy: f64,
        point: (f32, f32),
        now: f64,
    ) -> Result<bool, String> {
        let HeldKind::Arrange(token) = held.kind else {
            unreachable!()
        };
        if !self.arrange_live(token) {
            return Ok(false);
        }
        let s = self.arrange.as_ref().unwrap();
        let b = s.binding;
        let list = self.host.kernel().node_by_key(b.list).unwrap().id;
        let value = Value {
            x: held.primary.value.x,
            y: held.primary.value.y + dy + (self.scroll_of(list).1 - s.scroll) as f64,
            ..Value::ZERO
        };
        if !finite(value) {
            return Err("Arrange position outside finite layout range".into());
        }
        self.host.arrange_clock(now)?;
        if !self.host.hold_update(held.primary.token, value, now)? {
            return Ok(false);
        }
        // A hold seek may advance a concurrent Height curve. Refine actual layout
        // before certifying this point, and never discard the other dirty properties.
        self.clamp_scroll();
        self.queue_collections();
        if let Some(e) = self.refine_collections() {
            return Err(e);
        }
        if !self.arrange_live(token) {
            return Ok(false);
        }
        let base = self.arrange_base(b.wrapper).unwrap();
        let visual = Value {
            x: base.x + value.x,
            y: base.y + value.y,
            ..Value::ZERO
        };
        held.velocity.push(now / 1000., visual);
        let s = self.arrange.as_mut().unwrap();
        s.point = point;
        s.edge_clock = now;
        let port = s.mapping.port;
        let clip = s.mapping.clip;
        let g = self.host.runner().reorder_geometry(b.list).unwrap();
        let height = self
            .host
            .kernel()
            .node_by_key(b.wrapper)
            .unwrap()
            .frame
            .height as f64;
        // An offport final point must invalidate the old certification even when
        // the source's center happens to remain visible after a distant catch.
        let inside = point.0 >= clip.0
            && point.0 <= clip.0 + clip.2
            && point.1 >= clip.1
            && point.1 <= clip.1 + clip.3;
        let y = if inside {
            visual.y + height / 2. - port.1 as f64 + g.scroll_top
        } else {
            g.scroll_top + g.port_height + 1.
        };
        let accepted = self.host.arrange_preview(token, g, y)?;
        self.dirty = true;
        Ok(accepted)
    }
    pub(super) fn end_arrange(
        &mut self,
        held: &Hold,
        drop: bool,
        now: f64,
    ) -> Result<bool, String> {
        let HeldKind::Arrange(token) = held.kind else {
            unreachable!()
        };
        if self.arrange.as_ref().is_none_or(|s| s.token != token) {
            return Ok(false);
        }
        let live = self.arrange_live(token);
        // Stale cleanup has no new clock. Accepted receipt paths already sought
        // the latest declaration at receipt time before retire_pointer calls us.
        let now = if live {
            self.host.arrange_clock(now)?;
            now
        } else {
            self.host.now()
        };
        let velocity = if drop && live {
            held.velocity.estimate(now / 1000.)
        } else {
            Value::ZERO
        };
        if !velocity.x.is_finite() || !velocity.y.is_finite() {
            return Err("invalid Arrange velocity".into());
        }
        let captures = self
            .host
            .runner()
            .reorder_frame(token)
            .map(|f| f.wrappers)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|w| {
                let base = self.arrange_base(w.wrapper)?;
                let p = self
                    .host
                    .engine()
                    .value(motion_node(w.wrapper), Property::Translate)?;
                let value = Value {
                    x: base.x + p.x,
                    y: base.y + p.y,
                    ..Value::ZERO
                };
                Some((w.wrapper, value))
            })
            .collect::<Vec<_>>();
        if captures.iter().any(|(_, v)| !finite(*v)) {
            return Err("invalid Arrange capture".into());
        }
        self.arrange.as_mut().unwrap().phase = Phase::Committing;
        let mut error = None;
        let accepted = if drop && live {
            let g = self.host.runner().reorder_geometry(token.list()).unwrap();
            match self.host.arrange_drop(token, held.primary.token, g) {
                Ok(v) => v,
                Err(e) => {
                    error = Some(e);
                    false
                }
            }
        } else {
            false
        };
        if !accepted {
            error = error.or(self.host.arrange_cancel(token).err());
        }
        error = error.or(self.after_commit());
        // The action and feedback have completed. Rebase only surviving exact
        // generations; deletion/replacement cannot attach motion to a new row.
        let source = self.arrange.as_ref().unwrap().binding.wrapper;
        for (key, visual) in captures {
            let Some(base) = self.arrange_base(key) else {
                continue;
            };
            let value = Value {
                x: visual.x - base.x,
                y: visual.y - base.y,
                ..Value::ZERO
            };
            if !finite(value) {
                error = error.or(Some("invalid Arrange rebase".into()));
                continue;
            }
            if key == source {
                if self.host.has_hold(held.primary.token) {
                    error = error.or(self.host.hold_update(held.primary.token, value, now).err());
                }
            } else {
                error = error.or(self.host.arrange_rebase_neighbor(key, value, now).err());
            }
        }
        error = error.or(self
            .host
            .hold_end(held.primary.token, HoldEnd::Release { velocity }, now)
            .err());
        self.dirty = true;
        self.arrange.as_mut().unwrap().phase = Phase::Settling;
        self.arrange_settled();
        error.map_or(Ok(accepted), Err)
    }
    fn finish_arrange_lease(&mut self, keep: bool) -> Result<(), String> {
        let Some(s) = self.arrange.take() else {
            return Ok(());
        };
        self.brush.arrange_lift = None;
        let result = self.host.arrange_finish(s.token);
        if !keep && s.pin_owned && self.collection_interaction() == Some(s.pin) {
            self.set_collection_interaction(None);
        }
        self.queue_collections();
        self.dirty = true;
        result
    }
    pub(super) fn arrange_settled(&mut self) {
        let finished = self.arrange.as_ref().is_some_and(|s| {
            s.phase == Phase::Settling
                && (!self
                    .host
                    .engine()
                    .is_active(motion_node(s.binding.wrapper), Property::Translate)
                    || self.host.runner().reorder_frame(s.token).is_none())
        });
        if finished {
            if let Err(e) = self.finish_arrange_lease(false) {
                self.host.log(e);
            }
        }
    }
    /// Existing display pump demand: held-only Arrange is quiescent except when
    /// the pointer's edge zone can advance this actual clamped scrollport.
    pub fn needs_animation_frame(&self) -> bool {
        self.host.motion() || self.arrange_edge().is_some()
    }
    fn arrange_edge(&self) -> Option<(ViewId, f32)> {
        let s = self.arrange.as_ref()?;
        if !self.arrange_live(s.token) {
            return None;
        }
        let id = self.host.kernel().node_by_key(s.binding.list)?.id;
        let c = s.mapping.clip;
        let y = s.point.1;
        if s.point.0 < c.0 || s.point.0 > c.0 + c.2 || y < c.1 - 24. || y > c.1 + c.3 + 24. {
            return None;
        }
        let speed = if y < c.1 + 24. {
            -300. * ((c.1 + 24. - y) / 24.).min(1.)
        } else if y > c.1 + c.3 - 24. {
            300. * ((y - (c.1 + c.3 - 24.)) / 24.).min(1.)
        } else {
            0.
        };
        let top = self.scroll_of(id).1;
        let max = *self.collection_scroll_limits().get(&id)?;
        ((speed < 0. && top > 0.) || (speed > 0. && top < max)).then_some((id, speed))
    }
    pub(super) fn tick_arrange(&mut self, now: f64) {
        self.retire_pointer();
        self.arrange_settled();
        let Some((id, speed)) = self.arrange_edge() else {
            return;
        };
        let s = self.arrange.as_ref().unwrap();
        let dt = ((now - s.edge_clock) / 1000.).clamp(0., 0.05) as f32;
        let max = self.collection_scroll_limits()[&id];
        let old = self.scroll_of(id).1;
        let next = (old + speed * dt).clamp(0., max);
        self.arrange.as_mut().unwrap().edge_clock = now;
        if next == old {
            return;
        }
        self.scroll.entry(id).or_default().1 = next;
        self.collection_scrolled_by_arrange(id);
        if let Some(point) = self.contact_position() {
            if let Err(e) = self.pointer_move(point.0, point.1, now) {
                self.host.log(e);
            }
        }
    }
}

#[cfg(test)]
#[path = "arrange_tests.rs"]
mod tests;
