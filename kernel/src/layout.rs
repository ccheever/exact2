//! Layout: the engine wrapper and frame publication.
//!
//! [`LayoutTree`] owns the Taffy tree as a derived structure over the arena's
//! columns. Text leaves carry their slot as the Taffy node context so the
//! measure closure can hand the host measurer the leaf's runs. After a pass,
//! [`compute`] walks the root, publishes absolute frames into the arena's frame
//! column, resolves exclusions into sparse leaf coordinates, and returns
//! independent frame-change and flow-change receipts.
//!
//! An engine fault is never a panic: it is recorded, reported as
//! [`LayoutError::Engine`], and the kernel rebuilds the tree from the columns.

use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use taffy::tree::{Baselines, LayoutInput};
use taffy::util::{MaybeResolve, ResolveOrZero};
use taffy::TraversePartialTree;

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
    /// Leaves whose resolved exclusions changed bitwise, in preorder (LLP 1043.000 D4).
    pub flow_changed: Vec<NodeKey>,
    /// Intersecting paragraphs whose height required measurement; M8 enables flow.
    pub flow_skipped: Vec<NodeKey>,
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

/// Keep the full block/flex offer working set; four slots evicted within one
/// pass and repeated native measurement (LLP 1044 F6).
const LEAF_OFFERS: usize = 16;

// @ref LLP 1043.000 §3 D3 — keep the proof with Taffy's measured leaf.
#[derive(Default)]
struct MeasureContext {
    slot: u32,
    pass: u64,
    height_measured: bool,
    // At most LEAF_OFFERS per live leaf; text/style invalidation clears them.
    measurements: Vec<Measurement>,
}

#[derive(Clone, Copy)]
struct Measurement {
    width: AxisOffer,
    height: AxisOffer,
    metrics: TextMetrics,
}

/// The engine tree. Measured leaves retain their height proof and bounded offers.
pub struct LayoutTree {
    taffy: TaffyTree<MeasureContext>,
    pass: u64,
    fault: Option<String>,
    // One derived height, not an authored target or a second style graph.
    presented_height: Option<(NodeKey, NodeId, f32)>,
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
        LayoutTree {
            taffy,
            pass: 0,
            fault: None,
            presented_height: None,
        }
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
                MeasureContext {
                    slot,
                    ..Default::default()
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
        if self
            .presented_height
            .is_some_and(|(_, active, _)| active == node)
        {
            self.presented_height = None;
        }
        let r = self.taffy.remove(node);
        self.note("remove", r);
    }

    /// Replace authored lowering, retaining an active presentation height.
    /// Dirty only when the resulting full derived style changes. This is also
    /// the path for environment/intrinsic updates that do not bump the epoch.
    pub fn set_style(&mut self, node: NodeId, mut style: taffy::style::Style) {
        if let Some((_, active, px)) = self.presented_height {
            if active == node {
                style.size.height = taffy::style::Dimension::length(px);
            }
        }
        self.write_style(node, style);
    }

    fn write_style(&mut self, node: NodeId, style: taffy::style::Style) {
        if self.taffy.style(node).is_ok_and(|old| *old == style) {
            return;
        }
        self.clear_measurements(node);
        let r = self.taffy.set_style(node, style);
        self.note("set_style", r);
    }

    /// Install a preflighted sample, or restore current authored lowering.
    /// The caller validates generation, membership and eligibility before any
    /// change here; epochs belong to requests, not to this derived cache.
    pub(crate) fn present_height(&mut self, arena: &NodeArena, sample: Option<(u32, f32)>) {
        let next = match sample {
            Some((slot, px)) => {
                let Some(node) = arena.taffy(slot) else {
                    self.fault
                        .get_or_insert_with(|| "presented height has no engine node".into());
                    return;
                };
                Some((arena.key(slot), node, px))
            }
            None => None,
        };
        if self.presented_height == next {
            return;
        }
        if let Some((key, node, _)) = self.presented_height.take() {
            // Same-node samples can replace height directly. Restoring first
            // would dirty twice and momentarily reinstall an obsolete target.
            if next.is_none_or(|(next_key, _, _)| next_key != key) {
                if let Some(slot) = arena.resolve(key) {
                    self.write_style(node, taffy_style(arena, slot));
                }
            }
        }
        if let Some((key, node, px)) = next {
            let mut style = taffy_style(arena, key.index);
            style.size.height = taffy::style::Dimension::length(px);
            self.write_style(node, style);
        }
        self.presented_height = next;
    }

