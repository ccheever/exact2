//! The result-equality differential: incremental relayout after a random
//! sequence of mutations must equal, bit for bit, (a) a kernel rehydrated from
//! the columns alone and (b) a kernel that replayed every batch from scratch.
//!
//! Raw `f32` bits are compared, so a one-ULP drift between the incremental and
//! full paths is a failure rather than a rounding opinion.

use exact_kernel::{
    export, AlignItems, Dimension, Display, FlexDirection, JustifyContent, Kernel,
    MonospaceMeasurer, NodeType, Offer, Op, Overflow, PositionType, PropId, StyleId, StyleProps,
};

/// The second state is a stream of its own (`side`): draws from it leave the
/// trees the first stream builds, and so the seeds named below, as they were.
struct Rng(u64, u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed, seed ^ 0x9e37_79b9_7f4a_7c15 | 1)
    }

    fn side(&mut self, n: u64) -> u64 {
        let mut x = self.1;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.1 = x;
        x % n
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

const WORDS: [&str; 8] = [
    "train",
    "Caltrain",
    "San Jose",
    "departs",
    "10:42",
    "platform 2",
    "northbound",
    "delayed by 4 minutes",
];

fn random_style(rng: &mut Rng, node_type: NodeType) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    let dim = |rng: &mut Rng| match rng.below(4) {
        0 => Dimension::Auto,
        1 => Dimension::Points(rng.below(200) as f32),
        2 => Dimension::Percent(rng.below(100) as f32),
        _ => Dimension::Points(rng.below(50) as f32 + 0.5),
    };
    if rng.chance(2) {
        s.width = dim(rng);
        s.mask.set(StyleId::Width);
    }
    if rng.chance(2) {
        s.height = dim(rng);
        s.mask.set(StyleId::Height);
    }
    if rng.chance(3) {
        s.padding_left = Dimension::Points(rng.below(12) as f32);
        s.mask.set(StyleId::PaddingLeft);
        s.padding_top = Dimension::Points(rng.below(12) as f32);
        s.mask.set(StyleId::PaddingTop);
    }
    if rng.chance(3) {
        s.margin_top = dim(rng);
        s.mask.set(StyleId::MarginTop);
    }
    if rng.chance(3) {
        s.flex_direction = *rng.pick(&[
            FlexDirection::Row,
            FlexDirection::Column,
            FlexDirection::RowReverse,
        ]);
        s.mask.set(StyleId::FlexDirection);
    }
    if rng.chance(3) {
        s.flex_grow = rng.below(3) as f32;
        s.mask.set(StyleId::FlexGrow);
    }
    if rng.chance(4) {
        s.flex_shrink = 1.0;
        s.mask.set(StyleId::FlexShrink);
    }
    if rng.chance(4) {
        s.justify_content = *rng.pick(&[
            JustifyContent::Center,
            JustifyContent::SpaceBetween,
            JustifyContent::FlexEnd,
        ]);
        s.mask.set(StyleId::JustifyContent);
    }
    if rng.chance(4) {
        s.align_items = *rng.pick(&[
            AlignItems::Center,
            AlignItems::FlexStart,
            AlignItems::Stretch,
        ]);
        s.mask.set(StyleId::AlignItems);
    }
    if rng.chance(6) {
        s.position_type = PositionType::Absolute;
        s.mask.set(StyleId::PositionType);
        s.left = Dimension::Points(rng.below(30) as f32);
        s.mask.set(StyleId::Left);
        s.top = Dimension::Points(rng.below(30) as f32);
        s.mask.set(StyleId::Top);
        // LLP 1074 T1: an axis without an inset sits at its static position,
        // and an end inset measures from the containing block's far edge.
        match rng.side(6) {
            0 => s.left = Dimension::Auto,
            1 => s.top = Dimension::Auto,
            2 => (s.left, s.top) = (Dimension::Auto, Dimension::Auto),
            3 => {
                (s.right, s.left) = (s.left, Dimension::Auto);
                s.mask.set(StyleId::Right);
            }
            4 => {
                (s.bottom, s.top) = (s.top, Dimension::Auto);
                s.mask.set(StyleId::Bottom);
            }
            _ => {}
        }
    } else if rng.side(3) == 0 {
        // A positioned box is the containing block of the absolute boxes
        // under it; the rest are static and hand theirs up.
        s.position_type = PositionType::Relative;
        s.mask.set(StyleId::PositionType);
    }
    if rng.chance(4) {
        s.row_gap = rng.below(8) as f32;
        s.mask.set(StyleId::RowGap);
    }
    if rng.chance(5) {
        s.max_width = Dimension::Points(rng.below(150) as f32 + 20.0);
        s.mask.set(StyleId::MaxWidth);
    }
    if rng.chance(5) {
        s.border_width_left = rng.below(4) as f32;
        s.mask.set(StyleId::BorderWidthLeft);
        s.border_style_left = exact_kernel::BorderStyle::Solid;
        s.mask.set(StyleId::BorderStyleLeft);
    }
    if rng.chance(2) {
        s.display = Display::Flex;
        s.mask.set(StyleId::Display);
    }
    // A clipping box is a relayout boundary candidate wherever it sits.
    if node_type != NodeType::Text && rng.chance(3) {
        s.overflow_y = *rng.pick(&[Overflow::Hidden, Overflow::Scroll]);
        s.mask.set(StyleId::OverflowY);
    }
    if node_type == NodeType::Text {
        s.font_size = *rng.pick(&[10.0, 12.0, 16.0, 20.0]);
        s.mask.set(StyleId::FontSize);
        if rng.chance(3) {
            s.line_clamp = rng.below(3) as u32;
            s.mask.set(StyleId::LineClamp);
        }
    }
    Box::new(s)
}

