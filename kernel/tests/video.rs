//! Video uses the replaced-element measure and keeps its node across viewport changes.
use exact_kernel::{
    AlignItems, Display, FlexDirection, Kernel, NodeType, Offer, Op, StyleId, StyleProps,
};
#[test]
fn fallback_then_metadata_then_viewport_resize() {
    let mut kernel = Kernel::with_monospace();
    let mut style = StyleProps::default();
    style.display = Display::Flex;
    style.flex_direction = FlexDirection::Column;
    style.align_items = AlignItems::FlexStart;
    for id in [
        StyleId::Display,
        StyleId::FlexDirection,
        StyleId::AlignItems,
    ] {
        style.mask.set(id);
    }
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
                    patch: Box::new(style),
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Video,
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
        .compute_layout(1, Offer::definite(800.0, 900.0))
        .unwrap();
    let frame = kernel.node(2).unwrap().frame;
    assert_eq!((frame.width, frame.height), (300.0, 150.0));
    kernel.set_intrinsic_size(2, Some((720.0, 404.0))).unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 500.0))
        .unwrap();
    let frame = kernel.node(2).unwrap().frame;
    assert_eq!((frame.width, frame.height), (720.0, 404.0));
    kernel.set_intrinsic_size(2, None).unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 900.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 300.0);
}

// @ref LLP 1042 §2; LLP 1043.000 §6.3 — keep Patch 5 on the 0.14 adapter.
#[test]
fn high_resolution_video_keeps_ratio_constraints_and_only_used_box_overflow() {
    use exact_kernel::{StyleValue, ViewId};
    for fit in ["fill", "contain", "cover", "none", "scale-down"] {
        let mut kernel = Kernel::with_monospace();
        let style = |id: ViewId, rows: &[(StyleId, StyleValue)]| {
            let mut patch = StyleProps::default();
            for (id, value) in rows {
                patch.set_dynamic(*id, value).unwrap();
            }
            Op::SetStyle {
                id,
                patch: Box::new(patch),
            }
        };
        let n = StyleValue::Number;
        let t = |s: &str| StyleValue::Text(s.into());
        kernel
            .apply(
                0,
                1,
                &[
                    Op::CreateView {
                        id: 1,
                        node_type: NodeType::View,
                    },
                    Op::CreateView {
                        id: 2,
                        node_type: NodeType::Video,
                    },
                    style(
                        1,
                        &[
                            (StyleId::Width, n(200.)),
                            (StyleId::Display, t("flex")),
                            (StyleId::FlexDirection, t("column")),
                            (StyleId::AlignItems, t("flex-start")),
                        ],
                    ),
                    style(
                        2,
                        &[
                            (StyleId::Width, n(192.)),
                            (StyleId::MaxHeight, n(72.)),
                            (StyleId::ObjectFit, t(fit)),
                        ],
                    ),
                    Op::SetChildren {
                        id: 1,
                        children: vec![2],
                    },
                    Op::AttachRoot { id: 1 },
                ],
            )
            .unwrap();
        kernel.set_intrinsic_size(2, Some((7680., 4320.))).unwrap();
        kernel
            .compute_layout(1, Offer::definite(200., 900.))
            .unwrap();
        let video = kernel.node(2).unwrap();
        assert_eq!(
            (video.frame.width, video.frame.height),
            (128., 72.),
            "{fit}"
        );
        // Content is the children's used bounds, not the parent's own box.
        assert_eq!(kernel.node(1).unwrap().frame.width, 200., "{fit}");
        assert_eq!(kernel.node(1).unwrap().content, (128., 72.), "{fit}");
    }
}
