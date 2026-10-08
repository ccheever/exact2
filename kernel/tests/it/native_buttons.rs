//! Native semantic faces and pre-publication height-for-width. LLP 1069.011.001.
use exact_kernel::wire::Op;
use exact_kernel::*;
use std::cell::RefCell;
use std::rc::Rc;

type Answers = Rc<RefCell<(bool, Vec<ButtonMeasureRequest>)>>;
struct Buttons(Answers);
impl TextMeasurer for Buttons {
    fn button_measure(&mut self, request: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
        let mut state = self.0.borrow_mut();
        state.1.push(request.clone());
        let width = match request.width {
            AxisOffer::Definite(w) => w.min(240.0),
            AxisOffer::MinContent => 40.0,
            AxisOffer::MaxContent => 240.0,
        };
        Some(ButtonMeasure {
            width,
            height: 20.0 * (240.0 / width.max(1.0)).ceil() + 16.0,
            provisional: state.0,
        })
    }
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(request)
    }
}
fn rows(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    for &(id, value) in rows {
        let value = value
            .parse()
            .map_or_else(|_| StyleValue::Text(value.into()), StyleValue::Number);
        s.set_dynamic(id, &value).unwrap();
    }
    Box::new(s)
}
fn patch(k: &mut Kernel, id: u32, values: &[(StyleId, &str)]) {
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id,
            patch: rows(values),
        }],
    )
    .unwrap();
}
fn prop(id: u32, prop: PropId, value: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: PropValue::Str(value.into()),
    }
}
fn tree(measurer: Box<dyn TextMeasurer>) -> Kernel {
    let mut k = Kernel::new(measurer);
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
                node_type: NodeType::Control,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::Image,
            },
            prop(2, PropId::Type, "button"),
            prop(2, PropId::ButtonStyle, "filled"),
            prop(3, PropId::Text, "A long title"),
            prop(4, PropId::Text, "A subtitle"),
            prop(5, PropId::ImageSource, "symbol:send"),
            Op::SetStyle {
                id: 1,
                patch: rows(&[
                    (StyleId::Display, "flex"),
                    (StyleId::FlexDirection, "column"),
                    (StyleId::AlignItems, "start"),
                    (StyleId::FontSize, "30"),
                    (StyleId::FontWeight, "800"),
                    (StyleId::TextColor, "red"),
                ]),
            },
            Op::SetStyle {
                id: 2,
                patch: rows(&[(StyleId::BoxSizing, "border-box")]),
            },
            Op::SetChildren {
                id: 2,
                children: vec![5, 3, 4],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}