struct World {
    rng: Rng,
    next_id: u32,
    /// (id, type) of every live node.
    live: Vec<(u32, NodeType)>,
    /// parent → children, mirrored so batches stay valid.
    children: Vec<(u32, Vec<u32>)>,
    log: Vec<Vec<Op>>,
    /// Nodes mutations mostly leave alone (the panes' ancestors and panes).
    fixed: Vec<u32>,
}

impl World {
    fn children_of(&self, id: u32) -> Vec<u32> {
        self.children
            .iter()
            .find(|(p, _)| *p == id)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    fn set_children(&mut self, id: u32, kids: Vec<u32>) {
        match self.children.iter_mut().find(|(p, _)| *p == id) {
            Some(entry) => entry.1 = kids,
            None => self.children.push((id, kids)),
        }
    }

    /// Whether to leave a fixed node alone this time (nine times in ten).
    fn spared(&mut self, id: u32) -> bool {
        self.fixed.contains(&id) && self.rng.below(10) != 0
    }

    fn containers(&self) -> Vec<u32> {
        self.live
            .iter()
            .filter(|(_, t)| {
                matches!(
                    t,
                    NodeType::View | NodeType::ScrollView | NodeType::Pressable
                )
            })
            .map(|(id, _)| *id)
            .collect()
    }

    fn create(&mut self, ops: &mut Vec<Op>, node_type: NodeType) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        ops.push(Op::CreateView { id, node_type });
        ops.push(Op::SetStyle {
            id,
            patch: random_style(&mut self.rng, node_type),
        });
        if node_type == NodeType::Text {
            ops.push(Op::SetProp {
                id,
                prop: PropId::Text,
                value: (*self.rng.pick(&WORDS)).into(),
            });
        }
        self.live.push((id, node_type));
        id
    }

    fn attach(&mut self, ops: &mut Vec<Op>, parent: u32, child: u32) {
        let mut kids = self.children_of(parent);
        let at = self.rng.below(kids.len() as u64 + 1) as usize;
        kids.insert(at, child);
        self.set_children(parent, kids.clone());
        ops.push(Op::SetChildren {
            id: parent,
            children: kids,
        });
    }

