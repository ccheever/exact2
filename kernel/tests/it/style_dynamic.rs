//! `StyleProps::set_dynamic`: the one place an id-addressed producer turns an
//! untyped value into a row, refused typed, nothing changed on refusal.

use exact_kernel::{
    Color, ColorValue, Dimension, StyleId, StyleProps, StyleValue, StyleValueError,
};

#[test]
fn translate_text_is_a_narrow_css_pixel_subset_with_atomic_refusal() {
    let mut s = StyleProps::default();
    for (text, x, y) in [
        ("0", 0.0, 0.0),
        ("-0 +0", 0.0, 0.0),
        ("12px", 12.0, 0.0),
        ("-1.25px +.5PX", -1.25, 0.5),
        ("\t1e2px\n-2E1px\r\u{c}", 100.0, -20.0),
    ] {
        s.set_dynamic(StyleId::Translate, &StyleValue::Text(text.into()))
            .unwrap();
        assert_eq!(s.translate, exact_kernel::Vec2 { x, y }, "{text}");
    }
    let max = format!("{}px 0", f32::MAX as f64);
    s.set_dynamic(StyleId::Translate, &StyleValue::Text(max))
        .unwrap();
    assert_eq!(s.translate.x, f32::MAX);
    for text in [
        "",
        " ",
        "1 0",
        "0 1",
        "none",
        "1px,2px",
        "1px 2px 0px",
        "10% 0",
        "calc(1px + 2px) 0",
        "NaNpx 0",
        "infpx 0",
        "1e39px 0",
        "1e-999 0",
        "1.px 0",
        "1e+px 0",
        "+px 0",
        "0\u{b}0",
        "1px\u{a0}2px",
    ] {
        let before = s.clone();
        assert!(
            s.set_dynamic(StyleId::Translate, &StyleValue::Text(text.into()))
                .is_err(),
            "{text}"
        );
        assert_eq!(s, before, "refusal must not change row or mask: {text}");
    }
    s.set_dynamic(StyleId::Translate, &StyleValue::Vec2(5.0, -7.0))
        .unwrap();
    s.set_dynamic(StyleId::ShadowOffset, &StyleValue::Vec2(1.0, 2.0))
        .unwrap();
    let before = s.clone();
    assert!(s
        .set_dynamic(StyleId::ShadowOffset, &StyleValue::Text("1px 2px".into()))
        .is_err());
    assert!(s
        .set_dynamic(StyleId::Translate, &StyleValue::Number(0.0))
        .is_err());
    assert_eq!(s, before, "no general scalar/vector coercion");
}

#[test]
fn border_defaults_clear_and_wire_preserve_authored_width_separately_from_used_width() {
    use exact_kernel::{wire, Kernel, NodeType, Op, StyleMask};
    let mut style = StyleProps::default();
    assert_eq!(style.border_width_top, 3.0);
    assert_eq!(style.border_widths(), [0.0; 4]);
    assert_eq!(style.border_color_top, None);
    style
        .set_dynamic(StyleId::BorderWidthTop, &StyleValue::Number(8.0))
        .unwrap();
    style
        .set_dynamic(StyleId::BorderStyleTop, &StyleValue::Text("solid".into()))
        .unwrap();
    style
        .set_dynamic(StyleId::BorderStyleRight, &StyleValue::Text("solid".into()))
        .unwrap();
    style
        .set_dynamic(
            StyleId::BorderColorTop,
            &StyleValue::Text("currentColor".into()),
        )
        .unwrap();
    style
        .set_dynamic(
            StyleId::BorderColorRight,
            &StyleValue::Text("#00000000".into()),
        )
        .unwrap();
    assert_eq!(style.border_widths(), [8.0, 3.0, 0.0, 0.0]);
    assert!(style
        .set_dynamic(StyleId::BorderColorTop, &StyleValue::Auto)
        .is_err());
    assert!(style
        .set_dynamic(StyleId::BorderStyleTop, &StyleValue::Text("dashed".into()))
        .is_err());
    let mut k = Kernel::with_monospace();
    k.apply_frame(&wire::encode(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(style),
            },
        ],
    ))
    .unwrap();
    assert_eq!(
        k.node(1).unwrap().style.border_colors(Color::WHITE.into()),
        [
            Color::WHITE.into(),
            Color::TRANSPARENT.into(),
            Color::WHITE.into(),
            Color::WHITE.into()
        ]
    );
    let mut hidden = StyleProps::default();
    hidden
        .set_dynamic(StyleId::BorderStyleTop, &StyleValue::Text("hidden".into()))
        .unwrap();
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(hidden),
        }],
    )
    .unwrap();
    assert_eq!(
        k.node(1).unwrap().style.border_widths(),
        [0.0, 3.0, 0.0, 0.0]
    );
    assert_eq!(k.node(1).unwrap().style.border_width_top, 8.0);
    k.apply(
        0,
        3,
        &[Op::ClearStyle {
            id: 1,
            mask: StyleMask::of(StyleId::BorderStyleRight),
        }],
    )
    .unwrap();
    assert_eq!(k.node(1).unwrap().style.border_widths(), [0.0; 4]);
}

