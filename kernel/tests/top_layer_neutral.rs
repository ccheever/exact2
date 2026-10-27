//! The top layer beside the neutral look's route header (LLP 1115.000 D17),
//! whose platform start is `position: sticky`: a binary of its own, because
//! the metrics column is the process's. A popover keeps a sticky only its
//! author wrote; the start's is no author's, so such a header with
//! `popover` goes to the top layer as any other popover does.
use exact_kernel::style::metrics::{self, Column};
use exact_kernel::{
    Kernel, NodeType, Offer, Op, PositionType, PropId, PropValue, StyleId, StyleProps, StyleValue,
    ViewId,
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

fn prop(id: ViewId, prop: PropId, value: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: PropValue::Str(value.into()),
    }
}

/// A route (1, a column) holding a header (2, `top: 10`) and a box after it
/// (3); the header a popover, and sticky as its author wrote it when `sticky`.
fn route(popover: bool, sticky: bool) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=3 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.push(prop(1, PropId::NavigationKey, "1"));
    ops.push(style(
        1,
        &[
            (StyleId::Display, StyleValue::Text("flex".into())),
            (StyleId::FlexDirection, StyleValue::Text("column".into())),
        ],
    ));
    ops.push(prop(2, PropId::SemanticTag, "header"));
    ops.push(style(
        2,
        &[
            (StyleId::Height, StyleValue::Number(40.0)),
            (StyleId::Top, StyleValue::Number(10.0)),
        ],
    ));
    if sticky {
        // Written, as the runner marks a row its author wrote (LLP 1115.000 D2).
        let mut patch = StyleProps::default();
        patch
            .set_dynamic(StyleId::PositionType, &StyleValue::Text("sticky".into()))
            .unwrap();
        patch.written = patch.mask;
        ops.push(Op::SetStyle {
            id: 2,
            patch: Box::new(patch),
        });
    }
    if popover {
        ops.push(prop(2, PropId::Popover, "auto"));
    }
    ops.push(style(3, &[(StyleId::Height, StyleValue::Number(50.0))]));
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2, 3],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(390.0, 800.0)).unwrap();
    k
}

#[test]
fn a_popover_keeps_only_a_sticky_its_author_wrote() {
    metrics::set_column(Column::Neutral);

    let k = route(false, false);
    assert_eq!(
        k.node(2).unwrap().layout_position(),
        PositionType::Sticky,
        "the neutral route header's start is sticky"
    );
    let below = |k: &Kernel| k.node(2).unwrap().frame.height;
    // A constraint, and a place among the sticky nodes a host is sent.
    let sticks = |k: &Kernel| {
        let key = k.node(2).unwrap().key;
        let listed = k.sticky_nodes().contains(&key);
        assert_eq!(k.sticky_constraint(key).is_some(), listed);
        listed
    };
    assert_eq!(
        k.node(3).unwrap().frame.y,
        below(&k),
        "in flow: the box after it"
    );
    assert_eq!(
        k.node(2).unwrap().frame.y,
        0.0,
        "a sticky box's inset is no offset"
    );
    assert!(sticks(&k), "the sticky machinery takes it");

    let k = route(true, false);
    assert_eq!(
        k.node(2).unwrap().layout_position(),
        PositionType::Absolute,
        "a popover: the start's sticky is no author's"
    );
    assert_eq!(
        k.node(3).unwrap().frame.y,
        0.0,
        "out of flow: it takes no room"
    );
    assert_eq!(
        (k.node(2).unwrap().frame.y, k.host_block_size(2).is_some()),
        (4.0, true),
        "in its viewport block: the default margin, without the sticky inset"
    );
    assert!(!sticks(&k), "and the sticky machinery lets it go");

    let k = route(true, true);
    assert_eq!(
        k.node(2).unwrap().layout_position(),
        PositionType::Sticky,
        "a popover its author made sticky keeps it"
    );
    assert_eq!(
        k.node(3).unwrap().frame.y,
        below(&k) + 8.,
        "in flow, including both default popover margins"
    );
    assert!(sticks(&k), "sticky");
}
