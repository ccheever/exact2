//! A change inside an absolutely positioned box replays at its containing
//! block (`absolute.rs`): a step strip's cells (`flex: 1`, `position:
//! relative`, so their row asks their content's sizes) gain and lose a
//! playhead dot. Each case compares every frame and scroll extent, bit for
//! bit, with a fresh engine.
use crate::{Kernel, MonospaceMeasurer, NodeType, Offer, Op, StyleId, StyleProps};
use crate::{StyleValue, ViewId};

const OFFER: Offer = Offer {
    width: crate::AxisOffer::Definite(402.0),
    height: crate::AxisOffer::Definite(874.0),
};

fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn text(s: &str) -> StyleValue<'_> {
    StyleValue::Text(s.into())
}

/// A page (1) holding a strip (2) of sixteen cells (10..26), each with a
/// light (30..46) in flow.
struct Strip {
    k: Kernel,
    batch: u64,
    /// The roots laid out, in order.
    roots: Vec<ViewId>,
}

impl Strip {
    fn new() -> Strip {
        let k = Kernel::with_monospace();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            style(
                2,
                &[
                    (StyleId::Display, text("flex")),
                    (StyleId::FlexDirection, text("row")),
                    (StyleId::ColumnGap, StyleValue::Number(4.0)),
                ],
            ),
        ];
        for cell in 10..26 {
            ops.push(Op::CreateView {
                id: cell,
                node_type: NodeType::View,
            });
            ops.push(style(
                cell,
                &[
                    (StyleId::Display, text("flex")),
                    (StyleId::FlexDirection, text("column")),
                    (StyleId::AlignItems, text("center")),
                    (StyleId::FlexGrow, StyleValue::Number(1.0)),
                    (StyleId::Height, StyleValue::Number(22.0)),
                    (StyleId::PositionType, text("relative")),
                ],
            ));
            ops.push(Op::CreateView {
                id: cell + 20,
                node_type: NodeType::View,
            });
            ops.push(style(
                cell + 20,
                &[
                    (StyleId::Width, StyleValue::Number(5.0)),
                    (StyleId::Height, StyleValue::Number(5.0)),
                ],
            ));
            ops.push(Op::SetChildren {
                id: cell,
                children: vec![cell + 20],
            });
        }
        ops.push(Op::SetChildren {
            id: 2,
            children: (10..26).collect(),
        });
        ops.push(Op::SetChildren {
            id: 1,
            children: vec![2],
        });
        ops.push(Op::AttachRoot { id: 1 });
        let mut strip = Strip {
            k,
            batch: 1,
            roots: vec![1],
        };
        strip.apply(&ops);
        strip
    }

    /// A playhead dot `id` in `cell`, `left` from its edge.
    fn dot(&mut self, id: ViewId, cell: ViewId, left: f64) -> Vec<Op> {
        vec![
            Op::CreateView {
                id,
                node_type: NodeType::View,
            },
            style(
                id,
                &[
                    (StyleId::PositionType, text("absolute")),
                    (StyleId::Top, StyleValue::Number(3.0)),
                    (StyleId::Left, StyleValue::Number(left)),
                    (StyleId::Width, StyleValue::Number(5.6)),
                    (StyleId::Height, StyleValue::Number(5.6)),
                ],
            ),
            Op::SetChildren {
                id: cell,
                children: vec![cell + 20, id],
            },
        ]
    }

    fn apply(&mut self, ops: &[Op]) {
        self.batch += 1;
        self.k.apply(0, self.batch, ops).unwrap();
        let mut fresh = self.k.rehydrate(Box::new(MonospaceMeasurer::default()));
        for root in &self.roots {
            self.k.compute_layout(*root, OFFER).unwrap();
            fresh.compute_layout(*root, OFFER).unwrap();
        }
        let (a, b) = (self.k.arena(), fresh.arena());
        for slot in a.iter_live() {
            let id = a.local_id(slot);
            let other = b.slot_of(id).unwrap();
            assert!(a.frame(slot).bits_eq(b.frame(other)), "frame of {id}");
            let bits = |(w, h): (f32, f32)| (w.to_bits(), h.to_bits());
            assert_eq!(
                bits(a.content(slot)),
                bits(b.content(other)),
                "scroll extent of {id}"
            );
        }
    }

    fn held(&self) -> usize {
        self.k.tree().boundary_replays
    }
}

#[test]
fn a_dot_made_moved_and_dropped_lays_out_only_its_cells() {
    let mut strip = Strip::new();
    let ops = strip.dot(100, 12, 3.0);
    strip.apply(&ops);
    assert_eq!(strip.held(), 1, "made in one cell");
    // Moved: dropped from one cell, made in the next.
    let mut ops = vec![
        Op::SetChildren {
            id: 12,
            children: vec![32],
        },
        Op::DestroyView { id: 100 },
    ];
    ops.extend(strip.dot(101, 13, 3.0));
    strip.apply(&ops);
    assert_eq!(strip.held(), 2, "one replay per cell");
    // Its own inset moves.
    strip.apply(&[style(101, &[(StyleId::Left, StyleValue::Number(6.0))])]);
    assert_eq!(strip.held(), 1);
}

/// A change under the second of two roots waits for that root's layout.
#[test]
fn a_dot_under_a_second_root_stays_local() {
    let mut strip = Strip::new();
    strip.roots = vec![300, 1];
    strip.apply(&[
        Op::CreateView {
            id: 300,
            node_type: NodeType::View,
        },
        Op::AttachRoot { id: 300 },
    ]);
    let ops = strip.dot(100, 12, 3.0);
    strip.apply(&ops);
    assert_eq!(strip.held(), 1);
}

#[test]
fn what_the_block_s_ancestors_read_lays_out_from_the_root() {
    // A dot that reaches past its cell moves the strip's scroll extent.
    let mut strip = Strip::new();
    let ops = strip.dot(100, 25, 40.0);
    strip.apply(&ops);
    assert_eq!(strip.held(), 0);
    // An in-flow child changes the cell's own content.
    let mut strip = Strip::new();
    strip.apply(&[style(32, &[(StyleId::Width, StyleValue::Number(30.0))])]);
    assert_eq!(strip.held(), 0);
    // A dot made in a cell whose light is resized in the same batch.
    let mut strip = Strip::new();
    let mut ops = strip.dot(100, 12, 3.0);
    ops.push(style(32, &[(StyleId::Width, StyleValue::Number(30.0))]));
    strip.apply(&ops);
    assert_eq!(strip.held(), 0);
    // The dot joins the flow.
    let mut strip = Strip::new();
    let ops = strip.dot(100, 12, 3.0);
    strip.apply(&ops);
    strip.apply(&[style(100, &[(StyleId::PositionType, text("relative"))])]);
    assert_eq!(strip.held(), 0);
    // A block cell: margins inside it may collapse through it.
    let mut strip = Strip::new();
    strip.apply(&[style(12, &[(StyleId::Display, text("block"))])]);
    let ops = strip.dot(100, 12, 3.0);
    strip.apply(&ops);
    assert_eq!(strip.held(), 0);
    // A static cell does not contain the dot: the strip's page does.
    let mut strip = Strip::new();
    strip.apply(&[style(12, &[(StyleId::PositionType, text("static"))])]);
    let ops = strip.dot(100, 12, 3.0);
    strip.apply(&ops);
    assert_eq!(strip.held(), 0);
}
