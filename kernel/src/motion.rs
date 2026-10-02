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
//!
//! Colour is its own pass ([`Kernel::paint_sync`], LLP 1062 D2): a native
//! host reports appearance to [`PaintMotion`], which resolves `light-dark()`
//! and keeps the record of which nodes own paint motion. [`LayoutMotion`]
//! observes boxes after layout; the hosts only present the resulting values.

mod layout;
mod paint;
pub use layout::{layout_presented, LayoutMotion};
pub use paint::PaintMotion;

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

/// The generation-checked kernel key packed into an engine node number.
pub fn node_key(node: u64) -> NodeKey {
    NodeKey {
        index: node as u32,
        generation: (node >> 32) as u32,
    }
}

/// Everything the motion engine must hear about one commit, in the order it
/// must hear it: forgotten nodes, retired properties, then per node its
/// `transition` row and targets.
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
    /// Each created or touched node's `animation` row (LLP 1055 D5).
    pub animations: Vec<(u64, Animations)>,
    /// Each created or touched node's drag timeline rows, and each consumer
    /// whose name the commit resolved anew (LLP 1057.003 D4): whether the
    /// timeline it drives reads `x`, and what its animations follow (the
    /// source node its name resolved to, and their range).
    pub timelines: Vec<TimelineRows>,
    /// The sync's eligible targets; ordinary receipt sync has four per node.
    pub changes: Vec<Change>,
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
            engine.set_animations(*node, animations)?;
        }
        for (node, source, binding) in &self.timelines {
            engine.set_drag_timeline(*node, *source);
            engine.set_animation_timeline(*node, *binding);
        }
        Ok(())
    }
}

/// One node's drag timeline rows, as the engine takes them: whether the
/// timeline it drives reads `x`, and what its animations follow over which
/// range.
pub type TimelineRows = (
    u64,
    Option<bool>,
    Option<(exact_motion::NamedTimeline, [f64; 2])>,
);

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

/// Which nodes own paint motion, and which paint properties each owns: the
/// record (LLP 1062 D2), so a commit retires exactly what an earlier
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

/// The appearance a node's `light-dark()` colours resolve by: one for the
/// whole tree (`bool`), or per node (a function of its key), for a host
/// whose views each have their own (LLP 1062 D4).
pub trait Appearance {
    /// Whether `key` resolves dark.
    fn dark(&self, key: NodeKey) -> bool;
}

impl Appearance for bool {
    fn dark(&self, _: NodeKey) -> bool {
        *self
    }
}

impl<F: Fn(NodeKey) -> bool> Appearance for F {
    fn dark(&self, key: NodeKey) -> bool {
        self(key)
    }
}

fn paint_bit(property: Property) -> u16 {
    Property::PAINT
        .iter()
        .position(|p| *p == property)
        .map_or(0, |i| 1 << i)
}

fn color(c: ColorValue, dark: bool) -> Value {
    let c = c.resolve(dark);
    Value::rgba8(c.r(), c.g(), c.b(), c.a())
}

/// A node's paint targets under an appearance (LLP 1055.000 D6, LLP 1062
/// D2): only for the paint properties its `transition`, `animation` or
/// `exit-animation` names, so a node that animates no colour costs nothing.
/// `light-dark()` resolves by `dark`, `color` is inherited, and a shadow's
/// opacity folds into its colour's alpha. `None` for a property that is not
/// a colour now: paint `none` or a paint server (CSS: a discrete pair, no
/// transition). An exit's colours are owned while the node lives, so the
/// engine has the value its exit keyframes start over (LLP 1063).
///
/// A `currentcolor` border side's target is the computed `color`. Its
/// computed value is the keyword whatever `color` does, so CSS starts no
/// transition on it while it stays `currentcolor`, and paints it in the
/// element's animating `color` frame by frame: a host settles such a side
/// at once and paints it in the view's presented `color`
/// ([`Kernel::current_color_sides`]); only a change to or from an explicit
/// colour moves it.
pub fn color_targets(
    node: &crate::kernel::NodeRef<'_>,
    dark: bool,
) -> Vec<(Property, Option<Value>)> {
    let s = node.style;
    if s.transition.0.is_empty() && s.animation.0.is_empty() && s.exit_animation.0.is_empty() {
        return Vec::new();
    }
    let animated: Vec<Property> = s
        .animation
        .properties()
        .into_iter()
        .chain(s.exit_animation.properties())
        .collect();
    // `fill` and `stroke` paint only in an `svg`; a box's computed paint
    // is the initial one and moves nothing.
    let svg = node.node_type == NodeType::Svg || node.node_type.is_svg_element();
    let wanted: Vec<Property> = Property::PAINT
        .into_iter()
        .filter(|p| svg || !matches!(p, Property::Fill | Property::Stroke))
        .filter(|p| animated.contains(p) || s.transition.0.iter().any(|t| t.property.covers(*p)))
        .collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let text = node.text_color();
    let paint = |id: StyleId| match node.computed(id) {
        crate::style::RowValue::Paint(crate::svg::Paint::Color(c)) => Some(color(*c, dark)),
        crate::style::RowValue::Paint(crate::svg::Paint::CurrentColor) => Some(color(text, dark)),
        _ => None,
    };
    let [top, right, bottom, left] = s.border_colors(text);
    // @ref LLP 1076 D4 — the engine moves the list's first shadow; the
    // rest change at once (declared in LLP 1001).
    let first = s.box_shadow.0.first();
    let shadow = first.map_or(crate::style::Color::TRANSPARENT, |f| f.color.resolve(dark));
    let alpha = shadow.a() as f64 / 255.0;
    let unit = |c: u8| c as f64 / 255.0;
    wanted
        .into_iter()
        .map(|p| {
            let value = match p {
                Property::Color => Some(color(text, dark)),
                Property::BackgroundColor => Some(color(s.background_color, dark)),
                Property::Fill => paint(StyleId::Fill),
                Property::Stroke => paint(StyleId::Stroke),
                Property::BorderTopColor => Some(color(top, dark)),
                Property::BorderRightColor => Some(color(right, dark)),
                Property::BorderBottomColor => Some(color(bottom, dark)),
                Property::BorderLeftColor => Some(color(left, dark)),
                Property::TintColor => Some(color(s.tint_color, dark)),
                Property::BoxShadow => Some(first.map_or(Value::ZERO, |f| {
                    Value::four(f.offset.x as f64, f.offset.y as f64, f.blur as f64, 0.0)
                })),
                _ => Some(Value::rgba(
                    unit(shadow.r()),
                    unit(shadow.g()),
                    unit(shadow.b()),
                    alpha,
                )),
            };
            (p, value)
        })
        .collect()
}

