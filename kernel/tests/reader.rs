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
    k.apply(0, 1, &reader::initial(false, true, true)).unwrap();
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
    let mut ops = reader::initial(false, true, true);
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
