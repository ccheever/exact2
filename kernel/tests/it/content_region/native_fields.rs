//! Native editors use the same content box in every publication. LLP 1104 D5.
use super::support::*;
use exact_kernel::*;

struct Fields;
impl TextMeasurer for Fields {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(request)
    }
    fn field_chrome(&mut self, _: &FieldChromeRequest) -> FieldChrome {
        FieldChrome {
            top: 3.0,
            right: 5.0,
            bottom: 7.0,
            left: 11.0,
            minimum_height: 44.0,
            provisional: false,
        }
    }
}

fn fields() -> Kernel {
    let mut k = fixture_with(Box::new(Fields));
    let mut style = StyleProps::default();
    for (id, value) in [
        (StyleId::Width, 100.0),
        (StyleId::Height, 50.0),
        (StyleId::PaddingTop, 2.0),
        (StyleId::PaddingRight, 4.0),
        (StyleId::PaddingBottom, 6.0),
        (StyleId::PaddingLeft, 8.0),
    ] {
        style.set_dynamic(id, &StyleValue::Number(value)).unwrap();
    }
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 7,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 8,
                node_type: NodeType::TextInput,
            },
            Op::CreateView {
                id: 9,
                node_type: NodeType::TextInput,
            },
            Op::SetStyle {
                id: 8,
                patch: Box::new(style.clone()),
            },
            Op::SetStyle {
                id: 9,
                patch: Box::new(style),
            },
            Op::SetProp {
                id: 8,
                prop: PropId::Value,
                value: "row field".into(),
            },
            Op::SetProp {
                id: 9,
                prop: PropId::Value,
                value: "shell field".into(),
            },
            Op::SetChildren {
                id: 7,
                children: vec![8],
            },
            Op::SetChildren {
                id: 3,
                children: vec![4, 7],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 6, 9],
            },
        ],
    )
    .unwrap();
    k
}

#[test]
fn region_row_and_shell_fields_publish_the_ordinary_content_rect() {
    let mut ordinary = fields();
    ordinary
        .compute_layout(1, Offer::definite(400.0, 300.0))
        .unwrap();
    let expected = Some(Frame {
        x: 19.0,
        y: 5.0,
        width: 100.0,
        height: 50.0,
    });
    assert_eq!(ordinary.node(8).unwrap().field_content_rect(), expected);
    assert_eq!(ordinary.node(9).unwrap().field_content_rect(), expected);

    let mut region = fields();
    register(&mut region);
    let pending = pass(&mut region, 400.0, 1);
    assert!(!pending.current);
    assert_eq!(region.node(9).unwrap().field_content_rect(), expected);
    let current = ready(&mut region, 400.0, 1);
    assert!(current.current_frame(key(&region, 8)).is_some());
    for id in [8, 9] {
        assert_eq!(region.node(id).unwrap().field_content_rect(), expected);
        let a = ordinary.node(id).unwrap().frame;
        let b = region.node(id).unwrap().frame;
        assert_eq!((a.width, a.height), (b.width, b.height));
    }

    // A resize projects the retained row at a new origin without changing its
    // local editor rectangle, then publishes the new candidate's rectangle.
    let retained = pass(&mut region, 300.0, 1);
    assert!(!retained.current);
    assert_eq!(region.node(8).unwrap().field_content_rect(), expected);
    ready(&mut region, 300.0, 1);
    assert_eq!(region.node(8).unwrap().field_content_rect(), expected);

    let mut padding = StyleProps::default();
    padding
        .set_dynamic(StyleId::PaddingLeft, &StyleValue::Number(14.0))
        .unwrap();
    region
        .apply(
            0,
            0,
            &[
                Op::SetStyle {
                    id: 8,
                    patch: Box::new(padding),
                },
                Op::SetProp {
                    id: 8,
                    prop: PropId::Value,
                    value: "candidate row field".into(),
                },
            ],
        )
        .unwrap();
    let retained = pass(&mut region, 300.0, 1);
    assert!(!retained.current);
    assert_eq!(
        region.node(8).unwrap().field_content_rect(),
        expected,
        "retained editor geometry must not use the candidate's padding"
    );
    ready(&mut region, 300.0, 1);
    assert_eq!(
        region.node(8).unwrap().field_content_rect(),
        Some(Frame {
            x: 25.0,
            y: 5.0,
            width: 100.0,
            height: 50.0,
        })
    );
}

#[test]
fn hidden_region_and_shell_fields_clear_and_restore_the_published_content_rect() {
    let mut region = fields();
    register(&mut region);
    ready(&mut region, 400.0, 1);
    for (id, field) in [(7, 8), (9, 9)] {
        let expected = region.node(field).unwrap().field_content_rect();
        assert!(expected.is_some());
        for display in ["none", "block"] {
            let mut style = StyleProps::default();
            style
                .set_dynamic(StyleId::Display, &StyleValue::Text(display.into()))
                .unwrap();
            region
                .apply(
                    0,
                    0,
                    &[Op::SetStyle {
                        id,
                        patch: Box::new(style),
                    }],
                )
                .unwrap();
            ready(&mut region, 400.0, 1);
            assert_eq!(
                region.node(field).unwrap().field_content_rect(),
                if display == "none" { None } else { expected }
            );
        }
    }
}
