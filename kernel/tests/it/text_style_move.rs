//! LLP 1115.000 D12.2: a bare `font-size` style fills a weight only where
//! nothing above says one. A container moved under a parent that says the
//! weight its own bare style filled computes the same weight, but a bare
//! style below it no longer fills: that paragraph's weight and its metric
//! revision change.
use exact_kernel::*;

fn patch(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut s = StyleProps::default();
    for (row, value) in rows {
        s.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}

fn children(id: u32, children: &[u32]) -> Op {
    Op::SetChildren {
        id,
        children: children.to_vec(),
    }
}

#[test]
fn a_move_under_a_said_weight_reweights_a_bare_style_below() {
    let style = |s: &'static str| StyleValue::Text(s.into());
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        0,
        &[
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
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::Text,
            },
            patch(2, &[(StyleId::FontWeight, StyleValue::Number(600.0))]),
            patch(4, &[(StyleId::FontSize, style("-exact-headline"))]),
            patch(5, &[(StyleId::FontSize, style("-exact-body"))]),
            Op::SetProp {
                id: 5,
                prop: PropId::Text,
                value: PropValue::Str("Body".into()),
            },
            children(1, &[2, 3]),
            children(3, &[4]),
            children(4, &[5]),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400., 400.)).unwrap();
    let weight = |k: &Kernel, id| k.node(id).unwrap().text_style().font_weight;
    assert_eq!((weight(&k, 4), weight(&k, 5)), (600, 400));
    let before = k.node(5).unwrap().paragraph_stamp().unwrap();
    k.apply(0, 1, &[children(3, &[]), children(2, &[4])])
        .unwrap();
    assert_eq!(
        (weight(&k, 4), weight(&k, 5)),
        (600, 600),
        "said above now: the body's bare weight does not fill"
    );
    let after = k.node(5).unwrap().paragraph_stamp().unwrap();
    assert!(!after.same_metrics(&before), "remeasured at the new weight");
}
