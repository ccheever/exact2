//! Rows inserted at the top of a long list.
use crate::support::reader::{number, style, text};
use exact_kernel::StyleId::*;
use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId, StyleValue, TextMeasureRequest,
    TextMeasurer, TextMetrics,
};
use std::{cell::Cell, rc::Rc};

struct Counting(Rc<Cell<usize>>, MonospaceMeasurer);
impl TextMeasurer for Counting {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        self.0.set(self.0.get() + 1);
        self.1.measure(r)
    }
}

const LIST: u32 = 3;
const SECTION: u32 = 4;

fn row(ops: &mut Vec<Op>, base: u32, n: u32) -> u32 {
    let [swipe, content, mark, column, line, from, date, subject, preview, chevron, button] =
        std::array::from_fn(|i| base + i as u32);
    for (id, node_type) in [
        (swipe, NodeType::ScrollView),
        (content, NodeType::View),
        (mark, NodeType::View),
        (column, NodeType::View),
        (line, NodeType::View),
        (from, NodeType::Text),
        (date, NodeType::Text),
        (subject, NodeType::Text),
        (preview, NodeType::Text),
        (chevron, NodeType::View),
        (button, NodeType::View),
    ] {
        ops.push(Op::CreateView { id, node_type });
    }
    ops.extend([
        style(
            swipe,
            &[
                (Display, text("flex")),
                (OverflowX, text("scroll")),
                (OverflowY, text("hidden")),
            ],
        ),
        style(
            content,
            &[
                (Display, text("flex")),
                (FlexDirection, text("row")),
                (Width, StyleValue::Percent(100.0)),
                (BoxSizing, text("border-box")),
                (FlexShrink, number(0.0)),
                (PaddingTop, number(12.0)),
                (PaddingBottom, number(12.0)),
                (PaddingLeft, number(16.0)),
                (PaddingRight, number(16.0)),
                (ColumnGap, number(10.0)),
            ],
        ),
        style(
            mark,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (Width, number(10.0)),
                (FlexShrink, number(0.0)),
            ],
        ),
        style(
            column,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (FlexGrow, number(1.0)),
                (FlexBasis, StyleValue::Percent(0.0)),
                (MinWidth, number(0.0)),
                (RowGap, number(3.0)),
            ],
        ),
        style(
            line,
            &[
                (Display, text("flex")),
                (FlexDirection, text("row")),
                (FlexWrap, text("wrap")),
                (AlignItems, text("baseline")),
                (ColumnGap, number(8.0)),
            ],
        ),
        style(
            from,
            &[
                (FlexGrow, number(1.0)),
                (FlexBasis, number(192.0)),
                (MinWidth, number(0.0)),
            ],
        ),
        style(chevron, &[(Width, number(8.0)), (Height, number(14.0))]),
        style(button, &[(Width, number(74.0)), (FlexShrink, number(0.0))]),
        Op::SetChildren {
            id: swipe,
            children: vec![content, button],
        },
        Op::SetChildren {
            id: content,
            children: vec![mark, column, chevron],
        },
        Op::SetChildren {
            id: column,
            children: vec![line, subject, preview],
        },
        Op::SetChildren {
            id: line,
            children: vec![from, date],
        },
    ]);
    for (id, value) in [
        (from, format!("Sender {n}")),
        (date, "9:41 AM".to_string()),
        (subject, format!("Subject of message {n}")),
        (
            preview,
            format!("The preview of message {n}, long enough to wrap onto a second line."),
        ),
    ] {
        ops.push(Op::SetProp {
            id,
            prop: PropId::Text,
            value: value.into(),
        });
    }
    swipe
}

/// Two rows inserted at the top of a list of 330 lay out those two and leave
/// the others' layouts as they were: only their positions change (a
/// UITableView insert measures the new cells alone).
#[test]
fn rows_inserted_at_the_top_of_a_long_list_measure_only_themselves() {
    let count = Rc::new(Cell::new(0));
    let mut k = Kernel::new(Box::new(Counting(
        count.clone(),
        MonospaceMeasurer::default(),
    )));
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: LIST,
            node_type: NodeType::List,
        },
        Op::CreateView {
            id: SECTION,
            node_type: NodeType::View,
        },
        style(
            1,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (Width, StyleValue::Percent(100.0)),
                (Height, StyleValue::Percent(100.0)),
            ],
        ),
        style(
            LIST,
            &[
                (FlexGrow, number(1.0)),
                (MinHeight, number(0.0)),
                (Display, text("flex")),
                (FlexDirection, text("column")),
            ],
        ),
        Op::SetChildren {
            id: 1,
            children: vec![LIST],
        },
        Op::SetChildren {
            id: LIST,
            children: vec![SECTION],
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut rows = Vec::new();
    for n in 0..330 {
        rows.push(row(&mut ops, 100 + n * 16, n));
    }
    ops.push(Op::SetChildren {
        id: SECTION,
        children: rows.clone(),
    });
    k.apply(0, 1, &ops).unwrap();
    let offer = Offer::definite(402., 874.);
    k.compute_layout(1, offer).unwrap();
    let per_row = count.get() / rows.len();
    let before: Vec<_> = rows.iter().map(|&r| k.node(r).unwrap().frame).collect();
    count.set(0);
    let mut ops = Vec::new();
    let new = [row(&mut ops, 90_000, 9_000), row(&mut ops, 90_100, 9_001)];
    ops.push(Op::SetChildren {
        id: SECTION,
        children: new.iter().chain(&rows).copied().collect(),
    });
    k.apply(0, 2, &ops).unwrap();
    k.compute_layout(1, offer).unwrap();
    let shift = k.node(new[0]).unwrap().frame.height + k.node(new[1]).unwrap().frame.height;
    for (&r, b) in rows.iter().zip(&before) {
        let f = k.node(r).unwrap().frame;
        assert!((f.y - b.y - shift).abs() < 0.01 && (f.height, f.width) == (b.height, b.width));
    }
    // The new rows' texts are longer than any before them: the list's
    // max-content width grows, which a stretched list must not read.
    assert!(
        count.get() <= 3 * per_row,
        "{} measures, {per_row} a row",
        count.get()
    );
}
