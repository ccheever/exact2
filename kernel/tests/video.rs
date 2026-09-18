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
