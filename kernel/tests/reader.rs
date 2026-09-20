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
/// (vendor/taffy, patch 7), so these regressions ask for it. `max-width` still
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
    // vendor/taffy patch 7. A scroll container's automatic minimum size is
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
