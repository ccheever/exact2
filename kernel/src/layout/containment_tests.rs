//! A virtualized list's window changes replay at the list (LLP 1044 §4.5):
//! a flex column holds a header and a `flex: 1` clipping list of rows and
//! spacers, as `list virtualized=true` lowers. Each case compares every frame
//! and scroll extent, bit for bit, with a fresh engine.
use crate::{Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId, StyleId, StyleProps};
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
fn text(s: &str) -> StyleValue {
    StyleValue::Text(s.into())
}

/// The shape, with rows `first..first + count` mounted between spacers.
struct List {
    k: Kernel,
    next: ViewId,
    rows: Vec<(usize, ViewId, ViewId)>,
    spacers: [ViewId; 2],
    first_item: bool,
}

const LIST: ViewId = 3;

impl List {
    /// `first_item` puts the list before the header, where the column
    /// takes its own baseline from it.
    fn new(first_item: bool, list_rows: &[(StyleId, StyleValue)]) -> List {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: LIST,
                node_type: NodeType::List,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::View,
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "Messages".into(),
            },
            style(
                1,
                &[
                    (StyleId::Display, text("flex")),
                    (StyleId::FlexDirection, text("column")),
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::Height, StyleValue::Percent(100.0)),
                ],
            ),
            style(2, &[(StyleId::FlexShrink, StyleValue::Number(0.0))]),
            style(
                LIST,
                &[
                    (StyleId::FlexGrow, StyleValue::Number(1.0)),
                    (StyleId::FlexBasis, StyleValue::Percent(0.0)),
                    (StyleId::MinHeight, StyleValue::Number(0.0)),
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::OverflowX, text("hidden")),
                ],
            ),
            style(LIST, list_rows),
            Op::SetChildren {
                id: 1,
                children: if first_item {
                    vec![LIST, 2]
                } else {
                    vec![2, LIST]
                },
            },
            Op::AttachRoot { id: 1 },
        ];
        for spacer in [4, 5] {
            ops.push(style(
                spacer,
                &[
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::FlexShrink, StyleValue::Number(0.0)),
                ],
            ));
        }
        k.apply(0, 1, &ops).unwrap();
        let mut list = List {
            k,
            next: 100,
            rows: Vec::new(),
            spacers: [4, 5],
            first_item,
        };
        list.window(0, 12);
        list
    }

    /// A row as the runner builds one: a flex column wrapper, a padded
    /// column, a title and a body.
    fn row(&mut self, ops: &mut Vec<Op>, index: usize) -> (ViewId, ViewId) {
        let [wrapper, column, title, body] = [0, 1, 2, 3].map(|i| self.next + i);
        self.next += 4;
        for (id, node_type) in [
            (wrapper, NodeType::View),
            (column, NodeType::View),
            (title, NodeType::Text),
            (body, NodeType::Text),
        ] {
            ops.push(Op::CreateView { id, node_type });
        }
        ops.extend([
            style(
                wrapper,
                &[
                    (StyleId::Display, text("flex")),
                    (StyleId::FlexDirection, text("column")),
                    (StyleId::FlexShrink, StyleValue::Number(0.0)),
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::BoxSizing, text("border-box")),
                ],
            ),
            style(
                column,
                &[
                    (StyleId::Display, text("flex")),
                    (StyleId::FlexDirection, text("column")),
                    (StyleId::PaddingTop, StyleValue::Number(21.0)),
                    (StyleId::PaddingLeft, StyleValue::Number(16.0)),
                    (StyleId::RowGap, StyleValue::Number(10.0)),
                ],
            ),
            Op::SetProp {
                id: title,
                prop: PropId::Text,
                value: format!("Message {index}").into(),
            },
            Op::SetProp {
                id: body,
                prop: PropId::Text,
                value: "Yes! I found a place near the park. "
                    .repeat(1 + index % 3)
                    .into(),
            },
            Op::SetChildren {
                id: column,
                children: vec![title, body],
            },
            Op::SetChildren {
                id: wrapper,
                children: vec![column],
            },
        ]);
        (wrapper, body)
    }

    /// Mount rows `first..first + count`, reusing mounted ones, with the
    /// spacers standing in for the rest at 130 points a row.
    fn window(&mut self, first: usize, count: usize) {
        let mut ops = Vec::new();
        let mut rows = Vec::new();
        for index in first..first + count {
            match self.rows.iter().position(|r| r.0 == index) {
                Some(at) => rows.push(self.rows.remove(at)),
                None => {
                    let (wrapper, body) = self.row(&mut ops, index);
                    rows.push((index, wrapper, body));
                }
            }
        }
        for (_, wrapper, _) in self.rows.drain(..) {
            ops.push(Op::DestroyView { id: wrapper });
        }
        self.rows = rows;
        let [top, bottom] = self.spacers;
        ops.push(style(
            top,
            &[(StyleId::Height, StyleValue::Number(first as f64 * 130.0))],
        ));
        ops.push(style(
            bottom,
            &[(
                StyleId::Height,
                StyleValue::Number((10_000 - first - count) as f64 * 130.0),
            )],
        ));
        let children = std::iter::once(top)
            .chain(self.rows.iter().map(|r| r.1))
            .chain(std::iter::once(bottom))
            .collect();
        ops.push(Op::SetChildren { id: LIST, children });
        self.apply(&ops);
    }

    fn apply(&mut self, ops: &[Op]) {
        let batch = self.next as u64;
        self.k.apply(0, batch, ops).unwrap();
        self.k.compute_layout(1, OFFER).unwrap();
        let mut fresh = self.k.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, OFFER).unwrap();
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
fn window_changes_measurement_and_inserts_lay_out_only_the_list() {
    let mut list = List::new(false, &[]);
    // Scrolling: rows leave the top, arrive at the bottom, spacers resize.
    for first in 1..30 {
        list.window(first, 12);
        assert_eq!(list.held(), 1, "window at {first}");
    }
    // A mounted row's content changes its height.
    let body = list.rows[4].2;
    list.apply(&[Op::SetProp {
        id: body,
        prop: PropId::Text,
        value: "Shorter.".into(),
    }]);
    assert_eq!(list.held(), 1);
    // A row is inserted in the middle of the window, then removed.
    let mut ops = Vec::new();
    let (wrapper, body) = list.row(&mut ops, 99_999);
    let mut children: Vec<_> = list.k.node(LIST).unwrap().children();
    children.insert(5, wrapper);
    ops.push(Op::SetChildren {
        id: LIST,
        children: children.clone(),
    });
    list.apply(&ops);
    assert_eq!(list.held(), 1);
    list.apply(&[Op::SetProp {
        id: body,
        prop: PropId::Text,
        value: "An inserted row grows. ".repeat(9).into(),
    }]);
    assert_eq!(list.held(), 1);
    children.remove(5);
    list.apply(&[
        Op::SetChildren { id: LIST, children },
        Op::DestroyView { id: wrapper },
    ]);
    assert_eq!(list.held(), 1);
    // The list's own frame never moved; nothing outside it was published.
    let header = list.k.arena().slot_of(2).unwrap();
    assert!(!list
        .k
        .arena()
        .flags(header)
        .has(crate::NodeFlags::GEOMETRY_CHANGED));
}

