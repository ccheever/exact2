//! LLP 1075.003 §9.11: a sheet's content laid out alone reads no viewport
//! height through native measurement either — a field's floor or a native
//! button's padding (Astra's r6).
use exact_kernel::*;

struct Native;
impl TextMeasurer for Native {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(request)
    }
    fn field_chrome(&mut self, _: &FieldChromeRequest) -> FieldChrome {
        FieldChrome {
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
            left: 4.0,
            minimum_height: 30.0,
            provisional: false,
        }
    }
    /// A button as tall as 20 points and its top padding.
    fn button_measure(&mut self, request: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
        let pad = match request.style.button.padding_top {
            Dimension::Points(v) => v,
            _ => 0.0,
        };
        Some(ButtonMeasure {
            width: 100.0,
            height: 20.0 + pad,
            provisional: false,
        })
    }
}

fn rows(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut style = StyleProps::default();
    for &(id, value) in rows {
        let value = value
            .parse()
            .map_or_else(|_| StyleValue::Text(value.into()), StyleValue::Number);
        style.set_dynamic(id, &value).unwrap();
    }
    Box::new(style)
}

/// A 390-wide root holding a route (2) whose one child (3) is `node_type`
/// styled `child`.
fn route(node_type: NodeType, child: &[(StyleId, &str)], props: &[(PropId, &str)]) -> Kernel {
    let mut k = Kernel::new(Box::new(Native));
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::CreateView { id: 3, node_type },
        Op::SetStyle {
            id: 1,
            patch: rows(&[(StyleId::Width, "100%"), (StyleId::Height, "100%")]),
        },
        Op::SetStyle {
            id: 2,
            patch: rows(&[
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
                (StyleId::Height, "100%"),
            ]),
        },
        Op::SetStyle {
            id: 3,
            patch: rows(child),
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
    for &(prop, value) in props {
        ops.push(Op::SetProp {
            id: 3,
            prop,
            value: PropValue::Str(value.into()),
        });
    }
    k.apply(0, 1, &ops).unwrap();
    let font = ControlFont {
        family: "Platform".into(),
        family_id: 7,
        size: 17.0,
        weight: 400,
        style: FontStyle::Normal,
    };
    k.set_env(Env {
        control_text_styles: Some(ControlTextStyles {
            field: font.clone(),
            textarea: font.clone(),
            button: font,
        }),
        ..Env::default()
    })
    .unwrap();
    k
}

fn fitted(k: &mut Kernel, height: f32) -> f32 {
    k.compute_layout(1, Offer::definite(390.0, height)).unwrap();
    let key = k.arena().key_of(2).unwrap();
    k.fit_content_height(key).unwrap()
}

#[test]
fn a_native_fields_viewport_floor_does_not_read_the_sheet() {
    let mut k = route(NodeType::TextInput, &[(StyleId::MinHeight, "100vh")], &[]);
    let tall = fitted(&mut k, 844.0);
    assert!(
        tall < 100.0,
        "the field's own height, not the sheet's: {tall}"
    );
    assert_eq!(fitted(&mut k, 200.0), tall);
}

#[test]
fn a_native_buttons_viewport_padding_does_not_read_the_sheet() {
    let mut k = route(
        NodeType::Control,
        &[(StyleId::PaddingTop, "100vh")],
        &[(PropId::Type, "button")],
    );
    assert_eq!(k.node(3).map(|n| n.node_type), Some(NodeType::Control));
    assert_eq!(fitted(&mut k, 844.0), 20.0);
    assert_eq!(fitted(&mut k, 200.0), 20.0);
}
