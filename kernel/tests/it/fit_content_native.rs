//! LLP 1075.003 §9.11: in a fit-content sheet the height units are the
//! screen's (`Env::screen`), through native measurement too — a field's
//! floor, a native button's padding — so the measure is the same at any
//! sheet height and the layout agrees with it (Astra's r6 cases).
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
        screen: Some((390.0, 844.0)),
        ..Env::default()
    })
    .unwrap();
    k
}

/// The route's measure with the sheet `height` tall, and the child's laid
/// out height there.
fn fitted(k: &mut Kernel, height: f32) -> (f32, f32) {
    k.compute_layout(1, Offer::definite(390.0, height)).unwrap();
    let key = k.arena().key_of(2).unwrap();
    (
        k.fit_content_height(key).unwrap(),
        k.node(3).unwrap().frame.height,
    )
}

/// Its floor is the screen's 844 points (and the field's chrome below it)
/// at every sheet height, measured and laid out alike.
#[test]
fn a_native_fields_viewport_floor_is_the_screens() {
    let mut k = route(NodeType::TextInput, &[(StyleId::MinHeight, "100vh")], &[]);
    let (fit, laid) = fitted(&mut k, 844.0);
    assert!(fit >= 844.0 && fit == laid, "{fit} {laid}");
    for sheet in [200.0, 44.0] {
        assert_eq!(fitted(&mut k, sheet), (fit, laid), "{sheet}");
    }
}

#[test]
fn a_native_buttons_viewport_padding_is_the_screens() {
    let mut k = route(
        NodeType::Control,
        &[(StyleId::PaddingTop, "10vh")],
        &[(PropId::Type, "button")],
    );
    assert_eq!(k.node(3).map(|n| n.node_type), Some(NodeType::Control));
    for sheet in [844.0, 200.0, 44.0] {
        let (fit, laid) = fitted(&mut k, sheet);
        assert!(
            (fit - 104.4).abs() < 0.01 && (laid - fit).abs() < 0.01,
            "{sheet}: {fit} {laid}"
        );
    }
}

fn height(k: &mut Kernel, value: &str) -> f32 {
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 3,
            patch: rows(&[(StyleId::Height, value), (StyleId::FlexShrink, "0")]),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(300.0, 200.0)).unwrap();
    k.node(3).unwrap().frame.height
}

/// The height units and `vmin`/`vmax` read the screen; `vw` the viewport;
/// without a screen, all read the viewport.
#[test]
fn the_screen_is_what_the_height_units_read() {
    let mut k = route(NodeType::View, &[], &[]);
    for (value, want) in [
        ("50vh", 422.0),
        ("10vmin", 39.0),
        ("10vmax", 84.4),
        ("10vw", 30.0),
    ] {
        let got = height(&mut k, value);
        assert!((got - want).abs() < 0.01, "{value}: {got}");
    }
    let env = Env {
        screen: None,
        ..k.env()
    };
    k.set_env(env).unwrap();
    for (value, want) in [("50vh", 100.0), ("10vmin", 20.0), ("10vmax", 30.0)] {
        let got = height(&mut k, value);
        assert!((got - want).abs() < 0.01, "{value}: {got}");
    }
}
