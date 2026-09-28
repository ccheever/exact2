//! A canvas holds children (LLP 1014 D1): they are laid out in its box — the
//! web's `layoutsubtree`: block flow, the canvas as containing block — and
//! never size it. Its size is its rows', else its natural 300×150 and
//! ratio 2:1, as a replaced element's (`browser_replaced.rs`).

use exact_kernel::{Dimension, Kernel, NodeType, Offer, Op, StyleId, StyleProps};

fn sized(width: f32, height: f32) -> StyleProps {
    let mut s = StyleProps::default();
    s.width = Dimension::Points(width);
    s.mask.set(StyleId::Width);
    s.height = Dimension::Points(height);
    s.mask.set(StyleId::Height);
    s
}

/// A root view holding a canvas of `canvas` style, holding one view of
/// `child` style. Returns the kernel; the canvas is 2, the child 3.
fn tree(canvas: StyleProps, child: StyleProps) -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Canvas,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(canvas),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(child),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    kernel
}

fn frame(kernel: &Kernel, id: u32) -> (f32, f32, f32, f32) {
    let f = kernel.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn children_are_laid_out_in_the_canvas_box() {
    // A block child of a 300×150 canvas fills its width and keeps its own
    // height, at the canvas's origin: block flow with the canvas as the
    // containing block.
    let kernel = tree(sized(300.0, 150.0), sized(0.0, 40.0).without_width());
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 300.0, 150.0));
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 300.0, 40.0));
}

#[test]
fn children_never_size_the_canvas() {
    // A child taller than the canvas overflows it; the canvas stays its
    // rows' size, as a bare <div> with an explicit height does.
    let kernel = tree(sized(300.0, 150.0), sized(0.0, 400.0).without_width());
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 300.0, 150.0));
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 300.0, 400.0));
}

#[test]
fn a_percentage_child_resolves_against_the_canvas() {
    let mut child = StyleProps::default();
    child.height = Dimension::Percent(50.0);
    child.mask.set(StyleId::Height);
    let kernel = tree(sized(200.0, 100.0), child);
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 200.0, 50.0));
}

/// Rows from `(row, value)` pairs.
fn rows(rows: &[(StyleId, Dimension)]) -> StyleProps {
    let mut s = StyleProps::default();
    for (id, value) in rows {
        match id {
            StyleId::Width => s.width = *value,
            StyleId::Height => s.height = *value,
            _ => unreachable!(),
        }
        s.mask.set(*id);
    }
    s
}

#[test]
fn a_canvas_with_children_is_its_natural_size() {
    // A bare canvas holding a child larger than it: 300×150, the child laid
    // out in that box (block flow fills its width) and overflowing it.
    let child = sized(0.0, 400.0).without_width();
    let kernel = tree(StyleProps::default(), child.clone());
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 300.0, 150.0));
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 300.0, 400.0));
    // A width gives the height through the natural ratio, children aside.
    let kernel = tree(rows(&[(StyleId::Width, Dimension::Percent(100.0))]), child);
    assert_eq!(frame(&kernel, 2), (0.0, 0.0, 390.0, 195.0));
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 390.0, 400.0));
    // A percentage child resolves against the natural box.
    let mut half = StyleProps::default();
    half.height = Dimension::Percent(50.0);
    half.mask.set(StyleId::Height);
    let kernel = tree(StyleProps::default(), half);
    assert_eq!(frame(&kernel, 3), (0.0, 0.0, 300.0, 75.0));
}

trait WithoutWidth {
    fn without_width(self) -> Self;
}

impl WithoutWidth for StyleProps {
    /// Drop the width row: a block child takes the containing block's width.
    fn without_width(mut self) -> Self {
        self.width = Dimension::Auto;
        self.mask.clear(StyleId::Width);
        self
    }
}
