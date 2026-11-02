use crate::generated::TextDecorationLine;

#[test]
fn both_lines_are_css_two_keywords_in_either_order() {
    let both = TextDecorationLine::UnderlineLineThrough;
    assert_eq!(
        TextDecorationLine::from_css("underline line-through"),
        Some(both)
    );
    assert_eq!(
        TextDecorationLine::from_css(" line-through  underline "),
        Some(both)
    );
    assert_eq!(
        TextDecorationLine::from_css("underline"),
        Some(TextDecorationLine::Underline)
    );
    assert_eq!(
        TextDecorationLine::from_css("none"),
        Some(TextDecorationLine::None)
    );
    // CSS keywords ignore ASCII case, as the shorthand's already do.
    assert_eq!(
        TextDecorationLine::from_css("Line-Through UNDERLINE"),
        Some(both)
    );
    // The name the web writes as it is: CSS, not a joined token.
    assert_eq!(both.name(), "underline line-through");
    for refused in [
        "underline-line-through",
        "underline underline",
        "overline",
        "",
        "none underline",
    ] {
        assert_eq!(TextDecorationLine::from_css(refused), None, "{refused:?}");
    }
}
