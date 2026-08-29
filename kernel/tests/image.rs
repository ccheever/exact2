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
            node_type: NodeType::Image,
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

/// Frames land on the point grid; compare within a point.
fn near(got: (f32, f32), want: (f32, f32), what: &str) {
    assert!(
        (got.0 - want.0).abs() < 1.0 && (got.1 - want.1).abs() < 1.0,
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
fn one_dimension_set_gives_the_other_by_the_intrinsic_ratio() {
    let (mut kernel, image) = tree(Some(96.0));
    kernel
        .set_intrinsic_size(image, Some((320.0, 120.0)))
        .unwrap();
    assert_eq!(frame(&mut kernel, image), (96.0, 36.0));
    // A set ratio row wins over the intrinsic one.
    let mut square = StyleProps::default();
    square.aspect_ratio = 1.0;
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
fn in_a_block_parent_an_auto_width_image_fills_it_a_declared_deviation() {
    // CSS gives a replaced element in block flow its intrinsic width; Taffy's
    // block layout stretches an auto-width child to the container. Declared
    // in LLP 1001 §1 until the kernel special-cases it; the ratio still holds.
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
    // 390 × (120/320) = 146.25, rounded to the point grid.
    assert_eq!((f.width, f.height), (390.0, 146.0));
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
        (106.67, 40.0),
        "max-height keeps the ratio",
    );
    near(
        loaded(&[(MinWidth, 400.0)], false),
        (400.0, 150.0),
        "min-width keeps the ratio",
    );
    near(
        loaded(&[(Width, 96.0), (MaxHeight, 20.0)], false),
        (53.33, 20.0),
        "a set width, then max-height: the width follows",
    );
    near(
        loaded(&[(MaxWidth, 100.0), (MaxHeight, 20.0)], false),
        (53.33, 20.0),
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
