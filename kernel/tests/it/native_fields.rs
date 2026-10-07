//! Platform typography and field geometry. @ref LLP 1104 D4–D5, §4.
use exact_kernel::*;
use std::{cell::RefCell, rc::Rc};

type Answers = Rc<RefCell<(FieldChrome, Vec<FieldChromeRequest>)>>;
struct Fields(Answers);
impl TextMeasurer for Fields {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let font = request.paragraph.strut;
        TextMetrics {
            width: 100.0,
            height: font.font_size * 1.25,
            first_baseline: Some(font.font_size),
        }
    }
    fn field_chrome(&mut self, request: &FieldChromeRequest) -> FieldChrome {
        let mut state = self.0.borrow_mut();
        state.1.push(request.clone());
        state.0
    }
}
fn font(size: f32, id: u16) -> ControlFont {
    ControlFont {
        family: format!("Platform {id}"),
        family_id: id,
        size,
        weight: 500,
        style: FontStyle::Normal,
    }
}
fn env(size: f32) -> Env {
    Env {
        control_text_styles: Some(ControlTextStyles {
            field: font(size, 7),
            textarea: font(size + 2.0, 6),
            button: font(13.0, 0),
        }),
        ..Env::default()
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
fn tree(chrome: FieldChrome) -> (Kernel, Answers) {
    let state = Rc::new(RefCell::new((chrome, Vec::new())));
    let mut k = Kernel::new(Box::new(Fields(state.clone())));
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::TextInput,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::TextInput,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::TextInput,
            },
            prop(3, PropId::Text, "beside"),
            prop(5, PropId::SemanticTag, "textarea"),
            Op::SetStyle {
                id: 4,
                patch: rows(&[(StyleId::Appearance, "none")]),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3, 4, 5],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    (k, state)
}
fn layout(k: &mut Kernel) -> LayoutReceipt {
    k.compute_layout(1, Offer::definite(600.0, 500.0)).unwrap()
}
fn number(k: &Kernel, id: u32, row: StyleId) -> f64 {
    match k.node(id).unwrap().computed(row) {
        RowValue::Number(v) => v,
        other => panic!("{other:?}"),
    }
}
fn chrome() -> FieldChrome {
    FieldChrome {
        top: 3.0,
        right: 5.0,
        bottom: 7.0,
        left: 11.0,
        minimum_height: 0.0,
        provisional: false,
    }
}

#[test]
fn native_typography_stops_authored_wins_and_clear_restores_every_starting_row() {
    let (mut k, _) = tree(FieldChrome::default());
    let ancestor = rows(&[
        (StyleId::FontSize, "32"),
        (StyleId::FontFamily, "3"),
        (StyleId::FontWeight, "800"),
        (StyleId::FontStyle, "italic"),
        (StyleId::LineHeight, "2"),
        (StyleId::LetterSpacing, "4"),
        (StyleId::TextTransform, "uppercase"),
        (StyleId::TextIndent, "9"),
        (StyleId::TextShadow, "1px 2px red"),
        (StyleId::TextAlign, "center"),
        (StyleId::TextColor, "red"),
    ]);
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: ancestor.clone(),
        }],
    )
    .unwrap();
    assert!(k.set_env(env(18.0)).unwrap());
    let native = k.node(2).unwrap().computed_style(StyleMask::INHERITED);
    assert_eq!(
        (
            native.font_size,
            native.font_family,
            native.font_weight,
            native.font_style
        ),
        (18.0, 7, 500, FontStyle::Normal)
    );
    assert_eq!(native.line_height, LineHeight::Normal);
    assert_eq!((native.letter_spacing, native.text_indent), (0.0, 0.0));
    assert_eq!(native.text_transform, TextTransform::None);
    assert_eq!(native.text_align, TextAlign::Start);
    assert_eq!(native.text_shadow, StyleProps::default().text_shadow);
    assert_eq!(
        native.text_color,
        ColorValue::Role(style::roles::role("FieldText").unwrap())
    );
    for id in [3, 4] {
        let inherited = k.node(id).unwrap().computed_style(StyleMask::INHERITED);
        assert_eq!(inherited.font_size, 32.0);
        assert_eq!(inherited.font_style, FontStyle::Italic);
        assert_eq!(inherited.text_color, ancestor.text_color);
    }
    assert_eq!(k.node(5).unwrap().text_style().font_size, 20.0);
    let initial = native.clone();
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 2,
            patch: ancestor.clone(),
        }],
    )
    .unwrap();
    for row in ancestor.mask.iter() {
        assert_eq!(k.node(2).unwrap().computed(row), ancestor.get(row));
    }
    k.apply(
        0,
        0,
        &[Op::ClearStyle {
            id: 2,
            mask: ancestor.mask,
        }],
    )
    .unwrap();
    let restored = k.node(2).unwrap().computed_style(StyleMask::INHERITED);
    for row in ancestor.mask.iter() {
        assert_eq!(restored.get(row), initial.get(row), "{row:?}");
        assert!(restored.mask.has(row), "hosts must receive reset {row:?}");
        assert_eq!(k.node(2).unwrap().source_of(row), None);
    }
    let runs = k.node(2).unwrap().text_runs();
    assert_eq!(runs[0].style, k.node(2).unwrap().text_style());
    let slot = k.node(2).unwrap().key.index;
    assert_eq!(k.arena().paragraph(slot).strut, runs[0].style);
    assert_eq!(k.arena().paragraph(slot).text_indent, 0.0);
    assert_eq!(k.arena().paragraph(slot).text_align, TextAlign::Left);
}

