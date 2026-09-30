//! Which nodes the page makes `position: relative` (LLP 1001 §1).
//!
//! The kernel has no `static`: every node is a containing block for an
//! absolute child, its insets move it, and nodes paint in tree order. A page
//! box is `static` unless said otherwise, which costs it no layer to paint,
//! hit-test and composite, so the page makes a node `relative` only where
//! that shows: over an absolute child, with insets or a `z-index`, or after
//! something in tree order that paints with the positioned — a positioned box
//! or a stacking context anywhere in an earlier sibling's subtree — which a
//! static box would otherwise paint under (CSS 2 Appendix E paints the
//! positioned and the stacking contexts after the in-flow boxes).
//!
//! The live host keeps each node's facts and re-decides a sibling list only
//! when a node in it changed ([`Host::relayer`]); the document writer and an
//! ahead-of-time build decide the same with [`relative`] and [`layered`].

use exact_kernel::id::IdMap;
use exact_kernel::{Kernel, NodeRef, NodeType, PositionType, PropId, StyleId, ViewId};
use exact_runner::DataSource;

use super::element::{css_style, host_css, tag_for};
use super::Host;
use crate::batch::Batch;
use crate::css;

/// What a node's own rows and props bring to the page's painting order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Paint {
    /// Names its `position`, or the host positions it (a canvas, whose
    /// surface it holds; a Markdown editor, whose lines' markers it holds).
    pub positioned: bool,
    /// May make a stacking context, which paints with the positioned: an
    /// opacity, a transform, a filter, a clip or mask, a blend, an
    /// isolation, a transition or animation (which may move them), a press's
    /// scale, a material's backdrop, a navigation screen or a modal.
    pub stacks: bool,
    /// Insets or a `z-index`, which only a positioned box honours.
    pub insets: bool,
    /// Outside the page's box painting: an element inside an `svg`, a head.
    pub outside: bool,
}

/// The rows that may make a stacking context ([`Paint::stacks`]).
pub const STACKS: [StyleId; 18] = [
    StyleId::Opacity,
    StyleId::BackdropBlur,
    StyleId::Translate,
    StyleId::Scale,
    StyleId::Rotate,
    StyleId::ClipPath,
    StyleId::Animation,
    StyleId::Transform,
    StyleId::SvgMask,
    StyleId::Filter,
    StyleId::MixBlendMode,
    StyleId::Isolation,
    StyleId::Transition,
    StyleId::LayoutTransition,
    StyleId::PressScale,
    StyleId::DragTimeline,
    StyleId::AnimationTimeline,
    StyleId::ExitAnimation,
];
/// Insets and `z-index` ([`Paint::insets`]).
pub const INSETS: [StyleId; 5] = [
    StyleId::Top,
    StyleId::Right,
    StyleId::Bottom,
    StyleId::Left,
    StyleId::ZIndex,
];

/// A node's [`Paint`].
pub fn paint(node: &NodeRef<'_>) -> Paint {
    let m = &node.style.mask;
    let props = node.props;
    let editor =
        node.node_type == NodeType::TextInput && props.str(PropId::Markup) == Some("markdown");
    Paint {
        positioned: m.has(StyleId::PositionType) || node.node_type == NodeType::Canvas || editor,
        stacks: STACKS.iter().any(|s| m.has(*s))
            || props.str(PropId::BackgroundMaterial).is_some()
            || props.str(PropId::NavigationKey).is_some()
            || props.str(PropId::NavigationPresentation) == Some("modal"),
        insets: INSETS.iter().any(|s| m.has(*s)),
        outside: node.node_type.is_svg_element() || node.node_type.is_metadata(),
    }
}

/// Whether an absolute child's containing block is `node`.
pub fn holds_absolute(kernel: &Kernel, node: &NodeRef<'_>) -> bool {
    node.has_absolute_child()
        && node.children().into_iter().any(|c| {
            kernel.node(c).is_some_and(|c| {
                c.style.position_type == PositionType::Absolute && !paint(&c).outside
            })
        })
}

/// Whether the page makes a node `relative`: it doesn't name its position,
/// and a positioned box shows (it holds an absolute child, or has insets or
/// a `z-index`), or something before it paints with the positioned
/// (`after`), which it would paint under as a static box — unless it is a
/// stacking context, which paints with them already.
pub fn relative(p: Paint, holds: bool, after: bool) -> bool {
    !p.outside && !p.positioned && (p.insets || holds || (after && !p.stacks))
}

/// Whether a subtree paints with the positioned: its root does, or a node
/// under it does (`children`).
pub fn layered(p: Paint, relative: bool, children: bool) -> bool {
    !p.outside && (p.positioned || p.stacks || relative || children)
}

/// What an ahead-of-time build knows of a node beyond its template rows.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dynamic {
    /// Its dynamic rows' and props' paint, joined with its literal ones'.
    pub paint: Paint,
    /// A dynamic `position` may make it absolute: its parent holds it.
    pub absolute: bool,
    /// An `each` row's root, which follows its own earlier copies.
    pub repeated: bool,
}