fn layout(k: &mut Kernel) -> Frame {
    k.compute_layout(1, Offer::definite(500.0, 500.0)).unwrap();
    k.node(2).unwrap().frame
}
#[test]
fn a_host_measurement_is_the_first_frame_and_wraps_at_each_width() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    let frame = layout(&mut k);
    assert_eq!((frame.width, frame.height), (240.0, 36.0));
    patch(&mut k, 2, &[(StyleId::Width, "80")]);
    let frame = layout(&mut k);
    assert_eq!((frame.width, frame.height), (80.0, 76.0));
    assert!(answers
        .borrow()
        .1
        .iter()
        .any(|r| r.width == AxisOffer::Definite(80.0)));
    patch(&mut k, 2, &[(StyleId::Width, "120")]);
    let frame = layout(&mut k);
    assert_eq!((frame.width, frame.height), (120.0, 56.0));
    let request = answers.borrow().1.last().unwrap().clone();
    assert_eq!(request.face.subtitle.as_deref(), Some("A subtitle"));
    assert_eq!(request.face.symbol.as_deref(), Some("send"));
    assert_eq!(request.button_style, "filled");
    assert_eq!(k.provisional_layouts(), 0);
}
#[test]
fn authored_padding_is_counted_once_in_offer_and_geometry() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    patch(
        &mut k,
        2,
        &[
            (StyleId::Width, "80"),
            (StyleId::PaddingTop, "8"),
            (StyleId::PaddingBottom, "8"),
            (StyleId::PaddingLeft, "10"),
            (StyleId::PaddingRight, "10"),
        ],
    );
    let frame = layout(&mut k);
    assert_eq!((frame.width, frame.height), (80.0, 76.0));
    assert!(answers
        .borrow()
        .1
        .iter()
        .any(|r| r.width == AxisOffer::Definite(80.0)));
    patch(&mut k, 2, &[(StyleId::Height, "100")]);
    assert_eq!(layout(&mut k).height, 100.0);
}
#[test]
fn unknown_keeps_default_then_host_intrinsic_size() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    let frame = layout(&mut k);
    assert_eq!(
        (frame.width, frame.height),
        ControlKind::Button.default_size()
    );
    k.set_intrinsic_size(2, Some((99.0, 44.0))).unwrap();
    let frame = layout(&mut k);
    assert_eq!((frame.width, frame.height), (99.0, 44.0));
}
#[test]
fn provisional_answers_are_counted_and_requeried_without_a_tree_edit() {
    let answers = Rc::new(RefCell::new((true, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 1);
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 2);
    answers.borrow_mut().0 = false;
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 2);
    answers.borrow_mut().0 = true;
    patch(&mut k, 1, &[(StyleId::Display, "none")]);
    layout(&mut k);
    assert_eq!(
        k.provisional_layouts(),
        2,
        "hidden controls are not measured"
    );
}
#[test]
fn the_face_has_a_subtitle_and_all_four_placements() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    for (direction, children, placement) in [
        ("row", vec![5, 3, 4], ButtonImagePlacement::Leading),
        ("row", vec![3, 4, 5], ButtonImagePlacement::Trailing),
        ("column", vec![5, 3, 4], ButtonImagePlacement::Top),
        ("column", vec![3, 4, 5], ButtonImagePlacement::Bottom),
    ] {
        patch(&mut k, 2, &[(StyleId::FlexDirection, direction)]);
        k.apply(0, 1, &[Op::SetChildren { id: 2, children }])
            .unwrap();
        let face = k.press_face(2).unwrap();
        assert_eq!(face.placement, placement);
        assert_eq!(face.title.as_deref(), Some("A long title"));
        assert_eq!(face.subtitle.as_deref(), Some("A subtitle"));
        assert!(
            !face.fits,
            "subtitle faces do not fit one-title projections"
        );
    }
}
fn font(size: f32) -> ControlFont {
    ControlFont {
        family: "Platform".into(),
        family_id: 7,
        size,
        weight: 500,
        style: FontStyle::Normal,
    }
}
fn env(size: f32) -> Env {
    Env {
        control_text_styles: Some(ControlTextStyles {
            field: font(16.0),
            textarea: font(16.0),
            button: font(size),
        }),
        ..Env::default()
    }
}
#[test]
fn computed_rows_keep_authored_provenance_and_the_child_wins() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    let s = k.button_face_style(2).unwrap();
    assert_eq!(s.title.font_size, 30.0, "web inherits page typography");
    assert!(
        !s.title.mask.has(StyleId::FontSize),
        "an ancestor is not authored face typography"
    );
    assert_eq!(s.title.text_align, TextAlign::Center);
    k.set_env(env(17.0)).unwrap();
    let s = k.button_face_style(2).unwrap();
    assert_eq!(s.title.font_size, 17.0, "Apple stops ancestor typography");
    assert_eq!(s.title.font_weight, 500);
    assert!(!s.title.mask.has(StyleId::FontSize));
    patch(
        &mut k,
        2,
        &[
            (StyleId::FontSize, "20"),
            (StyleId::FontWeight, "600"),
            (StyleId::LineClamp, "2"),
            (StyleId::ColumnGap, "4"),
            (StyleId::RowGap, "10"),
            (StyleId::PaddingTop, "1em"),
            (StyleId::BorderRadiusTopLeft, "12"),
            (StyleId::ControlSize, "large"),
            (StyleId::ControlCornerStyle, "capsule"),
        ],
    );
    patch(
        &mut k,
        3,
        &[(StyleId::FontSize, "24"), (StyleId::TextColor, "blue")],
    );
    patch(&mut k, 4, &[(StyleId::FontWeight, "400")]);
    patch(
        &mut k,
        5,
        &[
            (StyleId::TintColor, "green"),
            (StyleId::FontSize, "32"),
            (StyleId::FontWeight, "700"),
        ],
    );
    let s = k.button_face_style(2).unwrap();
    assert_eq!(s.title.font_size, 24.0);
    assert_eq!(s.title.font_weight, 600);
    assert_eq!(s.title.line_clamp, 2);
    assert!(s.title.mask.has(StyleId::LineClamp));
    assert!(s.title.mask.has(StyleId::TextColor));
    assert_eq!(s.subtitle.unwrap().font_weight, 400);
    assert_eq!(s.symbol.font_size, 32.0);
    assert_eq!(s.symbol.font_weight, 700);
    assert!(s.symbol.mask.has(StyleId::TintColor));
    assert_eq!(s.image_gap, Some(4.0));
    assert_eq!(s.button.padding_top, Dimension::Points(20.0));
    assert!(s.button.mask.has(StyleId::BorderRadiusTopLeft));
    assert_eq!(s.button.control_size, ControlSize::Large);
    assert_eq!(s.button.control_corner_style, ControlCornerStyle::Capsule);
    patch(&mut k, 2, &[(StyleId::FlexDirection, "column")]);
    assert_eq!(k.button_face_style(2).unwrap().image_gap, Some(10.0));
}
#[test]
fn an_environment_change_resolves_button_em_lengths_and_invalidates_layout() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    k.set_env(env(17.0)).unwrap();
    patch(&mut k, 2, &[(StyleId::PaddingTop, "1em")]);
    layout(&mut k);
    assert!(k.set_env(env(22.0)).unwrap());
    assert_eq!(
        k.button_face_style(2).unwrap().button.padding_top,
        Dimension::Points(22.0)
    );
    assert_eq!(k.button_face_style(2).unwrap().title.font_size, 22.0);
    layout(&mut k);
}
#[test]
fn an_invalid_button_measure_is_refused_before_geometry_publication() {
    struct Invalid;
    impl TextMeasurer for Invalid {
        fn button_measure(&mut self, _: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
            Some(ButtonMeasure {
                width: f32::NAN,
                height: 34.0,
                provisional: false,
            })
        }
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(request)
        }
    }
    let mut k = tree(Box::new(Invalid));
    assert!(matches!(
        k.compute_layout(1, Offer::definite(500.0, 500.0)),
        Err(KernelError::Layout(LayoutError::InvalidButtonMeasure(2)))
    ));
    assert_eq!(k.node(2).unwrap().frame, Frame::default());
}

