//! LLP 1075.003 §9.11: `navigationDetent` takes UIKit's named detents, CSS's
//! `fit-content` and positive point heights, space-separated; a literal with
//! any other word is refused, and `fit-content` goes alone or before `large`.

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
    ] {
        compile(value).unwrap_or_else(|e| panic!("{value:?}: {e}"));
    }
    for (value, word) in [
        ("content", "content"),
        ("fit-content huge", "huge"),
        ("0", "0"),
        ("-40", "-40"),
        ("300px", "300px"),
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
