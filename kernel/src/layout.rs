//! Layout: the engine wrapper and frame publication.
//!
//! [`LayoutTree`] owns the Taffy tree as a derived structure over the arena's
//! columns. Measured leaves carry their slot and a bounded offer cache as the
//! Taffy node context. The measure closure hands the host measurer the leaf's
//! runs only when that offer is missing. After a pass,
//! [`compute`] walks the root, publishes absolute frames into the arena's frame
//! column, and returns exactly the nodes whose frame bits changed — the
//! changed-geometry receipt.
//!
//! An engine fault is never a panic: it is recorded, reported as
//! [`LayoutError::Engine`], and the kernel rebuilds the tree from the columns.

use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use taffy::tree::MeasureOutput;

use crate::arena::NodeArena;
use crate::error::LayoutError;
use crate::generated::{FieldSizing, NodeType, StyleMask};
use crate::id::{AxisOffer, Frame, NodeFlags, NodeKey, Offer};
use crate::style::taffy_style;
use crate::text::{Paragraph, TextMeasureRequest, TextMeasurer, TextMetrics, TextRun};

/// What a layout pass changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutReceipt {
    /// The kernel epoch the frames belong to.
    pub epoch: u64,
    /// The root that was laid out.
    pub root: NodeKey,
    /// Every node whose absolute frame changed, in preorder.
    pub changed: Vec<NodeKey>,
}

fn to_available(offer: AxisOffer) -> AvailableSpace {
    match offer {
        AxisOffer::Definite(v) => AvailableSpace::Definite(v),
        AxisOffer::MaxContent => AvailableSpace::MaxContent,
        AxisOffer::MinContent => AvailableSpace::MinContent,
    }
}

fn from_available(space: AvailableSpace) -> AxisOffer {
    match space {
        AvailableSpace::Definite(v) => AxisOffer::Definite(v),
        AvailableSpace::MaxContent => AxisOffer::MaxContent,
        AvailableSpace::MinContent => AxisOffer::MinContent,
    }
}

struct MeasuredNode {
    slot: u32,
    // At most four offers per live node. Removing/rebuilding an engine node
    // drops these metrics; explicit text/style invalidation clears them.
    measurements: Vec<Measurement>,
}

#[derive(Clone, Copy)]
struct Measurement {
    width: AxisOffer,
    height: AxisOffer,
    metrics: TextMetrics,
}

/// The engine tree. Measured leaves retain a bounded cache of exact offers.
pub struct LayoutTree {
    taffy: TaffyTree<MeasuredNode>,
    fault: Option<String>,
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutTree {
    /// An empty tree.
    pub fn new() -> Self {
        // Frames are CSS pixel geometry, not a host's raster grid. Rounding
        // here loses subpixel edits and snaps Retina views to whole points.
        let mut taffy = TaffyTree::new();
        taffy.disable_rounding();
        LayoutTree { taffy, fault: None }
    }

    fn note(&mut self, what: &str, result: Result<impl Sized, taffy::TaffyError>) {
        if let Err(e) = result {
            if self.fault.is_none() {
                self.fault = Some(format!("{what}: {e:?}"));
            }
        }
    }

    /// Whether the engine reported a fault since the last rebuild.
    pub fn faulted(&self) -> bool {
        self.fault.is_some()
    }

    /// Engine nodes.
    pub fn node_count(&self) -> usize {
        self.taffy.total_node_count()
    }

    /// Allocate a leaf; `measured` leaves carry their slot for the measure closure.
    pub fn new_leaf(&mut self, style: taffy::style::Style, slot: u32, measured: bool) -> NodeId {
        let result = if measured {
            self.taffy.new_leaf_with_context(
                style,
                MeasuredNode {
                    slot,
                    measurements: Vec::new(),
                },
            )
        } else {
            self.taffy.new_leaf(style)
        };
        match result {
            Ok(node) => node,
            Err(e) => {
                // Unreachable: leaf allocation cannot fail. Record it and hand back a
                // placeholder that every later call reports as a fault.
                self.fault.get_or_insert_with(|| format!("new_leaf: {e:?}"));
                NodeId::from(usize::MAX)
            }
        }
    }

    /// Remove a node.
    pub fn remove(&mut self, node: NodeId) {
        let r = self.taffy.remove(node);
        self.note("remove", r);
    }

    /// Replace a node's style (marks it dirty).
    pub fn set_style(&mut self, node: NodeId, style: taffy::style::Style) {
        self.clear_measurements(node);
        let r = self.taffy.set_style(node, style);
        self.note("set_style", r);
    }

    /// Replace a node's ordered children.
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) {
        self.clear_measurements(parent);
        let r = self.taffy.set_children(parent, children);
        self.note("set_children", r);
    }

