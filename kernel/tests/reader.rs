//! Browser-backed regression for LLP 1035.000.001, independent of platform fonts.
#[path = "support/reader.rs"]
mod reader;
use exact_kernel::Offer;

#[test]
fn percentage_reader_matches_browser_and_has_no_phantom_scroll_range() {
    // Chrome 152, 2026-09-12: fixtures/reader.html is literal HTML/CSS,
    // not Exact lowering. 12 padded blocks, 10px glyph advances, 26px lines.
    // The 120-character word overflows horizontally: its intrinsic width
    // exceeds max-width and used to poison the cached auto height.
    // Rows: viewport, border-box, column width, ordinary/long-word heights.
    let expected = [
        (360.0, true, 360.0, 6598.0, 9718.0),
        (720.0, true, 720.0, 2542.0, 5974.0),
        (1000.0, true, 720.0, 2542.0, 5974.0),
        (360.0, false, 424.0, 5038.0, 7846.0),
        (720.0, false, 784.0, 2230.0, 4102.0),
        (1000.0, false, 784.0, 2230.0, 4102.0),
    ];
    for long_word in [false, true] {
        for workaround in [false, true] {
            for (width, border_box, column_width, ordinary_height, long_height) in expected {
                let height = if long_word {
                    long_height
                } else {
                    ordinary_height
                };
                let scroll_height = height + 96.0;
                let mut k = reader::kernel();
                k.apply(0, 1, &reader::initial(workaround, border_box, long_word))
                    .unwrap();
                k.compute_layout(1, Offer::definite(width, 800.0)).unwrap();
                let column = k.node(4).unwrap();
                let last = k.node(reader::LAST).unwrap();
                let bottom = last.frame.y + last.frame.height - column.frame.y;
                for (label, actual, wanted) in [
                    ("column width", column.frame.width, column_width),
                    ("column height", column.frame.height, height),
                    ("last block", bottom, height),
                    ("scroll extent", k.node(2).unwrap().content.1, scroll_height),
                ] {
                    assert!((actual-wanted).abs() <= 0.02,"{label}: {actual} != {wanted}; viewport={width}, border-box={border_box}, workaround={workaround}");
                }
            }
        }
    }
}

/// The reader with its scroller made an ordinary box, so that its flex row
/// owes it an intrinsic probe: a flex item's automatic minimum is its
/// min-content size unless it is a scroll container, whose minimum is zero.
/// Taffy used to make that probe for every item and discard it where it was
/// not needed — the scroller included, which is how the reader itself came to
/// exercise the cache rule below. It is measured only where it is used now
/// (vendor/taffy, patch 8), so these regressions ask for it. `max-width` still
/// clamps the column, and every length asserted below is unchanged.
fn probed(mut ops: Vec<exact_kernel::Op>) -> Vec<exact_kernel::Op> {
    ops.push(reader::style(
        2,
        &[
            (exact_kernel::StyleId::OverflowX, reader::text("visible")),
            (exact_kernel::StyleId::OverflowY, reader::text("visible")),
        ],
    ));
    ops
}

#[test]
fn intrinsic_probe_is_remeasured_at_the_definite_content_width() {
    use exact_kernel::{AxisOffer, Kernel, TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::RefCell, rc::Rc};
    struct Tracked(Rc<RefCell<Vec<(AxisOffer, f32)>>>);
    impl TextMeasurer for Tracked {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            let result = reader::measurer().measure(request);
            self.0.borrow_mut().push((request.width, result.height));
            result
        }
    }
    let measurements = Rc::new(RefCell::new(Vec::new()));
    let mut k = Kernel::new(Box::new(Tracked(measurements.clone())));
    k.apply(0, 1, &probed(reader::initial(false, true, true)))
        .unwrap();
    k.compute_layout(1, Offer::definite(720.0, 800.0)).unwrap();
    let initial = measurements.borrow().len();
    // 720 border-box - 64 column padding - 28 code padding = 628.
    assert!(measurements
        .borrow()
        .iter()
        .any(|(offer, height)| *offer == AxisOffer::Definite(628.0) && *height == 468.0));
    assert!(measurements
        .borrow()
        .iter()
        .any(|(offer, height)| *offer == AxisOffer::MinContent && *height == 1716.0));
    assert_eq!(k.node(4).unwrap().frame.height, 5974.0);
    k.compute_layout(1, Offer::definite(720.0, 800.0)).unwrap();
    assert_eq!(
        measurements.borrow().len(),
        initial,
        "unchanged layout must retain its measurement cache"
    );
}

