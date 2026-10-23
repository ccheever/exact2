use super::*;
#[test]
fn clamp_does_not_replace_layout_visibility_or_scrolling() {
    for (row, value) in [
        (StyleId::Display, "flex"),
        (StyleId::Display, "grid"),
        (StyleId::Display, "none"),
        (StyleId::OverflowX, "scroll"),
        (StyleId::OverflowY, "scroll"),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(row, &exact_kernel::StyleValue::Text(value.into()))
            .unwrap();
        let (before, _) = css_text(&style, &[]);
        style
            .set_dynamic(StyleId::LineClamp, &exact_kernel::StyleValue::Number(2.0))
            .unwrap();
        let (after, skipped) = css_text(&style, &[]);
        assert_eq!(before, after, "{row:?}: {value}");
        assert_eq!(
            skipped.iter().map(|s| s.row).collect::<Vec<_>>(),
            [StyleId::LineClamp]
        );
        style
            .set_dynamic(StyleId::LineClamp, &exact_kernel::StyleValue::Number(0.0))
            .unwrap();
        assert!(css_text(&style, &[]).1.is_empty());
    }
}

#[test]
fn exclusion_rows_keep_authored_css_visible() {
    let mut s = StyleProps::default();
    for (id, value) in [
        (StyleId::WrapFlow, "both"),
        (StyleId::ShapeOutside, "circle()"),
        (StyleId::ShapeMargin, "8px"),
    ] {
        s.set_dynamic(id, &exact_kernel::StyleValue::Text(value.into()))
            .unwrap();
    }
    let (css, skipped) = css_text(&s, &[]);
    assert!(css.contains("shape-outside:circle(closest-side at 50% 50%);"));
    assert!(css.contains("shape-margin:8px;"));
    assert!(css.contains("wrap-flow:both;"));
    assert!(skipped.is_empty());
}

/// LLP 1034 §8: the browser resolves `light-dark()` by the inherited
/// `color-scheme`; an unset (`normal`) row follows the parent's.
#[test]
fn color_scheme_is_passed_through() {
    for (value, want) in [
        ("dark", "color-scheme:dark;"),
        ("light", "color-scheme:light;"),
        ("normal", "color-scheme:inherit;"),
    ] {
        let mut s = StyleProps::default();
        s.set_dynamic(
            StyleId::ColorScheme,
            &exact_kernel::StyleValue::Text(value.into()),
        )
        .unwrap();
        let (css, skipped) = css_text(&s, &[]);
        assert!(css.contains(want), "{value}: {css}");
        assert!(skipped.is_empty());
    }
}
