use super::{RegionFrame, RegionGeometry, RegionOffset, OWNER, REGION_NODES};
use crate::{
    arena::NodeArena, layout::LayoutTree, style::taffy_style, Frame, LayoutError, NodeType, Offer,
    TextMeasurer,
};
use std::collections::HashMap;
use taffy::prelude::NodeId;

/// Entirely local handles. Arena's ordinary engine mapping is never touched.
pub(super) struct Derived {
    tree: LayoutTree,
    slots: Vec<u32>,
    nodes: HashMap<u32, NodeId>,
    root: NodeId,
}
impl Derived {
    pub fn build(
        arena: &NodeArena,
        root: u32,
        cut: Option<u32>,
        branch: Option<u32>,
        bounded: bool,
    ) -> Result<Self, LayoutError> {
        let mut tree = LayoutTree::new();
        let mut slots = Vec::new();
        let mut nodes = HashMap::new();
        let mut stack = vec![root];
        while let Some(s) = stack.pop() {
            if bounded && slots.len() == REGION_NODES {
                return Err(LayoutError::ContentRegion("mounted node limit"));
            }
            slots.push(s);
            nodes.insert(
                s,
                tree.new_leaf(
                    taffy_style(arena, s),
                    s,
                    arena.node_type(s).is_measured_leaf(),
                ),
            );
            if Some(s) == cut {
                if let Some(b) = branch {
                    stack.push(b)
                }
            } else {
                stack.extend(arena.children(s).iter().rev().copied());
            }
        }
        for &s in &slots {
            if arena.node_type(s) == NodeType::Text {
                continue;
            }
            let children = if Some(s) == cut {
                branch
                    .into_iter()
                    .filter_map(|b| nodes.get(&b).copied())
                    .collect()
            } else {
                arena
                    .children(s)
                    .iter()
                    .filter_map(|c| nodes.get(c).copied())
                    .collect::<Vec<_>>()
            };
            tree.set_children(nodes[&s], &children);
        }
        let root = nodes[&root];
        Ok(Self {
            tree,
            slots,
            nodes,
            root,
        })
    }
    /// Retain the actual owner as the containing block. It is not an arena root
    /// and receives no global root auto-width repair. Only its external resolved
    /// size/location is supplied; child lowering/inheritance is unchanged.
    pub fn constrain_owner(&mut self, arena: &NodeArena, slot: u32, frame: Frame) {
        let mut style = taffy_style(arena, slot);
        style.size = taffy::geometry::Size {
            width: taffy::style::Dimension::length(frame.width),
            height: taffy::style::Dimension::length(frame.height),
        };
        style.min_size = taffy::geometry::Size {
            width: taffy::style::LengthPercentageAuto::length(frame.width),
            height: taffy::style::LengthPercentageAuto::length(frame.height),
        };
        style.max_size = style.min_size;
        style.margin = taffy::geometry::Rect {
            left: taffy::style::LengthPercentageAuto::length(0.),
            right: taffy::style::LengthPercentageAuto::length(0.),
            top: taffy::style::LengthPercentageAuto::length(0.),
            bottom: taffy::style::LengthPercentageAuto::length(0.),
        };
        style.inset = taffy::geometry::Rect {
            left: taffy::style::LengthPercentageAuto::auto(),
            right: taffy::style::LengthPercentageAuto::auto(),
            top: taffy::style::LengthPercentageAuto::auto(),
            bottom: taffy::style::LengthPercentageAuto::auto(),
        };
        self.tree.set_style(self.root, style);
    }
    pub fn compute(
        &mut self,
        arena: &NodeArena,
        m: &mut dyn TextMeasurer,
        offer: Offer,
    ) -> Result<(), LayoutError> {
        self.tree
            .compute_mapped(self.root, offer, arena, m, |s| self.nodes.get(&s).copied())
    }
    pub fn paint_offers(&self, arena: &NodeArena) -> Vec<(u32, f32)> {
        self.slots
            .iter()
            .filter_map(|&s| {
                if !arena.node_type(s).is_measured_leaf()
                    || arena.node_type(s) == NodeType::Image
                    || arena.node_type(s) == NodeType::Control
                    || arena.is_inline_run(s)
                {
                    return None;
                }
                let mut ancestor = Some(s);
                while let Some(a) = ancestor {
                    if arena.style(a).display == crate::Display::None {
                        return None;
                    }
                    ancestor = arena.parent(a);
                }
                let l = self.tree.layout(self.nodes[&s]);
                Some((
                    s,
                    (l.size.width
                        - l.padding.left
                        - l.padding.right
                        - l.border.left
                        - l.border.right)
                        .max(0.),
                ))
            })
            .collect()
    }
    pub fn frames(
        &self,
        arena: &NodeArena,
        root: u32,
        cut: Option<u32>,
        branch: Option<u32>,
    ) -> Result<RegionGeometry, LayoutError> {
        let mut frames = Vec::with_capacity(self.slots.len());
        let mut offsets = Vec::with_capacity(self.slots.len());
        let mut stack = vec![(root, 0., 0., OWNER)];
        while let Some((s, x, y, parent)) = stack.pop() {
            let l = self.tree.layout(self.nodes[&s]);
            let inline = arena.is_inline_run(s);
            let frame = if inline {
                Frame::default()
            } else {
                Frame {
                    x: x + l.location.x,
                    y: y + l.location.y,
                    width: l.size.width,
                    height: l.size.height,
                }
            };
            if ![
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                l.scrollable_overflow_rect.right,
                l.scrollable_overflow_rect.bottom,
            ]
            .into_iter()
            .all(f32::is_finite)
            {
                return Err(LayoutError::ContentRegion("nonfinite candidate geometry"));
            }
            // The constrained owner supplies only the external origin/size.
            // Omit it from publication, keeping direct-child parent=OWNER.
            let next_parent = if s == root {
                if frame.x != 0. || frame.y != 0. || inline {
                    return Err(LayoutError::ContentRegion("nonzero derived owner origin"));
                }
                OWNER
            } else {
                if frames.len() == REGION_NODES {
                    return Err(LayoutError::ContentRegion("mounted node limit"));
                }
                let ordinal = frames.len() as u32;
                frames.push(RegionFrame {
                    node: arena.key(s),
                    frame,
                    content: (
                        l.scrollable_overflow_rect.right,
                        l.scrollable_overflow_rect.bottom,
                    ),
                    height_measured: self.tree.height_measured(self.nodes[&s]),
                });
                offsets.push(RegionOffset {
                    parent,
                    x: l.location.x,
                    y: l.location.y,
                    inline,
                });
                ordinal
            };
            if Some(s) == cut {
                if let Some(b) = branch {
                    stack.push((b, frame.x, frame.y, next_parent))
                }
            } else {
                for &c in arena.children(s).iter().rev() {
                    stack.push((c, frame.x, frame.y, next_parent))
                }
            }
        }
        Ok(RegionGeometry { frames, offsets })
    }
}