    /// A content region trial does not compose with a height projection yet.
    pub(crate) fn has_presented_height(&self) -> bool {
        self.presented_height.is_some()
    }

    /// Keep a registered region's content out of shell sizing. No-op when
    /// already cut, so unchanged shell layout continues to use its own cache.
    pub(crate) fn cut_children(&mut self, node: NodeId) {
        if self.taffy.child_count(node) > 0 {
            self.set_children(node, &[]);
        }
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
        self.compute_mapped(root, offer, arena, measurer, |s| arena.taffy(s))
    }

    /// Region trial trees supply their local handle map; arena handles belong
    /// solely to the ordinary tree and must never be indexed in a trial tree.
    pub(crate) fn compute_mapped(
        &mut self,
        root: NodeId,
        offer: Offer,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
        node_for: impl Fn(u32) -> Option<NodeId>,
    ) -> Result<(), LayoutError> {
        if let Some(fault) = &self.fault {
            return Err(LayoutError::Engine(fault.clone()));
        }
        // @ref LLP 1043.000 §3 D3 — invalidate only the text whose height
        // proof matters to live wrapping contexts. Clean roots retain their
        // cached proof; hidden, detached and other-root exclusions do no work.
        if self.is_dirty(root) && !arena.exclusion_slots.is_empty() {
            let mut contexts = std::collections::BTreeSet::new();
            for &slot in &arena.exclusion_slots {
                let mut at = Some(slot);
                while let Some(s) = at {
                    if arena.style(s).display == crate::Display::None {
                        break;
                    }
                    let Some(node) = node_for(s) else {
                        break;
                    };
                    if node == root {
                        if let Some(parent) = arena.parent(slot) {
                            contexts.insert(parent);
                        }
                        break;
                    }
                    at = arena
                        .parent(s)
                        .filter(|&p| node_for(p) == self.taffy.parent(node));
                }
            }
            let mut stack: Vec<_> = contexts.into_iter().collect();
            let mut seen = std::collections::BTreeSet::new();
            while let Some(slot) = stack.pop() {
                if !seen.insert(slot)
                    || arena.style(slot).display == crate::Display::None
                    || crate::flow::is_exclusion(arena, slot)
                {
                    continue;
                }
                if arena.node_type(slot) == NodeType::Text && !arena.is_inline_run(slot) {
                    if let Some(node) = node_for(slot) {
                        self.mark_dirty(node); // Taffy clears this leaf and its ancestors.
                        if let Some(context) = self.taffy.get_node_context_mut(node) {
                            context.height_measured = false;
                        }
                    }
                }
                stack.extend(arena.children(slot));
            }
        }
        let available = Size {
            width: to_available(offer.width),
            height: to_available(offer.height),
        };
        self.pass += 1;
        let pass = self.pass;
        let mut runs: Vec<TextRun<'_>> = Vec::new();
        let mut invalid_metrics = None;
        let measure = |inputs: LayoutInput,
                       _node,
                       context: Option<&mut MeasureContext>,
                       style: &taffy::Style| {
            let mut first_baseline = None;
            // @ref LLP 1043.000 §3 D3 — upstream supplies the full layout input.
            // The proof is separate from the ordinary size-only callback offers.
            let height_known = match (inputs.run_mode, inputs.sizing_mode) {
                (taffy::tree::RunMode::PerformLayout, taffy::tree::SizingMode::InherentSize) => {
                    inputs
                        .known_dimensions
                        .height
                        .or_else(|| {
                            style
                                .size
                                .maybe_resolve(inputs.parent_size, |_, _| 0.0)
                                .maybe_apply_aspect_ratio(style.aspect_ratio)
                                .height
                        })
                        .is_some()
                }
                // ContentSize ignores authored dimensions; ComputeSize carries
                // exactly upstream's border-box known dimensions (old Patch 7).
                _ => inputs.known_dimensions.height.is_some(),
            };
            let mut output = taffy::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, space| {
                    let Some(context) = context else {
                        return Size::ZERO;
                    };
                    // Clear on the first actual measurement of this pass. A cache
                    // hit retains the proof of the pass that produced that layout.
                    if context.pass != pass {
                        context.pass = pass;
                        context.height_measured = false;
                    }
                    context.height_measured |= !height_known;
                    let slot = context.slot;
                    if arena.node_type(slot).is_replaced() {
                        // A replaced element: its intrinsic size where nothing
                        // is known (Taffy has already applied the aspect ratio
                        // to a known dimension); nothing at all before it loads,
                        // as a broken `<img>` is 0×0.
                        let Some((iw, ih)) = arena.intrinsic(slot).or_else(|| {
                            (arena.node_type(slot) == NodeType::Video).then_some((300.0, 150.0))
                        }) else {
                            return Size::ZERO;
                        };
                        return Size {
                            width: known.width.unwrap_or(iw),
                            height: known.height.unwrap_or(ih),
                        };
                    }
                    let width = if arena.node_type(slot) == NodeType::TextInput
                        && arena.style(slot).field_sizing == FieldSizing::Fixed
                    {
                        // A control's preferred row count does not increase when
                        // CSS constrains its width below the preferred columns.
                        AxisOffer::MaxContent
                    } else {
                        // The leaf engine has folded its known border-box size
                        // into content space, less padding and border. Wrap where
                        // the host paints, not at the wider border box.
                        from_available(space.width)
                    };
                    let height = from_available(space.height);
                    // Reuse before flattening runs or crossing the host seam. Height
                    // stays in the key, and the 0.14 proof above updates on hits too.
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
                            return Size::ZERO;
                        }
                        // Direction and alignment inherit (a paragraph inside a
                        // centred column centres, as in CSS); the rest are its own.
                        let request = TextMeasureRequest {
                            exclusions: &[],
                            runs: &runs,
                            paragraph: Paragraph::from_style(
                                &arena.computed_style(slot, StyleMask::INHERITED),
                            ),
                            width,
                            height,
                        };
                        let metrics = match arena.paragraph_stamp(slot) {
                            Some(stamp) => measurer.measure_identified(&stamp, &request),
                            None => measurer.measure(&request),
                        };
                        if !metrics.is_valid() {
                            invalid_metrics.get_or_insert_with(|| arena.local_id(slot));
                            return Size::ZERO;
                        }
                        if context.measurements.len() == LEAF_OFFERS {
                            context.measurements.remove(0);
                        }
                        context.measurements.push(Measurement {
                            width,
                            height,
                            metrics,
                        });
                        metrics
                    };
                    first_baseline = metrics.first_baseline;
                    Size {
                        width: metrics.width,
                        height: metrics.height,
                    }
                },
            );
            // Patch 2's separate API is unnecessary: the upstream callback owns
            // LayoutOutput, including baselines in border-box coordinates.
            let inset = style
                .padding
                .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0)
                + style
                    .border
                    .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0);
            output.baselines = Baselines::from_first(first_baseline.map(|b| b + inset.top));
            output
        };
        let result = self
            .taffy
            .compute_layout_with_measure(root, available, measure);
        result.map_err(|e| LayoutError::Engine(format!("compute_layout: {e:?}")))?;
        if let Some(view) = invalid_metrics {
            return Err(LayoutError::InvalidTextMetrics(view));
        }
        Ok(())
    }

    pub(crate) fn height_measured(&self, node: NodeId) -> bool {
        self.taffy
            .get_node_context(node)
            .is_some_and(|c| c.height_measured)
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

/// Lay out `root_slot` and publish frames and resolved flow. The receipt uses
/// epoch zero; the kernel facade supplies its transaction epoch. Inline
/// runs have no geometry of their own: their frames are zero and they never
/// appear in the changed list, but their dirty bits are consumed like any
/// other node's.
pub fn compute(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root_slot: u32,
    offer: Offer,
) -> Result<LayoutReceipt, LayoutError> {
    let root = arena
        .taffy(root_slot)
        .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
    tree.compute(root, offer, arena, measurer)?;

    let mut changed = Vec::new();
    let mut exclusions = Vec::new();
    let mut stack = vec![(root_slot, 0.0, 0.0, false)];
    while let Some((slot, ox, oy, hidden)) = stack.pop() {
        let hidden = hidden || arena.style(slot).display == crate::Display::None;
        // @ref LLP 1043.000 §3 D2–D4 — collect in publication/document order.
        if !hidden && crate::flow::is_exclusion(arena, slot) {
            exclusions.push(slot);
        }
        let frame = if arena.is_inline_run(slot) {
            Frame::default()
        } else {
            let Some(node) = arena.taffy(slot) else {
                continue;
            };
            let l = tree.layout(node);
            arena.set_content(
                slot,
                (
                    l.scrollable_overflow_rect.right,
                    l.scrollable_overflow_rect.bottom,
                ),
            );
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
            changed.push(arena.key(slot));
        } else {
            flags.remove(NodeFlags::GEOMETRY_CHANGED);
        }
        for child in arena.children(slot).iter().rev() {
            stack.push((*child, frame.x, frame.y, hidden));
        }
    }
    let (flow_changed, flow_skipped) = crate::flow::resolve(
        arena,
        root_slot,
        &exclusions,
        |arena, slot| {
            arena
                .taffy(slot)
                .is_none_or(|node| tree.height_measured(node))
        },
        |_| true,
    );
    Ok(LayoutReceipt {
        epoch: 0,
        root: arena.key(root_slot),
        changed,
        flow_changed,
        flow_skipped,
    })
}

#[cfg(test)]
mod upstream_layout_differential {
    use super::*;
    use crate::{
        Kernel, MonospaceMeasurer, Op, PropId, PropValue, StyleId, StyleProps, StyleValue,
        TextMetrics,
    };

    fn random(seed: &mut u64, bound: u64) -> f64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed % bound) as f64
    }
    fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
        let mut patch = StyleProps::default();
        for (id, value) in rows {
            patch.set_dynamic(*id, value).unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    }
    fn t(s: &str) -> StyleValue {
        StyleValue::Text(s.into())
    }
    fn n(x: f64) -> StyleValue {
        StyleValue::Number(x)
    }
    #[derive(Default)]
    struct Metrics(Vec<[u32; 5]>);
    impl TextMeasurer for Metrics {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            let m = MonospaceMeasurer::default().measure(r);
            let offer = |a| match a {
                AxisOffer::Definite(n) => n.to_bits(),
                AxisOffer::MinContent => u32::MAX,
                AxisOffer::MaxContent => u32::MAX - 1,
            };
            self.0.push([
                offer(r.width),
                offer(r.height),
                m.width.to_bits(),
                m.height.to_bits(),
                m.first_baseline.unwrap_or(-1.).to_bits(),
            ]);
            m
        }
    }
    #[test]
    fn seeded_512_trees_match_fresh_frames_content_and_baselines() {
        let mut seed = 0x1043_0007_d1ff_u64;
        let mut measured = 0;
        for case in 0..512 {
            let mut k = Kernel::with_monospace();
            let mut ops = Vec::new();
            for id in 1..=32 {
                let kind = if id <= 3 {
                    NodeType::View
                } else if id == 5 || id == 7 || id % 5 == 0 {
                    NodeType::Image
                } else {
                    NodeType::Text
                };
                ops.push(Op::CreateView {
                    id,
                    node_type: kind,
                });
                if kind == NodeType::Text {
                    ops.push(Op::SetProp {
                        id,
                        prop: PropId::Text,
                        value: PropValue::Str(
                            "alpha beta longerword ".repeat(2 + random(&mut seed, 16) as usize),
                        ),
                    });
                    ops.push(style(
                        id,
                        &[
                            (StyleId::FontSize, n(10. + random(&mut seed, 20))),
                            (StyleId::PaddingLeft, n(random(&mut seed, 40))),
                            (StyleId::PaddingTop, n(random(&mut seed, 20))),
                            (StyleId::BorderWidthRight, n(random(&mut seed, 8))),
                            (StyleId::BorderWidthBottom, n(random(&mut seed, 5))),
                            (
                                StyleId::BoxSizing,
                                t(if case % 2 == 0 {
                                    "border-box"
                                } else {
                                    "content-box"
                                }),
                            ),
                        ],
                    ));
                }
            }
            ops.extend([
                style(
                    1,
                    &[
                        (StyleId::Width, n(320. + random(&mut seed, 480))),
                        (StyleId::Height, n(700.)),
                        (
                            StyleId::Display,
                            t(if case % 3 == 0 { "block" } else { "flex" }),
                        ),
                        (
                            StyleId::FlexDirection,
                            t(if case % 2 == 0 { "row" } else { "column" }),
                        ),
                        (StyleId::AlignItems, t("baseline")),
                    ],
                ),
                style(
                    2,
                    &[
                        (StyleId::Width, StyleValue::Percent(80.)),
                        (StyleId::Display, t("flex")),
                        (StyleId::FlexDirection, t("column")),
                    ],
                ),
                style(
                    3,
                    &[
                        (StyleId::Width, StyleValue::Percent(75.)),
                        (
                            StyleId::Display,
                            t(if case % 2 == 0 { "flex" } else { "block" }),
                        ),
                        (StyleId::AlignItems, t("baseline")),
                    ],
                ),
                style(
                    4,
                    &[
                        (StyleId::Width, n(300.)),
                        (StyleId::PaddingLeft, n(50.)),
                        (StyleId::PaddingRight, n(50.)),
                        (StyleId::FlexShrink, n(1.)),
                    ],
                ),
                style(
                    5,
                    &[
                        (StyleId::Width, n(100.)),
                        (StyleId::Height, n(0.)),
                        (StyleId::MarginTop, n(20. + random(&mut seed, 20))),
                        (StyleId::MarginBottom, n(30.)),
                    ],
                ),
                style(
                    6,
                    &[(StyleId::Width, n(250.)), (StyleId::FlexShrink, n(1.))],
                ),
                style(
                    7,
                    &[
                        (StyleId::Width, n(40. + random(&mut seed, 90))),
                        (StyleId::AspectRatio, n(2.)),
                    ],
                ),
                style(8, &[(StyleId::Width, StyleValue::Percent(90.))]),
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 4, 5, 6, 7],
                },
                Op::SetChildren {
                    id: 2,
                    children: vec![3],
                },
                Op::SetChildren {
                    id: 3,
                    children: (8..=32).collect(),
                },
                Op::AttachRoot { id: 1 },
            ]);
            k.apply(0, 0, &ops).unwrap();
            for id in [5, 7, 10, 15, 20, 25, 30] {
                k.set_intrinsic_size(
                    id,
                    Some((
                        100. + random(&mut seed, 200) as f32,
                        40. + random(&mut seed, 200) as f32,
                    )),
                )
                .unwrap();
            }
            let mut a = k.arena().clone();
            let mut b = a.clone();
            let mut incremental = LayoutTree::rebuild(&mut a);

            let (mut ma, mut mb) = (Metrics::default(), Metrics::default());
            for width in [320., 611.5, 480.] {
                let root = a.slot_of(1).unwrap();
                compute(
                    &mut a,
                    &mut incremental,
                    &mut ma,
                    root,
                    Offer::definite(width, 900.),
                )
                .unwrap();
                let mut fresh = LayoutTree::rebuild(&mut b);
                mb.0.clear();
                ma.0.clear();
                compute(
                    &mut b,
                    &mut fresh,
                    &mut mb,
                    root,
                    Offer::definite(width, 900.),
                )
                .unwrap();
                for slot in a.iter_live() {
                    let x = incremental.layout(a.taffy(slot).unwrap());
                    let y = fresh.layout(b.taffy(slot).unwrap());
                    let bits = |v: taffy::tree::Layout| {
                        [
                            v.location.x,
                            v.location.y,
                            v.size.width,
                            v.size.height,
                            v.scrollable_overflow_rect.right,
                            v.scrollable_overflow_rect.bottom,
                            v.border.left,
                            v.border.top,
                            v.padding.left,
                            v.padding.top,
                        ]
                        .map(f32::to_bits)
                    };
                    assert_eq!(bits(x), bits(y), "tree {case}, node {slot}, offer {width}");
                }
                measured += mb.0.len();
            }
        }
        assert!(measured > 10_000); // an empty or bypassed measurer cannot pass
        println!("Upstream differential: 512 seeded trees x 3 offers, 49152 node layouts equal to fresh; {measured} fresh measurements");
    }
}