#[test]
fn changing_button_style_requests_layout_in_the_same_commit() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    layout(&mut k);
    let receipt = k
        .apply(0, 2, &[prop(2, PropId::ButtonStyle, "plain")])
        .unwrap();
    assert!(receipt.layout_invalidated);
    layout(&mut k);
    assert_eq!(answers.borrow().1.last().unwrap().button_style, "plain");
    let receipt = k
        .apply(
            0,
            3,
            &[Op::ClearProp {
                id: 2,
                prop: PropId::ButtonStyle,
            }],
        )
        .unwrap();
    assert!(receipt.layout_invalidated);
    layout(&mut k);
    assert_eq!(answers.borrow().1.last().unwrap().button_style, "bordered");
}

#[test]
fn ordinary_buttons_reset_ancestor_transforms_while_projected_faces_keep_them() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    patch(&mut k, 1, &[(StyleId::TextTransform, "uppercase")]);
    assert_eq!(
        k.press_face(2).unwrap().title.as_deref(),
        Some("A long title")
    );
    k.set_env(env(17.0)).unwrap();
    assert_eq!(
        k.press_face(2).unwrap().title.as_deref(),
        Some("A long title")
    );
    k.apply(0, 2, &[prop(2, PropId::AccessibilityRole, "tab")])
        .unwrap();
    assert_eq!(
        k.press_face(2).unwrap().title.as_deref(),
        Some("A LONG TITLE")
    );
}

#[test]
fn unrelated_layout_does_not_remeasure_native_buttons() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    layout(&mut k);
    answers.borrow_mut().1.clear();
    patch(&mut k, 1, &[(StyleId::Height, "600")]);
    layout(&mut k);
    assert_eq!(answers.borrow().1.len(), 0);
}

#[test]
fn two_text_custom_face_keeps_the_projected_name() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 6,
                node_type: NodeType::Pressable,
            },
            prop(3, PropId::Text, "Account"),
            prop(4, PropId::Text, "alice@example.com"),
            Op::SetChildren {
                id: 2,
                children: vec![],
            },
            Op::SetChildren {
                id: 6,
                children: vec![3, 4],
            },
            Op::SetChildren {
                id: 1,
                children: vec![6],
            },
        ],
    )
    .unwrap();
    let face = k.press_face(6).unwrap();
    // Projections take the semantic title only for a fitting face; otherwise
    // they join the rendered children, as menu rows did before subtitles.
    let name = if face.fits {
        face.title.unwrap()
    } else {
        [3, 4]
            .iter()
            .map(|&id| k.node(id).unwrap().props.str(PropId::Text).unwrap())
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(name, "Account alice@example.com");
}

