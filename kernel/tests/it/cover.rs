//! What a native container covers of a box (LLP 1075.003 §3.5, §3.7): a
//! navigation bar over a route's top edge adds to its padding, the header
//! whose heading the bar shows takes no space, and clearing either lays the
//! route out as authored again.

use exact_kernel::{Env, HostCover, Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue};

fn style(rows: &[(StyleId, StyleValue)]) -> StyleProps {
    let mut s = StyleProps::default();
    for (id, value) in rows {
        s.set_dynamic(*id, value).unwrap();
    }
    s
}

/// A root, one route filling it (a flex column padded by the top inset as
/// an app under `viewport-fit=cover` pads it), a 52-point header and a list.
fn route() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let column = |extra: &[(StyleId, StyleValue)]| {
        let mut rows = vec![
            (StyleId::Display, StyleValue::Text("flex".into())),
            (StyleId::FlexDirection, StyleValue::Text("column".into())),
        ];
        rows.extend_from_slice(extra);
        style(&rows)
    };
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(style(&[(StyleId::Height, StyleValue::Percent(100.0))])),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(column(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Top, StyleValue::Number(0.0)),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Right, StyleValue::Number(0.0)),
                (
                    StyleId::PaddingTop,
                    StyleValue::Text("env(safe-area-inset-top)".into()),
                ),
            ])),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(52.0)),
                (StyleId::FlexShrink, StyleValue::Number(0.0)),
            ])),
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 4,
            patch: Box::new(style(&[(StyleId::FlexGrow, StyleValue::Number(1.0))])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn frame(kernel: &mut Kernel, id: u32) -> (f32, f32, f32, f32) {
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = kernel.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn a_bar_over_the_top_edge_adds_to_the_authored_padding() {
    let mut kernel = route();
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    assert_eq!(
        frame(&mut kernel, 3),
        (0.0, 62.0, 390.0, 52.0),
        "as authored"
    );
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 3),
        (0.0, 106.0, 390.0, 52.0),
        "the bar's 44 below the status bar's 62"
    );
    assert_eq!(frame(&mut kernel, 4), (0.0, 158.0, 390.0, 686.0));
}

#[test]
fn a_header_the_bar_replaces_takes_no_space_until_the_cover_is_cleared() {
    let mut kernel = route();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(frame(&mut kernel, 3), (0.0, 0.0, 0.0, 0.0));
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 44.0, 390.0, 800.0),
        "the list starts under the bar"
    );
    kernel.set_host_cover(3, None).unwrap();
    kernel.set_host_cover(2, None).unwrap();
    assert_eq!(frame(&mut kernel, 4), (0.0, 52.0, 390.0, 792.0));
}

#[test]
fn a_cover_on_a_percent_padding_keeps_the_percent() {
    let mut kernel = route();
    let patch = style(&[(StyleId::PaddingTop, StyleValue::Percent(10.0))]);
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    // Padding percentages resolve against the containing block's width.
    assert_eq!(frame(&mut kernel, 3), (0.0, 83.0, 390.0, 52.0));
}

#[test]
fn a_negative_or_nonfinite_edge_is_refused() {
    let mut kernel = route();
    assert!(kernel
        .set_host_cover(2, Some(HostCover::Edges([-1.0, 0.0, 0.0, 0.0])))
        .is_err());
    assert!(kernel
        .set_host_cover(2, Some(HostCover::Edges([f32::NAN, 0.0, 0.0, 0.0])))
        .is_err());
    assert!(kernel.set_host_cover(99, Some(HostCover::Whole)).is_err());
}