    fn destroy(&mut self, ops: &mut Vec<Op>, id: u32) {
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            stack.extend(self.children_of(n));
            self.live.retain(|(l, _)| *l != n);
            self.children.retain(|(p, _)| *p != n);
        }
        for entry in self.children.iter_mut() {
            entry.1.retain(|c| *c != id);
        }
        ops.push(Op::DestroyView { id });
    }

    fn initial(&mut self) -> Vec<Op> {
        let mut ops = Vec::new();
        let root = self.create(&mut ops, NodeType::View);
        ops.push(Op::SetStyle {
            id: root,
            patch: {
                let mut s = StyleProps::default();
                s.width = Dimension::Points(320.0);
                s.mask.set(StyleId::Width);
                s.height = Dimension::Points(480.0);
                s.mask.set(StyleId::Height);
                Box::new(s)
            },
        });
        ops.push(Op::AttachRoot { id: root });
        for _ in 0..24 {
            let parent = *self.rng.pick(&self.containers());
            let node_type = *self.rng.pick(&[
                NodeType::View,
                NodeType::View,
                NodeType::Text,
                NodeType::ScrollView,
                NodeType::Pressable,
                NodeType::Image,
            ]);
            let child = self.create(&mut ops, node_type);
            self.attach(&mut ops, parent, child);
        }
        self.log.push(ops.clone());
        ops
    }

    fn mutate(&mut self) -> Vec<Op> {
        let mut ops = Vec::new();
        for _ in 0..(1 + self.rng.below(4)) {
            match self.rng.below(6) {
                0 | 1 => {
                    let (id, node_type) = *self.rng.pick(&self.live);
                    if self.spared(id) {
                        continue;
                    }
                    ops.push(Op::SetStyle {
                        id,
                        patch: random_style(&mut self.rng, node_type),
                    });
                }
                2 => {
                    let texts: Vec<u32> = self
                        .live
                        .iter()
                        .filter(|(_, t)| *t == NodeType::Text)
                        .map(|(id, _)| *id)
                        .collect();
                    if let Some(id) = texts.first().copied().map(|_| *self.rng.pick(&texts)) {
                        ops.push(Op::SetProp {
                            id,
                            prop: PropId::Text,
                            value: (*self.rng.pick(&WORDS)).into(),
                        });
                    }
                }
                3 => {
                    let parent = *self.rng.pick(&self.containers());
                    if self.spared(parent) {
                        continue;
                    }
                    let node_type = *self.rng.pick(&[NodeType::View, NodeType::Text]);
                    let child = self.create(&mut ops, node_type);
                    self.attach(&mut ops, parent, child);
                }
                4 => {
                    // Destroy a non-root subtree.
                    let candidates: Vec<u32> = self
                        .live
                        .iter()
                        .map(|(id, _)| *id)
                        .filter(|id| *id != 1)
                        .collect();
                    if !candidates.is_empty() {
                        let id = *self.rng.pick(&candidates);
                        if self.spared(id) {
                            continue;
                        }
                        self.destroy(&mut ops, id);
                    }
                }
                _ => {
                    // Reparent a non-root node under another container that is not its descendant.
                    let candidates: Vec<u32> = self
                        .live
                        .iter()
                        .map(|(id, _)| *id)
                        .filter(|id| *id != 1)
                        .collect();
                    if candidates.is_empty() {
                        continue;
                    }
                    let child = *self.rng.pick(&candidates);
                    if self.spared(child) {
                        continue;
                    }
                    let mut descendants = vec![child];
                    let mut stack = vec![child];
                    while let Some(n) = stack.pop() {
                        for c in self.children_of(n) {
                            descendants.push(c);
                            stack.push(c);
                        }
                    }
                    let targets: Vec<u32> = self
                        .containers()
                        .into_iter()
                        .filter(|c| !descendants.contains(c))
                        .collect();
                    if targets.is_empty() {
                        continue;
                    }
                    let parent = *self.rng.pick(&targets);
                    let old_parent = self
                        .children
                        .iter()
                        .find(|(_, c)| c.contains(&child))
                        .map(|(p, _)| *p);
                    if let Some(old) = old_parent {
                        if old == parent {
                            continue;
                        }
                        let kids: Vec<u32> = self
                            .children_of(old)
                            .into_iter()
                            .filter(|c| *c != child)
                            .collect();
                        self.set_children(old, kids.clone());
                        ops.push(Op::SetChildren {
                            id: old,
                            children: kids,
                        });
                    }
                    self.attach(&mut ops, parent, child);
                }
            }
        }
        self.log.push(ops.clone());
        ops
    }
}