impl Kernel {
    /// An SVG shape's two animatable geometry rows (LLP 1055 D6): its
    /// computed `stroke-dashoffset` (inherited, as SVG says), and a circle's
    /// `r`.
    pub fn svg_targets(&self, key: NodeKey) -> Vec<(Property, Value)> {
        let Some(node) = self.node_by_key(key) else {
            return Vec::new();
        };
        let mut mask = StyleMask::EMPTY;
        mask.set(StyleId::StrokeDashoffset);
        let offset = node.computed_style(mask).stroke_dashoffset;
        let mut out = vec![(Property::StrokeDashoffset, Value::scalar(offset as f64))];
        // `r` animates as a length in user units; a percentage radius
        // resolves against its viewport at paint time and is not a target.
        if let (NodeType::SvgCircle, Dimension::Points(r)) = (node.node_type, node.style.r) {
            out.push((Property::R, Value::scalar(r as f64)));
        }
        // @ref LLP 1055.000 D15 — the geometry rows a shape draws from,
        // when they are lengths in user units; a percentage resolves at
        // paint time and is not a target.
        let s = node.style;
        let rows: &[(Property, Dimension)] = match node.node_type {
            NodeType::SvgCircle => &[(Property::Cx, s.cx), (Property::Cy, s.cy)],
            NodeType::SvgEllipse => &[
                (Property::Cx, s.cx),
                (Property::Cy, s.cy),
                (Property::Rx, s.rx),
                (Property::Ry, s.ry),
            ],
            NodeType::SvgRect => &[
                (Property::X, s.x),
                (Property::Y, s.y),
                (Property::Rx, s.rx),
                (Property::Ry, s.ry),
            ],
            _ => &[],
        };
        for (p, d) in rows {
            if let Dimension::Points(v) = d {
                out.push((*p, Value::scalar(*v as f64)));
            }
        }
        out
    }

    /// The box a node's `layout-transition` animates (LLP 1063), when it
    /// declares one: its laid-out origin and size, relative to the box it is
    /// placed in. That is its parent, except for the root of a virtualized
    /// list's row: its wrapper (the node carrying `listItemKey`) exists only
    /// to position it, so the row is placed in the list content and a wrapper
    /// moving is the row moving. `None` too while it or an ancestor is
    /// `display: none`: it has no box, and one shown again is first seen, as
    /// CSS starts no transition from `display: none`.
    pub fn layout_box(&self, key: NodeKey) -> Option<Value> {
        let node = self.node_by_key(key)?;
        node.style.layout_transition.matching(Property::Layout)?;
        let arena = self.arena();
        let mut slot = Some(key.index);
        while let Some(s) = slot {
            if arena.style(s).display == Display::None {
                return None;
            }
            slot = arena.parent(s);
        }
        let mut parent = node.parent.and_then(|p| self.node(p));
        if parent
            .as_ref()
            .is_some_and(|p| p.props.str(PropId::ListItemKey).is_some())
        {
            parent = parent.and_then(|p| p.parent).and_then(|p| self.node(p));
        }
        let (px, py) = parent.map_or((0.0, 0.0), |p| (p.frame.x, p.frame.y));
        let f = node.frame;
        Some(Value::four(
            (f.x - px) as f64,
            (f.y - py) as f64,
            f.width as f64,
            f.height as f64,
        ))
    }