#[test]
fn changes_the_list_cannot_contain_lay_out_from_the_root() {
    // The column's first item gives it its baseline: a scroll that moves
    // the first row's text changes what the column reads.
    let mut list = List::new(true, &[]);
    assert!(list.first_item);
    list.window(3, 12);
    assert_eq!(list.held(), 0);
    // A list its content sizes (no flex, auto height) grows with its rows.
    let mut list = List::new(
        false,
        &[
            (StyleId::FlexGrow, StyleValue::Number(0.0)),
            (StyleId::FlexBasis, StyleValue::Auto),
        ],
    );
    list.window(0, 13);
    assert_eq!(list.held(), 0);
    // The header is outside the list.
    let mut list = List::new(false, &[]);
    list.apply(&[Op::SetProp {
        id: 2,
        prop: PropId::Text,
        value: "Messages and more messages".into(),
    }]);
    assert_eq!(list.held(), 0);
    // The list restyled in the batch that changes its rows: its parent asks
    // it something new, so its saved inputs are stale.
    let mut list = List::new(false, &[]);
    let body = list.rows[2].2;
    list.apply(&[
        Op::SetProp {
            id: body,
            prop: PropId::Text,
            value: "Changed.".into(),
        },
        style(LIST, &[(StyleId::FlexGrow, StyleValue::Number(0.0))]),
    ]);
    assert_eq!(list.held(), 0);
}
