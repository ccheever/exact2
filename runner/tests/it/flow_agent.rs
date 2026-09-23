//! @ref LLP 1043.000 §3 D4 — agent layout facts include exclusions and skips.
use exact_kernel::*;
use exact_plan::{builder::PlanBuilder, Value};
use exact_runner::{agent, DataError, DataSource, Runner};
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
}
#[test]
fn layout_reports_resolved_and_skipped_paragraphs() {
    let mut b = PlanBuilder::new(SCHEMA_DIGEST, 1);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let plan = b.finish().unwrap();
    let mut runner = Runner::boot(
        plan,
        Empty,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mut root = StyleProps {
        width: Dimension::Points(600.),
        height: Dimension::Points(400.),
        ..Default::default()
    };
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
    runner
        .kernel_mut()
        .apply(
            0,
            0,
            &[
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
                    value: PropValue::Str("Words".into()),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    runner
        .kernel_mut()
        .compute_layout(1, Offer::definite(600., 400.))
        .unwrap();
    let json = agent::node(&runner, 2);
    assert!(
        json.contains("\"flow_shapes\":[{\"kind\":\"Circle\",\"cx\":60,\"cy\":60,\"r\":60}]"),
        "{json}"
    );
    assert!(!json.contains("flow_skipped"));
    runner
        .kernel_mut()
        .apply(
            0,
            0,
            &[Op::ClearStyle {
                id: 2,
                mask: StyleMask::of(StyleId::Height),
            }],
        )
        .unwrap();
    let receipt = runner
        .kernel_mut()
        .compute_layout(1, Offer::definite(600., 400.))
        .unwrap();
    runner.report_flow_skipped(&receipt.flow_skipped);
    runner.report_flow_skipped(&receipt.flow_skipped);
    let json = agent::node(&runner, 2);
    assert!(json.contains("\"flow_skipped\":\"Taffy measured this paragraph's height; auto-height flow requires M8\""),"{json}");
    assert!(!json.contains("flow_shapes"));
    assert_eq!(
        runner
            .journal()
            .filter(|line| line.contains(
                "wrap-flow: text #2 has auto height and is not flowed (LLP 1043.000 stage 2)"
            ))
            .count(),
        1
    );
}