/// Compute using the existing shell tree, staging frames without arena writes.
/// The candidate tree above has a separate map and cannot overwrite these IDs.
pub(super) fn shell(
    arena: &NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root: u32,
    cut: u32,
    offer: Offer,
) -> Result<Vec<RegionFrame>, LayoutError> {
    let engine_root = arena
        .taffy(root)
        .ok_or_else(|| LayoutError::Engine("shell root absent".into()))?;
    let owner = arena
        .taffy(cut)
        .ok_or_else(|| LayoutError::Engine("region owner absent".into()))?;
    tree.cut_children(owner);
    tree.compute(engine_root, offer, arena, measurer)?;
    let mut frames = Vec::new();
    let mut stack = vec![(root, 0., 0.)];
    while let Some((s, x, y)) = stack.pop() {
        let n = arena
            .taffy(s)
            .ok_or_else(|| LayoutError::Engine("shell node absent".into()))?;
        let l = tree.layout(n);
        let frame = if arena.is_inline_run(s) {
            Frame::default()
        } else {
            Frame {
                x: x + l.location.x,
                y: y + l.location.y,
                width: l.size.width,
                height: l.size.height,
            }
        };
        if ![
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            l.scrollable_overflow_rect.right,
            l.scrollable_overflow_rect.bottom,
        ]
        .into_iter()
        .all(f32::is_finite)
        {
            return Err(LayoutError::ContentRegion("nonfinite shell geometry"));
        }
        frames.push(RegionFrame {
            node: arena.key(s),
            frame,
            content: (
                l.scrollable_overflow_rect.right,
                l.scrollable_overflow_rect.bottom,
            ),
            height_measured: tree.height_measured(n),
        });
        if s != cut {
            for &c in arena.children(s).iter().rev() {
                stack.push((c, frame.x, frame.y))
            }
        }
    }
    Ok(frames)
}
