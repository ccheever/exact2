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

#[test]
fn a_drawn_default_button_a_class_tint_and_a_components_second_use_are_warned() {
    // A default button lowering draws itself (a `background-color` makes it the bare box): its symbol is warned.
    let drawn = "component App\n  action go\n  view\n    column\n      button press=go background-color=\"-exact-secondary-label\"\n        image \"symbol:lock\" -exact-tint-color=\"-exact-secondary-label\"\n        text \"Lock\"\n";
    assert!(
        warned(drawn).contains(&(TINT.to_string(), 6)),
        "{:?}",
        warned(drawn)
    );
    // A class's tint is the image's.
    let class = "style Muted\n  -exact-tint-color=\"-exact-secondary-label\"\n\ncomponent App\n  view\n    column\n      image \"symbol:heart\" class=Muted\n";
    assert!(
        warned(class).iter().any(|(id, _)| id == TINT),
        "{:?}",
        warned(class)
    );
    // A component's image, a raster in its first use and a symbol in its second: the second is warned, at the
    // component's line, once.
    let uses = |first: &str, second: &str| {
        format!("component App\n  view\n    column\n      Icon(src=\"{first}\")\n      Icon(src=\"{second}\")\ncomponent Icon\n  props\n    src: string\n  view\n    image src -exact-tint-color=\"-exact-secondary-label\"\n")
    };
    for (first, second) in [
        ("https://example.com/a.png", "symbol:heart"),
        ("symbol:heart", "https://example.com/a.png"),
        ("symbol:heart", "symbol:star"),
    ] {
        let tints: Vec<_> = warned(&uses(first, second))
            .into_iter()
            .filter(|(id, _)| id == TINT)
            .collect();
        assert_eq!(tints, [(TINT.to_string(), 10)], "{first} then {second}");
    }
}

#[test]
fn a_tint_from_a_components_prop_is_shown_as_written_not_as_one_uses_value() {
    let src = "component App\n  view\n    column\n      Icon(tint=\"-exact-secondary-label\")\n      Icon(tint=\"-exact-tertiary-label\")\ncomponent Icon\n  props\n    tint: string\n  view\n    image \"symbol:heart\" -exact-tint-color=tint\n";
    let (_, warnings) = contract::compile_warned(src);
    let tints: Vec<_> = warnings.iter().filter(|w| w.id == TINT).collect();
    assert_eq!(tints.len(), 1, "{warnings:?}");
    assert!(
        tints[0].message.contains("write `color=…` instead"),
        "{}",
        tints[0].message
    );
}

#[test]
fn a_native_face_under_an_if_or_with_a_button_style_keeps_its_tint() {
    for (attrs, image) in [
        (" buttonStyle=\"filled\"", "        image \"symbol:lock\" -exact-tint-color=\"-exact-secondary-label\"\n"),
        ("", "        if on\n          image \"symbol:lock\" -exact-tint-color=\"-exact-secondary-label\"\n"),
    ] {
        let src = format!("component App\n  state on = true\n  action go\n  view\n    column\n      button{attrs} press=go\n{image}        text \"Lock\"\n");
        assert!(!warned(&src).iter().any(|(id, _)| id == TINT), "{src}");
    }
}

#[test]
fn a_drawn_button_in_a_component_used_twice_with_other_rows_still_warns() {
    // Its two uses' widths differ, so lowering's record of the shared button is merged away: the literal
    // `appearance="none"` still says it draws itself.
    let src = "component App\n  action go\n  view\n    column\n      Lock(w=40, press=go)\n      Lock(w=80, press=go)\ncomponent Lock\n  props\n    w: number\n    press: action\n  view\n    button appearance=\"none\" width=w press=press\n      image \"symbol:heart\" -exact-tint-color=\"-exact-secondary-label\"\n";
    let tints: Vec<_> = warned(src)
        .into_iter()
        .filter(|(id, _)| id == TINT)
        .collect();
    assert_eq!(tints, [(TINT.to_string(), 13)]);
}

#[test]
fn a_class_colour_a_state_tint_and_a_row_a_face_shares_get_their_own_repairs() {
    let message = |src: &str| {
        let (_, warnings) = contract::compile_warned(src);
        warnings
            .into_iter()
            .find(|w| w.id == TINT)
            .map(|w| w.message)
    };
    // The class's `color` draws it: the tint is dropped, not turned into a second `color`.
    let class = "style Icon\n  color=\"-exact-secondary-label\"\n  -exact-tint-color=\"-exact-tertiary-label\"\n\ncomponent App\n  view\n    column\n      image \"symbol:heart\" class=Icon\n";
    assert!(
        message(class).is_some_and(|m| m.contains("drop it")),
        "{:?}",
        message(class)
    );
    // A state's tint is that state's `color`.
    let state = "component App\n  view\n    column\n      image \"symbol:heart\" color=\"-exact-label\"\n        :disabled\n          -exact-tint-color=\"-exact-tertiary-label\"\n";
    assert!(
        message(state).is_some_and(|m| m.contains("write `:disabled color=")),
        "{:?}",
        message(state)
    );
    // One with that state's `color` already: dropped, not a second `:disabled color`.
    let both = state.replace(
        "        :disabled\n",
        "        :disabled\n          color=\"-exact-secondary-label\"\n",
    );
    assert!(
        message(&both).is_some_and(|m| m.contains("drop it")),
        "{:?}",
        message(&both)
    );
    // A class a native face also takes: the free use gets its own colour, the shared row is kept.
    let shared = "style Tint\n  -exact-tint-color=\"-exact-secondary-label\"\n\ncomponent App\n  action go\n  view\n    column\n      button press=go\n        image \"symbol:lock\" class=Tint\n        text \"Lock\"\n      image \"symbol:heart\" class=Tint\n";
    assert!(
        message(shared)
            .is_some_and(|m| m.contains("also colours a native button's face")
                && !m.contains("write `color=")),
        "{:?}",
        message(shared)
    );
}
