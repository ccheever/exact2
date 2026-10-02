//! A replaced element: an image is laid out from the intrinsic size the host
//! reports, keeping its ratio when one dimension is set, as CSS sizes `<img>`;
//! min/max constraints resolve by CSS 2.1 §10.4's table for replaced elements.

use exact_kernel::{
    AlignItems, Dimension, Display, FlexDirection, Kernel, LayoutError, MonospaceMeasurer,
    NodeType, Offer, Op, StyleId, StyleProps,
};

/// A flex column with one image in it; `stretch` false pins the items at
/// their own size (`align-items: flex-start`), true leaves CSS's default.
fn tree_with(image: StyleProps, stretch: bool) -> (Kernel, u32) {
    measured_tree(NodeType::Image, image, stretch)
}

fn measured_tree(kind: NodeType, image: StyleProps, stretch: bool) -> (Kernel, u32) {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.display = Display::Flex;
    root.mask.set(StyleId::Display);
    root.flex_direction = FlexDirection::Column;
    root.mask.set(StyleId::FlexDirection);
    if !stretch {
        root.align_items = AlignItems::FlexStart;
        root.mask.set(StyleId::AlignItems);
    }
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
            node_type: kind,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(image),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    (kernel, 2)
}

/// A non-stretching flex column with one image of the given width.
fn tree(width: Option<f32>) -> (Kernel, u32) {
    let mut image = StyleProps::default();
    if let Some(w) = width {
        image.width = Dimension::Points(w);
        image.mask.set(StyleId::Width);
    }
    tree_with(image, false)
}

/// An image style from (row, points) pairs.
fn image_style(rows: &[(StyleId, f32)]) -> StyleProps {
    let mut s = StyleProps::default();
    for &(id, v) in rows {
        match id {
            StyleId::Width => s.width = Dimension::Points(v),
            StyleId::Height => s.height = Dimension::Points(v),
            StyleId::MinWidth => s.min_width = Dimension::Points(v),
            StyleId::MaxWidth => s.max_width = Dimension::Points(v),
            StyleId::MaxHeight => s.max_height = Dimension::Points(v),
            other => panic!("not a dimension row: {other:?}"),
        }
        s.mask.set(id);
    }
    s
}

fn frame(kernel: &mut Kernel, image: u32) -> (f32, f32) {
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = kernel.node(image).unwrap().frame;
    (f.width, f.height)
}

/// Ratio calculations retain fractional CSS pixels; allow floating-point error.
fn near(got: (f32, f32), want: (f32, f32), what: &str) {
    assert!(
        (got.0 - want.0).abs() < 0.001 && (got.1 - want.1).abs() < 0.001,
        "{what}: got {got:?}, want {want:?}"
    );
}

/// A 320×120 image (ratio 8:3) with these rows, loaded.
fn loaded(rows: &[(StyleId, f32)], stretch: bool) -> (f32, f32) {
    let (mut kernel, image) = tree_with(image_style(rows), stretch);
    kernel
        .set_intrinsic_size(image, Some((320.0, 120.0)))
        .unwrap();
    frame(&mut kernel, image)
}

#[test]
fn an_image_is_nothing_until_it_loads_then_its_intrinsic_size() {
    let (mut kernel, image) = tree(None);
    assert_eq!(
        frame(&mut kernel, image),
        (0.0, 0.0),
        "no intrinsic size and no rows: the measure is 0×0"
    );
    kernel
        .set_intrinsic_size(image, Some((320.0, 120.0)))
        .unwrap();
    assert_eq!(frame(&mut kernel, image), (320.0, 120.0));
    kernel.set_intrinsic_size(image, None).unwrap();
    assert_eq!(frame(&mut kernel, image), (0.0, 0.0), "forgotten again");
}