/// Frames and scroll extents, as bits.
fn frames(k: &Kernel) -> Vec<(u32, [u32; 6])> {
    k.rows(None)
        .unwrap()
        .into_iter()
        .map(|r| {
            let (w, h) = k.arena().content(k.arena().slot_of(r.id).unwrap());
            (
                r.id,
                [
                    r.frame.x.to_bits(),
                    r.frame.y.to_bits(),
                    r.frame.width.to_bits(),
                    r.frame.height.to_bits(),
                    w.to_bits(),
                    h.to_bits(),
                ],
            )
        })
        .collect()
}

fn run(seed: u64, rounds: usize) {
    let mut world = World {
        rng: Rng::new(seed),
        next_id: 1,
        live: Vec::new(),
        children: Vec::new(),
        log: Vec::new(),
        fixed: Vec::new(),
    };
    let first = world.initial();
    compare(seed, rounds, world, first);
}

/// A list's shape: a flex column holding a header and clipping panes sized
/// every way a pane is (flex, fixed, auto, percent, first or not), with most
/// mutations inside the panes, where a relayout boundary replays them.
fn run_panes(seed: u64, rounds: usize) {
    let mut world = World {
        rng: Rng::new(seed),
        next_id: 1,
        live: Vec::new(),
        children: Vec::new(),
        log: Vec::new(),
        fixed: Vec::new(),
    };
    let mut ops = Vec::new();
    let root = world.create(&mut ops, NodeType::View);
    let mut s = StyleProps::default();
    s.width = Dimension::Points(320.0);
    s.height = Dimension::Points(480.0);
    s.display = Display::Flex;
    s.flex_direction = FlexDirection::Column;
    for id in [
        StyleId::Width,
        StyleId::Height,
        StyleId::Display,
        StyleId::FlexDirection,
    ] {
        s.mask.set(id);
    }
    ops.push(Op::SetStyle {
        id: root,
        patch: Box::new(s),
    });
    ops.push(Op::AttachRoot { id: root });
    let header = world.create(&mut ops, NodeType::Text);
    let mut panes = vec![header];
    for _ in 0..3 {
        let pane = world.create(&mut ops, NodeType::ScrollView);
        let mut s = StyleProps::default();
        match world.rng.below(4) {
            0 => {
                s.flex_grow = 1.0;
                s.flex_basis = Dimension::Percent(0.0);
                s.min_height = Dimension::Points(0.0);
                s.mask.set(StyleId::FlexGrow);
                s.mask.set(StyleId::FlexBasis);
                s.mask.set(StyleId::MinHeight);
            }
            1 => {
                s.height = Dimension::Points(world.rng.below(200) as f32);
                s.mask.set(StyleId::Height);
            }
            2 => {
                s.width = Dimension::Percent(world.rng.below(100) as f32);
                s.mask.set(StyleId::Width);
            }
            _ => {}
        }
        if world.rng.chance(2) {
            s.display = Display::Flex;
            s.mask.set(StyleId::Display);
        }
        ops.push(Op::SetStyle {
            id: pane,
            patch: Box::new(s),
        });
        panes.push(pane);
    }
    let at = world.rng.below(4) as usize;
    panes.swap(0, at);
    world.set_children(root, panes.clone());
    ops.push(Op::SetChildren {
        id: root,
        children: panes.clone(),
    });
    world.fixed = std::iter::once(root).chain(panes).collect();
    for _ in 0..24 {
        let parent = *world.rng.pick(&world.containers()[1..]);
        let node_type = *world
            .rng
            .pick(&[NodeType::View, NodeType::View, NodeType::Text]);
        let child = world.create(&mut ops, node_type);
        world.attach(&mut ops, parent, child);
    }
    world.log.push(ops.clone());
    compare(seed, rounds, world, ops);
}

