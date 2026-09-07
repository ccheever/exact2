//! The result-equality differential: incremental relayout after a random
//! sequence of mutations must equal, bit for bit, (a) a kernel rehydrated from
//! the columns alone and (b) a kernel that replayed every batch from scratch.
//!
//! Raw `f32` bits are compared, so a one-ULP drift between the incremental and
//! full paths is a failure rather than a rounding opinion.

use exact_kernel::{
    export, AlignItems, Dimension, Display, FlexDirection, JustifyContent, Kernel,
    MonospaceMeasurer, NodeType, Offer, Op, PositionType, PropId, StyleId, StyleProps,
};

struct Rng(u64);

impl Rng {
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
    }
    if rng.chance(2) {
        s.display = Display::Flex;
        s.mask.set(StyleId::Display);
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

fn frames(k: &Kernel) -> Vec<(u32, [u32; 4])> {
    k.rows(None)
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.id,
                [
                    r.frame.x.to_bits(),
                    r.frame.y.to_bits(),
                    r.frame.width.to_bits(),
                    r.frame.height.to_bits(),
                ],
            )
        })
        .collect()
}

fn run(seed: u64, rounds: usize) {
    let mut world = World {
        rng: Rng(seed),
        next_id: 1,
        live: Vec::new(),
        children: Vec::new(),
        log: Vec::new(),
    };
    let mut kernel = Kernel::with_monospace();
    let offer = Offer::definite(320.0, 480.0);
    kernel.apply(0, 0, &world.initial()).unwrap();
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
fn long_run_single_seed() {
    run(0x5eed_c0de, 200);
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
                align_items: Some(taffy::AlignItems::Center),
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
    let measure = |known: Size<Option<f32>>,
                   space: Size<AvailableSpace>,
                   _,
                   context: Option<&mut bool>,
                   _: &Style| {
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