#[test]
fn scaled_image_pixels_do_not_enlarge_scrollable_overflow() {
    // A 132px Tapback displayed at 32px contributed its unscaled bitmap to
    // Messages' reply scroll extent, scrolling the short reply out of view.
    // CSS Overflow §2.1: replaced content overflow is ink, never scrollable.
    for fit in [
        exact_kernel::ObjectFit::Fill,
        exact_kernel::ObjectFit::Contain,
        exact_kernel::ObjectFit::Cover,
        exact_kernel::ObjectFit::None,
        exact_kernel::ObjectFit::ScaleDown,
    ] {
        for extent in [32.0, 160.0] {
            for decorated in [false, true] {
                let mut style = image_style(&[(StyleId::Width, extent), (StyleId::Height, extent)]);
                style.object_fit = fit;
                style.mask.set(StyleId::ObjectFit);
                if decorated {
                    for id in [
                        StyleId::BorderStyleTop,
                        StyleId::BorderStyleRight,
                        StyleId::BorderStyleBottom,
                        StyleId::BorderStyleLeft,
                    ] {
                        style
                            .set_dynamic(id, &exact_kernel::StyleValue::Text("solid".into()))
                            .unwrap();
                    }
                    for id in [
                        StyleId::PaddingLeft,
                        StyleId::PaddingRight,
                        StyleId::PaddingTop,
                        StyleId::PaddingBottom,
                    ] {
                        style
                            .set_dynamic(id, &exact_kernel::StyleValue::Number(3.0))
                            .unwrap();
                    }
                    for id in [
                        StyleId::BorderWidthLeft,
                        StyleId::BorderWidthRight,
                        StyleId::BorderWidthTop,
                        StyleId::BorderWidthBottom,
                    ] {
                        style
                            .set_dynamic(id, &exact_kernel::StyleValue::Number(2.0))
                            .unwrap();
                    }
                }
                let (mut kernel, image) = tree_with(style, false);
                // Replacement changes the natural ratio and pixel dimensions,
                // but two authored dimensions still determine the displayed box.
                for intrinsic in [(132.0, 132.0), (264.0, 132.0)] {
                    kernel.set_intrinsic_size(image, Some(intrinsic)).unwrap();
                    let outer = extent + if decorated { 10.0 } else { 0.0 };
                    assert_eq!(frame(&mut kernel, image), (outer, outer));
                    let padding_box = extent + if decorated { 6.0 } else { 0.0 };
                    near(
                        kernel.node(image).unwrap().content,
                        (padding_box, padding_box),
                        "image's used padding box",
                    );
                    near(
                        kernel.node(1).unwrap().content,
                        (outer, outer),
                        "ancestor overflow uses displayed image",
                    );
                }
            }
        }
    }
}

#[test]
fn a_grid_image_can_compress_below_its_natural_width() {
    // The existing Taffy replaced-item marker also controls the automatic
    // grid minimum. A percentage maximum must allow this 132px image to
    // fit the 60px left after the fixed 20px track (CSS Sizing §5.2.2).
    let mut image = StyleProps::default();
    image.max_width = Dimension::Percent(100.0);
    image.mask.set(StyleId::MaxWidth);
    let (mut kernel, id) = tree_with(image, false);
    let mut grid = StyleProps::default();
    grid.display = Display::Grid;
    grid.mask.set(StyleId::Display);
    grid.width = Dimension::Points(80.0);
    grid.mask.set(StyleId::Width);
    grid.grid_template_columns = exact_kernel::GridTracks::from_tracks(vec![
        exact_kernel::GridTrack::Auto,
        exact_kernel::GridTrack::Points(20.0),
    ]);
    grid.mask.set(StyleId::GridTemplateColumns);
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(grid),
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(id, Some((132.0, 132.0))).unwrap();
    near(
        frame(&mut kernel, id),
        (60.0, 60.0),
        "compressed grid image",
    );
    // The kernel reports natural extent; a host floors scrollWidth by the
    // client width. The empty fixed track must not create extra scrolling.
    let root = kernel.node(1).unwrap();
    assert_eq!(root.content.0.max(root.frame.width), 80.0);
}

#[test]
fn one_dimension_set_gives_the_other_by_the_intrinsic_ratio() {
    let (mut kernel, image) = tree(Some(96.0));
    kernel
        .set_intrinsic_size(image, Some((320.0, 120.0)))
        .unwrap();
    assert_eq!(frame(&mut kernel, image), (96.0, 36.0));
    // A set ratio row wins over the intrinsic one.
    let mut square = StyleProps::default();
    square.aspect_ratio = exact_kernel::ratio::AspectRatio::parse("1").unwrap();
    square.mask.set(StyleId::AspectRatio);
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: image,
                patch: Box::new(square),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, image), (96.0, 96.0));
}

#[test]
fn an_unknown_view_is_refused() {
    let (mut kernel, _) = tree(None);
    assert!(kernel.set_intrinsic_size(99, Some((1.0, 1.0))).is_err());
}

#[test]
fn in_a_block_parent_an_auto_width_image_keeps_its_intrinsic_size() {
    // CSS replaced block sizing: an auto width uses the natural bitmap width.
    let mut kernel = Kernel::with_monospace();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Image,
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel.set_intrinsic_size(2, Some((320.0, 120.0))).unwrap();
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = kernel.node(2).unwrap().frame;
    // Upstream 0.14 removes the old block-stretch deviation.
    assert_eq!((f.width, f.height), (320.0, 120.0));
}

