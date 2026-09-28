//! `rem` and `em` over a root font size the host sets (LLP 1069.000 D3), as
//! CSS computes them: `rem` against the root, `em` against the element's
//! font size — for `font-size` itself, the parent's — and `px` never scales.
//!
//! The expected numbers are literal Chrome's (headless, 2026-09-27): the same
//! three `div`s as plain HTML under `html { font-size: 16px }` and `24px`,
//! measured with `getBoundingClientRect` against the outer box and
//! `getComputedStyle` — `b` at (0, 24) 400 × 48, `c` 40 wide, letter-spacing
//! 20px, fonts 20/40/40/16; at 24: (0, 32) 600 × 72, 60, 30px, 30/60/60/24.

use exact_kernel::{
    Dimension, Kernel, KernelError, LayoutError, NodeType, Offer, Op, RowValue, StyleId,
    StyleProps, StyleValue,
};

fn style(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    for (id, text) in rows {
        let value = match text.parse::<f64>() {
            Ok(n) => StyleValue::Number(n),
            Err(_) => StyleValue::Text(text.to_string()),
        };
        s.set_dynamic(*id, &value).unwrap();
    }
    Box::new(s)
}

/// 1: a column, `padding: 1rem`, `font-size: 1.25em` (of the root's).
/// 2: its child, `font-size: 2em` (of 1's), `width: 10em` (of its own),
///    `height: 3rem`, `margin-top: 8px` — pixels that never scale.
/// 3: 2's child, no font size: it inherits 2's computed pixels;
///    `width: 1em` of those, `letter-spacing: 0.5em`.
/// 4: a root sibling with no font size at all: the root's, `medium`.
fn tree() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: style(&[
                (StyleId::PaddingTop, "1rem"),
                (StyleId::FontSize, "1.25em"),
                (StyleId::Width, "390"),
            ]),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: style(&[
                (StyleId::FontSize, "2em"),
                (StyleId::Width, "10em"),
                (StyleId::Height, "3rem"),
                (StyleId::MarginTop, "8"),
            ]),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: style(&[
                (StyleId::Width, "1em"),
                (StyleId::Height, "1px"),
                (StyleId::LetterSpacing, "0.5em"),
            ]),
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
        Op::CreateView {
            id: 4,
            node_type: NodeType::View,
        },
        Op::AttachRoot { id: 4 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn font(kernel: &Kernel, id: u32) -> f32 {
    match kernel.node(id).unwrap().computed(StyleId::FontSize) {
        RowValue::Number(n) => n as f32,
        other => panic!("{other:?}"),
    }
}

fn layout(kernel: &mut Kernel) -> ((f32, f32), (f32, f32, f32), f32) {
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = |id| kernel.node(id).unwrap().frame;
    (
        (f(2).x, f(2).y),
        (f(2).width, f(2).height, f(3).width),
        kernel.node(3).unwrap().style.letter_spacing,
    )
}

#[test]
fn at_the_initial_root_size_rem_and_em_are_css_pixels() {
    let mut kernel = tree();
    assert_eq!(kernel.root_font_size(), 16.0);
    assert_eq!(font(&kernel, 1), 20.0, "1.25em of 16");
    assert_eq!(font(&kernel, 2), 40.0, "2em of the parent's 20");
    assert_eq!(font(&kernel, 3), 40.0, "inherits the computed pixels");
    assert_eq!(font(&kernel, 4), 16.0, "medium");
    let ((x, y), (w, h, w3), spacing) = layout(&mut kernel);
    assert_eq!((x, y), (0.0, 16.0 + 8.0), "padding 1rem, margin 8px");
    assert_eq!((w, h), (400.0, 48.0), "10em of 40, 3rem of 16");
    assert_eq!(w3, 40.0, "1em of the inherited 40");
    assert_eq!(spacing, 20.0, "0.5em of 40");
    assert_eq!(
        kernel.node(2).unwrap().style.height,
        Dimension::Points(48.0),
        "every reader sees pixels"
    );
}

#[test]
fn a_new_root_size_rescales_rem_and_em_but_never_px() {
    let mut kernel = tree();
    layout(&mut kernel);
    assert_eq!(kernel.set_root_font_size(24.0), Ok(true));
    assert_eq!(kernel.set_root_font_size(24.0), Ok(false), "the same size");
    let receipt = kernel.apply(0, 2, &[]).unwrap();
    assert!(receipt.layout_invalidated);
    for id in [1, 2, 3, 4] {
        assert!(
            receipt
                .touched
                .iter()
                .any(|k| kernel.node(id).unwrap().key == *k),
            "{id} is republished: {receipt:?}"
        );
    }
    assert_eq!(font(&kernel, 1), 30.0);
    assert_eq!(font(&kernel, 2), 60.0);
    assert_eq!(font(&kernel, 3), 60.0);
    assert_eq!(font(&kernel, 4), 24.0, "medium is the root's size");
    let ((_, y), (w, h, w3), spacing) = layout(&mut kernel);
    assert_eq!(y, 24.0 + 8.0, "the margin stays 8px");
    assert_eq!((w, h, w3, spacing), (600.0, 72.0, 60.0, 30.0));
    // Back to 16: the same pixels as before.
    kernel.set_root_font_size(16.0).unwrap();
    kernel.apply(0, 3, &[]).unwrap();
    assert_eq!(font(&kernel, 4), 16.0);
    let ((_, y), (w, h, w3), spacing) = layout(&mut kernel);
    assert_eq!((y, w, h, w3, spacing), (24.0, 400.0, 48.0, 40.0, 20.0));
}

#[test]
fn a_parent_font_size_change_reaches_em_below_it() {
    let mut kernel = tree();
    layout(&mut kernel);
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: style(&[(StyleId::FontSize, "10")]),
            }],
        )
        .unwrap();
    assert_eq!(font(&kernel, 2), 20.0, "2em of 10px");
    let (_, (w, ..), spacing) = layout(&mut kernel);
    assert_eq!((w, spacing), (200.0, 10.0));
    // Written again the same relative way: no change at all.
    let receipt = kernel
        .apply(
            0,
            3,
            &[Op::SetStyle {
                id: 2,
                patch: style(&[(StyleId::FontSize, "2em")]),
            }],
        )
        .unwrap();
    assert!(receipt.touched.is_empty(), "{receipt:?}");
    // Moving 3 to a root: it inherits the root's size now.
    kernel
        .apply(
            0,
            4,
            &[
                Op::SetChildren {
                    id: 2,
                    children: vec![],
                },
                Op::AttachRoot { id: 3 },
            ],
        )
        .unwrap();
    assert_eq!(kernel.node(3).unwrap().style.width, Dimension::Points(16.0));
}

#[test]
fn an_unusable_root_size_is_refused_and_kept_across_a_reset() {
    let mut kernel = tree();
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            kernel.set_root_font_size(bad),
            Err(KernelError::from(LayoutError::InvalidRootFontSize))
        );
    }
    kernel.set_root_font_size(20.0).unwrap();
    kernel.apply(0, 2, &[]).unwrap();
    kernel.reset();
    assert_eq!(kernel.root_font_size(), 20.0, "the host's, not the tree's");
    let mut kernel2 = kernel;
    let ops = [
        Op::CreateView {
            id: 9,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 9,
            patch: style(&[(StyleId::Width, "2rem")]),
        },
        Op::AttachRoot { id: 9 },
    ];
    kernel2.apply(0, 3, &ops).unwrap();
    assert_eq!(
        kernel2.node(9).unwrap().style.width,
        Dimension::Points(40.0)
    );
}
