//! An image in a paragraph's text (LLP 1011.002): its paragraph's measurer
//! reserves its margin box in the line, the line grows to hold it as CSS's
//! line box does, and the kernel places the image where the box went.

use exact_kernel::{
    Frame, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue, INLINE_BOX_TEXT,
};

// The reference measurer at 16 px: a 9.6 px advance, a 19.2 px line whose
// baseline is 15.36 px down, an 8 px x-height.
const ADVANCE: f32 = 9.6;

fn rows(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch
            .set_dynamic(*row, &StyleValue::Text((*value).into()))
            .unwrap();
    }
    Box::new(patch)
}

/// `text` holding a run, an image and a run, in a 400 px column.
fn paragraph(image: &[(StyleId, &str)], before: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Text,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::Text,
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::Image,
        },
        Op::CreateView {
            id: 5,
            node_type: NodeType::Text,
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: before.into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::ImageSource,
            value: "app:/emoji.png".into(),
        },
        Op::SetProp {
            id: 5,
            prop: PropId::Text,
            value: " ef".into(),
        },
        Op::SetStyle {
            id: 1,
            patch: rows(&[(StyleId::Width, "400px")]),
        },
        Op::SetStyle {
            id: 2,
            patch: rows(&[(StyleId::FontSize, "16px")]),
        },
    ];
    ops.extend([
        Op::SetStyle {
            id: 4,
            patch: rows(image),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    k
}

fn frame(k: &Kernel, id: u32) -> Frame {
    k.node(id).unwrap().frame
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn an_image_takes_its_box_in_the_line_and_the_line_grows_to_hold_it() {
    let k = paragraph(
        &[(StyleId::Height, "1.375em"), (StyleId::Width, "1.375em")],
        "ab cd",
    );
    let runs = k.node(2).unwrap().text_runs();
    assert_eq!(runs.len(), 3);
    assert_eq!(&*runs[1].text, INLINE_BOX_TEXT);
    let reserved = runs[1].inline.expect("the image is a box");
    assert!(close(reserved.width, 22.0) && close(reserved.height, 22.0));
    // `baseline`: its bottom on the baseline, 22 px above it, so the line
    // is 22 + 3.84 px tall and the image starts after "ab cd".
    let p = frame(&k, 2);
    assert!(close(p.height, 25.84), "{p:?}");
    let image = frame(&k, 4);
    assert!(close(image.x, 5.0 * ADVANCE), "{image:?}");
    assert!(close(image.y, 0.0) && close(image.width, 22.0) && close(image.height, 22.0));
}

#[test]
fn middle_centres_the_image_half_the_x_height_above_the_baseline() {
    let k = paragraph(
        &[
            (StyleId::Height, "22px"),
            (StyleId::Width, "22px"),
            (StyleId::VerticalAlign, "middle"),
        ],
        "ab",
    );
    // Its top 15 px above the baseline (11 + 4), 7 below: the line keeps
    // its 15.36 px above and reaches 7 below.
    let p = frame(&k, 2);
    assert!(close(p.height, 15.36 + 7.0), "{p:?}");
    let image = frame(&k, 4);
    assert!(close(image.y, 0.36), "{image:?}");
}

#[test]
fn a_length_raises_it_and_text_bottom_sits_it_on_the_descent() {
    let raised = paragraph(
        &[
            (StyleId::Height, "10px"),
            (StyleId::Width, "10px"),
            (StyleId::BaselineShift, "4px"),
        ],
        "ab",
    );
    // Bottom 4 px above the baseline: top at 15.36 - 14.
    assert!(close(frame(&raised, 4).y, 15.36 - 14.0));
    assert!(close(frame(&raised, 2).height, 19.2));
    let bottom = paragraph(
        &[
            (StyleId::Height, "10px"),
            (StyleId::Width, "10px"),
            (StyleId::VerticalAlign, "text-bottom"),
        ],
        "ab",
    );
    // Its bottom on the line's descent, 3.84 px below the baseline.
    let image = frame(&bottom, 4);
    assert!(close(image.y + image.height, 19.2), "{image:?}");
}

#[test]
fn an_auto_width_follows_the_natural_ratio_once_it_loads_and_the_line_reflows() {
    let mut k = paragraph(&[(StyleId::Height, "20px")], "ab");
    // Before it loads it has no ratio: zero wide, as an `<img>` is.
    assert!(close(frame(&k, 4).width, 0.0));
    k.set_intrinsic_size(4, Some((64.0, 32.0))).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    let image = frame(&k, 4);
    assert!(
        close(image.width, 40.0) && close(image.height, 20.0),
        "{image:?}"
    );
    assert!(close(image.x, 2.0 * ADVANCE));
    // Its line grew to hold it: 20 px above the baseline, 3.84 below.
    assert!(close(frame(&k, 2).height, 23.84));
}

#[test]
fn an_image_that_does_not_fit_wraps_to_the_next_line() {
    let mut k = paragraph(
        &[(StyleId::Height, "20px"), (StyleId::Width, "30px")],
        "ab cd",
    );
    k.apply(
        1,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: rows(&[(StyleId::Width, "60px")]),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    // "ab cd" is 48 px: the box goes to the second line, which is 20 px
    // above its baseline and 3.84 below.
    let image = frame(&k, 4);
    assert!(close(image.x, 0.0), "{image:?}");
    assert!(close(image.y, 19.2), "{image:?}");
    assert!(frame(&k, 2).height > 19.2 + 20.0);
}

#[test]
fn a_hidden_image_is_no_run() {
    let k = paragraph(
        &[
            (StyleId::Height, "20px"),
            (StyleId::Width, "30px"),
            (StyleId::Display, "none"),
        ],
        "ab",
    );
    let runs = k.node(2).unwrap().text_runs();
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.inline.is_none()));
}

#[test]
fn an_image_on_a_line_the_clamp_cuts_shows_nothing() {
    let mut k = paragraph(
        &[(StyleId::Height, "10px"), (StyleId::Width, "30px")],
        "ab cd",
    );
    k.apply(
        1,
        2,
        &[
            Op::SetStyle {
                id: 1,
                patch: rows(&[(StyleId::Width, "60px")]),
            },
            Op::SetStyle {
                id: 2,
                patch: {
                    let mut patch = StyleProps::default();
                    patch
                        .set_dynamic(StyleId::LineClamp, &StyleValue::Number(1.0))
                        .unwrap();
                    Box::new(patch)
                },
            },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    // "ab cd" fills the one line; the box was on the second.
    let image = frame(&k, 4);
    assert!(image.width == 0.0 && image.height == 0.0, "{image:?}");
}

#[test]
fn alignment_reads_the_paragraphs_font_not_the_images() {
    let own = paragraph(
        &[
            (StyleId::Height, "20px"),
            (StyleId::Width, "20px"),
            (StyleId::FontSize, "40px"),
            (StyleId::VerticalAlign, "text-bottom"),
        ],
        "ab",
    );
    // Its bottom on the 16 px text's descent, 3.84 px below the baseline,
    // whatever its own font size (which its `em` lengths would read): the
    // line is its 16.16 px above the baseline and that descent below.
    let image = frame(&own, 4);
    assert!(close(frame(&own, 2).height, 20.0), "{:?}", frame(&own, 2));
    assert!(
        close(image.y, 0.0) && close(image.height, 20.0),
        "{image:?}"
    );
}

#[test]
fn an_authored_ratio_relates_the_border_box_under_border_box_sizing() {
    let k = paragraph(
        &[
            (StyleId::Width, "100px"),
            (StyleId::PaddingLeft, "10px"),
            (StyleId::PaddingRight, "10px"),
            (StyleId::PaddingTop, "10px"),
            (StyleId::PaddingBottom, "10px"),
            (StyleId::BoxSizing, "border-box"),
            (StyleId::AspectRatio, "2"),
        ],
        "ab",
    );
    let image = frame(&k, 4);
    assert!(
        close(image.width, 100.0) && close(image.height, 50.0),
        "{image:?}"
    );
}