#[test]
fn the_css_replaced_element_constraint_table() {
    use StyleId::*;
    near(
        loaded(&[(Height, 30.0)], false),
        (80.0, 30.0),
        "height only",
    );
    near(
        loaded(&[(Width, 50.0), (Height, 50.0)], false),
        (50.0, 50.0),
        "both set",
    );
    near(
        loaded(&[(MaxWidth, 100.0)], false),
        (100.0, 37.5),
        "max-width keeps the ratio",
    );
    near(
        loaded(&[(MaxHeight, 40.0)], false),
        (40.0 * 320.0 / 120.0, 40.0),
        "max-height keeps the ratio",
    );
    near(
        loaded(&[(MinWidth, 400.0)], false),
        (400.0, 150.0),
        "min-width keeps the ratio",
    );
    // CSS 2.1 §10.4's table is for neither dimension set; a set width
    // stays, and the height by ratio is clamped on its own (Chrome 154).
    near(
        loaded(&[(Width, 96.0), (MaxHeight, 20.0)], false),
        (96.0, 20.0),
        "a set width, then max-height: the width stays",
    );
    near(
        loaded(&[(MaxWidth, 100.0), (MaxHeight, 20.0)], false),
        (20.0 * 320.0 / 120.0, 20.0),
        "both maxima violated: the tighter one wins",
    );
}

#[test]
fn in_a_stretching_flex_column_an_auto_width_image_fills_it_too() {
    // CSS `align-items: stretch` applies to a replaced element as well, so
    // here the kernel and the web agree (the block-flow case is the deviation).
    near(
        loaded(&[], true),
        (390.0, 146.25),
        "stretched, height by ratio",
    );
}

#[test]
fn a_reported_size_survives_rehydration_but_not_replay() {
    let (mut kernel, image) = tree(Some(96.0));
    kernel
        .set_intrinsic_size(image, Some((320.0, 120.0)))
        .unwrap();
    assert_eq!(frame(&mut kernel, image), (96.0, 36.0));
    let mut again = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_eq!(
        frame(&mut again, image),
        (96.0, 36.0),
        "the column is cloned, so a rehydrated kernel knows the size"
    );
    let (mut replayed, image) = tree(Some(96.0));
    assert_eq!(
        frame(&mut replayed, image),
        (96.0, 0.0),
        "a replay of the batches does not carry it: the host reports again"
    );
}

#[test]
fn a_size_is_refused_for_a_non_image_and_when_not_finite_and_positive() {
    let (mut kernel, image) = tree(None);
    assert_eq!(
        kernel.set_intrinsic_size(1, Some((320.0, 120.0))),
        Err(LayoutError::NotAnImage(1).into())
    );
    for bad in [
        (0.0, 120.0),
        (320.0, -1.0),
        (f32::INFINITY, 120.0),
        (320.0, f32::NAN),
    ] {
        assert_eq!(
            kernel.set_intrinsic_size(image, Some(bad)),
            Err(LayoutError::InvalidIntrinsicSize(image).into()),
            "{bad:?}"
        );
    }
    assert_eq!(frame(&mut kernel, image), (0.0, 0.0), "nothing was stored");
}

