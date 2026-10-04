//! The page's environment (LLP 1001 §2): an `env(safe-area-inset-*)` length
//! resolves against the insets the host sets, and a change of the insets
//! lays the tree out again — only the nodes that read them.

use exact_kernel::{
    Dimension, Edge, Env, Kernel, KernelError, LayoutError, MonospaceMeasurer, NodeType, Offer, Op,
    Rect, SegmentVar, StyleId, StyleProps, StyleValue,
};

/// A root padded by the insets (top plain, bottom plus 12) with one child.
fn tree() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(
        StyleId::PaddingTop,
        &StyleValue::Text("env(safe-area-inset-top)".into()),
    )
    .unwrap();
    root.set_dynamic(
        StyleId::PaddingBottom,
        &StyleValue::Text("calc(env(safe-area-inset-bottom) + 12px)".into()),
    )
    .unwrap();
    root.set_dynamic(StyleId::Height, &StyleValue::Percent(100.0))
        .unwrap();
    root.set_dynamic(StyleId::BoxSizing, &StyleValue::Text("border-box".into()))
        .unwrap();
    let mut child = StyleProps::default();
    child
        .set_dynamic(StyleId::Height, &StyleValue::Number(40.0))
        .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(child),
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

fn frame(kernel: &Kernel, id: u32) -> (f32, f32, f32, f32) {
    let f = kernel.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn env_lengths_are_zero_until_the_host_sets_the_insets() {
    let mut kernel = tree();
    assert_eq!(kernel.env(), Env::default());
    assert_eq!(
        kernel.node(1).unwrap().style.padding_top,
        Dimension::Env(Edge::Top, 0.0)
    );
    assert_eq!(
        kernel.node(1).unwrap().style.padding_bottom,
        Dimension::Env(Edge::Bottom, 12.0)
    );
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 390.0, 40.0));
}

#[test]
fn setting_the_insets_relayouts_the_nodes_that_read_them() {
    let mut kernel = tree();
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert!(kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    assert_eq!(
        kernel.env(),
        Env {
            viewport_width: 390.0,
            viewport_height: 844.0,
            ..Env::new(62.0, 0.0, 34.0, 0.0)
        }
    );
    let receipt = kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert_eq!(
        frame(&kernel, 2),
        (0.0, 62.0, 390.0, 40.0),
        "the child sits under the top inset"
    );
    assert_eq!(
        receipt.changed.len(),
        1,
        "only the child moved: {receipt:?}"
    );
    // The same insets again: nothing to re-derive.
    assert!(!kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    // A rotation: new insets, a new layout.
    assert!(kernel.set_env(Env::new(0.0, 62.0, 21.0, 62.0)).unwrap());
    kernel
        .compute_layout(1, Offer::definite(844.0, 390.0))
        .unwrap();
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 844.0, 40.0));
}

#[test]
fn a_tree_without_env_lengths_has_nothing_to_relayout() {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::PaddingTop, &StyleValue::Number(8.0))
        .unwrap();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(root),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    assert!(!kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    assert_eq!(kernel.env().top, 62.0, "the environment is kept regardless");
}

#[test]
fn a_non_finite_inset_is_refused_and_the_environment_survives_a_reset() {
    let mut kernel = tree();
    assert_eq!(
        kernel.set_env(Env::new(f32::NAN, 0.0, 0.0, 0.0)),
        Err(KernelError::from(LayoutError::InvalidEnv))
    );
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    kernel.reset();
    assert_eq!(
        kernel.env(),
        Env::new(62.0, 0.0, 34.0, 0.0),
        "the host's, not the tree's"
    );
    // A rehydrated kernel carries it too, and lays out the same.
    let mut kernel = tree();
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let mut again = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_eq!(again.env(), kernel.env());
    again
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert_eq!(frame(&again, 2), frame(&kernel, 2));
}

#[test]
fn env_lengths_travel_the_wire() {
    use exact_kernel::wire;
    let mut kernel = tree();
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    let bytes = kernel.export(None).unwrap();
    assert!(!bytes.is_empty());
    // The same tree through a frame: the rows decode to the same lengths.
    let mut root = StyleProps::default();
    root.set_dynamic(
        StyleId::MarginLeft,
        &StyleValue::Text("calc(env(safe-area-inset-left) - 2px)".into()),
    )
    .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 7,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 7,
            patch: Box::new(root),
        },
        Op::AttachRoot { id: 7 },
    ];
    let frame = wire::encode(0, 2, &ops);
    let mut other = Kernel::with_monospace();
    other.apply_frame(&frame).unwrap();
    assert_eq!(
        other.node(7).unwrap().style.margin_left,
        Dimension::Env(Edge::Left, -2.0)
    );
}

/// LLP 1078 D3: a root with two panes sized by the segment variables — the
/// detail pane positioned at the second segment's left — and the host's
/// `set_segments`, `set_env`'s twin.
fn segmented_tree() -> Kernel {
    exact_kernel::link_segments();
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Height, &StyleValue::Percent(100.0))
        .unwrap();
    let mut list = StyleProps::default();
    list.set_dynamic(
        StyleId::Width,
        &StyleValue::Text("env(viewport-segment-width 0 0)".into()),
    )
    .unwrap();
    list.set_dynamic(StyleId::Height, &StyleValue::Number(40.0))
        .unwrap();
    let mut detail = StyleProps::default();
    detail
        .set_dynamic(StyleId::PositionType, &StyleValue::Text("absolute".into()))
        .unwrap();
    detail
        .set_dynamic(
            StyleId::Left,
            &StyleValue::Text("env(viewport-segment-left 1 0)".into()),
        )
        .unwrap();
    detail
        .set_dynamic(
            StyleId::Top,
            &StyleValue::Text("env(viewport-segment-top 1 0)".into()),
        )
        .unwrap();
    detail
        .set_dynamic(
            StyleId::Width,
            &StyleValue::Text("calc(env(viewport-segment-width 1 0) - 10px)".into()),
        )
        .unwrap();
    detail
        .set_dynamic(StyleId::Height, &StyleValue::Number(40.0))
        .unwrap();
    let mut plain = StyleProps::default();
    plain
        .set_dynamic(StyleId::Height, &StyleValue::Number(20.0))
        .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(list),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(detail),
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 4,
            patch: Box::new(plain),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3, 4],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