/// The views the page makes `relative` in a template tree (every arm of a
/// region a sibling), with each node's [`Dynamic`] facts: the decisions
/// the live host makes for any tree the template can build.
pub fn relatives(
    kernel: &Kernel,
    roots: &[ViewId],
    dynamic: &dyn Fn(ViewId) -> Dynamic,
) -> std::collections::BTreeSet<ViewId> {
    fn walk(
        kernel: &Kernel,
        id: ViewId,
        after: bool,
        dynamic: &dyn Fn(ViewId) -> Dynamic,
        out: &mut std::collections::BTreeSet<ViewId>,
    ) -> bool {
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let (own, d) = (paint(&node), dynamic(id));
        let p = Paint {
            positioned: own.positioned || d.paint.positioned,
            stacks: own.stacks || d.paint.stacks,
            insets: own.insets || d.paint.insets,
            outside: own.outside,
        };
        let children = node.children();
        let holds = holds_absolute(kernel, &node) || children.iter().any(|c| dynamic(*c).absolute);
        let mut under = false;
        for child in &children {
            under |= walk(kernel, *child, under, dynamic, out);
        }
        let rel = relative(p, holds, after || (d.repeated && layered(p, false, under)));
        if rel {
            out.insert(id);
        }
        layered(p, rel, under)
    }
    let mut out = std::collections::BTreeSet::new();
    let mut after = false;
    for root in roots {
        after |= walk(kernel, *root, after, dynamic, &mut out);
    }
    out
}

/// `css` with the page's `relative` when the node takes it.
pub fn with_relative(mut css: String, relative: bool) -> String {
    if relative {
        css.push_str("position:relative;");
    }
    css
}

/// What the live host knows of one node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    paint: Paint,
    /// An absolute child's containing block.
    holds: bool,
    /// A node under it paints with the positioned.
    children: bool,
    /// The page makes it `relative`.
    relative: bool,
}

/// Every live node's [`Facts`], by view.
#[derive(Default)]
pub(crate) struct Layers(IdMap<ViewId, Facts>);

impl Layers {
    /// Whether the page makes `id` relative.
    pub(crate) fn relative(&self, id: ViewId) -> bool {
        self.0.get(&id).is_some_and(|f| f.relative)
    }

    pub(crate) fn forget(&mut self, id: ViewId) {
        self.0.remove(&id);
    }
}

impl<D: DataSource> Host<D> {
    /// Before a batch's creates and updates: the paint of every node it
    /// creates or updates (`changed`, the final tree's views), then each
    /// sibling list one of them may move, deepest first, so a parent reads
    /// its children's final facts. A node the batch neither creates nor
    /// updates whose `relative` moved is restyled here; the rest take theirs
    /// in `create` and `update`.
    pub(super) fn relayer(&mut self, changed: &[ViewId], batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let mut lists = std::collections::BinaryHeap::new();
        let mut queued = std::collections::HashSet::new();
        let depth = |id: Option<ViewId>| {
            let mut n = 0u32;
            let mut at = id.and_then(|id| kernel.node(id));
            while let Some(node) = at {
                n += 1;
                at = node.parent.and_then(|p| kernel.node(p));
            }
            n
        };
        let mut queue = |id: Option<ViewId>, lists: &mut std::collections::BinaryHeap<_>| {
            if queued.insert(id) {
                lists.push((depth(id), id));
            }
        };
        for id in changed {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            let p = paint(&node);
            // Its own children may have moved; its siblings, if it is new
            // or its paint moved.
            queue(Some(*id), &mut lists);
            let was = self.layers.0.insert(
                *id,
                Facts {
                    paint: p,
                    ..Facts::default()
                },
            );
            if let Some(was) = was {
                let facts = self.layers.0.get_mut(id).expect("inserted");
                (facts.holds, facts.children, facts.relative) =
                    (was.holds, was.children, was.relative);
            }
            if was.is_none_or(|w| w.paint != p) {
                queue(node.parent, &mut lists);
            }
        }
        let changed: std::collections::HashSet<ViewId> = changed.iter().copied().collect();
        let mut restyle = Vec::new();
        while let Some((_, list)) = lists.pop() {
            let children = match list {
                Some(id) => kernel.node(id).map(|n| n.children()).unwrap_or_default(),
                None => self.page_roots(),
            };
            let (mut after, mut holds) = (false, false);
            for child in children {
                let Some(node) = kernel.node(child) else {
                    continue;
                };
                let facts = self.layers.0.entry(child).or_insert_with(|| Facts {
                    paint: paint(&node),
                    ..Facts::default()
                });
                if facts.paint.outside {
                    continue;
                }
                holds |= node.style.position_type == PositionType::Absolute;
                let rel = relative(facts.paint, facts.holds, after);
                if std::mem::replace(&mut facts.relative, rel) != rel && !changed.contains(&child) {
                    restyle.push(child);
                }
                after |= layered(facts.paint, rel, facts.children);
            }
            let Some(id) = list else { continue };
            let Some(facts) = self.layers.0.get_mut(&id) else {
                continue;
            };
            if (facts.holds, facts.children) != (holds, after) {
                (facts.holds, facts.children) = (holds, after);
                queue(kernel.node(id).and_then(|n| n.parent), &mut lists);
            }
        }
        for id in restyle {
            let Some((node, m)) = kernel.node(id).zip(self.mirror.get_mut(&id)) else {
                continue;
            };
            let (text, _) = css::css_text(&css_style(kernel, &node), &self.font_names);
            let css = with_relative(
                host_css(&node, text, tag_for(&node, m.in_button)),
                self.layers.relative(id),
            );
            if css != m.css {
                batch.style(id, &css);
                m.css = css;
            }
        }
    }
}
