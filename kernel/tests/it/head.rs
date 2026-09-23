//! A document's metadata (`head`, LLP 1048.003 D1) takes no space: in a
//! gapped row it is as if absent, whatever display rows it is given.
use exact_kernel::{Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue, ViewId};

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

#[test]
fn a_head_takes_no_space_in_a_gapped_row() {
    let text = |s: &str| StyleValue::Text(s.into());
    let n = StyleValue::Number;
    let mut kernel = Kernel::with_monospace();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                style(
                    1,
                    &[
                        (StyleId::Display, text("flex")),
                        (StyleId::ColumnGap, n(10.0)),
                    ],
                ),
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::View,
                },
                style(2, &[(StyleId::Width, n(20.0)), (StyleId::Height, n(20.0))]),
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::Head,
                },
                // Rows a head is given change nothing: it is never laid out.
                style(
                    3,
                    &[(StyleId::Display, text("flex")), (StyleId::Width, n(50.0))],
                ),
                Op::CreateView {
                    id: 4,
                    node_type: NodeType::View,
                },
                style(4, &[(StyleId::Width, n(20.0)), (StyleId::Height, n(20.0))]),
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3, 4],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 300.0))
        .unwrap();
    let frame = |id| kernel.node(id).unwrap().frame;
    assert_eq!((frame(3).width, frame(3).height), (0.0, 0.0));
    // One gap between the two boxes, as if the head were not there.
    assert_eq!(frame(4).x, 30.0);
}