#[test]
fn hosts_receive_resolved_padding_and_each_corner() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    patch(&mut k, 1, &[(StyleId::Width, "300")]);
    patch(
        &mut k,
        2,
        &[
            (StyleId::Width, "80"),
            (StyleId::PaddingTop, "10%"),
            (StyleId::PaddingRight, "calc(5% + 2px)"),
            (StyleId::PaddingBottom, "3"),
            (StyleId::PaddingLeft, "4"),
            (StyleId::BorderRadiusTopLeft, "5"),
            (StyleId::BorderRadiusTopRight, "6"),
            (StyleId::BorderRadiusBottomRight, "7"),
            (StyleId::BorderRadiusBottomLeft, "8"),
        ],
    );
    layout(&mut k);
    let s = k.button_face_style(2).unwrap().button;
    assert_eq!(s.padding_top, Dimension::Points(30.0));
    assert_eq!(s.padding_right, Dimension::Points(17.0));
    assert_eq!(
        [
            s.border_radius_top_left,
            s.border_radius_top_right,
            s.border_radius_bottom_right,
            s.border_radius_bottom_left
        ],
        [
            Dimension::Points(5.0),
            Dimension::Points(6.0),
            Dimension::Points(7.0),
            Dimension::Points(8.0)
        ]
    );
    assert!(s.mask.has(StyleId::PaddingTop));
    let state = answers.borrow();
    let s = &state.1.last().unwrap().style.button;
    assert_eq!(s.padding_top, Dimension::Points(30.0));
    assert_eq!(s.padding_right, Dimension::Points(17.0));
}

#[test]
fn control_size_selects_the_font_for_em_and_invalidates_descendants() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    let mut e = env(13.0);
    e.button_fonts = Some(ButtonFonts {
        mini: Some(font(9.0)),
        small: Some(font(11.0)),
        medium: Some(font(13.0)),
        large: Some(font(17.0)),
    });
    k.set_env(e.clone()).unwrap();
    patch(
        &mut k,
        2,
        &[(StyleId::ControlSize, "mini"), (StyleId::Width, "10em")],
    );
    patch(&mut k, 3, &[(StyleId::FontSize, "2em")]);
    assert_eq!(k.button_face_style(2).unwrap().title.font_size, 18.0);
    assert_eq!(layout(&mut k).width, 90.0);
    for (size, px) in [("small", 11.0), ("medium", 13.0), ("large", 17.0)] {
        let receipt = k
            .apply(
                0,
                3,
                &[Op::SetStyle {
                    id: 2,
                    patch: rows(&[(StyleId::ControlSize, size)]),
                }],
            )
            .unwrap();
        assert!(receipt.layout_invalidated);
        assert_eq!(k.button_face_style(2).unwrap().title.font_size, 2.0 * px);
        assert_eq!(layout(&mut k).width, 10.0 * px);
    }
    e.button_fonts.as_mut().unwrap().large = Some(font(20.0));
    assert!(k.set_env(e).unwrap());
    assert_eq!(layout(&mut k).width, 200.0);
    assert_eq!(k.button_face_style(2).unwrap().title.font_size, 40.0);
    assert!(k.set_env(env(13.0)).unwrap());
    assert_eq!(
        layout(&mut k).width,
        130.0,
        "old hosts retain their fallback"
    );
}

#[test]
fn face_edits_remeasure_then_the_next_layout_is_free() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    layout(&mut k);
    for op in [
        prop(3, PropId::Text, "New title"),
        prop(4, PropId::Text, "New subtitle"),
        prop(5, PropId::ImageSource, "symbol:add"),
        Op::SetStyle {
            id: 3,
            patch: rows(&[(StyleId::FontSize, "22")]),
        },
        Op::SetStyle {
            id: 5,
            patch: rows(&[(StyleId::TintColor, "blue")]),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
    ] {
        answers.borrow_mut().1.clear();
        k.apply(0, 3, &[op]).unwrap();
        layout(&mut k);
        assert!(
            !answers.borrow().1.is_empty(),
            "changed face must be measured"
        );
        answers.borrow_mut().1.clear();
        layout(&mut k);
        assert!(answers.borrow().1.is_empty(), "final offers must be reused");
    }
    let face = k.press_face(2).unwrap();
    assert_eq!(face.title.as_deref(), Some("New title"));
    assert_eq!(face.subtitle.as_deref(), Some("New subtitle"));
    assert_eq!(face.symbol.as_deref(), Some("add"));
}