fn compare(seed: u64, rounds: usize, mut world: World, first: Vec<Op>) {
    let mut kernel = Kernel::with_monospace();
    let offer = Offer::definite(320.0, 480.0);
    kernel.apply(0, 0, &first).unwrap();
    kernel.compute_layout(1, offer).unwrap();

    for round in 0..rounds {
        let ops = world.mutate();
        kernel
            .apply(0, round as u64 + 1, &ops)
            .unwrap_or_else(|e| panic!("seed {seed} round {round}: {e}\nops: {ops:?}"));
        kernel.compute_layout(1, offer).unwrap();
        let incremental = frames(&kernel);

        // (a) Rehydrate from columns: fresh engine, same authored state.
        let mut rehydrated = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
        rehydrated.compute_layout(1, offer).unwrap();
        assert_eq!(
            frames(&rehydrated),
            incremental,
            "seed {seed} round {round}: rehydrated layout differs from incremental"
        );

        // (b) Replay every batch from scratch.
        let mut replay = Kernel::with_monospace();
        for (i, batch) in world.log.iter().enumerate() {
            replay.apply(0, i as u64, batch).unwrap();
        }
        replay.compute_layout(1, offer).unwrap();
        assert_eq!(
            frames(&replay),
            incremental,
            "seed {seed} round {round}: replayed layout differs from incremental"
        );

        // The authored columns agree too, not just the geometry.
        let a = export::decode(&kernel.export(None).unwrap()).unwrap();
        let b = export::decode(&replay.export(None).unwrap()).unwrap();
        assert_eq!(
            a.styles, b.styles,
            "seed {seed} round {round}: styles diverged"
        );
        assert_eq!(
            a.props, b.props,
            "seed {seed} round {round}: props diverged"
        );
        assert_eq!(a.rows.len(), b.rows.len());
    }
}

#[test]
fn incremental_relayout_is_result_equal_to_full_relayout() {
    for seed in [1, 2, 3, 5, 8, 13, 21, 34] {
        run(seed, 40);
    }
}

#[test]
fn contained_pane_relayout_is_result_equal_to_full_relayout() {
    for seed in 1..=24 {
        run_panes(seed, 40);
    }
}

#[test]
fn long_run_single_seed() {
    run(0x5eed_c0de, 200);
}

// Seeds that once differed from a fresh layout, each through an input Taffy's
// layout cache key left out (vendor/taffy/EXACT-PATCHES.md, patch 11).

/// Round 13: a parent turns from block to flex, and its block child's final
/// layout, with its first child's 4.5-point margin collapsed through it, was
/// reused as a flex item's (`vertical_margins_are_collapsible`, `sizing_mode`).
#[test]
fn seed_2423214_block_child_of_a_new_flex_parent() {
    run(2423214, 14);
}

/// Round 7: an item's own percentage height, resolved against a 144-point
/// parent under `InherentSize`, answered a `ContentSize` probe with no parent
/// height (`sizing_mode`, the parent's height).
#[test]
fn pane_seed_405_percentage_height_probe() {
    run_panes(405, 40);
}

/// Round 13: a measurement whose `height: 86%` resolved against a 62-point
/// parent answered the same probe once the parent's height was auto (the
/// parent's height: the measurement key compared only its width).
#[test]
fn pane_seed_503_percentage_height_after_the_parent_loses_its_height() {
    run_panes(503, 40);
}

/// Round 27: a block parent's `InherentSize` width probe, which applies the
/// child's own size styles, answered a flex base-size probe, which ignores
/// them: 58 points for 110 (`sizing_mode`, `vertical_margins_are_collapsible`).
#[test]
fn pane_seed_1412_styled_width_probe_as_a_flex_basis() {
    run_panes(1412, 40);
}

