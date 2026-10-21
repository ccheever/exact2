//! LLP 1075.003 §9.11, §9.13: `navigationDetent` takes UIKit's named
//! detents, CSS's `fit-content`, positive point heights and percentages of
//! the sheet's maximum, space-separated; a literal with any other word is
//! refused, `fit-content` goes alone or before `large`, and the others
//! ascend on every screen.

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
        "300 500 large",
        "45% medium large",
        "45% 100%",
    ] {
        compile(value).unwrap_or_else(|e| panic!("{value:?}: {e}"));
    }
    // UIKit takes detents shortest to tallest. A point height and a share of
    // the maximum cross on some screen (300 is above 45% of 600, below 45%
    // of 800), so they do not mix; one kind ascends, and `large` goes last.
    for (value, word) in [
        ("300 45% large", "45%"),
        ("medium 300", "300"),
        ("500 300", "300"),
        ("60% medium", "medium"),
        ("50% medium", "medium"),
        ("large medium", "medium"),
        ("large 300", "300"),
        // `100%` is `large`: the same height, so one of them.
        ("100% large", "large"),
        ("45% 100% large", "large"),
    ] {
        let error = compile(value).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value")
                && error.contains(&format!("given \"{word}\" in \"{value}\""))
                && error.contains("shortest to tallest on every screen"),
            "{value}: {error}"
        );
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

/// A stack whose routes are sheets: `route` on each route, `inner` on a box
/// inside it.
fn sheet(route: &str, inner: &str) -> String {
    format!(
        "routes nav\n  home \"/\"\n    menu \"/menu\"\ncomponent App\n  state detents: string = \"medium large\"\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"\n      for e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` navigationPresentation=\"modal\" {route}\n          column {inner}\n            text e.name\n"
    )
}

/// A sheet opens at the detent `navigationInitialDetent` names, one of the
/// words of its route's `navigationDetent` (`large` when it says none); a
/// literal naming another height is refused, a computed list is not
/// checked, and off a route, where no host reads it, it is refused.
#[test]
fn the_initial_detent_is_one_of_the_sheets_heights() {
    let compile = |route: &str| contract::compile(&sheet(route, ""));
    for route in [
        "navigationDetent=\"medium large\" navigationInitialDetent=\"large\"",
        "navigationDetent=\"medium large\" navigationInitialDetent=\"medium\"",
        "navigationDetent=\"300 500 large\" navigationInitialDetent=\"500\"",
        "navigationInitialDetent=\"large\"",
        "navigationDetent=\"\" navigationInitialDetent=\"large\"",
        "navigationDetent=(detents) navigationInitialDetent=\"medium\"",
        "navigationDetent=\"medium large\" navigationInitialDetent=\"\"",
    ] {
        compile(route).unwrap_or_else(|e| panic!("{route:?}: {e}"));
    }
    for (route, says) in [
        (
            "navigationDetent=\"medium large\" navigationInitialDetent=\"45%\"",
            "`navigationDetent=\"medium large\"`",
        ),
        ("navigationInitialDetent=\"medium\"", "`large` alone"),
    ] {
        let error = compile(route).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value")
                && error.contains("is not one of this sheet's heights")
                && error.contains(says),
            "{route}: {error}"
        );
    }
    for value in ["tall", "medium large", "0"] {
        let error = compile(&format!(
            "navigationDetent=\"medium large\" navigationInitialDetent=\"{value}\""
        ))
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("`navigationInitialDetent` takes one word"),
            "{value:?}: {error}"
        );
    }
    // Off a route: a box in one, and a keyed element no navigation root holds.
    for src in [
        sheet("navigationDetent=\"medium large\"", "navigationInitialDetent=\"large\""),
        "component App\n  view\n    column\n      column navigationKey=\"sheet\" navigationPresentation=\"modal\" navigationDetent=\"medium large\" navigationInitialDetent=\"large\"\n        text \"Sheet\"\n".into(),
    ] {
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(
            error.contains("`navigationInitialDetent` is read only on a route"),
            "{error}"
        );
    }
}
