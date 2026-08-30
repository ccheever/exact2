//! The page's environment (LLP 1001 §2): an `env(safe-area-inset-*)` length
//! resolves against the insets the host sets, and a change of the insets
//! lays the tree out again — only the nodes that read them.

use exact_kernel::{
    Dimension, Edge, Env, Kernel, KernelError, LayoutError, MonospaceMeasurer, NodeType, Offer, Op,
    StyleId, StyleProps, StyleValue,
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
    assert_eq!(kernel.env(), Env::new(62.0, 0.0, 34.0, 0.0));
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
fn a_keyboard_inset_pads_the_bottom_without_changing_the_viewport() {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(
        StyleId::PaddingBottom,
        &StyleValue::Text("env(keyboard-inset-height)".into()),
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
            ],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert_eq!(
        kernel.node(1).unwrap().style.padding_bottom,
        Dimension::Keyboard(0.0)
    );
    assert_eq!(kernel.node(1).unwrap().frame.height, 844.0);
    assert!(kernel
        .set_env(Env::new(62.0, 0.0, 34.0, 0.0).with_keyboard(335.0))
        .unwrap());
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    assert_eq!(
        kernel.node(1).unwrap().frame.height,
        844.0,
        "the layout viewport does not shrink"
    );
    // The child is in the content box: the 335 of keyboard padding is
    // inside the same 844, not instead of it.
    assert_eq!(kernel.node(2).unwrap().frame.height, 40.0);
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