/// The Duo at 130°: two 455.5-point columns with a 40-point band between.
fn duo_segments() -> Vec<Rect> {
    vec![
        Rect::new(0.0, 0.0, 455.5, 669.0),
        Rect::new(495.5, 0.0, 455.5, 669.0),
    ]
}

#[test]
fn segment_lengths_take_initial_values_until_the_host_sets_segments() {
    let mut kernel = segmented_tree();
    assert_eq!(kernel.env().cols, 1);
    kernel
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    // One segment: `width` is auto (a block fills; an empty absolute box
    // shrinks to nothing), `left` and `top` are auto (the static position,
    // after the list), the calc width is auto too.
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 951.0, 40.0));
    assert_eq!(frame(&kernel, 3), (0.0, 40.0, 0.0, 40.0));
    assert_eq!(frame(&kernel, 4), (0.0, 40.0, 951.0, 20.0));
}

#[test]
fn setting_the_segments_relayouts_the_nodes_that_read_them() {
    let mut kernel = segmented_tree();
    kernel
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    assert!(kernel.set_segments(2, 1, duo_segments()).unwrap());
    let env = kernel.env();
    assert_eq!((env.cols, env.rows, env.segments.len()), (2, 1, 2));
    let receipt = kernel
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 455.5, 40.0), "the list pane");
    assert_eq!(
        frame(&kernel, 3),
        (495.5, 0.0, 445.5, 40.0),
        "the detail pane, past the band"
    );
    assert_eq!(frame(&kernel, 4), (0.0, 40.0, 951.0, 20.0), "untouched");
    assert_eq!(receipt.changed.len(), 2, "the two panes: {receipt:?}");
    // The same grid again: nothing to re-derive.
    assert!(!kernel.set_segments(2, 1, duo_segments()).unwrap());
    // Flat: one segment, the initial values return.
    assert!(kernel.set_segments(1, 1, vec![]).unwrap());
    kernel
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 951.0, 40.0));
    // The insets are their own call, and leave the grid alone (every
    // `env()` reader is re-derived either way).
    kernel.set_segments(2, 1, duo_segments()).unwrap();
    assert!(kernel.set_env(Env::new(0.0, 84.0, 34.0, 0.0)).unwrap());
    assert_eq!(kernel.env().cols, 2);
    assert_eq!(kernel.env().right, 84.0);
}

#[test]
fn a_malformed_grid_is_refused_and_the_segments_survive_a_reset() {
    let mut kernel = segmented_tree();
    let refused = Err(KernelError::from(LayoutError::InvalidSegments));
    assert_eq!(kernel.set_segments(2, 1, vec![]), refused);
    assert_eq!(kernel.set_segments(0, 1, vec![]), refused);
    assert_eq!(
        kernel.set_segments(1, 1, vec![Rect::new(0.0, 0.0, 1.0, 1.0)]),
        refused
    );
    assert_eq!(
        kernel.set_segments(
            2,
            1,
            vec![
                Rect::new(0.0, 0.0, f32::NAN, 1.0),
                Rect::new(0.0, 0.0, 1.0, 1.0)
            ]
        ),
        refused
    );
    assert_eq!(kernel.env().cols, 1, "nothing applied");
    kernel.set_segments(2, 1, duo_segments()).unwrap();
    kernel.reset();
    assert_eq!(
        kernel.env().segments,
        duo_segments(),
        "the host's, not the tree's"
    );
    let mut kernel = segmented_tree();
    kernel.set_segments(2, 1, duo_segments()).unwrap();
    kernel
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    let mut again = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_eq!(again.env(), kernel.env());
    again
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    assert_eq!(frame(&again, 3), frame(&kernel, 3));
}

#[test]
fn segment_lengths_travel_the_wire() {
    use exact_kernel::wire;
    exact_kernel::link_segments();
    let mut root = StyleProps::default();
    root.set_dynamic(
        StyleId::MarginLeft,
        &StyleValue::Text("calc(env(viewport-segment-right 0 1) - 2px)".into()),
    )
    .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 7,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 7,
            patch: Box::new(root),
        },
        Op::AttachRoot { id: 7 },
    ];
    let frame = wire::encode(0, 2, &ops);
    let mut other = Kernel::with_monospace();
    other.apply_frame(&frame).unwrap();
    assert_eq!(
        other.node(7).unwrap().style.margin_left,
        Dimension::Segment(SegmentVar::Right, 0, 1, -2.0)
    );
    let bytes = other.export(None).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn viewport_lengths_resolve_against_the_window_and_follow_resize() {
    let mut k = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Width, &StyleValue::Text("50vw".into()))
        .unwrap();
    root.set_dynamic(StyleId::Height, &StyleValue::Text("25dvh".into()))
        .unwrap();
    root.set_dynamic(StyleId::PaddingTop, &StyleValue::Text("5vmin".into()))
        .unwrap();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(root),
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    assert_eq!(frame(&k, 1), (0.0, 0.0, 400.0, 180.0));
    k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    assert_eq!(frame(&k, 1), (0.0, 0.0, 200.0, 220.0));
}
