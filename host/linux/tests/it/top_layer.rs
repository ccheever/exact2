//! Stage 14 B on Linux: with no shell to cover it (headless, a framebuffer)
//! a popover is laid out in the viewport's block, a space of its own, which
//! is no part of its scroller's extent.
use exact_kernel::*;

#[test]
fn a_popover_in_its_block_adds_nothing_to_its_scrollers_extent() {
    let mut k = Kernel::with_monospace();
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
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::ScrollView,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        style(
            2,
            &[
                (StyleId::Width, StyleValue::Number(200.0)),
                (StyleId::Height, StyleValue::Number(100.0)),
            ],
        ),
        style(
            3,
            &[
                (StyleId::Width, StyleValue::Text("50%".into())),
                (StyleId::Height, StyleValue::Number(30.0)),
            ],
        ),
        Op::SetProp {
            id: 3,
            prop: PropId::Popover,
            value: "auto".into(),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    assert_eq!(
        k.node(3).unwrap().frame.width,
        400.0,
        "in the viewport's block"
    );
    let scroller = k.node(2).unwrap();
    assert_eq!(
        exact_linux::paint::content_size(&scroller, &k),
        (200.0, 100.0)
    );
}
