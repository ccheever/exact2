//! `lower-symbol-tint`: `-exact-tint-color` on a `symbol:` image warns, naming `color=` (theme2's 17:23Z ruling,
//! 2026-10-23). A symbol draws in its `color` on every host; the tint colours a raster or a native control's face.

use crate::warnings::warned;

const TINT: &str = "lower-symbol-tint";

fn app(image: &str) -> String {
    format!("component App\n  view\n    column\n      {image}\n")
}

#[test]
fn a_tinted_symbol_warns_with_the_color_to_write() {
    let src = app("image \"symbol:heart\" -exact-tint-color=\"-exact-secondary-label\"");
    assert_eq!(warned(&src), [(TINT.to_string(), 4)]);
    let (_, warnings) = contract::compile_warned(&src);
    let message = &warnings.iter().find(|w| w.id == TINT).unwrap().message;
    assert!(
        message.contains("write `color=\"-exact-secondary-label\"` instead"),
        "{message}"
    );
    // A computed tint is named as `…`; one beside a `color` is said to be dropped.
    let computed = app("image \"symbol:heart\" -exact-tint-color=(true ? \"red\" : \"blue\")");
    let (_, warnings) = contract::compile_warned(&computed);
    assert!(
        warnings
            .iter()
            .any(|w| w.id == TINT && w.message.contains("write `color=…` instead")),
        "{warnings:?}"
    );
    let both = app("image \"symbol:heart\" color=\"red\" -exact-tint-color=\"blue\"");
    let (_, warnings) = contract::compile_warned(&both);
    assert!(
        warnings
            .iter()
            .any(|w| w.id == TINT && w.message.contains("drop it")),
        "{warnings:?}"
    );
}

#[test]
fn a_symbol_by_template_or_choice_warns_and_a_raster_or_an_unknown_source_does_not() {
    for image in [
        "image `symbol:${\"heart\"}` -exact-tint-color=\"-exact-secondary-label\"",
        "image (true ? \"symbol:heart\" : \"symbol:star\") -exact-tint-color=\"-exact-secondary-label\"",
    ] {
        assert_eq!(warned(&app(image)), [(TINT.to_string(), 4)], "{image}");
    }
    // A raster's tint is a template's (LLP 1011 §3); a source that may be either has the benefit of the doubt;
    // `color` alone is the repair, not a warning.
    for image in [
        "image \"https://example.com/a.png\" -exact-tint-color=\"-exact-secondary-label\"",
        "image (true ? \"symbol:heart\" : \"https://example.com/a.png\") -exact-tint-color=\"-exact-secondary-label\"",
        "image \"symbol:heart\" color=\"-exact-secondary-label\"",
    ] {
        assert!(
            !warned(&app(image)).iter().any(|(id, _)| id == TINT),
            "{image}"
        );
    }
}

#[test]
fn a_native_buttons_face_keeps_its_tint_and_a_drawn_buttons_symbol_warns() {
    let button = |appearance: &str| {
        format!("component App\n  action go\n  view\n    column\n      button{appearance} press=go\n        image \"symbol:lock\" -exact-tint-color=\"-exact-secondary-label\"\n        text \"Lock\"\n")
    };
    // The platform's button (its default, or `appearance="auto"`) gives its face its tint (LLP 1115 D4).
    for appearance in ["", " appearance=\"auto\""] {
        assert!(
            !warned(&button(appearance)).iter().any(|(id, _)| id == TINT),
            "{appearance}"
        );
    }
    // A button that draws itself is a box: its symbol is any symbol.
    assert!(warned(&button(" appearance=\"none\"")).contains(&(TINT.to_string(), 6)));
}
