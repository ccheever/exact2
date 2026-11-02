use crate::generated::TouchAction;

#[test]
fn pinch_zoom_is_the_css_vocabulary_and_pans_drop_it() {
    let parse = |s| TouchAction::from_name(s).unwrap();
    assert_eq!(
        TouchAction::ALL.len(),
        34,
        "CSS's whole grammar, canonical order"
    );
    assert!(parse("pinch-zoom").pinch_zoom() && parse("auto").pinch_zoom());
    assert!(parse("manipulation").pinch_zoom());
    assert!(!parse("none").pinch_zoom() && !parse("pan-x pan-y").pinch_zoom());
    assert_eq!(parse("pinch-zoom").pans(), TouchAction::None);
    assert_eq!(
        parse("pan-left pan-y pinch-zoom").pans(),
        parse("pan-left pan-y")
    );
    assert_eq!(parse("pan-y").pans(), TouchAction::PanY);
    assert!(
        TouchAction::from_name("pinch-zoom pan-y").is_none(),
        "only CSS's canonical order"
    );
}
