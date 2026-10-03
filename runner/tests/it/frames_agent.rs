//! @ref LLP 1080.001 D2 — the kernel's half of `layout agree`: every live
//! node's parent-relative frame in preorder, its own `display: none` and
//! transform rows as bits, and a cap that says the list is incomplete.
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
fn frames_answer_each_node_in_preorder_with_its_bits() {
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
    let mut moved = StyleProps {
        width: Dimension::Points(100.),
        height: Dimension::Points(50.),
        ..Default::default()
    };
    moved.mask.set(StyleId::Width);
    moved.mask.set(StyleId::Height);
    moved.mask.set(StyleId::Translate);
    let mut gone = StyleProps {
        display: exact_kernel::Display::None,
        ..Default::default()
    };
    gone.mask.set(StyleId::Display);
    let ops = [
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(moved),
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(gone),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::AttachRoot { id: 1 },
    ];
    runner.kernel_mut().apply(0, 0, &ops).unwrap();
    runner
        .kernel_mut()
        .compute_layout(1, Offer::definite(600., 400.))
        .unwrap();
    let json = agent::handle(&runner, r#"{"op":"frames"}"#);
    assert!(
        json.contains(
            r#""nodes":[[1,null,0,0,600,400,0],[2,1,0,0,100,50,2],[3,1,0,0,0,0,1]],"complete":true"#
        ),
        "{json}"
    );
    let cut = agent::handle(&runner, r#"{"op":"frames","limit":2}"#);
    assert!(
        cut.ends_with(r#"[2,1,0,0,100,50,2]],"complete":false}"#),
        "{cut}"
    );
}
