//! Standalone time reserves its measured box, without adding painted runs.
//! @ref LLP 1126 D6; LLP 1126.000 WP3 (inline reservation descoped).
use super::*;
use exact_kernel::{Kernel, NodeType, Offer, Op, PropId, StyleProps};

#[test]
fn standalone_time_reserves_its_box_at_the_real_font() {
    for (font, family) in [
        (
            include_bytes!("../../tests/fonts/Inter-Regular.ttf").as_slice(),
            "Inter",
        ),
        (
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice(),
            "DejaVu Sans",
        ),
    ] {
        for size in [15.0, 28.0] {
            let engine = Rc::new(RefCell::new(TextEngine::with_catalog(
                catalog::Catalog::from_bytes(&[font], family),
            )));
            let mut kernel = Kernel::new(Box::new(Measurer(engine.clone())));
            let mut row = StyleProps::default();
            row.set_dynamic(
                exact_kernel::StyleId::Display,
                &exact_kernel::StyleValue::Text("flex".into()),
            )
            .unwrap();
            row.set_dynamic(
                exact_kernel::StyleId::FontSize,
                &exact_kernel::StyleValue::Number(size),
            )
            .unwrap();
            kernel
                .apply(
                    0,
                    1,
                    &[
                        Op::CreateView {
                            id: 1,
                            node_type: NodeType::View,
                        },
                        Op::AttachRoot { id: 1 },
                        Op::SetStyle {
                            id: 1,
                            patch: Box::new(row),
                        },
                        Op::CreateView {
                            id: 2,
                            node_type: NodeType::Text,
                        },
                        Op::CreateView {
                            id: 3,
                            node_type: NodeType::Text,
                        },
                        Op::SetChildren {
                            id: 1,
                            children: vec![2, 3],
                        },
                        Op::SetProp {
                            id: 2,
                            prop: PropId::Text,
                            value: "9 min. ago".into(),
                        },
                        Op::SetProp {
                            id: 2,
                            prop: PropId::TextReservation,
                            value: "59 min, 59 sec ago".into(),
                        },
                        Op::SetProp {
                            id: 3,
                            prop: PropId::Text,
                            value: "Following".into(),
                        },
                    ],
                )
                .unwrap();
            kernel
                .compute_layout(1, Offer::definite(1000., 200.))
                .unwrap();
            let frames = (kernel.node(2).unwrap().frame, kernel.node(3).unwrap().frame);
            let node = kernel.node(2).unwrap();
            let mut runs = node.text_runs();
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].text, "9 min. ago");
            runs[0].text = "59 min, 59 sec ago".into();
            let request = TextMeasureRequest {
                runs: &runs,
                paragraph: node.paragraph(),
                width: AxisOffer::MaxContent,
                height: AxisOffer::MaxContent,
                exclusions: &[],
            };
            let widest = engine
                .borrow_mut()
                .measure(&Spec::from_request(&request), AxisOffer::MaxContent)
                .width;
            assert!(
                frames.0.width >= widest,
                "{family} {size}: {} < {widest}",
                frames.0.width
            );
            for (batch, text) in [(2, "10 min. ago"), (3, "59 min, 59 sec ago")] {
                kernel
                    .apply(
                        0,
                        batch,
                        &[Op::SetProp {
                            id: 2,
                            prop: PropId::Text,
                            value: text.into(),
                        }],
                    )
                    .unwrap();
                kernel
                    .compute_layout(1, Offer::definite(1000., 200.))
                    .unwrap();
                assert_eq!(
                    frames,
                    (kernel.node(2).unwrap().frame, kernel.node(3).unwrap().frame),
                    "{family} {size}"
                );
                assert_eq!(kernel.node(2).unwrap().text_runs()[0].text, text);
            }
            eprintln!(
                "{family} {size} pt: stable {} x {} pt; widest advance {widest}",
                frames.0.width, frames.0.height
            );
        }
    }
}

#[test]
fn standalone_time_frame_stays_constant_across_runner_clock_steps() {
    let plan = contract::compile("component App\n  view\n    row\n      time datetime=-120000 format=\"relative\" testId=\"clock\"\n      time datetime=0 format=\"timer\" testId=\"timer\"\n      text \"Following\" testId=\"following\"\n").unwrap();
    let engine = Rc::new(RefCell::new(TextEngine::with_catalog(
        catalog::Catalog::from_bytes(
            &[include_bytes!("../../tests/fonts/Inter-Regular.ttf")],
            "Inter",
        ),
    )));
    let (mut host, error) =
        crate::Host::boot(&plan.encode(), (), Box::new(Measurer(engine)), 1000., 200.).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(host.set_time(0., 0.).is_none());
    let read = |host: &crate::Host<()>| {
        let k = host.kernel();
        let time = k.node_by_key(k.find_by_test_id("clock")[0]).unwrap();
        let timer = k.node_by_key(k.find_by_test_id("timer")[0]).unwrap();
        let sibling = k.node_by_key(k.find_by_test_id("following")[0]).unwrap();
        (
            time.props.str(PropId::Text).unwrap().to_string(),
            (time.frame, timer.frame, sibling.frame),
        )
    };
    let (text, before) = read(&host);
    assert_eq!(text, "2 min. ago");
    assert!(
        before.1.width < 60.,
        "timer reserves only mm:ss: {:?}",
        before.1
    );
    for (now, expected) in [(60000., "3 min. ago"), (480000., "10 min. ago")] {
        assert!(host.advance(now).is_none());
        let (text, after) = read(&host);
        assert_eq!(text, expected);
        assert_eq!(before, after);
    }
}