#[test]
fn every_dynamic_codec_fills_its_row_and_marks_the_mask() {
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::Width, &StyleValue::Percent(50.0))
        .unwrap();
    s.set_dynamic(StyleId::Height, &StyleValue::Auto).unwrap();
    s.set_dynamic(StyleId::FlexGrow, &StyleValue::Number(1.0))
        .unwrap();
    s.set_dynamic(StyleId::FontWeight, &StyleValue::Number(700.0))
        .unwrap();
    s.set_dynamic(StyleId::ZIndex, &StyleValue::Number(-2.0))
        .unwrap();
    s.set_dynamic(
        StyleId::BackgroundColor,
        &StyleValue::Text("#ff000080".into()),
    )
    .unwrap();
    s.set_dynamic(StyleId::TextColor, &StyleValue::Number(0x1122_33ff as f64))
        .unwrap();
    s.set_dynamic(StyleId::FlexDirection, &StyleValue::Text("column".into()))
        .unwrap();
    s.set_dynamic(StyleId::ShadowOffset, &StyleValue::Vec2(1.0, 2.0))
        .unwrap();
    assert_eq!(s.width, Dimension::Percent(50.0));
    assert_eq!(s.height, Dimension::Auto);
    assert_eq!(s.flex_grow, 1.0);
    assert_eq!(s.font_weight, 700);
    assert_eq!(s.z_index, -2);
    assert_eq!(
        s.background_color,
        ColorValue::Fixed(Color::rgba(255, 0, 0, 128))
    );
    assert_eq!(s.text_color, ColorValue::Fixed(Color(0x1122_33ff)));
    assert_eq!(s.flex_direction, exact_kernel::FlexDirection::Column);
    assert_eq!((s.shadow_offset.x, s.shadow_offset.y), (1.0, 2.0));
    for id in [
        StyleId::Width,
        StyleId::Height,
        StyleId::FlexGrow,
        StyleId::FontWeight,
        StyleId::ZIndex,
        StyleId::BackgroundColor,
        StyleId::TextColor,
        StyleId::FlexDirection,
        StyleId::ShadowOffset,
    ] {
        assert!(s.mask.has(id), "{id:?} marked");
    }
    assert_eq!(s.mask.count(), 9);
}