    /// The border sides a node paints in `currentcolor` (CSS's initial
    /// `border-color`) and draws at all: a host paints them in the view's
    /// presented `color` while that moves, own or inherited (LLP 1062 D1).
    pub fn current_color_sides(&self, key: NodeKey) -> Vec<Property> {
        let Some(node) = self.node_by_key(key) else {
            return Vec::new();
        };
        let s = node.style;
        let colors = [
            s.border_color_top,
            s.border_color_right,
            s.border_color_bottom,
            s.border_color_left,
        ];
        let sides = [
            Property::BorderTopColor,
            Property::BorderRightColor,
            Property::BorderBottomColor,
            Property::BorderLeftColor,
        ];
        sides
            .into_iter()
            .zip(colors)
            .zip(s.border_widths())
            .filter(|((_, c), w)| c.is_none() && *w > 0.0)
            .map(|((side, _), _)| side)
            .collect()
    }

    /// Restate a commit's paint for a native engine (LLP 1062 D2), after
    /// [`Self::motion_sync`] has set the nodes' rows: each created or
    /// touched node's [`color_targets`], and a retirement for each property
    /// it owned and no longer has a colour for. The web never calls this;
    /// the browser transitions paint itself.
    pub fn paint_sync(
        &self,
        receipt: &CommitReceipt,
        dark: impl Appearance,
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
        dark: impl Appearance,
        owners: &mut PaintOwners,
    ) -> MotionSync {
        let mut sync = MotionSync::default();
        for key in keys {
            self.adopt_paint(key, dark.dark(key), owners, &mut sync);
        }
        sync
    }

    /// Re-resolve every owner's paint under a new appearance. A `light-dark()`
    /// target that changes transitions under the node's row, as a browser's
    /// computed value does when `color-scheme` changes (LLP 1062 D4).
    pub fn paint_resync(&self, dark: impl Appearance, owners: &mut PaintOwners) -> MotionSync {
        let keys: Vec<NodeKey> = owners.nodes().map(node_key).collect();
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
        if let Some(node) = self.node_by_key(key) {
            for (property, value) in color_targets(&node, dark) {
                // Not a colour now (`none`, a server): nothing interpolates,
                // and the row shows as authored.
                let Some(value) = value else {
                    continue;
                };
                mask |= paint_bit(property);
                sync.changes.push(Change {
                    node: id,
                    property,
                    value,
                    velocity: None,
                });
            }
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
            self.motion_sync_node(*key, &mut sync);
        }
        // A consumer the commit left alone whose name now finds another
        // timeline (LLP 1057.003 D4).
        for key in &receipt.timelines {
            if let Some(slot) = self.arena().resolve(*key) {
                sync.timelines
                    .push(crate::timeline::rows(self.arena(), slot));
            }
        }
        // A `display` change cancels (`none`) or restarts every animation
        // below it (LLP 1055.000 D15): only descendants with a row are told.
        for key in &receipt.display_changed {
            let Some(node) = self.node_by_key(*key) else {
                continue;
            };
            let mut stack = node.children();
            while let Some(id) = stack.pop() {
                let Some(d) = self.node(id) else {
                    continue;
                };
                if !d.style.animation.0.is_empty() {
                    let row = if self.hidden(&d) {
                        Animations::default()
                    } else {
                        d.style.animation.clone()
                    };
                    sync.animations.push((motion_node(d.key), row));
                }
                stack.extend(d.children());
            }
        }
        sync
    }

    /// Whether the node or an ancestor is `display: none`: CSS runs no
    /// animation there (CSS Animations 1 §3).
    fn hidden(&self, node: &crate::kernel::NodeRef<'_>) -> bool {
        let mut cur = Some(node.id);
        while let Some(id) = cur {
            let Some(n) = self.node(id) else {
                return false;
            };
            if n.style.display == Display::None {
                return true;
            }
            cur = n.parent;
        }
        false
    }

    /// Append one live node's `transition` row, targets and `animation` row
    /// to `sync`: what a commit says about a node it created or touched, and
    /// what a host's boot says about every node (LLP 1055 D5: an animation
    /// starts when its node is first seen, boot included).
    pub fn motion_sync_node(&self, key: NodeKey, sync: &mut MotionSync) {
        {
            let Some(node) = self.node_by_key(key) else {
                return;
            };
            let key = &key;
            let id = motion_node(*key);
            sync.transitions.push((id, node.style.transition.clone()));
            for (property, value) in targets(node.style) {
                sync.changes.push(Change {
                    node: id,
                    property,
                    value,
                    velocity: None,
                });
            }
            if node.node_type.is_svg_shape() {
                for (property, value) in self.svg_targets(*key) {
                    sync.changes.push(Change {
                        node: id,
                        property,
                        value,
                        velocity: None,
                    });
                }
            }
            sync.layout.push((id, node.style.layout_transition.clone()));
            // An empty row is how a removed animation reaches the engine; an
            // empty row on a node that never had one costs one map lookup.
            // A hidden node runs none (LLP 1055.000 D15).
            let row = if !node.style.animation.0.is_empty() && self.hidden(&node) {
                Animations::default()
            } else {
                node.style.animation.clone()
            };
            sync.animations.push((id, row));
            sync.timelines
                .push(crate::timeline::rows(self.arena(), key.index));
        }
    }
}