#[test]
fn block_intrinsic_probes_cannot_leave_cached_final_children_wrapped() {
    // Fieldnotes: a padded block button in a wrapping row within a column.
    // Mounting Discard triggers min-content probes on the retained Save button.
    // Its outer box stays 99x43, but the probe used to leave its child at 41x38.
    // These are the observed CoreText metrics, without a platform dependency.
    use taffy::prelude::*;
    let mut tree = TaffyTree::<bool>::new();
    let label = tree.new_leaf_with_context(Style::default(), true).unwrap();
    let button = tree
        .new_with_children(
            Style {
                display: taffy::Display::Block,
                padding: Rect {
                    left: length(12.0_f32),
                    right: length(12.0_f32),
                    top: length(12.0_f32),
                    bottom: length(12.0_f32),
                },
                ..Style::default()
            },
            &[label],
        )
        .unwrap();
    let other_label = tree.new_leaf_with_context(Style::default(), false).unwrap();
    let other_button = tree
        .new_with_children(
            Style {
                display: taffy::Display::Block,
                padding: Rect {
                    left: length(12.0_f32),
                    right: length(12.0_f32),
                    top: length(12.0_f32),
                    bottom: length(12.0_f32),
                },
                ..Style::default()
            },
            &[other_label],
        )
        .unwrap();
    let row = tree
        .new_with_children(
            Style {
                display: taffy::Display::Flex,
                flex_wrap: FlexWrap::Wrap,
                align_items: Some(taffy::AlignItems::CENTER),
                gap: Size {
                    width: length(10.0_f32),
                    height: length(0.0_f32),
                },
                ..Style::default()
            },
            &[button],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                ..Style::default()
            },
            &[row],
        )
        .unwrap();
    let measure =
        |inputs: taffy::tree::LayoutInput, _, context: Option<&mut bool>, style: &Style| {
            taffy::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, space| {
                    let is_save = *context.unwrap();
                    let full = if is_save { 75.0 } else { 122.0 };
                    let narrow = if is_save { 41.0 } else { 65.0 };
                    let available = known.width.unwrap_or(match space.width {
                        AvailableSpace::MinContent => narrow,
                        AvailableSpace::MaxContent => full,
                        AvailableSpace::Definite(width) => width,
                    });
                    Size {
                        width: known
                            .width
                            .unwrap_or(if available < full { narrow } else { full }),
                        height: known
                            .height
                            .unwrap_or(if available < full { 38.0 } else { 19.0 }),
                    }
                },
            )
        };
    let offer = Size {
        width: AvailableSpace::Definite(322.0),
        height: AvailableSpace::MaxContent,
    };
    tree.compute_layout_with_measure(root, offer, measure)
        .unwrap();
    assert_eq!(
        tree.layout(label).unwrap().size,
        Size {
            width: 75.0,
            height: 19.0
        }
    );
    tree.set_children(row, &[button, other_button]).unwrap();
    tree.compute_layout_with_measure(root, offer, measure)
        .unwrap();
    assert_eq!(
        tree.layout(button).unwrap().size,
        Size {
            width: 99.0,
            height: 43.0
        }
    );
    assert_eq!(
        tree.layout(label).unwrap().size,
        Size {
            width: 75.0,
            height: 19.0
        }
    );
}

use crate::support::reader;

#[test]
fn reader_resize_replacement_and_recreation_equal_replay_and_rehydration() {
    // Preserve frames AND descendant scroll overflow across cache invalidations.
    fn geometry(k: &Kernel) -> Vec<(u32, [u32; 6])> {
        k.rows(None)
            .unwrap()
            .iter()
            .map(|r| {
                let n = k.node(r.id).unwrap();
                (
                    r.id,
                    [
                        n.frame.x.to_bits(),
                        n.frame.y.to_bits(),
                        n.frame.width.to_bits(),
                        n.frame.height.to_bits(),
                        n.content.0.to_bits(),
                        n.content.1.to_bits(),
                    ],
                )
            })
            .collect()
    }
    for workaround in [false, true] {
        for border_box in [false, true] {
            let mut k = reader::kernel();
            let mut log = vec![reader::initial(workaround, border_box, true)];
            k.apply(0, 0, &log[0]).unwrap();
            let replace = Op::SetProp {
                id: 210,
                prop: PropId::Text,
                value: reader::LONG_TEXT.repeat(13).into(),
            };
            let mut recreate = vec![Op::CreateView {
                id: 10,
                node_type: NodeType::View,
            }];
            recreate.extend(reader::block(10, true));
            recreate.push(Op::SetChildren {
                id: 4,
                children: (5..=reader::LAST).collect(),
            });
            let steps = vec![
                (360.0, vec![]),
                (1000.0, vec![]),
                (360.0, vec![]),
                (360.0, vec![replace]),
                (
                    1000.0,
                    vec![reader::style(
                        4,
                        &[(StyleId::MaxWidth, reader::number(540.0))],
                    )],
                ),
                (360.0, vec![Op::DestroyView { id: 10 }]),
                (360.0, recreate),
            ];
            for (round, (width, ops)) in steps.into_iter().enumerate() {
                k.apply(0, round as u64 + 1, &ops).unwrap();
                log.push(ops);
                let offer = Offer::definite(width, 800.0);
                k.compute_layout(1, offer).unwrap();
                let mut rehydrated = k.rehydrate(Box::new(reader::measurer()));
                rehydrated.compute_layout(1, offer).unwrap();
                let mut replay = reader::kernel();
                for (epoch, batch) in log.iter().enumerate() {
                    replay.apply(0, epoch as u64, batch).unwrap();
                }
                replay.compute_layout(1, offer).unwrap();
                assert_eq!(
                    geometry(&k),
                    geometry(&rehydrated),
                    "rehydrated, round {round}"
                );
                assert_eq!(geometry(&k), geometry(&replay), "replayed, round {round}");
                let col = k.node(4).unwrap();
                let last = k.node(reader::LAST).unwrap();
                assert!(
                    (k.node(2).unwrap().content.1 - col.frame.height - 96.0).abs() < 0.02,
                    "scroll padding, round {round}"
                );
                assert!(
                    (col.frame.height - (last.frame.y + last.frame.height - col.frame.y)).abs()
                        < 0.02,
                    "phantom range, round {round}"
                );
            }
        }
    }
}