// @ref LLP 1043.000 §3 D3 — final measure dimensions must not clip content.
#[test]
fn resolved_measure_dimensions_preserve_reader_and_shrunk_text_bits() {
    use exact_kernel::{NodeType, Op, PropId, StyleId::*};
    use reader::{number as n, style, text as t};
    fn geometry(k: &exact_kernel::Kernel, id: u32) -> [u32; 6] {
        let node = k.node(id).unwrap();
        let f = node.frame;
        [f.x, f.y, f.width, f.height, node.content.0, node.content.1].map(f32::to_bits)
    }
    let mut k = reader::kernel();
    k.apply(0, 1, &reader::initial(false, true, true)).unwrap();
    k.compute_layout(1, Offer::definite(720.0, 800.0)).unwrap();
    // Literal reader.html Chrome dimensions: 720 - 64 - 28 = 628;
    // 18 lines * 26 = 468, 12 * (468 + 28) + 11 * 2 = 5974.
    for (id, expected) in [
        (205, [46.0, 38.0, 628.0, 468.0, 1200.0, 468.0]),
        // CSS Overflow: the non-scrolling column contributes the child's
        // overflow end (32 + 14 + 1200), not another 32px end padding.
        // The HTML fixture pins heights, not this old implementation value.
        (4, [0.0, 24.0, 720.0, 5974.0, 1246.0, 5974.0]),
    ] {
        assert_eq!(geometry(&k, id), expected.map(f32::to_bits), "node {id}");
    }
    let mut k = reader::kernel();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            style(
                1,
                &[
                    (Display, t("flex")),
                    (FlexDirection, t("column")),
                    (Width, n(628.0)),
                    (Height, n(52.0)),
                    (FontSize, n(16.0)),
                    (LineHeight, t("26px")),
                ],
            ),
            style(2, &[(MinHeight, n(0.0))]),
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: reader::LONG_TEXT.repeat(6).into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(628.0, 52.0)).unwrap();
    // The same 18-line source flex-shrinks to two lines of box height;
    // its natural overflowing content still has all 18 lines.
    assert_eq!(
        geometry(&k, 2),
        [0.0, 0.0, 628.0, 52.0, 1200.0, 468.0].map(f32::to_bits)
    );
    assert_eq!(
        geometry(&k, 1),
        [0.0, 0.0, 628.0, 52.0, 1200.0, 468.0].map(f32::to_bits)
    );
}

#[test]
fn matching_paragraph_offers_do_not_cross_the_measurer_twice() {
    use exact_kernel::{
        AxisOffer, Kernel, Op, PropId, TextMeasureRequest, TextMeasurer, TextMetrics,
    };
    use std::{cell::RefCell, rc::Rc};
    type Calls = Rc<RefCell<Vec<(AxisOffer, AxisOffer)>>>;
    struct Tracked(Calls);
    impl TextMeasurer for Tracked {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            if request.runs[0].text.starts_with("watched ") {
                self.0.borrow_mut().push((request.width, request.height));
            }
            reader::measurer().measure(request)
        }
    }
    let calls = Calls::default();
    let mut kernel = Kernel::new(Box::new(Tracked(calls.clone())));
    let mut ops = probed(reader::initial(false, true, true));
    ops.push(Op::SetProp {
        id: 205,
        prop: PropId::Text,
        value: format!("watched {}", reader::LONG_TEXT.repeat(6)).into(),
    });
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(720.0, 800.0))
        .unwrap();
    let calls = calls.borrow();
    assert!(
        calls.len() > 1,
        "the fixture must exercise different offers"
    );
    for (i, offer) in calls.iter().enumerate() {
        assert!(
            !calls[..i].contains(offer),
            "duplicate measurement: {calls:?}"
        );
    }
}

#[test]
fn a_scroller_in_a_flex_row_is_not_probed_for_a_minimum_it_does_not_use() {
    // vendor/taffy patch 8 (trunk 0.9.2 patch 7). A scroll container's automatic minimum size is
    // zero, so the flex row holding the reader owes it no min-content pass.
    // Taffy made one anyway and discarded it: the scroller's whole content
    // laid out at no width, every paragraph wrapped a word to a line, on
    // every layout pass — and, the two passes sharing one cache slot, every
    // row laid out again at its real width after it (LLP 1044 F6).
    use exact_kernel::{AxisOffer, Kernel, TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::RefCell, rc::Rc};
    struct Tracked(Rc<RefCell<Vec<AxisOffer>>>);
    impl TextMeasurer for Tracked {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            self.0.borrow_mut().push(request.width);
            reader::measurer().measure(request)
        }
    }
    let offers = Rc::new(RefCell::new(Vec::new()));
    let mut k = Kernel::new(Box::new(Tracked(offers.clone())));
    k.apply(0, 1, &reader::initial(false, true, true)).unwrap();
    k.compute_layout(1, Offer::definite(720.0, 800.0)).unwrap();
    let offers = offers.borrow();
    assert!(!offers.is_empty(), "the fixture must measure its text");
    for offer in offers.iter() {
        assert_eq!(
            *offer,
            AxisOffer::Definite(628.0),
            "only the column's content width is ever offered: {offers:?}"
        );
    }
    assert_eq!(k.node(4).unwrap().frame.height, 5974.0);
}