#[test]
fn none_keeps_existing_inheritance_and_restores_it_after_platform_fonts() {
    let (mut k, _) = tree(FieldChrome::default());
    patch(
        &mut k,
        1,
        &[
            (StyleId::FontSize, "32"),
            (StyleId::FontStyle, "italic"),
            (StyleId::TextIndent, "9"),
        ],
    );
    for id in [2, 3, 4, 5] {
        assert_eq!(number(&k, id, StyleId::FontSize), 32.0);
    }
    k.set_env(env(18.0)).unwrap();
    assert!(k.set_env(Env::default()).unwrap());
    for id in [2, 3, 4, 5] {
        assert_eq!(number(&k, id, StyleId::FontSize), 32.0);
        assert_eq!(number(&k, id, StyleId::TextIndent), 9.0);
        assert_eq!(
            k.node(id).unwrap().text_style().font_style,
            FontStyle::Italic
        );
    }
}

#[test]
fn em_uses_platform_font_and_env_rederives_only_its_readers_without_a_commit() {
    let (mut k, state) = tree(FieldChrome::default());
    patch(&mut k, 1, &[(StyleId::FontSize, "40")]);
    patch(
        &mut k,
        2,
        &[
            (StyleId::PaddingTop, "1em"),
            (StyleId::PaddingRight, "1em"),
            (StyleId::PaddingBottom, "1em"),
            (StyleId::PaddingLeft, "1em"),
        ],
    );
    k.set_env(env(18.0)).unwrap();
    layout(&mut k);
    assert_eq!(
        k.node(2).unwrap().style.padding_top,
        Dimension::Points(18.0)
    );
    let stamps: Vec<_> = (2..=5)
        .map(|id| k.node(id).unwrap().paragraph_stamp().unwrap())
        .collect();
    let epoch = k.epoch();
    assert!(k.set_env(env(24.0)).unwrap());
    assert_eq!(k.epoch(), epoch);
    assert_eq!(
        k.node(2).unwrap().style.padding_top,
        Dimension::Points(24.0)
    );
    for id in [2, 5] {
        let n = k.node(id).unwrap();
        assert!(k.arena().flags(n.key.index).has(NodeFlags::TEXT_DIRTY));
        assert!(k.arena().flags(n.key.index).has(NodeFlags::STYLE_DIRTY));
        assert!(!stamps[(id - 2) as usize].same_metrics(&n.paragraph_stamp().unwrap()));
    }
    for id in [3, 4] {
        let n = k.node(id).unwrap();
        assert!(!k.arena().flags(n.key.index).has(NodeFlags::TEXT_DIRTY));
        assert!(stamps[(id - 2) as usize].same_metrics(&n.paragraph_stamp().unwrap()));
    }
    layout(&mut k);
    assert_eq!(k.node(2).unwrap().field_content_rect().unwrap().y, 24.0);
    assert!(state
        .borrow()
        .1
        .iter()
        .any(|r| r.kind == FieldKind::Textarea && r.font.size == 26.0));
    assert!(!k.set_env(env(24.0)).unwrap());
}