#[test]
fn resolved_padding_tracks_containing_width_with_both_button_axes_fixed() {
    let answers = Rc::new(RefCell::new((false, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers)));
    patch(
        &mut k,
        2,
        &[
            (StyleId::Width, "80"),
            (StyleId::Height, "100"),
            (StyleId::PaddingTop, "10%"),
            (StyleId::PaddingLeft, "calc(5% + 2px)"),
        ],
    );
    for (width, top, left) in [(300.0, 30.0, 17.0), (200.0, 20.0, 12.0)] {
        patch(&mut k, 1, &[(StyleId::Width, &width.to_string())]);
        layout(&mut k);
        let s = k.button_face_style(2).unwrap().button;
        assert_eq!(s.padding_top, Dimension::Points(top));
        assert_eq!(s.padding_left, Dimension::Points(left));
    }
}

#[test]
fn clearing_control_size_and_authored_font_restore_the_platform_em_base() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    let mut e = env(13.0);
    e.button_fonts = Some(ButtonFonts {
        mini: Some(font(9.0)),
        ..ButtonFonts::default()
    });
    k.set_env(e.clone()).unwrap();
    patch(
        &mut k,
        2,
        &[
            (StyleId::ControlSize, "mini"),
            (StyleId::FontSize, "20"),
            (StyleId::Width, "10em"),
        ],
    );
    assert_eq!(layout(&mut k).width, 200.0);
    k.apply(
        0,
        3,
        &[Op::ClearStyle {
            id: 2,
            mask: StyleMask::of(StyleId::FontSize),
        }],
    )
    .unwrap();
    assert_eq!(layout(&mut k).width, 90.0);
    k.apply(
        0,
        4,
        &[Op::ClearStyle {
            id: 2,
            mask: StyleMask::of(StyleId::ControlSize),
        }],
    )
    .unwrap();
    assert_eq!(layout(&mut k).width, 130.0);
    e.button_fonts.as_mut().unwrap().mini = Some(font(f32::NAN));
    assert!(matches!(
        k.set_env(e),
        Err(KernelError::Layout(LayoutError::InvalidEnv))
    ));
}

#[test]
fn a_fully_sized_provisional_button_is_still_withheld() {
    let answers = Rc::new(RefCell::new((true, Vec::new())));
    let mut k = tree(Box::new(Buttons(answers.clone())));
    patch(
        &mut k,
        2,
        &[(StyleId::Width, "80"), (StyleId::Height, "100")],
    );
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 1);
    answers.borrow_mut().0 = false;
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 1);
    answers.borrow_mut().1.clear();
    layout(&mut k);
    assert!(answers.borrow().1.is_empty());
}

#[test]
fn unresolved_host_still_publishes_resolved_button_padding() {
    let mut k = tree(Box::new(MonospaceMeasurer::default()));
    patch(&mut k, 1, &[(StyleId::Width, "300")]);
    patch(&mut k, 2, &[(StyleId::PaddingTop, "10%")]);
    layout(&mut k);
    assert_eq!(
        k.button_face_style(2).unwrap().button.padding_top,
        Dimension::Points(30.0)
    );
}

#[test]
fn a_scale_only_measurement_revision_remeasures_without_a_tree_edit() {
    use std::cell::Cell;
    struct Scaled {
        scale: Rc<Cell<u64>>,
        calls: Rc<Cell<usize>>,
    }
    impl TextMeasurer for Scaled {
        fn measure_revision(&self) -> u64 {
            self.scale.get()
        }
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(r)
        }
        fn button_measure(&mut self, _: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
            self.calls.set(self.calls.get() + 1);
            let scale = self.scale.get() as f32;
            Some(ButtonMeasure {
                width: 100.0,
                height: (30.2 * scale).ceil() / scale,
                provisional: false,
            })
        }
    }
    let scale = Rc::new(Cell::new(1));
    let calls = Rc::new(Cell::new(0));
    let mut k = tree(Box::new(Scaled {
        scale: scale.clone(),
        calls: calls.clone(),
    }));
    assert_eq!(layout(&mut k).height, 31.0);
    let env = k.env();
    calls.set(0);
    scale.set(2);
    assert_eq!(layout(&mut k).height, 30.5);
    assert!(calls.get() > 0);
    assert_eq!(k.env(), env, "only the host scale changed");
    calls.set(0);
    layout(&mut k);
    assert_eq!(calls.get(), 0, "the new revision's offers are reused");
}
