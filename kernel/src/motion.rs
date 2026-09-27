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
    BoxSizing, Display, InterpolateSize, NodeType, PropId, StyleId, StyleMask, StyleProps,
};
use crate::id::NodeKey;
use crate::kernel::Kernel;
use crate::style::{ColorValue, Dimension};
use crate::txn::CommitReceipt;
use exact_motion::{Animations, Change, Engine, EngineError, Property, Transitions, Value};
use std::collections::BTreeMap;

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

/// Which nodes own paint motion, and which paint properties each owns: the
/// host's record (LLP 1062 D2), so a commit retires exactly what an earlier
/// one adopted and an appearance change re-resolves exactly those.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaintOwners(BTreeMap<u64, u16>);

impl PaintOwners {
    /// Whether `node` owns `property`.
    pub fn owns(&self, node: u64, property: Property) -> bool {
        self.0
            .get(&node)
            .is_some_and(|mask| mask & paint_bit(property) != 0)
    }

    /// Every owning node.
    pub fn nodes(&self) -> impl Iterator<Item = u64> + '_ {
        self.0.keys().copied()
    }
}

fn paint_bit(property: Property) -> u16 {
    1 << (property as u8 - Property::BackgroundColor as u8)
}

fn color(c: ColorValue, dark: bool) -> Value {
    let c = c.resolve(dark);
    Value::rgba8([c.r(), c.g(), c.b(), c.a()])
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

/// A `path` node's stroke fractions, as engine values (LLP 1065 D4). Only
/// a path's are synced: no other node draws a stroke, so no other node
/// spends engine slots on them.
pub fn stroke_targets(style: &StyleProps) -> [(Property, Value); 2] {
    [
        (
            Property::StrokeStart,
            Value::scalar(style.stroke_start as f64),
        ),
        (Property::StrokeEnd, Value::scalar(style.stroke_end as f64)),
    ]
}

/// Every target the engine hears for one node: the four compositor rows,
/// and a path's stroke fractions. What a commit and a host's boot both sync.
pub fn node_targets(
    node_type: NodeType,
    style: &StyleProps,
) -> impl Iterator<Item = (Property, Value)> {
    let strokes = (node_type == NodeType::Path).then(|| stroke_targets(style));
    targets(style)
        .into_iter()
        .chain(strokes.into_iter().flatten())
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

    /// A node's paint targets under an appearance: each paint property its
    /// `transition` starts an easing for or its `animation` names, with its
    /// computed value — `light-dark()` resolved by `dark`, `currentcolor`
    /// borders as the computed `color`, the shadow's opacity in its alpha.
    /// A node that names none owns no paint motion: its host paints style.
    pub fn paint_targets(&self, key: NodeKey, dark: bool) -> Vec<(Property, Value)> {
        let Some(node) = self.node_by_key(key) else {
            return Vec::new();
        };
        let s = node.style;
        if s.transition.0.is_empty() && s.animation.0.is_empty() {
            return Vec::new();
        }
        let named = |p: Property| {
            s.transition.matching(p).is_some_and(|t| t.starts())
                || s.animation.0.iter().any(|a| a.keyframes.affects(p))
        };
        let text = node.text_color();
        let [top, right, bottom, left] = s.border_colors(text);
        let shadow = s.shadow_color.resolve(dark);
        let alpha = shadow.a() as f64 / 255.0 * (s.shadow_opacity as f64).clamp(0.0, 1.0);
        let unit = |c: u8| c as f64 / 255.0;
        Property::PAINT
            .into_iter()
            .filter(|p| named(*p))
            .map(|p| {
                let value = match p {
                    Property::BackgroundColor => color(s.background_color, dark),
                    Property::Color => color(text, dark),
                    Property::BorderTopColor => color(top, dark),
                    Property::BorderRightColor => color(right, dark),
                    Property::BorderBottomColor => color(bottom, dark),
                    Property::BorderLeftColor => color(left, dark),
                    Property::TintColor => color(s.tint_color, dark),
                    Property::BoxShadow => Value::four(
                        s.shadow_offset.x as f64,
                        s.shadow_offset.y as f64,
                        s.shadow_radius as f64,
                        0.0,
                    ),
                    _ => Value::rgba(unit(shadow.r()), unit(shadow.g()), unit(shadow.b()), alpha),
                };
                (p, value)
            })
            .collect()
    }

    /// Restate a commit's paint for a native engine (LLP 1062 D2), after
    /// [`Self::motion_sync`] has set the nodes' rows: each created or
    /// touched node's [`Self::paint_targets`], and a retirement for each
    /// property it owned and no longer names. The web never calls this; the
    /// browser transitions paint itself.
    pub fn paint_sync(
        &self,
        receipt: &CommitReceipt,
        dark: bool,
        owners: &mut PaintOwners,
    ) -> MotionSync {
        for key in &receipt.destroyed {
            owners.0.remove(&motion_node(*key));
        }
        let keys = receipt.created.iter().chain(receipt.touched.iter());
        self.paint_adopt(keys.copied(), dark, owners)
    }

    /// [`Self::paint_sync`] for chosen nodes: a host's boot, which hears the
    /// whole tree, and [`Self::paint_resync`].
    pub fn paint_adopt(
        &self,
        keys: impl IntoIterator<Item = NodeKey>,
        dark: bool,
        owners: &mut PaintOwners,
    ) -> MotionSync {
        let mut sync = MotionSync::default();
        for key in keys {
            self.adopt_paint(key, dark, owners, &mut sync);
        }
        sync
    }

    /// Re-resolve every owner's paint under a new appearance. A `light-dark()`
    /// target that changes transitions under the node's row, as a browser's
    /// computed value does when `color-scheme` changes (LLP 1062 D4).
    pub fn paint_resync(&self, dark: bool, owners: &mut PaintOwners) -> MotionSync {
        let keys: Vec<NodeKey> = owners
            .nodes()
            .map(|node| NodeKey {
                index: node as u32,
                generation: (node >> 32) as u32,
            })
            .collect();
        self.paint_adopt(keys, dark, owners)
    }

    fn adopt_paint(
        &self,
        key: NodeKey,
        dark: bool,
        owners: &mut PaintOwners,
        sync: &mut MotionSync,
    ) {
        let id = motion_node(key);
        let old = owners.0.get(&id).copied().unwrap_or(0);
        let mut mask = 0;
        for (property, value) in self.paint_targets(key, dark) {
            mask |= paint_bit(property);
            sync.changes.push(Change {
                node: id,
                property,
                value,
                velocity: None,
            });
        }
        for property in Property::PAINT {
            if old & !mask & paint_bit(property) != 0 {
                sync.retired.push((id, property));
            }
        }
        if mask == 0 {
            owners.0.remove(&id);
        } else {
            owners.0.insert(id, mask);
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
            for (property, value) in node_targets(node.node_type, node.style) {
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