#[test]
fn relative_font_rows_start_at_platform_and_other_em_rows_use_authored_font() {
    let (mut k, _) = tree(FieldChrome::default());
    k.set_env(env(18.0)).unwrap();
    patch(&mut k, 1, &[(StyleId::FontSize, "40")]);
    patch(
        &mut k,
        2,
        &[(StyleId::FontSize, "2em"), (StyleId::PaddingTop, "1em")],
    );
    assert_eq!(number(&k, 2, StyleId::FontSize), 36.0);
    assert_eq!(
        k.node(2).unwrap().style.padding_top,
        Dimension::Points(36.0)
    );
    k.set_env(env(20.0)).unwrap();
    assert_eq!(number(&k, 2, StyleId::FontSize), 40.0);
    assert_eq!(
        k.node(2).unwrap().style.padding_top,
        Dimension::Points(40.0)
    );
}

#[test]
fn content_box_sizes_and_constraints_add_chrome_padding_and_border() {
    let (mut k, _) = tree(chrome());
    patch(
        &mut k,
        2,
        &[
            (StyleId::Width, "100"),
            (StyleId::Height, "40"),
            (StyleId::MinHeight, "50"),
            (StyleId::MaxWidth, "80"),
            (StyleId::PaddingTop, "2"),
            (StyleId::PaddingRight, "4"),
            (StyleId::PaddingBottom, "6"),
            (StyleId::PaddingLeft, "8"),
            (StyleId::BorderWidthTop, "1"),
            (StyleId::BorderWidthRight, "1"),
            (StyleId::BorderWidthBottom, "1"),
            (StyleId::BorderWidthLeft, "1"),
            (StyleId::BorderStyleTop, "solid"),
            (StyleId::BorderStyleRight, "solid"),
            (StyleId::BorderStyleBottom, "solid"),
            (StyleId::BorderStyleLeft, "solid"),
        ],
    );
    layout(&mut k);
    let n = k.node(2).unwrap();
    assert_eq!((n.frame.width, n.frame.height), (110.0, 70.0));
    assert_eq!(
        n.field_content_rect(),
        Some(Frame {
            x: 20.0,
            y: 6.0,
            width: 80.0,
            height: 50.0
        })
    );
    assert_eq!(
        n.style.padding_left,
        Dimension::Points(8.0),
        "chrome never authors padding"
    );
    assert_eq!(
        n.style.border_width_top, 1.0,
        "chrome never authors a border"
    );
    assert_eq!(k.node(4).unwrap().field_content_rect(), None);
}

#[test]
fn minimum_frame_height_wins_even_over_short_height_and_max_height() {
    let (mut k, _) = tree(FieldChrome {
        minimum_height: 44.0,
        ..chrome()
    });
    patch(
        &mut k,
        2,
        &[(StyleId::Height, "5"), (StyleId::MaxHeight, "8")],
    );
    layout(&mut k);
    assert_eq!(k.node(2).unwrap().frame.height, 44.0);
    assert_eq!(
        k.node(2).unwrap().field_content_rect().unwrap().height,
        34.0
    );
    // A textarea ignores the single-line minimum.
    patch(&mut k, 5, &[(StyleId::Height, "5")]);
    layout(&mut k);
    assert_eq!(k.node(5).unwrap().frame.height, 15.0);
}

#[test]
fn baseline_uses_centered_line_but_textarea_keeps_top_baseline() {
    let (mut k, _) = tree(chrome());
    k.set_env(env(16.0)).unwrap();
    patch(
        &mut k,
        1,
        &[
            (StyleId::Display, "flex"),
            (StyleId::AlignItems, "baseline"),
        ],
    );
    patch(&mut k, 2, &[(StyleId::Height, "60")]);
    patch(&mut k, 5, &[(StyleId::Height, "60")]);
    layout(&mut k);
    let text = k.node(3).unwrap().frame;
    let field = k.node(2).unwrap();
    let content = field.field_content_rect().unwrap();
    let baseline = field.frame.y + content.y + (content.height - 20.0) / 2.0 + 16.0;
    assert_eq!(baseline, text.y + 16.0);
    let area = k.node(5).unwrap();
    assert_eq!(
        area.frame.y + area.field_content_rect().unwrap().y + 18.0,
        baseline
    );
}

