//! `StyleProps::set_dynamic`: the one place an id-addressed producer turns an
//! untyped value into a row, refused typed, nothing changed on refusal.

use exact_kernel::{Color, Dimension, StyleId, StyleProps, StyleValue, StyleValueError};

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
    assert_eq!(s.background_color, Color::rgba(255, 0, 0, 128));
    assert_eq!(s.text_color, Color(0x1122_33ff));
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
                expected: "number",
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
                expected: "number",
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
