//! Which nodes the page makes `isolation: isolate` (LLP 1001 §1, LLP 1074 T1).
//!
//! The kernel paints in tree order: a box paints over everything before it
//! in the tree. A page paints its positioned boxes and its stacking contexts
//! after its in-flow boxes (CSS 2 Appendix E), so a static box that follows
//! one of those in tree order — a positioned box or a stacking context
//! anywhere in an earlier sibling's subtree — would paint under it. The page
//! makes such a box a stacking context, which paints in tree order with them
//! and, unlike `position: relative`, is no containing block: `position` is
//! the plan's own, and an absolutely positioned box is placed against its
//! nearest positioned ancestor on the page as in the kernel.
//!
//! The live host keeps each node's facts and re-decides a sibling list only
//! when a node in it changed ([`Host::relayer`]); the document writer and an
//! ahead-of-time build decide the same with [`isolated`] and [`layered`].

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
    /// Its `position` is not `static`, or the host positions it (a canvas,
    /// whose surface it holds; a Markdown editor, whose lines' markers it
    /// holds).
    pub positioned: bool,
    /// May make a stacking context, which paints with the positioned: an
    /// opacity, a transform, a filter, a clip or mask, a blend, an
    /// isolation, a transition or animation (which may move them), a press's
    /// scale, a material's backdrop, a navigation screen or a modal.
    pub stacks: bool,
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
/// A node's [`Paint`].
pub fn paint(kernel: &Kernel, node: &NodeRef<'_>) -> Paint {
    let parent = node.parent.and_then(|id| kernel.node(id));
    paint_of(&node.facts(), parent.map(|n| n.style.display))
}

/// [`paint`], from a node's facts.
pub fn paint_of(
    node: &exact_kernel::NodeFacts<'_>,
    parent: Option<exact_kernel::Display>,
) -> Paint {
    let m = &node.style.mask;
    let props = node.props;
    let editor =
        node.node_type == NodeType::TextInput && props.str(PropId::Markup) == Some("markdown");
    Paint {
        positioned: node.style.position_type != PositionType::Static
            || node.node_type == NodeType::Canvas
            || editor,
        stacks: (matches!(
            parent,
            Some(exact_kernel::Display::Flex | exact_kernel::Display::Grid)
        ) && m.has(StyleId::ZIndex))
            || STACKS.iter().any(|s| m.has(*s))
            || props.str(PropId::BackgroundMaterial).is_some()
            || props.str(PropId::NavigationKey).is_some()
            || props.str(PropId::NavigationPresentation) == Some("modal"),
        outside: node.node_type.is_svg_element() || node.node_type.is_metadata(),
    }
}

/// Whether the page isolates a node: it is static and no stacking context,
/// and something before it paints with the positioned (`after`), which it
/// would otherwise paint under.
pub fn isolated(p: Paint, after: bool) -> bool {
    !p.outside && !p.positioned && !p.stacks && after
}

/// Whether a subtree paints with the positioned: its root does, or a node
/// under it does (`children`).
pub fn layered(p: Paint, isolated: bool, children: bool) -> bool {
    !p.outside && (p.positioned || p.stacks || isolated || children)
}

/// What an ahead-of-time build knows of a node beyond its template rows.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dynamic {
    /// Its dynamic rows' and props' paint, joined with its literal ones'.
    pub paint: Paint,
    /// An `each` row's root, which follows its own earlier copies.
    pub repeated: bool,
}

/// The views the page isolates in a template tree (every arm of a region a
/// sibling), with each node's [`Dynamic`] facts: the decisions the live host
/// makes for any tree the template can build.
pub fn isolates(
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
        let (own, d) = (paint(kernel, &node), dynamic(id));
        let p = Paint {
            positioned: own.positioned || d.paint.positioned,
            stacks: own.stacks || d.paint.stacks,
            outside: own.outside,
        };
        let children = node.children();
        let mut under = false;
        for child in &children {
            under |= walk(kernel, *child, under, dynamic, out);
        }
        let own = isolated(p, after || (d.repeated && layered(p, false, under)));
        if own {
            out.insert(id);
        }
        layered(p, own, under)
    }
    let mut out = std::collections::BTreeSet::new();
    let mut after = false;
    for root in roots {
        after |= walk(kernel, *root, after, dynamic, &mut out);
    }
    out
}

/// `css` with the page's `isolation` when the node takes it.
pub fn with_isolation(mut css: String, isolated: bool) -> String {
    if isolated {
        css.push_str("isolation:isolate;");
    }
    css
}

/// What the live host knows of one node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    paint: Paint,
    /// A node under it paints with the positioned.
    children: bool,
    /// The page isolates it.
    isolated: bool,
}

/// Every live node's [`Facts`], by view.
#[derive(Default)]
pub(crate) struct Layers(IdMap<ViewId, Facts>);

impl Layers {
    /// Whether the page isolates `id`.
    pub(crate) fn isolated(&self, id: ViewId) -> bool {
        self.0.get(&id).is_some_and(|f| f.isolated)
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
    /// updates whose isolation moved is restyled here; the rest take theirs
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
            let p = paint(kernel, &node);
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
                (facts.children, facts.isolated) = (was.children, was.isolated);
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
            let mut after = false;
            for child in children {
                let Some(node) = kernel.node(child) else {
                    continue;
                };
                let facts = self.layers.0.entry(child).or_insert_with(|| Facts {
                    paint: paint(kernel, &node),
                    ..Facts::default()
                });
                // A parent display change or a reparent changes flex/grid item paint.
                facts.paint = paint(kernel, &node);
                if facts.paint.outside {
                    continue;
                }
                let own = isolated(facts.paint, after);
                if std::mem::replace(&mut facts.isolated, own) != own && !changed.contains(&child) {
                    restyle.push(child);
                }
                after |= layered(facts.paint, own, facts.children);
            }
            let Some(id) = list else { continue };
            let Some(facts) = self.layers.0.get_mut(&id) else {
                continue;
            };
            if facts.children != after {
                facts.children = after;
                queue(kernel.node(id).and_then(|n| n.parent), &mut lists);
            }
        }
        for id in restyle {
            let Some((node, m)) = kernel.node(id).zip(self.mirror.get_mut(&id)) else {
                continue;
            };
            let (text, _) = css::css_text(&css_style(kernel, &node), &self.font_names);
            let css = with_isolation(
                host_css(&node, text, tag_for(&node, m.in_button)),
                self.layers.isolated(id),
            );
            if css != m.css {
                batch.style(id, &css);
                m.css = css;
            }
        }
    }
}
