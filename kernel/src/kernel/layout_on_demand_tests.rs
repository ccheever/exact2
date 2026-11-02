use super::*;
use crate::{NodeType, PropId, StyleId, StyleValue};

fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn create(id: u32, node_type: NodeType) -> Op {
    Op::CreateView { id, node_type }
}
fn text(id: u32, value: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::Text,
        value: value.into(),
    }
}
fn frames(k: &Kernel) -> Vec<(ViewId, Frame, (f32, f32))> {
    let mut out: Vec<_> = (1..=9)
        .filter_map(|id| k.node(id).map(|n| (id, n.frame, n.content)))
        .collect();
    out.sort_by_key(|(id, ..)| *id);
    out
}

/// The same commits, before and after a first layout, lay out the same
/// in a kernel that builds its engine tree on demand and in one that
/// mirrors every commit, and the on-demand one holds no engine node until
/// a layout is asked for.
#[test]
fn a_kernel_with_layout_on_demand_lays_out_as_a_mirrored_one() {
    let first = [
        create(1, NodeType::View),
        create(2, NodeType::View),
        create(3, NodeType::Text),
        create(4, NodeType::Image),
        create(5, NodeType::Text),
        create(6, NodeType::View),
        style(
            1,
            &[(StyleId::FlexDirection, StyleValue::Text("column".into()))],
        ),
        style(
            2,
            &[
                (StyleId::Width, StyleValue::Number(200.0)),
                (StyleId::Height, StyleValue::Number(80.0)),
            ],
        ),
        text(3, "a paragraph of text that wraps"),
        text(5, "short"),
        Op::SetChildren {
            id: 2,
            children: vec![3, 6],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 4, 5],
        },
        Op::AttachRoot { id: 1 },
    ];
    let second = [
        style(2, &[(StyleId::Width, StyleValue::Number(120.0))]),
        text(5, "a longer line of text now"),
        Op::DestroyView { id: 6 },
    ];
    let third = [
        create(7, NodeType::View),
        style(7, &[(StyleId::Height, StyleValue::Number(30.0))]),
        Op::SetChildren {
            id: 1,
            children: vec![2, 7, 4, 5],
        },
    ];
    let offer = Offer::definite(300.0, 600.0);
    let mut mirrored = Kernel::with_monospace();
    let mut on_demand = Kernel::with_monospace_on_demand();
    for k in [&mut mirrored, &mut on_demand] {
        k.apply(0, 1, &first).unwrap();
        k.apply(0, 2, &second).unwrap();
        k.set_intrinsic_size(4, Some((40.0, 20.0))).unwrap();
        k.set_env(Env::new(10.0, 0.0, 5.0, 0.0)).unwrap();
    }
    assert_eq!(on_demand.engine_nodes(), 0);
    assert_eq!(mirrored.engine_nodes(), mirrored.live_count());
    for k in [&mut mirrored, &mut on_demand] {
        k.compute_layout(1, offer).unwrap();
    }
    assert_eq!(on_demand.engine_nodes(), on_demand.live_count());
    assert_eq!(frames(&on_demand), frames(&mirrored));
    let box2 = on_demand.node(2).unwrap().frame;
    assert_eq!((box2.width, box2.height), (120.0, 80.0));
    assert!(on_demand.node(5).unwrap().frame.y >= 80.0);
    // Mirrored from its first layout on, it keeps up with later commits.
    for k in [&mut mirrored, &mut on_demand] {
        k.apply(0, 3, &third).unwrap();
        k.compute_layout(1, offer).unwrap();
    }
    assert_eq!(on_demand.engine_nodes(), on_demand.live_count());
    assert_eq!(frames(&on_demand), frames(&mirrored));
    assert_eq!(on_demand.node(7).unwrap().frame.height, 30.0);
    // A reset returns it to building on demand.
    on_demand.reset();
    on_demand.apply(0, 4, &first).unwrap();
    assert_eq!(on_demand.engine_nodes(), 0);
}
