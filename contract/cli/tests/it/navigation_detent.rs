//! LLP 1075.003 §9.11, §9.13: `navigationDetent` takes UIKit's named
//! detents, CSS's `fit-content`, positive point heights and percentages of
//! the sheet's maximum, space-separated; a literal with any other word is
//! refused, and `fit-content` goes alone or before `large`.

#[test]
fn the_detent_words_lower_and_others_are_refused() {
    let compile = |value: &str| {
        contract::compile(&format!(
            "component App\n  view\n    column navigationKey=\"a\" navigationPresentation=\"modal\" navigationDetent=\"{value}\"\n"
        ))
    };
    for value in [
        "",
        "large",
        "medium large",
        "300 large",
        "fit-content",
        "fit-content large",
        "128.5",
        "45%",
        "45% large",
        "33.3% 66.6% large",
        "100%",
    ] {
        compile(value).unwrap_or_else(|e| panic!("{value:?}: {e}"));
    }
    for (value, word) in [
        ("content", "content"),
        ("fit-content huge", "huge"),
        ("0", "0"),
        ("-40", "-40"),
        ("300px", "300px"),
        ("NaN", "NaN"),
        ("inf", "inf"),
        ("0%", "0%"),
        ("-45%", "-45%"),
        ("150%", "150%"),
        ("%", "%"),
        ("NaN%", "NaN%"),
        ("45 %", "%"),
        ("fit-content 300", "fit-content 300"),
        ("300 fit-content", "300 fit-content"),
        ("fit-content medium", "fit-content medium"),
        ("large fit-content", "large fit-content"),
    ] {
        let error = compile(value).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value") && error.contains(&format!("given \"{word}\"")),
            "{value}: {error}"
        );
    }
}

/// A sheet's grabber is the author's to say, `visible` or `hidden`, else
/// the platform's (`auto`); any other word is refused.
#[test]
fn the_grabber_words_lower_and_others_are_refused() {
    let compile = |value: &str| {
        contract::compile(&format!(
            "component App\n  view\n    column navigationKey=\"a\" navigationPresentation=\"modal\" navigationGrabber=\"{value}\"\n"
        ))
    };
    for value in ["visible", "hidden", "auto"] {
        compile(value).unwrap_or_else(|e| panic!("{value:?}: {e}"));
    }
    for value in ["", "true", "shown"] {
        let error = compile(value).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value") && error.contains("`navigationGrabber` takes"),
            "{value}: {error}"
        );
    }
}
