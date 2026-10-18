use super::*;
use exact_kernel::{Kernel, NodeType, Offer, Op, PropId, StyleProps};

#[test]
fn inline_time_reserves_width_with_a_shaper_that_hangs_spaces() {
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
            let mut ops = Vec::new();
            for id in 1..=4 {
                ops.push(Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                });
            }
            let mut style = StyleProps::default();
            style
                .set_dynamic(
                    exact_kernel::StyleId::FontSize,
                    &exact_kernel::StyleValue::Number(size),
                )
                .unwrap();
            ops.extend([
                Op::AttachRoot { id: 1 },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3, 4],
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::Text,
                    value: "Updated ".into(),
                },
                Op::SetProp {
                    id: 3,
                    prop: PropId::Text,
                    value: "9 min. ago".into(),
                },
                Op::SetProp {
                    id: 3,
                    prop: PropId::TextReservation,
                    value: "59 min, 59 sec ago".into(),
                },
                Op::SetProp {
                    id: 4,
                    prop: PropId::Text,
                    value: " here".into(),
                },
            ]);
            kernel.apply(0, 1, &ops).unwrap();
            kernel
                .compute_layout(1, Offer::definite(420., 200.))
                .unwrap();
            assert!(kernel.node(3).unwrap().time_padding().is_some());
            let width = |k: &Kernel| {
                let node = k.node(1).unwrap();
                let runs = node.text_runs();
                let request = TextMeasureRequest {
                    runs: &runs,
                    paragraph: node.paragraph(),
                    width: AxisOffer::MaxContent,
                    height: AxisOffer::MaxContent,
                    exclusions: &[],
                };
                engine
                    .borrow_mut()
                    .measure(&Spec::from_request(&request), AxisOffer::MaxContent)
                    .width
            };
            let before = width(&kernel);
            kernel
                .apply(
                    0,
                    2,
                    &[Op::SetProp {
                        id: 3,
                        prop: PropId::Text,
                        value: "10 min. ago".into(),
                    }],
                )
                .unwrap();
            kernel
                .compute_layout(1, Offer::definite(420., 200.))
                .unwrap();
            let after = width(&kernel);
            assert!(
                (after - before).abs() <= 1.0,
                "{family} {size}: {before} -> {after}"
            );
        }
    }
}
