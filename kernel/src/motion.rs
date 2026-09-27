//! The kernel→motion seam: a commit, restated as what the engine needs to hear.
//!
//! @ref LLP 1002 §3 (the frame); LLP 1003 §4 (the seam)
//!
//! Motion's inputs are style. After a commit, the animatable rows of every
//! node the commit created or touched are the engine's new targets, each
//! node's `transition` row is how it gets there, and its `animation` row is
//! what plays over them — exactly what a browser reads from computed style. Nothing else crosses: no bindings, no
//! shared values, no second graph. A destroyed node is forgotten. Numeric
//! height is a separate, explicitly registered host trial: the ordinary seam
//! and boot targets remain the four compositor properties.
//!
//! The engine keys nodes by a number the host chooses; here it is the
//! generation-checked [`NodeKey`] packed into a `u64` ([`motion_node`]), so a
//! reused slot never inherits its predecessor's motion.

use crate::generated::{
    BoxSizing, Display, InterpolateSize, PropId, StyleId, StyleMask, StyleProps,
};
use crate::id::NodeKey;
use crate::kernel::Kernel;
use crate::style::Dimension;
use crate::txn::CommitReceipt;
use exact_motion::{Animations, Change, Engine, EngineError, Property, Transitions, Value};

/// The engine's node number for a kernel node.
pub fn motion_node(key: NodeKey) -> u64 {
    ((key.generation as u64) << 32) | key.index as u64
}

/// Everything the motion engine must hear about one commit, in the order it
/// must hear it: forgotten nodes, retired properties, then per node its
/// `transition` row and targets, then its `animation` row.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MotionSync {
    /// Nodes destroyed by the commit.
    pub removed: Vec<u64>,
    /// Properties that no longer have an eligible numeric target. A host must
    /// also retire their presentation projection/overlay and owned playback.
    /// Unlike `removed`, this preserves the node's other motion properties.
    pub retired: Vec<(u64, Property)>,
    /// Each created or touched node's `transition` row.
    pub transitions: Vec<(u64, Transitions)>,
    /// Each created or touched node's `layout-transition` row (LLP 1063).
    /// Its targets are not here: a host observes `Property::Layout` after
    /// layout, from the laid-out origin in the parent.
    pub layout: Vec<(u64, Transitions)>,
    /// The sync's eligible targets; ordinary receipt sync has four per node.
    pub changes: Vec<Change>,
    /// Each created or touched node's `animation` row (LLP 1057). Applied
    /// after the targets, so a node created animating has values to play
    /// over.
    pub animations: Vec<(u64, Animations)>,
}

impl MotionSync {
    /// Feed the engine, in order.
    pub fn apply(&self, engine: &mut Engine) -> Result<(), EngineError> {
        for node in &self.removed {
            engine.remove(*node);
        }
        for (node, property) in &self.retired {
            engine.remove_property(*node, *property);
        }
        for (node, transitions) in &self.transitions {
            engine.set_transitions(*node, transitions.clone())?;
        }
        for (node, transitions) in &self.layout {
            engine.set_layout_transition(*node, transitions)?;
        }
        for change in &self.changes {
            engine.observe(*change)?;
        }
        for (node, animations) in &self.animations {
            engine.set_animations(*node, animations.clone())?;
        }
        Ok(())
    }
}

/// The animatable rows of one style, as engine values. CSS's own property
/// vocabulary: `translate` (two lengths), `scale`, `rotate` (degrees),
/// `opacity`. Height is intentionally absent: only an explicitly registered
/// owner is adopted through [`Kernel::height_motion_sync`], including at boot.
pub fn targets(style: &StyleProps) -> [(Property, Value); 4] {
    [
        (
            Property::Translate,
            Value::new(style.translate.x as f64, style.translate.y as f64),
        ),
        (Property::Scale, Value::scalar(style.scale as f64)),
        (Property::Rotate, Value::scalar(style.rotate as f64)),
        (Property::Opacity, Value::scalar(style.opacity as f64)),
    ]
}

impl Kernel {
    /// Resolve an authored `heightDragFor` to its unique strict ancestor `id`.
    /// The complete handle-to-root path must be attached, displayed, enabled
    /// and non-inert. The target must be a numeric border-box height owner.
    /// Duplicate matching ancestors refuse even if one is ineligible. IDs on
    /// siblings, `testId`, and `nativeId` are deliberately not selectors here.
    ///
    /// This O(depth) read keeps no registry and adopts no motion property.
    /// Hosts retain both generation-checked keys with their live Height token
    /// and revalidate on each receipt/delivery before time or action dispatch.
    pub fn height_drag_target(&self, handle: NodeKey) -> Option<NodeKey> {
        let node = self.node_by_key(handle)?;
        let name = node.props.str(PropId::HeightDragFor)?;
        if name.is_empty() {
            return None;
        }
        let arena = self.arena();
        let mut slot = handle.index;
        let mut target = None;
        loop {
            let props = arena.props(slot);
            if arena.style(slot).display == Display::None
                || props.bool(PropId::Inert) == Some(true)
                || props.bool(PropId::Disabled) == Some(true)
            {
                return None;
            }
            if slot != handle.index && props.str(PropId::Id) == Some(name) {
                if target.is_some() {
                    return None;
                }
                target = Some(arena.key(slot));
            }
            if arena.is_root(slot) {
                break;
            }
            slot = arena.parent(slot)?;
        }
        let target = target?;
        let style = arena.style(target.index);
        if style.box_sizing != BoxSizing::BorderBox {
            return None;
        }
        self.height_target(target)?;
        Some(target)
    }