#[test]
fn refusals_are_typed_and_change_nothing() {
    let mut s = StyleProps::default();
    let before = s.clone();
    let cases: Vec<(StyleId, StyleValue, StyleValueError)> = vec![
        (
            StyleId::FlexGrow,
            StyleValue::Text("x".into()),
            StyleValueError::WrongKind {
                style: StyleId::FlexGrow,
                expected: "nonnegative number",
            },
        ),
        // CSS refuses a negative flex factor (LLP 1053 G3).
        (
            StyleId::FlexShrink,
            StyleValue::Number(-1.0),
            StyleValueError::WrongKind {
                style: StyleId::FlexShrink,
                expected: "nonnegative number",
            },
        ),
        (
            StyleId::FlexDirection,
            StyleValue::Text("sideways".into()),
            StyleValueError::UnknownEnumValue {
                style: StyleId::FlexDirection,
            },
        ),
        (
            StyleId::FlexGrow,
            StyleValue::Auto,
            StyleValueError::WrongKind {
                style: StyleId::FlexGrow,
                expected: "nonnegative number",
            },
        ),
        (
            StyleId::PaddingTop,
            StyleValue::Auto,
            StyleValueError::AutoNotAdmitted {
                style: StyleId::PaddingTop,
            },
        ),
        (
            StyleId::FontWeight,
            StyleValue::Number(70000.0),
            StyleValueError::OutOfRange {
                style: StyleId::FontWeight,
            },
        ),
        (
            StyleId::FontWeight,
            StyleValue::Number(1.5),
            StyleValueError::WrongKind {
                style: StyleId::FontWeight,
                expected: "integer",
            },
        ),
        (
            StyleId::BackgroundColor,
            StyleValue::Text("#12".into()),
            StyleValueError::BadColor {
                style: StyleId::BackgroundColor,
            },
        ),
        (
            StyleId::GridTemplateColumns,
            StyleValue::Number(1.0),
            StyleValueError::Unsupported {
                style: StyleId::GridTemplateColumns,
            },
        ),
    ];
    for (id, value, expected) in cases {
        assert_eq!(s.set_dynamic(id, &value), Err(expected));
    }
    assert_eq!(s, before, "a refused write changes nothing");
}

#[test]
fn enum_rows_resolve_names_and_others_do_not() {
    assert_eq!(
        StyleId::FlexDirection.enum_from_name("row-reverse"),
        Some(2)
    );
    assert_eq!(StyleId::FlexDirection.enum_from_name("diagonal"), None);
    assert_eq!(StyleId::Width.enum_from_name("auto"), None);
    assert_eq!(
        StyleId::FlexDirection.enum_names(),
        &["row", "column", "row-reverse", "column-reverse"]
    );
    for id in StyleId::ALL {
        let names = id.enum_names();
        // A keyword row (a bit set named by CSS keywords) lists its words too.
        let keywords = id == StyleId::FontVariantNumeric;
        assert_eq!(
            names.is_empty(),
            id.codec() != exact_kernel::StyleCodec::Enum && !keywords,
            "{id:?}"
        );
        for (ordinal, name) in names.iter().enumerate() {
            assert_eq!(
                id.enum_from_name(name),
                (!keywords).then_some(ordinal as u8),
                "{id:?}: {name}"
            );
            StyleProps::default()
                .set_dynamic(id, &StyleValue::Text((*name).into()))
                .unwrap();
        }
    }
}

#[test]
fn font_variant_numeric_is_a_keyword_bit_set() {
    // @ref LLP 1053 §0 G4 — `normal` is 0, `tabular-nums` bit 0; others refused.
    let set = |v: StyleValue| {
        let mut p = StyleProps::default();
        p.set_dynamic(StyleId::FontVariantNumeric, &v)
            .map(|_| p.font_variant_numeric)
    };
    assert_eq!(set(StyleValue::Text("normal".into())), Ok(0));
    assert_eq!(set(StyleValue::Text(" tabular-nums ".into())), Ok(1));
    for refused in [
        "oldstyle-nums",
        "tabular-nums tabular-nums",
        "normal tabular-nums",
        "",
        "1",
    ] {
        assert!(
            set(StyleValue::Text(refused.into())).is_err(),
            "{refused:?}"
        );
    }
    assert!(
        set(StyleValue::Number(1.0)).is_err(),
        "a keyword row takes no number"
    );
    assert_eq!(exact_kernel::FontVariantNumeric::css(1), "tabular-nums");
    assert_eq!(exact_kernel::FontVariantNumeric::css(0), "normal");
}

