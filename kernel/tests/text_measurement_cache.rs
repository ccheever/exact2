//! Exact-offer reuse must preserve the injected measurer's inputs and invalidation.
mod reader {
    use exact_kernel::{Op, StyleId, StyleProps, StyleValue};
    pub fn number(value: f64) -> StyleValue {
        StyleValue::Number(value)
    }
    pub fn text(value: &str) -> StyleValue {
        StyleValue::Text(value.into())
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
}

use exact_kernel::{
    AxisOffer, Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId, StyleId, TextMeasureRequest,
    TextMeasurer, TextMetrics,
};
fn initial() -> Vec<Op> {
    use reader::{number, style, text};
    use StyleId::*;
    vec![
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
        style(
            1,
            &[(Display, text("flex")), (FlexDirection, text("column"))],
        ),
        style(2, &[(Width, number(300.0)), (Height, number(1000.0))]),
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "some text that wraps around a paragraph ".repeat(8).into(),
        },
        Op::SetChildren {
            id: 1,
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
        Op::AttachRoot { id: 1 },
    ]
}

#[test]
fn mutations_and_slot_reuse_match_fresh_measurement() {
    use reader::{number, style};
    use StyleId::*;
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &initial()).unwrap();
    let offer = Offer::definite(600.0, 800.0);
    let mutations = vec![
        vec![],
        vec![style(2, &[(Height, number(2000.0))])],
        vec![style(2, &[(Width, number(180.0))])],
        vec![style(1, &[(FontSize, number(23.0))])],
        vec![Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "short".into(),
        }],
        vec![style(4, &[(FontSize, number(12.0))])],
        vec![Op::DestroyView { id: 4 }],
        vec![
            Op::CreateView {
                id: 4,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "replacement text".repeat(30).into(),
            },
            Op::SetChildren {
                id: 3,
                children: vec![4],
            },
        ],
        vec![Op::SetChildren {
            id: 2,
            children: vec![4, 3],
        }],
    ];
    for (round, ops) in mutations.into_iter().enumerate() {
        kernel.apply(round as u32 + 1, 1, &ops).unwrap();
        kernel.compute_layout(1, offer).unwrap();
        let mut fresh = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, offer).unwrap();
        for id in 1..=4 {
            if let Some(node) = kernel.node(id) {
                let expected = fresh.node(id).unwrap();
                assert_eq!(node.frame, expected.frame, "round {round}, node {id}");
                assert_eq!(node.content, expected.content, "round {round}, node {id}");
            }
        }
    }
}

#[test]
fn vertical_offers_remain_part_of_the_measurement_key() {
    struct HeightSensitive;
    impl TextMeasurer for HeightSensitive {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            TextMetrics {
                width: match request.height {
                    AxisOffer::Definite(h) => h / 2.0,
                    _ => 17.0,
                },
                height: 20.0,
                first_baseline: Some(15.0),
            }
        }
    }
    let mut kernel = Kernel::new(Box::new(HeightSensitive));
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 1,
                    prop: PropId::Text,
                    value: "height sensitive".into(),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    for height in [800.0, 350.0, 1200.0, 800.0] {
        let offer = Offer {
            width: AxisOffer::MaxContent,
            height: AxisOffer::Definite(height),
        };
        kernel.compute_layout(1, offer).unwrap();
        let mut fresh = kernel.rehydrate(Box::new(HeightSensitive));
        fresh.compute_layout(1, offer).unwrap();
        assert_eq!(kernel.node(1).unwrap().frame, fresh.node(1).unwrap().frame);
        assert_eq!(kernel.node(1).unwrap().frame.width, height / 2.0);
    }
}