    /// Mark a node (and its ancestors) dirty.
    pub fn mark_dirty(&mut self, node: NodeId) {
        self.clear_measurements(node);
        let r = self.taffy.mark_dirty(node);
        self.note("mark_dirty", r);
    }

    fn clear_measurements(&mut self, node: NodeId) {
        if let Some(context) = self.taffy.get_node_context_mut(node) {
            context.measurements.clear();
        }
    }

    /// Whether a node needs layout.
    pub fn is_dirty(&self, node: NodeId) -> bool {
        self.taffy.dirty(node).unwrap_or(true)
    }

    /// The engine's layout for a node, relative to its parent.
    pub fn layout(&self, node: NodeId) -> taffy::tree::Layout {
        self.taffy.layout(node).copied().unwrap_or_default()
    }

    /// Run the engine on `root` under `offer`.
    pub fn compute(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<(), LayoutError> {
        if let Some(fault) = &self.fault {
            return Err(LayoutError::Engine(fault.clone()));
        }
        let available = Size {
            width: to_available(offer.width),
            height: to_available(offer.height),
        };
        let mut runs: Vec<TextRun<'_>> = Vec::new();
        let mut invalid_metrics = None;
        let result = self.taffy.compute_layout_with_measure_and_baselines(
            root,
            available,
            |known: Size<Option<f32>>,
             space: Size<AvailableSpace>,
             _node,
             context: Option<&mut MeasuredNode>,
             _style| {
                let Some(context) = context else {
                    return MeasureOutput::ZERO;
                };
                let slot = context.slot;
                if arena.node_type(slot).is_replaced() {
                    // A replaced element: its intrinsic size where nothing
                    // is known (Taffy has already applied the aspect ratio
                    // to a known dimension); nothing at all before it loads,
                    // as a broken `<img>` is 0×0.
                    let Some((iw, ih)) = arena.intrinsic(slot).or_else(|| {
                        (arena.node_type(slot) == NodeType::Video).then_some((300.0, 150.0))
                    }) else {
                        return MeasureOutput::ZERO;
                    };
                    return MeasureOutput {
                        size: Size {
                            width: known.width.unwrap_or(iw),
                            height: known.height.unwrap_or(ih),
                        },
                        first_baselines: taffy::geometry::Point { x: None, y: None },
                    };
                }
                let width = if arena.node_type(slot) == NodeType::TextInput
                    && arena.style(slot).field_sizing == FieldSizing::Fixed
                {
                    // A control's preferred row count does not increase when
                    // CSS constrains its width below the preferred columns.
                    AxisOffer::MaxContent
                } else {
                    known
                        .width
                        .map(AxisOffer::Definite)
                        .unwrap_or_else(|| from_available(space.width))
                };
                let height = known
                    .height
                    .map(AxisOffer::Definite)
                    .unwrap_or_else(|| from_available(space.height));
                // Parent layout can revisit a leaf under the same text offer
                // after discarding its broader Taffy layout cache. Reuse the
                // metrics before flattening runs or crossing into the host.
                // Height is part of the key: an injected measurer may use it.
                let metrics = if let Some(cached) = context
                    .measurements
                    .iter()
                    .find(|m| m.width == width && m.height == height)
                {
                    cached.metrics
                } else {
                    runs.clear();
                    arena.text_runs(slot, &mut runs);
                    if runs.is_empty() {
                        return MeasureOutput::ZERO;
                    }
                    let metrics = measurer.measure(&TextMeasureRequest {
                        runs: &runs,
                        paragraph: Paragraph::from_style(
                            &arena.computed_style(slot, StyleMask::INHERITED),
                        ),
                        width,
                        height,
                    });
                    if !metrics.is_valid() {
                        invalid_metrics.get_or_insert_with(|| arena.local_id(slot));
                        return MeasureOutput::ZERO;
                    }
                    if context.measurements.len() == 4 {
                        context.measurements.remove(0);
                    }
                    context.measurements.push(Measurement {
                        width,
                        height,
                        metrics,
                    });
                    metrics
                };
                MeasureOutput {
                    size: Size {
                        width: known.width.unwrap_or(metrics.width),
                        height: known.height.unwrap_or(metrics.height),
                    },
                    first_baselines: taffy::geometry::Point {
                        x: None,
                        y: metrics.first_baseline,
                    },
                }
            },
        );
        result.map_err(|e| LayoutError::Engine(format!("compute_layout: {e:?}")))?;
        if let Some(view) = invalid_metrics {
            return Err(LayoutError::InvalidTextMetrics(view));
        }
        Ok(())
    }

    /// Reconstruct the whole engine tree from the arena's columns, writing the
    /// new handles back. This is rehydration: columns plus rebuild.
    pub fn rebuild(arena: &mut NodeArena) -> LayoutTree {
        let mut tree = LayoutTree::new();
        arena.clear_taffy();
        let slots: Vec<u32> = arena.iter_live().collect();
        for slot in &slots {
            let node = tree.new_leaf(
                taffy_style(arena, *slot),
                *slot,
                arena.node_type(*slot).is_measured_leaf(),
            );
            arena.set_taffy(*slot, Some(node));
        }
        for slot in &slots {
            if arena.node_type(*slot) == NodeType::Text {
                continue;
            }
            let children: Vec<NodeId> = arena
                .children(*slot)
                .iter()
                .filter_map(|c| arena.taffy(*c))
                .collect();
            if let Some(node) = arena.taffy(*slot) {
                tree.set_children(node, &children);
            }
        }
        tree
    }
}

const PASS_CLEARS: [NodeFlags; 4] = [
    NodeFlags::CREATED,
    NodeFlags::STYLE_DIRTY,
    NodeFlags::TEXT_DIRTY,
    NodeFlags::CHILDREN_DIRTY,
];

/// Lay out `root_slot` and publish frames. Returns the slots whose frame bits
/// changed (or that were laid out for the first time), in preorder. Inline
/// runs have no geometry of their own: their frames are zero and they never
/// appear in the changed list, but their dirty bits are consumed like any
/// other node's.
pub fn compute(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root_slot: u32,
    offer: Offer,
) -> Result<Vec<u32>, LayoutError> {
    let root = arena
        .taffy(root_slot)
        .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
    tree.compute(root, offer, arena, measurer)?;

    let mut changed = Vec::new();
    let mut stack: Vec<(u32, f32, f32)> = vec![(root_slot, 0.0, 0.0)];
    while let Some((slot, ox, oy)) = stack.pop() {
        let frame = if arena.is_inline_run(slot) {
            Frame::default()
        } else {
            let Some(node) = arena.taffy(slot) else {
                continue;
            };
            let l = tree.layout(node);
            arena.set_content(slot, (l.content_size.width, l.content_size.height));
            Frame {
                x: ox + l.location.x,
                y: oy + l.location.y,
                width: l.size.width,
                height: l.size.height,
            }
        };
        let first = arena.flags(slot).has(NodeFlags::CREATED);
        let moved = !arena.is_inline_run(slot) && (first || !arena.frame(slot).bits_eq(frame));
        arena.set_frame(slot, frame);
        let flags = arena.flags_mut(slot);
        for clear in PASS_CLEARS {
            flags.remove(clear);
        }
        if moved {
            flags.insert(NodeFlags::GEOMETRY_CHANGED);
            changed.push(slot);
        } else {
            flags.remove(NodeFlags::GEOMETRY_CHANGED);
        }
        for child in arena.children(slot).iter().rev() {
            stack.push((*child, frame.x, frame.y));
        }
    }
    Ok(changed)
}