// @ref LLP 1043.000 §3 D3/D4 — exclusions are pure derived geometry.
#[test]
fn moving_exclusions_equal_fresh_replay_and_rehydration() {
    use exact_kernel::{FlowShape, ShapeOutside, WrapFlow};
    let mut root = StyleProps::default();
    root.width = Dimension::Points(600.);
    root.height = Dimension::Points(400.);
    root.mask.set(StyleId::Width);
    root.mask.set(StyleId::Height);
    let mut ball = root.clone();
    ball.width = Dimension::Points(120.);
    ball.height = Dimension::Points(120.);
    ball.position_type = PositionType::Absolute;
    ball.top = Dimension::Points(0.);
    ball.mask.set(StyleId::Top);
    ball.wrap_flow = WrapFlow::Both;
    ball.shape_outside = ShapeOutside::parse("circle()").unwrap();
    for row in [
        StyleId::PositionType,
        StyleId::WrapFlow,
        StyleId::ShapeOutside,
    ] {
        ball.mask.set(row);
    }
    let mut log = vec![vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Text,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root.clone()),
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(root),
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(ball),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: exact_kernel::PropValue::Str("Flowing words".into()),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::AttachRoot { id: 1 },
    ]];
    let mut incremental = Kernel::with_monospace();
    incremental.apply(0, 0, &log[0]).unwrap();
    let offer = Offer::definite(600., 400.);
    for (i, x) in [0., 100., 250., 800., 75.].into_iter().enumerate() {
        let mut s = StyleProps::default();
        s.left = Dimension::Points(x);
        s.mask.set(StyleId::Left);
        log.push(vec![Op::SetStyle {
            id: 3,
            patch: Box::new(s),
        }]);
        incremental.apply(0, 0, log.last().unwrap()).unwrap();
        incremental.compute_layout(1, offer).unwrap();
        let expected: Vec<FlowShape> = incremental.node(2).unwrap().flow_shapes().to_vec();
        assert_eq!(expected.len(), usize::from(i != 3));
        let mut replay = Kernel::with_monospace();
        for batch in &log {
            replay.apply(0, 0, batch).unwrap();
        }
        replay.compute_layout(1, offer).unwrap();
        let mut rehydrated = incremental.rehydrate(Box::new(MonospaceMeasurer::default()));
        rehydrated.compute_layout(1, offer).unwrap();
        for other in [&replay, &rehydrated] {
            assert_eq!(other.node(2).unwrap().flow_shapes(), expected);
            assert!(other
                .node(2)
                .unwrap()
                .frame
                .bits_eq(incremental.node(2).unwrap().frame));
        }
    }
}

#[test]
fn upstream_leaf_keeps_border_box_compute_size_inputs() {
    use taffy::{
        compute_leaf_layout,
        geometry::Size,
        style::{Dimension as D, Style},
        tree::{LayoutInput, RunMode, SizingMode},
        AvailableSpace,
    };
    let mut seen = None;
    let style: Style = Style {
        size: Size {
            width: D::length(300.),
            height: D::auto(),
        },
        padding: taffy::geometry::Rect::length(50f32),
        box_sizing: taffy::BoxSizing::ContentBox,
        ..Default::default()
    };
    compute_leaf_layout(
        LayoutInput {
            known_dimensions: Size {
                width: Some(400.),
                height: None,
            },
            parent_size: Size {
                width: Some(600.),
                height: Some(500.),
            },
            available_space: Size {
                width: AvailableSpace::Definite(600.),
                height: AvailableSpace::MaxContent,
            },
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::InherentSize,
            ..LayoutInput::HIDDEN
        },
        &style,
        |_, _| 0.,
        |known, _| {
            seen = known.width;
            Size::ZERO
        },
    );
    assert_eq!(
        seen,
        Some(400.),
        "ComputeSize must pass upstream border-box known dimensions unchanged"
    );
}

