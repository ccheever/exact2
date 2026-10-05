//! Input into the acknowledged CPU picture. Live state can refuse, never
//! retarget. Contacts keep at most two weak scalar bindings and tiny identities;
//! they cannot pin a Picture, paragraph, resource, plan or retired argument set.
use super::*;
use crate::paint::{ActionNode, ActionSlot};
use exact_kernel::NodeKey;
use exact_motion::{HoldToken, Property, SpringDescriptor, Value};
use exact_runner::runner::ActionBinding;
use std::rc::{Rc, Weak};

pub(super) struct Target {
    pub(super) key: NodeKey,
    pub(super) kind: EventKind,
    binding: Weak<ActionBinding>,
    picture: Weak<()>,
}
pub(super) struct RetainedContact {
    hit: NodeKey,
    pub(super) press: Option<Target>,
    pub(super) swipe: Option<Target>,
}
enum MotionPhase {
    Held,
    Returning {
        target: Value,
        spring: Option<SpringDescriptor>,
    },
}
/// One paint-only authority, with no picture, source or action payload owner.
pub(super) struct MotionPermit {
    key: NodeKey,
    token: HoldToken,
    picture: Weak<()>,
    phase: MotionPhase,
}
impl MotionPermit {
    pub(super) fn allows<D: DataSource>(
        &self,
        host: &Host<D>,
        key: NodeKey,
        picture: &Rc<()>,
    ) -> bool {
        if self.key != key || !Weak::ptr_eq(&self.picture, &Rc::downgrade(picture)) {
            return false;
        }
        let engine = host.engine();
        match self.phase {
            MotionPhase::Held => host.has_hold(self.token),
            MotionPhase::Returning { target, spring } => {
                // Numeric equality alone cannot identify a foreign replacement.
                // The original token must still own this exact running return.
                engine.owns_return(self.token)
                    && !engine.is_held(self.token.node(), Property::Translate)
                    && engine.target(self.token.node(), Property::Translate) == Some(target)
                    && engine.spring_descriptor(self.token.node(), Property::Translate) == spring
            }
        }
    }
}
impl Target {
    fn matches(&self, other: &Self) -> bool {
        self.key == other.key
            && self.kind == other.kind
            && Weak::ptr_eq(&self.picture, &other.picture)
            && Weak::ptr_eq(&self.binding, &other.binding)
    }
}
impl<D: DataSource> Presenter<D> {
    fn retained_node(&self, key: NodeKey) -> Option<(&Rc<()>, &ActionNode)> {
        let region = self.host.content_region()?;
        let live = self.host.kernel().node_by_key(key)?;
        let (picture, old) = self.brush.retained_action_node(region, live.id)?;
        if old.key != key
            || old.blocked
            || !self.host.retained_action_eligible(key)
            || live
                .parent
                .and_then(|id| self.host.kernel().node(id))
                .map(|n| n.key)
                != old.parent
        {
            return None;
        }
        Some((picture, old))
    }

    // Err is an explicit captured/live barrier, distinct from no handler. In
    // either case we NEVER continue into ordinary live handler_target.
    fn retained_route(&self, hit: NodeKey, kind: EventKind) -> Result<Option<Target>, ()> {
        let view = self.host.kernel().node_by_key(hit).ok_or(())?.id;
        if !self.display.allows(self.host.kernel(), view) {
            return Err(());
        }
        self.retained_captured_route(hit, kind)
    }
    fn retained_captured_route(&self, hit: NodeKey, kind: EventKind) -> Result<Option<Target>, ()> {
        let mut key = hit;
        let mut selected = None;
        for _ in 0..exact_kernel::region::REGION_NODES {
            let (picture, old) = self.retained_node(key).ok_or(())?;
            if selected.is_none() {
                let slot = match kind {
                    EventKind::Press => &old.press,
                    EventKind::Swiperight => &old.swipe,
                    _ => return Err(()),
                };
                match slot {
                    ActionSlot::Refused => return Err(()),
                    ActionSlot::Bound(binding) => {
                        let node = self.host.kernel().node_by_key(key).ok_or(())?;
                        if !self.display.allows(self.host.kernel(), node.id) {
                            return Err(());
                        }
                        if kind == EventKind::Swiperight
                            && !old.swipe_matches(&node, self.host.kernel())
                        {
                            return Err(());
                        }
                        self.host
                            .runner()
                            .validate_action_binding(binding, kind)
                            .map_err(|_| ())?;
                        selected = Some(Target {
                            key,
                            kind,
                            binding: Rc::downgrade(binding),
                            picture: Rc::downgrade(picture),
                        });
                    }
                    ActionSlot::Absent => {}
                }
                if selected.is_none()
                    && kind == EventKind::Swiperight
                    && !matches!(old.press, ActionSlot::Absent)
                {
                    return Err(()); // a child Press, including an unsupported one
                }
            }
            // Qualify all captured ancestors even after selecting a handler;
            // never borrow B's parent chain in headless or display-owned mode.
            let Some(parent) = old.parent else {
                return Ok(selected);
            };
            let Some(node) = self.host.kernel().node_by_key(parent) else {
                return Err(());
            };
            if self
                .brush
                .retained_action_node(self.host.content_region().ok_or(())?, node.id)
                .is_none()
            {
                return Ok(selected); // never seek a shell handler outside A
            }
            key = parent;
        }
        Err(())
    }