#[test]
fn hex_colors_parse_in_all_four_css_forms() {
    assert_eq!(Color::parse_hex("#f00"), Some(Color::rgba(255, 0, 0, 255)));
    assert_eq!(Color::parse_hex("#f008"), Some(Color::rgba(255, 0, 0, 136)));
    assert_eq!(
        Color::parse_hex("#ff0000"),
        Some(Color::rgba(255, 0, 0, 255))
    );
    assert_eq!(
        Color::parse_hex("#ff000080"),
        Some(Color::rgba(255, 0, 0, 128))
    );
    assert_eq!(Color::parse_hex("ff0000"), None);
    assert_eq!(Color::parse_hex("#gg0000"), None);
}

#[test]
fn caret_auto_and_transparent_are_distinct_and_survive_the_wire() {
    use exact_kernel::{wire, DecodeError, RowValue};
    for value in [
        StyleValue::Auto,
        StyleValue::Text("auto".into()),
        StyleValue::Text("#00000000".into()),
        StyleValue::Text("#ffffff".into()),
        StyleValue::Text("light-dark(#ffffff, #112233)".into()),
    ] {
        let mut style = StyleProps::default();
        style.set_dynamic(StyleId::CaretColor, &value).unwrap();
        let mut bytes = wire::codec::Writer::new();
        bytes.optional_color(style.caret_color);
        let mut reader = wire::codec::Reader::new(bytes.as_slice());
        assert_eq!(reader.optional_color().unwrap(), style.caret_color);
        match value {
            StyleValue::Auto => assert_eq!(style.get(StyleId::CaretColor), RowValue::Enum("auto")),
            StyleValue::Text(ref text) if text == "#00000000" => {
                assert_eq!(style.caret_color, Some(Color::TRANSPARENT.into()));
            }
            _ => {}
        }
        let before = style.clone();
        assert!(style
            .set_dynamic(StyleId::CaretColor, &StyleValue::Text("not-a-color".into()))
            .is_err());
        assert_eq!(
            style, before,
            "a refused update preserves the authored caret"
        );
    }
    assert_eq!(
        wire::codec::Reader::new(&[2]).optional_color(),
        Err(DecodeError::BadColorValue(2))
    );
    assert!(wire::codec::Reader::new(&[1]).optional_color().is_err());
}

#[test]
fn clearing_a_caret_override_restores_inheritance_not_auto() {
    use exact_kernel::{wire, Kernel, NodeType, Op, StyleMask};
    let caret = |value| {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::CaretColor, &value).unwrap();
        Box::new(s)
    };
    let mut k = Kernel::with_monospace();
    let mask = StyleMask::of(StyleId::CaretColor);
    k.apply_frame(&wire::encode(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::TextInput,
            },
            Op::SetStyle {
                id: 1,
                patch: caret(StyleValue::Text("#ffffff".into())),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    ))
    .unwrap();
    assert_eq!(
        k.node(2).unwrap().computed_style(mask).caret_color,
        Some(Color::WHITE.into())
    );
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 2,
            patch: caret(StyleValue::Auto),
        }],
    )
    .unwrap();
    assert_eq!(
        k.node(2).unwrap().computed_style(mask).caret_color,
        None,
        "explicit auto stops inheritance"
    );
    k.apply(0, 3, &[Op::ClearStyle { id: 2, mask }]).unwrap();
    assert_eq!(
        k.node(2).unwrap().computed_style(mask).caret_color,
        Some(Color::WHITE.into())
    );
    k.apply(0, 4, &[Op::ClearStyle { id: 1, mask }]).unwrap();
    assert_eq!(k.node(2).unwrap().computed_style(mask).caret_color, None);
}