#[test]
fn measured_baselines_include_padding_and_inherited_direction_relayouts_boxes() {
    use exact_kernel::{StyleValue, TextMeasureRequest, TextMeasurer, TextMetrics};
    struct Measurer;
    impl TextMeasurer for Measurer {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            let h = r.paragraph.strut.font_size;
            TextMetrics {
                width: 20.,
                height: h,
                first_baseline: Some(h * 0.75),
            }
        }
    }
    let style = |id, rows: &[(StyleId, StyleValue)]| {
        let mut patch = StyleProps::default();
        for (row, value) in rows {
            patch.set_dynamic(*row, value).unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    };
    let n = StyleValue::Number;
    let t = |s: &str| StyleValue::Text(s.into());
    let mut k = Kernel::new(Box::new(Measurer));
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::Text,
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::Text,
        },
        style(1, &[(StyleId::Width, n(200.))]),
        style(
            2,
            &[
                (StyleId::Display, t("flex")),
                (StyleId::AlignItems, t("baseline")),
            ],
        ),
        style(
            3,
            &[(StyleId::FontSize, n(20.)), (StyleId::PaddingTop, n(3.))],
        ),
        style(4, &[(StyleId::FontSize, n(40.))]),
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: "small".into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "large".into(),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        },
        Op::AttachRoot { id: 1 },
    ];
    k.apply(0, 0, &ops).unwrap();
    k.compute_layout(1, Offer::definite(200., 500.)).unwrap();
    assert_eq!(k.node(3).unwrap().frame.y, 12.);
    assert_eq!(k.node(4).unwrap().frame.y, 0.);
    assert_eq!(k.node(3).unwrap().frame.x, 0.);
    k.apply(0, 0, &[style(1, &[(StyleId::Direction, t("rtl"))])])
        .unwrap();
    k.compute_layout(1, Offer::definite(200., 500.)).unwrap();
    assert_eq!(k.node(3).unwrap().frame.x, 180.);
    let mut fresh = k.rehydrate(Box::new(Measurer));
    fresh
        .compute_layout(1, Offer::definite(200., 500.))
        .unwrap();
    for id in 1..=4 {
        assert!(k
            .node(id)
            .unwrap()
            .frame
            .bits_eq(fresh.node(id).unwrap().frame));
    }
}

#[test]
fn default_block_alignment_preserves_nested_collapsed_margins() {
    use exact_kernel::StyleId::*;
    use reader::{number as n, style};
    let mut k = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=4)
        .map(|id| Op::CreateView {
            id,
            node_type: NodeType::View,
        })
        .collect();
    ops.extend([
        style(1, &[(Width, n(200.)), (PaddingTop, n(1.))]),
        style(2, &[(Height, n(50.)), (MarginBottom, n(40.))]),
        style(3, &[(MarginTop, n(60.))]),
        style(4, &[(Height, n(20.)), (MarginTop, n(100.))]),
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::SetChildren {
            id: 3,
            children: vec![4],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 0, &ops).unwrap();
    k.compute_layout(1, Offer::definite(200., 500.)).unwrap();
    // CSS 2.1 §8.3.1: 1px padding + 50px sibling + max(40, 60, 100).
    // An implicit align-content:stretch wrongly creates a BFC and gives 211.
    assert_eq!(k.node(3).unwrap().frame.y, 151.);
    assert_eq!(k.node(4).unwrap().frame.y, 151.);
    assert_eq!(k.node(1).unwrap().frame.height, 171.);
    // Negative control: explicitly non-normal alignment does establish a BFC.
    k.apply(
        0,
        0,
        &[style(3, &[(AlignContent, reader::text("stretch"))])],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(200., 500.)).unwrap();
    assert_eq!(k.node(4).unwrap().frame.y, 211.);
}