#[test]
fn provisional_counts_once_per_successful_layout_only_for_native_fields() {
    let (mut k, state) = tree(FieldChrome {
        provisional: true,
        ..chrome()
    });
    assert_eq!(k.provisional_layouts(), 0);
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 1);
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 2);
    assert!(state
        .borrow()
        .1
        .iter()
        .all(|r| matches!(r.kind, FieldKind::Field | FieldKind::Textarea)));
    state.borrow_mut().0.provisional = false;
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 2);
    patch(&mut k, 2, &[(StyleId::Appearance, "none")]);
    patch(&mut k, 5, &[(StyleId::Appearance, "none")]);
    state.borrow_mut().0.provisional = true;
    let calls = state.borrow().1.len();
    layout(&mut k);
    assert_eq!(k.provisional_layouts(), 2);
    assert_eq!(state.borrow().1.len(), calls);
    assert_eq!(k.node(2).unwrap().field_content_rect(), None);
    assert_eq!(k.node(2).unwrap().frame.width, 100.0);
}

#[test]
fn secure_search_and_textarea_requests_use_resolved_authored_fonts() {
    let (mut k, state) = tree(FieldChrome::default());
    k.set_env(env(18.0)).unwrap();
    k.apply(0, 0, &[prop(2, PropId::Type, "password")]).unwrap();
    patch(
        &mut k,
        2,
        &[
            (StyleId::FontSize, "22"),
            (StyleId::FontWeight, "700"),
            (StyleId::FontStyle, "italic"),
        ],
    );
    layout(&mut k);
    let state_read = state.borrow();
    let request = state_read
        .1
        .iter()
        .find(|r| r.kind == FieldKind::SecureField)
        .unwrap();
    assert_eq!(
        (request.font.size, request.font.weight, request.font.style),
        (22.0, 700, FontStyle::Italic)
    );
    assert_eq!(request.font.family, "Platform 7");
    assert_eq!(request.font.family_id, 7);
    drop(state_read);
    k.apply(0, 0, &[prop(2, PropId::Type, "search")]).unwrap();
    layout(&mut k);
    assert!(state
        .borrow()
        .1
        .iter()
        .any(|r| r.kind == FieldKind::SearchField));
}

#[test]
fn invalid_environment_sizes_and_chrome_are_refused_atomically() {
    let (mut k, state) = tree(FieldChrome::default());
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            k.set_env(env(size)),
            Err(KernelError::Layout(LayoutError::InvalidEnv))
        );
        assert_eq!(k.env(), Env::default());
    }
    let mut invalid = env(18.0);
    invalid.control_text_styles.as_mut().unwrap().button.size = 0.0;
    assert!(k.set_env(invalid).is_err());
    state.borrow_mut().0.left = f32::NAN;
    assert!(matches!(
        k.compute_layout(1, Offer::definite(600.0, 500.0)),
        Err(KernelError::Layout(LayoutError::InvalidFieldChrome(_)))
    ));
    assert_eq!(k.provisional_layouts(), 0);
    state.borrow_mut().0 = chrome();
    layout(&mut k);
    assert_eq!(k.node(2).unwrap().frame.width, 116.0);
}

#[test]
fn chrome_minimum_tracks_percentage_padding_and_changed_answers() {
    let (mut k, state) = tree(FieldChrome {
        minimum_height: 70.0,
        ..chrome()
    });
    patch(
        &mut k,
        2,
        &[
            (StyleId::Height, "5"),
            (StyleId::PaddingTop, "2%"),
            (StyleId::PaddingBottom, "2%"),
        ],
    );
    layout(&mut k);
    assert_eq!(k.node(2).unwrap().frame.height, 70.0);
    assert_eq!(
        k.node(2).unwrap().field_content_rect().unwrap().height,
        36.0
    );
    k.compute_layout(1, Offer::definite(400.0, 500.0)).unwrap();
    assert_eq!(k.node(2).unwrap().frame.height, 70.0);
    assert_eq!(
        k.node(2).unwrap().field_content_rect().unwrap().height,
        44.0
    );
    state.borrow_mut().0.minimum_height = 0.0;
    layout(&mut k);
    assert_eq!(k.node(2).unwrap().frame.height, 39.0);
}

#[test]
fn ancestor_typography_changes_do_not_invalidate_stopped_field_metrics() {
    let (mut k, _) = tree(FieldChrome::default());
    k.set_env(env(18.0)).unwrap();
    layout(&mut k);
    let native = k.node(2).unwrap().paragraph_stamp().unwrap();
    let bare = k.node(4).unwrap().paragraph_stamp().unwrap();
    patch(&mut k, 1, &[(StyleId::FontSize, "44")]);
    assert!(native.same_metrics(&k.node(2).unwrap().paragraph_stamp().unwrap()));
    assert!(!bare.same_metrics(&k.node(4).unwrap().paragraph_stamp().unwrap()));
}
