//! LLP 1035.000.001: the reader's centered, width-constrained flex column.
use exact_kernel::StyleId::*;
use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Op, PropId, StyleId, StyleProps, StyleValue,
};

pub const LAST: u32 = 16;
pub const TEXT: &str = "reader ";
pub const LONG_TEXT: &str = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop reader reader reader reader reader reader reader reader reader reader ";
pub fn measurer() -> MonospaceMeasurer {
    MonospaceMeasurer {
        advance_em: 0.625,
        ..MonospaceMeasurer::default()
    }
}
pub fn kernel() -> Kernel {
    Kernel::new(Box::new(measurer()))
}
pub fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
pub fn number(value: f64) -> StyleValue {
    StyleValue::Number(value)
}
pub fn text(value: &str) -> StyleValue {
    StyleValue::Text(value.into())
}
pub fn initial(workaround: bool, border_box: bool, long_word: bool) -> Vec<Op> {
    let mut ops: Vec<_> = (1..=LAST)
        .map(|id| Op::CreateView {
            id,
            node_type: if id == 2 {
                NodeType::ScrollView
            } else {
                NodeType::View
            },
        })
        .collect();
    ops.extend([
        style(
            1,
            &[(Width, StyleValue::Percent(100.0)), (Height, number(800.0))],
        ),
        style(
            2,
            &[
                (FlexGrow, number(1.0)),
                (FlexBasis, number(0.0)),
                (Height, StyleValue::Percent(100.0)),
                (OverflowX, text("hidden")),
            ],
        ),
        style(
            3,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (Width, StyleValue::Percent(100.0)),
                (AlignItems, text("center")),
                (PaddingTop, number(24.0)),
                (PaddingBottom, number(72.0)),
            ],
        ),
        style(
            4,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (
                    Width,
                    if workaround {
                        number(720.0)
                    } else {
                        StyleValue::Percent(100.0)
                    },
                ),
                (
                    MaxWidth,
                    if workaround {
                        StyleValue::Percent(100.0)
                    } else {
                        number(720.0)
                    },
                ),
                (
                    BoxSizing,
                    text(if border_box {
                        "border-box"
                    } else {
                        "content-box"
                    }),
                ),
                (PaddingLeft, number(32.0)),
                (PaddingRight, number(32.0)),
                (RowGap, number(2.0)),
                (FontSize, number(16.0)),
                (LineHeight, text("26px")),
            ],
        ),
        Op::CreateView {
            id: 70,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 71,
            node_type: NodeType::View,
        },
        style(
            70,
            &[
                (Display, text("flex")),
                (FlexDirection, text("column")),
                (Width, StyleValue::Percent(100.0)),
                (Height, StyleValue::Percent(100.0)),
            ],
        ),
        style(
            71,
            &[
                (Display, text("flex")),
                (FlexDirection, text("row")),
                (Width, StyleValue::Percent(100.0)),
                (FlexGrow, number(1.0)),
                (FlexBasis, number(0.0)),
                (MinHeight, number(0.0)),
            ],
        ),
        Op::SetChildren {
            id: 1,
            children: vec![70],
        },
        Op::SetChildren {
            id: 70,
            children: vec![71],
        },
        Op::SetChildren {
            id: 71,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 3,
            children: vec![4],
        },
        Op::SetChildren {
            id: 4,
            children: (5..=LAST).collect(),
        },
        Op::AttachRoot { id: 1 },
    ]);
    for id in 5..=LAST {
        ops.extend(block(id, long_word));
    }
    ops
}

pub fn block(id: u32, long_word: bool) -> Vec<Op> {
    Vec::from([
        Op::CreateView {
            id: id + 100,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: id + 200,
            node_type: NodeType::Text,
        },
        style(
            id + 100,
            &[
                (Width, StyleValue::Percent(100.0)),
                (BoxSizing, text("border-box")),
                (PaddingLeft, number(14.0)),
                (PaddingRight, number(14.0)),
                (PaddingTop, number(14.0)),
                (PaddingBottom, number(14.0)),
            ],
        ),
        Op::SetChildren {
            id: id + 100,
            children: vec![id + 200],
        },
        style(id, &[(Width, StyleValue::Percent(100.0))]),
        Op::SetChildren {
            id,
            children: vec![id + 100],
        },
        Op::SetProp {
            id: id + 200,
            prop: PropId::Text,
            value: if long_word {
                LONG_TEXT.repeat(6)
            } else {
                TEXT.repeat(60)
            }
            .into(),
        },
    ])
}