#[test]
fn line_height_preserves_kinds_through_dynamic_changes_wire_and_refusal() {
    use exact_kernel::{wire, Kernel, LineHeight, NodeType, Op};
    let mut style = StyleProps::default();
    assert_eq!(style.line_height, LineHeight::Normal);
    for (input, expected) in [
        (StyleValue::Number(1.5), LineHeight::Number(1.5)),
        (StyleValue::Text("24px".into()), LineHeight::Length(24.0)),
        (StyleValue::Text("normal".into()), LineHeight::Normal),
        (StyleValue::Number(0.0), LineHeight::Number(0.0)),
        (StyleValue::Text("0px".into()), LineHeight::Length(0.0)),
    ] {
        style.set_dynamic(StyleId::LineHeight, &input).unwrap();
        assert_eq!(style.line_height, expected);
        let mut kernel = Kernel::with_monospace();
        kernel
            .apply_frame(&wire::encode(
                0,
                1,
                &[
                    Op::CreateView {
                        id: 1,
                        node_type: NodeType::Text,
                    },
                    Op::SetStyle {
                        id: 1,
                        patch: Box::new(style.clone()),
                    },
                ],
            ))
            .unwrap();
        assert_eq!(kernel.node(1).unwrap().style.line_height, expected);
    }
    for invalid in [
        StyleValue::Number(-1.0),
        StyleValue::Number(f64::NAN),
        StyleValue::Number(f64::INFINITY),
        StyleValue::Text("-2px".into()),
        StyleValue::Text("NaNpx".into()),
        StyleValue::Text("1em".into()),
        StyleValue::Text("150%".into()),
        StyleValue::Text("24".into()),
    ] {
        let before = style.clone();
        assert!(style.set_dynamic(StyleId::LineHeight, &invalid).is_err());
        assert_eq!(style, before);
    }
}

#[test]
fn dimension_pixel_strings_use_css_numbers_and_refuse_invalid_values_atomically() {
    let mut style = StyleProps::default();
    for (text, expected) in [("0px", 0.), ("12.5PX", 12.5), (" -2e1px ", -20.), ("0", 0.)] {
        style
            .set_dynamic(StyleId::Height, &StyleValue::Text(text.into()))
            .unwrap();
        assert_eq!(style.height, Dimension::Points(expected));
    }
    for text in [
        "NaNpx", "infpx", "1e39px", "1.px", "12 px", "12", "10em", "0px 1px",
    ] {
        let before = style.clone();
        assert!(
            style
                .set_dynamic(StyleId::Height, &StyleValue::Text(text.into()))
                .is_err(),
            "{text}"
        );
        assert_eq!(style, before);
    }
}

#[test]
fn every_row_writes_its_own_field_and_no_other() {
    let candidates = [
        StyleValue::Number(3.0),
        StyleValue::Number(0.5),
        StyleValue::Percent(50.0),
        StyleValue::Vec2(1.0, 2.0),
        StyleValue::Text("#123456".into()),
        StyleValue::Text("opacity 1s".into()),
        StyleValue::Text("circle(50%)".into()),
        StyleValue::Text("12px".into()),
        StyleValue::Text("path(\"M 0 0 L 10 0 L 10 10 Z\")".into()),
        StyleValue::Text("rotate(10deg)".into()),
        StyleValue::Text("stroke".into()),
    ];
    let mut unwritten = Vec::new();
    let base = StyleProps::default();
    for id in StyleId::ALL {
        let names = id.enum_names();
        let values = names
            .iter()
            .rev()
            .map(|name| StyleValue::Text((*name).into()))
            .chain(candidates.iter().cloned());
        let mut written = None;
        for value in values {
            let mut s = base.clone();
            if s.set_dynamic(id, &value).is_ok() && s.get(id) != base.get(id) {
                written = Some(s);
                break;
            }
        }
        let Some(s) = written else {
            unwritten.push(id);
            continue;
        };
        for other in StyleId::ALL.into_iter().filter(|&other| other != id) {
            assert_eq!(s.get(other), base.get(other), "{id:?} wrote {other:?}");
        }
    }
    // Grid rows have no dynamic form; every other row was written.
    assert_eq!(
        unwritten,
        [
            StyleId::GridTemplateColumns,
            StyleId::GridTemplateRows,
            StyleId::GridColumn,
            StyleId::GridRow
        ]
    );
}

#[test]
fn style_ids_round_trip_through_their_names() {
    for id in StyleId::ALL {
        assert_eq!(StyleId::from_name(id.name()), Some(id));
    }
    for name in ["", "widt", "width ", "Width", "min_widthx"] {
        assert_eq!(StyleId::from_name(name), None, "{name:?}");
    }
}