    /// Shape and inherited opt-in for native content-height transitions.
    /// The host decides admission/lifetime; numeric-only preserves the existing
    /// explicit-owner path. Auto is measured separately, never read from the
    /// current presented frame. Inert boxes remain eligible while collapsing.
    pub fn height_transition_target(&self, owner: NodeKey) -> Option<(Dimension, bool)> {
        let node = self.node_by_key(owner)?;
        if node.style.box_sizing != BoxSizing::BorderBox
            || node.style.transition.matching(Property::Height).is_none()
            || self.arena().is_inline_run(owner.index)
        {
            return None;
        }
        let height = node.style.height;
        match height {
            Dimension::Points(px) if px.is_finite() && px >= 0.0 => {}
            Dimension::Auto => {}
            _ => return None,
        }
        let mut mask = StyleMask::EMPTY;
        mask.set(StyleId::InterpolateSize);
        let allowed = node.computed_style(mask).interpolate_size == InterpolateSize::AllowKeywords;
        let arena = self.arena();
        let mut slot = owner.index;
        loop {
            if arena.style(slot).display == Display::None {
                return None;
            }
            if arena.is_root(slot) {
                return Some((height, allowed));
            }
            slot = arena.parent(slot)?;
        }
    }

    /// The numeric CSS height of one explicitly registered host owner.
    /// Finite nonnegative pixel heights on independent, attached boxes qualify;
    /// auto, percentages, environment lengths, inline runs, detached nodes and
    /// display:none anywhere on the ancestor path do not. This is O(depth),
    /// with no scan or adoption of other numeric-height nodes.
    ///
    /// Layout still applies box sizing and min/max constraints. In particular
    /// the returned authored target is not the displayed height at takeover.
    pub fn height_target(&self, owner: NodeKey) -> Option<Value> {
        let node = self.node_by_key(owner)?;
        let Dimension::Points(px) = node.style.height else {
            return None;
        };
        let arena = self.arena();
        if !px.is_finite() || px < 0.0 || arena.is_inline_run(owner.index) {
            return None;
        }
        let mut slot = owner.index;
        loop {
            if arena.style(slot).display == Display::None {
                return None;
            }
            if arena.is_root(slot) {
                return Some(Value::scalar(px as f64));
            }
            slot = arena.parent(slot)?;
        }
    }

    /// Reconcile Height for the host's one explicitly registered trial owner.
    /// Call at registration/boot and after every commit or layout entry, even
    /// when the receipt does not touch the owner: ancestor hide/detach also
    /// revokes eligibility. No owner registry is retained by the kernel.
    ///
    /// Eligible input supplies the latest declaration and target; otherwise
    /// only Height is retired. Replacing/unregistering an owner requires the
    /// host to retire the previous Height before adopting another. Ordinary
    /// [`Self::motion_sync`] and [`targets`] never adopt Height implicitly.
    pub fn height_motion_sync(&self, owner: NodeKey) -> MotionSync {
        let id = motion_node(owner);
        let Some(value) = self.height_target(owner) else {
            return MotionSync {
                retired: vec![(id, Property::Height)],
                ..MotionSync::default()
            };
        };
        let node = self.node_by_key(owner).expect("validated height owner");
        MotionSync {
            transitions: vec![(id, node.style.transition.clone())],
            changes: vec![Change {
                node: id,
                property: Property::Height,
                value,
                velocity: None,
            }],
            ..MotionSync::default()
        }
    }

    /// Restate a commit for the motion engine. The receipt must be one this
    /// kernel produced; a key the commit destroyed resolves to nothing, which
    /// is exactly what makes it a removal.
    pub fn motion_sync(&self, receipt: &CommitReceipt) -> MotionSync {
        let mut sync = MotionSync {
            removed: receipt.destroyed.iter().copied().map(motion_node).collect(),
            ..MotionSync::default()
        };
        for key in receipt.created.iter().chain(receipt.touched.iter()) {
            let Some(node) = self.node_by_key(*key) else {
                continue;
            };
            let id = motion_node(*key);
            sync.transitions.push((id, node.style.transition.clone()));
            sync.layout.push((id, node.style.layout_transition.clone()));
            sync.animations.push((id, node.style.animation.clone()));
            for (property, value) in targets(node.style) {
                sync.changes.push(Change {
                    node: id,
                    property,
                    value,
                    velocity: None,
                });
            }
        }
        sync
    }
}