    pub(super) fn retained_down(&self, hit: NodeKey) -> Option<RetainedContact> {
        let press = self.retained_route(hit, EventKind::Press).ok().flatten();
        let swipe = self
            .retained_route(hit, EventKind::Swiperight)
            .ok()
            .flatten();
        (press.is_some() || swipe.is_some()).then_some(RetainedContact { hit, press, swipe })
    }
    pub(super) fn retained_swipe_candidate(&self, hit: NodeKey) -> Option<NodeKey> {
        self.retained_route(hit, EventKind::Swiperight)
            .ok()
            .flatten()
            .map(|t| t.key)
    }
    pub(super) fn retained_contact_live(&self, contact: &RetainedContact) -> bool {
        [&contact.press, &contact.swipe]
            .into_iter()
            .flatten()
            .all(|old| {
                // A's stale text descendant may cease to be a hit in a later
                // A repaint. Keep its captured chain/barriers, but require the
                // selected action node and ancestors in the actual ACK witness.
                self.retained_captured_route(contact.hit, old.kind)
                    .ok()
                    .flatten()
                    .is_some_and(|t| old.matches(&t))
            })
    }
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    fn retained_picture_current(&self, key: NodeKey, picture: &Weak<()>) -> bool {
        self.host
            .content_region()
            .and_then(|region| {
                let node = self.host.kernel().node_by_key(key)?;
                self.brush.retained_action_node(region, node.id)
            })
            .is_some_and(|(current, _)| Weak::ptr_eq(picture, &Rc::downgrade(current)))
    }
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(super) fn retained_contact_picture_current(&self, contact: &RetainedContact) -> bool {
        [&contact.press, &contact.swipe]
            .into_iter()
            .flatten()
            .all(|target| self.retained_picture_current(target.key, &target.picture))
    }
    pub(super) fn begin_retained_motion(&mut self, target: &Target, token: HoldToken) -> bool {
        if !self.host.has_hold(token)
            || token.node() != exact_kernel::motion::motion_node(target.key)
            || token.property() != Property::Translate
            || self.retained_binding(target).is_none()
        {
            return false;
        }
        self.retained_motion = Some(MotionPermit {
            key: target.key,
            token,
            picture: target.picture.clone(),
            phase: MotionPhase::Held,
        });
        true
    }
    pub(super) fn retained_motion_returned(&mut self, token: HoldToken) {
        let Some(permit) = self.retained_motion.as_mut().filter(|p| p.token == token) else {
            return;
        };
        // Called only after this exact token's successful end. No new hold or
        // observer callback can interleave with the scalar descriptor read.
        let engine = self.host.engine();
        if let Some(target) = engine
            .target(token.node(), Property::Translate)
            .filter(|_| engine.owns_return(token))
        {
            permit.phase = MotionPhase::Returning {
                target,
                spring: engine.spring_descriptor(token.node(), Property::Translate),
            };
        } else {
            self.retained_motion = None;
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(super) fn retire_retained_motion_picture(&mut self) {
        if self
            .retained_motion
            .as_ref()
            .is_some_and(|p| !self.retained_picture_current(p.key, &p.picture))
        {
            let permit = self.retained_motion.take().unwrap();
            // The Host rechecks exact Held/Returning serial ownership directly
            // before mutation; an identical foreign curve is left untouched.
            match self.host.snap_retained_motion(permit.token) {
                Ok(changed) => self.dirty |= changed,
                Err(error) => self.host.log(error),
            }
        }
    }
    fn retained_binding(&self, target: &Target) -> Option<Rc<ActionBinding>> {
        let current = self
            .retained_route(target.key, target.kind)
            .ok()
            .flatten()?;
        target
            .matches(&current)
            .then(|| target.binding.upgrade())
            .flatten()
    }
    pub(crate) fn retained_press(&mut self, hit: ViewId, now_ms: f64) -> Option<ViewId> {
        let key = self.host.kernel().node(hit)?.key;
        let target = self.retained_route(key, EventKind::Press).ok().flatten()?;
        self.retained_press_target(&target, now_ms)
    }
    pub(super) fn retained_press_target(&mut self, target: &Target, now_ms: f64) -> Option<ViewId> {
        let binding = self.retained_binding(target)?;
        let view = self.host.kernel().node_by_key(target.key)?.id;
        match self
            .host
            .dispatch_retained(target.key, &binding, EventKind::Press, now_ms)
        {
            Ok(false) => return None,
            Err(error) => self.host.log(error),
            Ok(true) => {}
        }
        if self.focus.take().is_some() {
            self.dirty = true;
            self.queue_collections();
        }
        if let Some(error) = self.after_commit() {
            self.host.log(error);
        }
        Some(view)
    }
    pub(super) fn retained_swipe_dispatch(
        &mut self,
        target: &Target,
        token: exact_motion::HoldToken,
        now_ms: f64,
    ) -> Result<bool, String> {
        let Some(binding) = self.retained_binding(target) else {
            return Ok(false);
        };
        let accepted = self
            .host
            .dispatch_retained_held(token, target.key, &binding, now_ms)?;
        if accepted {
            if let Some(error) = self.after_commit() {
                return Err(error);
            }
        }
        Ok(accepted)
    }
}