#[test]
fn a_projected_tablist_reserves_its_native_height_and_releases_it() {
    use exact_kernel::{PropId, PropValue};
    let (mut kernel, _) = tree(None);
    // A projection can disappear before its deferred measurement arrives.
    kernel.set_intrinsic_size(1, None).unwrap();
    kernel
        .apply(
            1,
            2,
            &[Op::SetProp {
                id: 1,
                prop: PropId::AccessibilityRole,
                value: PropValue::Str("tablist".into()),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, 1).1, 0.0);
    kernel.set_intrinsic_size(1, Some((390.0, 83.0))).unwrap();
    assert_eq!(frame(&mut kernel, 1).1, 83.0);
    let mut rebuilt = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_eq!(frame(&mut rebuilt, 1), (390.0, 83.0));
    kernel
        .apply(
            2,
            3,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(image_style(&[(StyleId::Height, 20.0)])),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, 1), (390.0, 83.0));
    kernel.set_intrinsic_size(1, Some((200.0, 49.0))).unwrap();
    assert_eq!(frame(&mut kernel, 1), (390.0, 49.0));
    kernel
        .apply(
            3,
            4,
            &[Op::ClearProp {
                id: 1,
                prop: PropId::AccessibilityRole,
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(1, None).unwrap();
    assert_eq!(frame(&mut kernel, 1).1, 20.0);
}

#[test]
fn a_tab_bar_minimum_moves_siblings_without_overriding_explicit_css_min_height() {
    use exact_kernel::{PropId, PropValue};
    for display in [Display::Block, Display::Flex] {
        let mut kernel = Kernel::with_monospace();
        let mut root = StyleProps::default();
        root.display = display;
        root.mask.set(StyleId::Display);
        root.flex_direction = FlexDirection::Column;
        root.mask.set(StyleId::FlexDirection);
        let mut ops: Vec<_> = (1..=3)
            .map(|id| Op::CreateView {
                id,
                node_type: NodeType::View,
            })
            .collect();
        ops.extend([
            Op::SetStyle {
                id: 1,
                patch: Box::new(root),
            },
            Op::SetStyle {
                id: 2,
                patch: Box::new(image_style(&[(StyleId::Height, 20.0)])),
            },
            Op::SetStyle {
                id: 3,
                patch: Box::new(image_style(&[(StyleId::Height, 40.0)])),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::AccessibilityRole,
                value: PropValue::Str("tablist".into()),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ]);
        kernel.apply(0, 1, &ops).unwrap();
        kernel.set_intrinsic_size(2, Some((390.0, 83.0))).unwrap();
        assert_eq!(frame(&mut kernel, 2), (390.0, 83.0));
        assert_eq!(kernel.node(3).unwrap().frame.y, 83.0);
        let mut explicit = StyleProps::default();
        explicit.min_height = Dimension::Points(0.0);
        explicit.mask.set(StyleId::MinHeight);
        kernel
            .apply(
                1,
                2,
                &[Op::SetStyle {
                    id: 2,
                    patch: Box::new(explicit),
                }],
            )
            .unwrap();
        assert_eq!(frame(&mut kernel, 2), (390.0, 20.0));
        assert_eq!(kernel.node(3).unwrap().frame.y, 20.0);
    }
}

// Native modules and built-in controls share preferred-size layout, without
// turning a widget into a replaced image or inferring a natural aspect ratio.
#[test]
fn native_modules_share_control_intrinsics_and_css_constraints() {
    use StyleId::*;
    for kind in [NodeType::NativeView, NodeType::Control] {
        for (rows, stretch, expected) in [
            (vec![], false, (120.0, 40.0)),
            (vec![(Width, 60.0)], false, (60.0, 40.0)),
            (vec![(Height, 20.0)], false, (120.0, 20.0)),
            (vec![(Width, 70.0), (Height, 25.0)], false, (70.0, 25.0)),
            (
                vec![(MaxWidth, 80.0), (MaxHeight, 30.0)],
                false,
                (80.0, 30.0),
            ),
            (vec![(MinWidth, 150.0)], false, (150.0, 40.0)),
            (vec![], true, (390.0, 40.0)),
        ] {
            let (mut kernel, id) = measured_tree(kind, image_style(&rows), stretch);
            kernel.set_intrinsic_size(id, Some((120.0, 40.0))).unwrap();
            near(
                frame(&mut kernel, id),
                expected,
                &format!("{kind:?} {rows:?}"),
            );
        }
    }
}

#[test]
fn native_content_growth_clear_and_padding_move_the_next_sibling() {
    let mut style = StyleProps::default();
    for id in [
        StyleId::PaddingLeft,
        StyleId::PaddingRight,
        StyleId::PaddingTop,
        StyleId::PaddingBottom,
    ] {
        style
            .set_dynamic(id, &exact_kernel::StyleValue::Number(4.0))
            .unwrap();
    }
    let (mut kernel, id) = measured_tree(NodeType::NativeView, style, false);
    kernel
        .apply(
            1,
            2,
            &[
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![id, 3],
                },
            ],
        )
        .unwrap();
    for (size, expected) in [
        (None, (8.0, 8.0)),
        (Some((120.0, 40.0)), (128.0, 48.0)),
        (Some((120.0, 80.0)), (128.0, 88.0)),
        (Some((120.0, 80.0)), (128.0, 88.0)),
        (None, (8.0, 8.0)),
    ] {
        kernel.set_intrinsic_size(id, size).unwrap();
        near(
            frame(&mut kernel, id),
            expected,
            "native content box plus padding",
        );
        assert_eq!(kernel.node(3).unwrap().frame.y, expected.1);
    }
    for bad in [
        (0.0, 40.0),
        (-1.0, 40.0),
        (120.0, f32::NAN),
        (f32::INFINITY, 40.0),
    ] {
        assert_eq!(
            kernel.set_intrinsic_size(id, Some(bad)),
            Err(LayoutError::InvalidIntrinsicSize(id).into())
        );
    }
}

#[test]
fn native_block_without_a_report_still_stretches_and_has_no_content_height() {
    let (mut kernel, id) = measured_tree(NodeType::NativeView, StyleProps::default(), false);
    let mut block = StyleProps::default();
    block.display = Display::Block;
    block.mask.set(StyleId::Display);
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(block),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, id), (390.0, 0.0));
    kernel.set_intrinsic_size(id, Some((120.0, 40.0))).unwrap();
    assert_eq!(frame(&mut kernel, id), (390.0, 40.0));
}
