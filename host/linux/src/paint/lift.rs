//! What a reorder lifts above the page (LLP 1041 §8.5, LLP 1094 D6): a row
//! carried within its own list, painted after its siblings inside that
//! list's clip; and a grouped drag's ghost, the row's subtree painted last,
//! over everything, under no ancestor's clip, at the contact.
//!
//! The ghost is the row itself, painted a second time where the contact
//! holds it: the runner hides the row in its list while the ghost stands
//! for it (`visibility: hidden` on its wrapper), so the ghost's walk shows
//! what that hides. Its boxes are not hit: a ghost is the web's
//! `pointer-events: none` popover.
use super::shadow::ShadowPaint;
use super::{Painter, Rect4, Shape, Walk};
use exact_kernel::{NodeKey, NodeRef, NodeType, PropId, ViewId};
use tiny_skia::Transform;

/// The lifted rows the presenter asks the painter for.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Lift {
    /// Within one list: the list and the carried row's wrapper.
    pub(crate) arrange: Option<(NodeKey, NodeKey)>,
    /// A grouped drag's ghost.
    pub(crate) ghost: Option<Ghost>,
}

/// A grouped drag's ghost (LLP 1094 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ghost {
    /// The wrapper whose subtree it paints: the row wherever it is now.
    pub(crate) wrapper: NodeKey,
    /// Its border box's top left, viewport points.
    pub(crate) at: (f32, f32),
    /// 1.03 while lifted (1 under reduced motion), easing to 1 as it lands.
    pub(crate) scale: f32,
    /// 1, or fading.
    pub(crate) opacity: f32,
    /// The lifted shadow's strength, 0 to 1, easing out as it lands.
    pub(crate) shadow: f32,
}

impl Painter {
    /// A node's children in paint order: a canvas's placed children first,
    /// then the rest by rank and tree order, and the row its list carries
    /// last, inside the list's clip.
    pub(super) fn children(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        ts: Transform,
        child_offset: (f32, f32),
        child_rect: Option<Rect4>,
    ) {
        let lift = self.lifted_child(walk, node);
        // A native button's children are its face, painted above (LLP 1069.011 D5).
        let native =
            node.node_type == NodeType::Control && node.props.str(PropId::Type) == Some("button");
        let children: Vec<_> = node
            .children()
            .into_iter()
            .filter(|id| Some(*id) != lift && !native)
            .collect();
        // @ref LLP 1083.000 D5 — a canvas's placed children first, by
        // projective depth, as the web gives them negative indices by
        // depth; then every other child by its rank, then tree order.
        let order: Vec<(ViewId, usize)> = children.iter().copied().zip(0..).collect();
        let mut order = order;
        order.sort_by(
            |(a, i), (b, j)| match (self.placements.get(a), self.placements.get(b)) {
                (Some(a), Some(b)) => a.depth().total_cmp(&b.depth()).then(i.cmp(j)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => {
                    let rank = |id: &ViewId| walk.ranks.get(id).copied().unwrap_or(0);
                    rank(a).cmp(&rank(b)).then(i.cmp(j))
                }
            },
        );
        let children: Vec<ViewId> = order.into_iter().map(|(id, _)| id).collect();
        // A scroller's rows are its children; a scroller holding one
        // container (a column of settings) keeps that container's children
        // apart instead, so one of them changing records only itself.
        let rows = self.has_rows(node);
        let group = rows && !self.rows_wrapper(node.key);
        let wrap = group && self.wraps_rows(walk, &children);
        if group {
            self.group_begin(walk, node.id);
        }
        for child in children {
            if wrap {
                self.wrapped(walk, child, ts, child_offset, child_rect);
            } else if rows {
                self.row(walk, child, ts, child_offset, child_rect);
            } else {
                self.node(walk, child, ts, child_offset, child_rect);
            }
        }
        if group {
            self.group_end(walk);
        }
        if let Some(child) = lift {
            self.node(walk, child, ts, child_offset, child_rect);
        }
    }

    /// The row a list carries within itself, painted after its siblings.
    fn lifted_child(&self, walk: &Walk<'_, '_>, list: &NodeRef<'_>) -> Option<ViewId> {
        let (owner, row) = self.lift.arrange?;
        let source = walk.scene.kernel.node_by_key(row)?;
        (owner == list.key && source.parent == Some(list.id)).then_some(source.id)
    }

    /// The ghost, over everything painted so far.
    pub(super) fn paint_ghost(&mut self, walk: &mut Walk<'_, '_>) {
        let Some(ghost) = self.lift.ghost else {
            return;
        };
        let Some(node) = walk.scene.kernel.node_by_key(ghost.wrapper) else {
            return;
        };
        let (w, h) = (node.frame.width, node.frame.height);
        let (cx, cy) = (ghost.at.0 + w / 2., ghost.at.1 + h / 2.);
        let ts = Transform::from_translate(cx, cy)
            .pre_scale(ghost.scale, ghost.scale)
            .pre_translate(-cx, -cy);
        let opacity = ghost.opacity.clamp(0., 1.);
        if opacity < 1. {
            self.backend.push_opacity(opacity);
        }
        // D6's look, the same on every host: `0 8px 24px` at 25% black.
        let rect: Rect4 = (ghost.at.0, ghost.at.1, w, h);
        if ghost.shadow > 0. {
            let shadow = ShadowPaint::lifted(ghost.shadow);
            for fill in shadow.fills(&Shape::rect(rect), [0.; 4]) {
                self.backend.fill_border(&fill, ts);
            }
        }
        let boxes = walk.boxes.len();
        let (reveal, page) = (walk.reveal, (node.frame.x - rect.0, node.frame.y - rect.1));
        walk.reveal = Some(node.id);
        // Painted after the walk, out of its ancestors: the row's own scheme
        // (LLP 1034 §8), else the app's.
        let previous = self.dark;
        self.dark = node.color_scheme_dark().unwrap_or(previous);
        self.node(walk, node.id, ts, page, None);
        self.dark = previous;
        walk.reveal = reveal;
        walk.boxes.truncate(boxes);
        if opacity < 1. {
            self.backend.pop_opacity();
        }
    }
}
