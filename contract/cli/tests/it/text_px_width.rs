use super::warnings::warned;

#[test]
fn role_text_in_a_px_width_is_named_with_its_repair() {
    let src = "component App\n  view\n    row\n      text \"Mon\" font-size=\"-exact-body\" width=48\n      column width=\"42px\"\n        text \"Low\" font-size=\"-exact-caption1\"\n        text \"High\" font-size=\"-exact-caption1\"\n";
    assert_eq!(
        warned(src),
        [
            ("lower-text-px-width".into(), 4),
            ("lower-text-px-width".into(), 5)
        ]
    );
    let (_, warnings) = contract::compile_warned(src);
    assert!(
        warnings
            .iter()
            .all(|w| w.message.contains("accessibility") && w.message.contains("rem")),
        "{warnings:?}"
    );
    let repaired = src
        .replace("width=48", "width=\"3rem\"")
        .replace("width=\"42px\"", "width=\"2.625rem\"");
    assert_eq!(warned(&repaired), []);
}

#[test]
fn text_px_width_follows_known_fonts_styles_and_branches() {
    for view in [
        "text \"Mon\" font-size=\"1rem\" width=48",
        "text \"Mon\" font=\"-exact-body\" width=\" 48px \"",
        "text \"Mon\" font=\"-exact-body\" width=\"48PX\"",
        "button width=48\n  text \"Mon\" font=\"-exact-body\"",
        "text \"Mon\" font-size=16 font=\"-apple-system-body\" width=48",
        "column font-size=\"-exact-body\"\n  text \"Mon\" width=48",
        "column font-size=\"-exact-body\"\n  text \"Mon\" font-size=\"1.2em\" width=48",
        "text \"Monday\" role=\"heading\" width=48",
        "column width=42\n  if true\n    text \"Low\" font-size=\"-exact-body\"",
        "column width=42\n  for day in [\"Mon\"] key=day\n    text day font-size=\"-exact-body\"",
        "column width=42\n  match some(\"Mon\")\n    case some(day)\n      text day font-size=\"-exact-body\"\n    case none\n      text \"None\"",
    ] {
        let src = format!("component App\n  view\n    column\n{}", view.lines().map(|l| format!("      {l}\n")).collect::<String>());
        let got = warned(&src);
        assert_eq!(got.len(), 1, "{src}\n{got:?}");
        assert_eq!(got[0].0, "lower-text-px-width", "{src}");
    }
    let styled = "style Day\n  width=48 font-size=\"-exact-body\"\ncomponent App\n  view\n    column\n      text \"Mon\" class=Day\n      text \"Tue\" class=Day width=\"3rem\"\n      text \"Wed\" class=Day font-size=16\n";
    assert_eq!(warned(styled), [("lower-text-px-width".into(), 6)]);
    let reused = "component Day\n  view\n    text \"Mon\" font-size=\"-exact-body\" width=48\ncomponent App\n  view\n    row\n      Day()\n      Day()\n";
    assert_eq!(warned(reused), [("lower-text-px-width".into(), 3)]);
}

#[test]
fn text_px_width_leaves_fixed_fonts_and_unknown_values_alone() {
    for view in [
        "text \"Mon\" width=48 font-size=16",
        "column font-size=\"-exact-body\"\n  text \"Mon\" width=48 font-size=\"16px\"",
        "text \"Mon\" role=\"heading\" width=48 font-size=16",
        "svg width=48 height=48\n  text \"Mon\" font-size=\"-exact-body\"",
        "text \"Mon\" width=48 font=\"-exact-body\" font-size=16",
        "column font-size=16\n  text \"Mon\" width=48 font-size=\"1.2em\"",
        "text \"Mon\" width=48 font-size=(true ? \"-exact-body\" : \"16px\")",
        "text \"Mon\" width=(true ? 48 : 60) font-size=\"-exact-body\"",
        "text \"Mon\" width=\"3rem\" font-size=\"-exact-body\"",
        "text \"Mon\" width=\"50%\" font-size=\"-exact-body\"",
        "text \"Mon\" width=0 font-size=\"-exact-body\"",
        "column width=48 display=\"none\"\n  text \"Mon\" font-size=\"-exact-body\"",
        "column width=48\n  image \"symbol:heart\" font-size=\"-exact-body\"",
    ] {
        let src = format!(
            "component App\n  view\n    column\n{}",
            view.lines()
                .map(|l| format!("      {l}\n"))
                .collect::<String>()
        );
        assert_eq!(warned(&src), [], "{src}");
    }
}

fn cases(views: &[(&str, bool)]) {
    let mut failures = Vec::new();
    for (view, expected) in views {
        let src = format!("style On\n  font-size=16\nstyle Off\n  font-size=20\ncomponent App\n  state on = true\n  view\n    column\n{}", view.lines().map(|l| format!("      {l}\n")).collect::<String>());
        let (result, warnings) = contract::compile_warned(&src);
        let got = warnings.iter().any(|w| w.id == "lower-text-px-width");
        if result.is_err() || got != *expected {
            failures.push(format!("{view}\nexpected warning: {expected}, got {got}; errors: {:?}; warnings: {warnings:?}", result.err()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn text_width_defaults_to_the_root_font() {
    cases(&[
        ("text \"Mon\" width=48", true),
        ("text \"Mon\" width=48 font-size=\"1em\"", true),
        ("column font-size=16\n  text \"Mon\" width=48", false),
        (
            "column font-size=\"16px\"\n  text \"Mon\" width=48 font-size=\"1em\"",
            false,
        ),
        ("text \"Mon\" width=48 font-size=(on ? 16 : 20)", false),
    ]);
}

#[test]
fn text_width_uses_visible_paragraph_runs() {
    cases(&[
        (
            "text width=48 font-size=\"-exact-body\"\n  text \"Mon\" font-size=16",
            false,
        ),
        (
            "text width=48 font-size=16\n  text \"Mon\" font-size=\"-exact-body\"",
            true,
        ),
        (
            "text width=48\n  text \"Mon\" font-size=16\n  text \"Tue\" font-size=20",
            false,
        ),
        (
            "text width=48\n  text \"Mon\" font-size=16\n  text \"Tue\"",
            true,
        ),
    ]);
}

#[test]
fn text_width_counts_text_fields() {
    cases(&[
        ("input value=\"Monday\" width=48 font=\"-exact-body\"", true),
        ("input value=\"Monday\" width=48", true),
        ("textarea value=\"Monday\" width=48", true),
        ("input value=\"Monday\" width=48 font-size=16", false),
        ("textarea value=\"Monday\" width=48 font-size=16", false),
        ("input type=\"checkbox\" checked=true width=48", false),
    ]);
}

#[test]
fn text_width_restarts_inheritance_at_native_controls() {
    cases(&[
        (
            "column font-size=16\n  button width=96\n    text \"Low\" font-size=\"1em\"",
            true,
        ),
        (
            "column font-size=16\n  button width=96 font-size=\"1em\"\n    text \"Low\"",
            true,
        ),
        (
            "column font-size=16\n  input value=\"Low\" width=96 font-size=\"1em\"",
            true,
        ),
        (
            "column font-size=16\n  textarea value=\"Low\" width=96",
            true,
        ),
        (
            "button width=96 font-size=16\n  text \"Low\" font-size=\"1em\"",
            false,
        ),
        (
            "button width=96\n  column font-size=16\n    text \"Low\" font-size=\"1em\"",
            false,
        ),
        (
            "column font-size=16\n  button width=96 appearance=\"none\"\n    text \"Low\"",
            false,
        ),
        (
            "column font-size=16\n  input value=\"Low\" width=96 appearance=\"none\"",
            false,
        ),
    ]);
}

#[test]
fn text_width_stays_silent_for_an_unresolved_class() {
    cases(&[
        (
            "column class=(on ? On : Off) width=48\n  text \"Mon\" font-size=\"-exact-body\"",
            false,
        ),
        (
            "text \"Mon\" class=(on ? On : Off) width=48 font-size=\"-exact-body\"",
            false,
        ),
        (
            "column class=(on ? On : Off) width=48 font-size=\"-exact-body\"\n  text \"Mon\"",
            false,
        ),
        (
            "column class=(on ? On : Off) width=48\n  text \"Mon\" font-size=16",
            false,
        ),
        (
            "text \"Mon\" class=(on ? On : Off) width=48 font-size=16",
            false,
        ),
        ("text \"Mon\" class=(on ? On : Off) width=48", false),
    ]);
}

#[test]
fn text_width_inherits_cleared_font_rows() {
    for word in ["unset", "inherit"] {
        cases(&[
            (&format!("column font-size=\"-exact-body\"\n  text \"Mon\" font-size=\"{word}\" width=48"), true),
            (&format!("column font-size=16\n  text \"Mon\" font-size=\"{word}\" width=48"), false),
            (&format!("column font-size=16\n  input value=\"Mon\" font-size=\"{word}\" width=48"), false),
            (&format!("column font-size=16\n  textarea value=\"Mon\" font-size=\"{word}\" width=48"), false),
            (&format!("column font-size=16\n  button width=96 font-size=\"{word}\"\n    text \"Mon\""), false),
            (&format!("column font-size=16\n  button font-size=\"{word}\"\n    text \"Mon\" font-size=\"1em\" width=48"), false),
            (&format!("column font-size=\"1rem\"\n  input value=\"Mon\" font-size=\"{word}\" width=48"), true),
            (&format!("column font-size=\"1rem\"\n  button width=96 font-size=\"{word}\"\n    text \"Mon\""), true),
        ]);
    }
}

#[test]
fn text_width_recognizes_absolute_lengths_and_math() {
    for width in [
        "36pt",
        "0.5in",
        "1.27cm",
        "12.7mm",
        "3pc",
        "calc(48px)",
        "calc(24px + 18pt)",
        "min(48px, 60px)",
    ] {
        cases(&[
            (&format!("text \"Mon\" width=\"{width}\""), true),
            (
                &format!("text \"Mon\" width=\"{width}\" font-size=16"),
                false,
            ),
        ]);
    }
    cases(&[
        ("text \"Mon\" width=\"3rem\"", false),
        ("text \"Mon\" width=\"3em\"", false),
        ("text \"Mon\" width=\"calc(50% + 24px)\"", false),
        ("text \"Mon\" width=\"calc(10vw + 24px)\"", false),
    ]);
}

#[test]
fn text_width_uses_lowered_control_appearance() {
    cases(&[
        ("column font-size=16\n  button \"Monday\" width=48", true),
        ("column font-size=16\n  button \"Monday\" width=48 font-size=16", false),
        ("column font-size=16\n  button \"Monday\" width=48 font-size=\"inherit\"", false),
        ("column font-size=16\n  input value=\"Mon\" width=48 background-color=\"transparent\"", false),
        ("column font-size=16\n  textarea value=\"Mon\" width=48 border-radius=4", false),
        ("column font-size=16\n  button width=48 background-color=\"transparent\"\n    text \"Mon\"", false),
        ("column font-size=16\n  button width=48\n    column\n      text \"Mon\"", false),
        ("column font-size=16\n  button width=48\n    text \"Mon\" width=48", false),
        ("column font-size=16\n  canvas width=100 height=100\n    button width=48\n      text \"Mon\"", false),
        ("column font-size=16\n  input value=\"Mon\" width=48", true),
        ("column font-size=16\n  button width=48\n    text \"Mon\"", true),
        ("column font-size=16\n  input value=\"Mon\" width=48 background-color=\"transparent\" font-size=\"1rem\"", true),
        ("column font-size=16\n  button width=48 background-color=\"transparent\"\n    text \"Mon\" font-size=\"1rem\"", true),
        ("button class=(on ? On : Off) width=48\n  text \"Mon\" font-size=\"1em\"", false),
        ("button class=(on ? On : Off) width=48\n  text \"Mon\" font-size=\"1rem\"", false),
    ]);
}

#[test]
fn text_width_checks_paragraph_boxes_not_inline_widths() {
    cases(&[
        (
            "text\n  text \"Mon\" width=48 font-size=\"-exact-body\"",
            false,
        ),
        ("text\n  if on\n    text \"Mon\" width=48", false),
        (
            "text\n  for day in [\"Mon\"] key=day\n    text day width=48",
            false,
        ),
        (
            "text width=48\n  text \"Mon\" font-size=\"-exact-body\"",
            true,
        ),
        ("text width=48\n  text \"Mon\" font-size=16", false),
        ("text width=48\n  if on\n    text \"Mon\"", true),
    ]);
}

#[test]
fn text_width_uses_grouped_list_typography() {
    cases(&[
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      column width=48\n        text \"Monday\"", true),
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      row\n        text \"Monday\" width=120", false),
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      row\n        text \"Monday\" width=120 flex-grow=0", true),
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      column width=48 font-size=16\n        text \"Monday\"", false),
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      column width=48 class=On\n        text \"Monday\"", false),
        ("column font-size=16\n  list appearance=\"auto\"\n    section\n      row\n        text \"Monday\" width=120 font-size=16", false),
        ("column font-size=16\n  list appearance=\"none\"\n    section\n      column width=48\n        text \"Monday\"", false),
    ]);
}

#[test]
fn text_width_walks_html_inside_svg_foreign_objects() {
    cases(&[
        ("svg\n  foreignObject width=200 height=80\n    text \"Monday\" font-size=\"-exact-body\" width=48", true),
        ("svg\n  g\n    foreignObject width=200 height=80\n      text \"Monday\" width=48", true),
        ("svg\n  foreignObject width=200 height=80\n    text \"Monday\" font-size=16 width=48", false),
        ("svg\n  foreignObject width=200 height=80\n    text \"Monday\" font-size=\"-exact-body\" width=\"3rem\"", false),
        ("svg width=48\n  text \"Monday\" font-size=\"-exact-body\"", false),
        ("svg\n  foreignObject width=200 height=80\n    svg width=48\n      text \"Monday\" font-size=\"-exact-body\"", false),
        ("svg\n  defs\n    symbol id=\"label\"\n      foreignObject width=200 height=80\n        text \"Monday\" width=48", false),
        ("column font-size=16\n  svg\n    foreignObject width=200 height=80\n      input value=\"Monday\" width=48", false),
        ("column font-size=16\n  svg\n    foreignObject width=200 height=80\n      button width=48\n        text \"Monday\"", false),
        ("column font-size=16\n  svg\n    foreignObject width=200 height=80\n      input value=\"Monday\" font-size=\"1rem\" width=48", true),
    ]);
}

#[test]
fn text_width_stops_at_separate_presentations() {
    let mut failures = Vec::new();
    for presentation in ["box id=\"p\" popover=\"auto\"", "dialog id=\"p\""] {
        for (width, content, expected) in [
            ("20rem", "text \"Mon\"", vec![]),
            ("20rem", "text \"Mon\" width=48", vec![5]),
            ("48px", "text \"Mon\"", vec![4]),
        ] {
            let src = format!("component App\n  view\n    box width=48\n      {presentation} width=\"{width}\"\n        {content}\n");
            let (compiled, warnings) = contract::compile_warned(&src);
            let lines: Vec<_> = warnings
                .iter()
                .filter(|w| w.id == "lower-text-px-width")
                .map(|w| w.span.line)
                .collect();
            if compiled.is_err() || lines != expected {
                failures.push(format!(
                    "{src}\nexpected {expected:?}, got {lines:?}; errors: {:?}",
                    compiled.err()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    cases(&[("box width=48\n  text \"In the box\"\n  box id=\"p\" popover=\"auto\" width=\"20rem\"\n    text \"In the popover\"", true)]);
}

#[test]
fn text_width_respects_growing_minimum_widths() {
    cases(&[
        (
            "text \"Mon\" width=48 min-width=\"3rem\" font-size=\"1rem\"",
            false,
        ),
        (
            "text \"Mon\" width=48 min-width=\"3em\" font-size=\"1rem\"",
            false,
        ),
        (
            "text \"Mon\" width=48 min-width=\"3rem\" max-width=48",
            false,
        ),
        (
            "text \"Mon\" width=48 min-width=\"calc(24px + 24px)\"",
            true,
        ),
        (
            "text \"Mon\" width=48 min-width=(on ? \"3rem\" : \"4rem\")",
            false,
        ),
        ("text \"Mon\" width=48 min-width=48", true),
        ("text \"Mon\" width=48 min-width=\"0rem\"", true),
        ("text \"Mon\" width=48 min-width=\"0em\"", true),
        (
            "box width=48 min-width=\"3em\" font-size=16\n  text \"Mon\" font-size=\"1rem\"",
            true,
        ),
    ]);
    let styled = "style Growing\n  min-width=\"3rem\"\ncomponent App\n  view\n    column\n      text \"Mon\" class=Growing width=48\n      text \"Tue\" class=Growing width=48 min-width=0\n";
    assert_eq!(warned(styled), [("lower-text-px-width".into(), 7)]);
}

#[test]
fn text_width_zero_font_multipliers_never_scale() {
    for size in ["0rem", "0em"] {
        cases(&[
            (
                &format!("text \"Mon\" width=48 font-size=\"{size}\""),
                false,
            ),
            (
                &format!("column font-size=\"{size}\"\n  text \"Mon\" width=48"),
                false,
            ),
            (
                &format!("column font-size=\"{size}\"\n  text \"Mon\" width=48 font-size=\"2em\""),
                false,
            ),
            (
                &format!("column font-size=\"{size}\"\n  text \"Mon\" width=48 font-size=\"1rem\""),
                true,
            ),
        ]);
    }
}

#[test]
fn text_width_stops_at_positioned_subtrees() {
    cases(&[
        ("box position=\"relative\" width=\"20rem\"\n  box width=48\n    text \"Mon\" position=\"absolute\" left=0 right=0 font-size=\"1rem\"", false),
        ("box width=48\n  box position=\"absolute\" width=\"20rem\"\n    text \"Mon\"", false),
        ("text \"Mon\" position=\"absolute\" width=48", true),
    ]);
    let src = "component App\n  view\n    box width=48\n      box position=\"absolute\" width=\"20rem\"\n        text \"Mon\" width=48\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 5)]);
    let (result, warnings) = contract::compile_warned(
        "component App\n  view\n    box width=48\n      text \"Mon\" position=\"fixed\"\n",
    );
    assert_eq!(result.unwrap_err().id, "lower-attr-value");
    assert!(!warnings.iter().any(|w| w.id == "lower-text-px-width"));

    cases(&[
        (
            "box width=48\n  text \"Mon\" position=(on ? \"absolute\" : \"relative\")",
            false,
        ),
        ("box width=48\n  text \"Mon\" position=\"relative\"", true),
    ]);
}

#[test]
fn text_width_skips_effectively_hidden_classes() {
    let src = "style A\n  display=\"none\"\nstyle B\n  display=\"none\"\ncomponent App\n  state on = true\n  view\n    box width=48\n      box class=(on ? A : B) width=48\n        text \"Mon\" font-size=\"1rem\" width=48\n";
    assert_eq!(warned(src), []);
    assert_eq!(warned(&src.replace("class=(on ? A : B)", "class=A")), []);
    assert_eq!(
        warned(&src.replace("display=\"none\"", "display=\"block\"")),
        [("lower-text-px-width".into(), 10)]
    );
}

#[test]
fn text_width_requires_a_known_width_constraint() {
    cases(&[
        ("row width=\"20rem\"\n  text \"Mon\" width=48 flex-basis=\"3rem\" flex-grow=0 flex-shrink=0 min-width=0 font-size=\"1rem\"", false),
        ("row\n  text \"Mon\" width=48 flex-grow=1", false),
        ("row\n  text \"Mon\" width=48 flex-grow=(on ? 0 : 1)", false),
        ("row\n  text \"Mon\" width=48 flex=\"1 0 auto\"", false),
        ("text \"Mon\" width=48 min-width=96", true),
        ("text \"Mon\" width=48 min-width=\"50%\"", false),
        ("box width=48 min-width=\"4em\" font-size=16\n  text \"Mon\" font-size=\"1rem\"", true),
        ("text \"Mon\" width=48 max-width=24", true),
        ("text \"Mon\" width=48 max-width=\"3rem\"", true),
        ("text \"Mon\" width=48 display=(on ? \"block\" : \"none\")", false),
        ("row\n  text \"Mon\" width=48 flex-grow=0 flex-shrink=0 min-width=0", true),
        ("text \"Mon\" width=48 min-width=24 max-width=96", true),
    ]);
    let src = "style Growing\n  flex-basis=\"3rem\"\ncomponent App\n  view\n    row width=\"20rem\"\n      text \"Mon\" width=48 class=Growing\n";
    assert_eq!(warned(src), []);
}

#[test]
fn text_width_proves_a_fixed_ceiling_after_bounds() {
    cases(&[
        ("text \"Mon\" max-width=24", true),
        ("text \"Mon\" width=\"auto\" max-width=24", true),
        ("text \"Mon\" width=\"3rem\" max-width=24", true),
        ("text \"Mon\" width=(on ? 48 : 96) max-width=24", true),
        ("text \"Mon\" max-width=\"3rem\"", false),
        ("text \"Mon\" min-width=96", false),
        (
            "box min-width=\"4em\" font-size=16\n  text \"Mon\" font-size=\"1rem\"",
            false,
        ),
        ("text \"Mon\" min-width=96 max-width=24", true),
        ("text \"Mon\" width=48 min-width=96 max-width=24", true),
        (
            "text \"Mon\" width=48 min-width=96 max-width=\"3rem\"",
            true,
        ),
        ("text \"Mon\" max-width=24 min-width=\"3rem\"", false),
        ("text \"Mon\" max-width=24 min-width=\"50%\"", false),
        ("text \"Mon\" max-width=24 min-width=(on ? 0 : 96)", false),
        (
            "text \"Mon\" width=48 max-width=(on ? \"3rem\" : \"6rem\")",
            true,
        ),
        (
            "box max-width=\"4em\" font-size=16\n  text \"Mon\" font-size=\"1rem\"",
            true,
        ),
        (
            "box max-width=\"4em\" font-size=\"1rem\"\n  text \"Mon\"",
            false,
        ),
    ]);
    let src = "style Cap\n  max-width=24\ncomponent App\n  view\n    column\n      text \"Mon\" class=Cap\n      text \"Tue\" class=Cap max-width=\"3rem\"\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 6)]);
}

#[test]
fn text_width_uses_the_parents_effective_flex_axis() {
    cases(&[
        ("row\n  text \"Mon\" width=48 flex=\"none\"", true),
        ("row\n  text \"Mon\" width=48 flex=\"0 0 auto\"", true),
        ("row\n  text \"Mon\" width=48 flex=1", false),
        ("row\n  text \"Mon\" max-width=24 flex=1 min-width=0", true),
        ("column\n  text \"Mon\" width=48 flex=1", true),
        ("column\n  text \"Mon\" width=48 flex=(on ? 1 : 2)", true),
        ("column\n  text \"Mon\" width=48 flex-basis=\"3rem\" flex-grow=1", true),
        ("box\n  text \"Mon\" width=48 flex=1", true),
        ("box display=\"grid\"\n  text \"Mon\" width=48 flex=1", true),
        ("row flex-direction=\"column\"\n  text \"Mon\" width=48 flex=1", true),
        ("column flex-direction=\"row-reverse\"\n  text \"Mon\" width=48 flex=1", false),
        ("column flex-direction=\"unset\"\n  text \"Mon\" width=48 flex=1", false),
        ("row display=\"block\"\n  text \"Mon\" width=48 flex=1", true),
        ("column display=\"unset\"\n  text \"Mon\" width=48 flex=1", true),
        ("box display=\"flex\"\n  text \"Mon\" width=48 flex=1", false),
        ("box display=\"flex\" flex-direction=\"column-reverse\"\n  text \"Mon\" width=48 flex=1", true),
        ("column flex-direction=(on ? \"row\" : \"column\")\n  text \"Mon\" width=48 flex=1", false),
        ("column class=(on ? On : Off)\n  text \"Mon\" width=48 flex=1 font-size=\"1rem\"", false),
    ]);
    let src = "style Axis\n  flex-direction=\"row\"\ncomponent App\n  view\n    box\n      column class=Axis\n        text \"Mon\" width=48 flex=1\n      column class=Axis flex-direction=\"column\"\n        text \"Tue\" width=48 flex=1\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 9)]);
}

#[test]
fn text_width_respects_flex_shorthand_order() {
    cases(&[
        (
            "row\n  text \"Mon\" width=48 flex=1 flex-grow=0 flex-basis=\"auto\"",
            true,
        ),
        (
            "row\n  text \"Mon\" width=48 flex-grow=0 flex-basis=\"auto\" flex=1",
            false,
        ),
        (
            "row\n  text \"Mon\" width=48 flex=\"none\" flex-grow=1",
            false,
        ),
        (
            "row\n  text \"Mon\" width=48 flex-grow=1 flex=\"none\"",
            true,
        ),
        (
            "row\n  text \"Mon\" width=48 flex=\"none\" flex-basis=\"3rem\"",
            false,
        ),
        (
            "row\n  text \"Mon\" width=48 flex-basis=\"3rem\" flex=\"none\"",
            true,
        ),
        (
            "row\n  text \"Mon\" width=48 flex=(on ? 1 : 2) flex-grow=0 flex-basis=\"auto\"",
            true,
        ),
    ]);
    let src = "style Grow\n  flex=1\nstyle Still\n  flex-grow=0 flex-basis=\"auto\"\ncomponent App\n  view\n    row\n      text \"Mon\" width=48 class=Grow flex-grow=0 flex-basis=\"auto\"\n      text \"Tue\" width=48 class=Still flex=1\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 8)]);
}

#[test]
fn text_width_descends_through_uncertain_display() {
    cases(&[
        ("text \"Mon\" width=48 display=\"unset\"", true),
        (
            "box display=(on ? \"block\" : \"none\")\n  text \"Mon\" width=48",
            true,
        ),
        (
            "box display=(on ? \"block\" : \"none\")\n  box\n    text \"Mon\" width=48 flex=1",
            true,
        ),
        (
            "box display=(on ? \"block\" : \"flex\")\n  text \"Mon\" width=48 flex=1",
            false,
        ),
        (
            "box width=48\n  text \"Mon\" display=(on ? \"block\" : \"none\")",
            false,
        ),
        ("box display=\"none\"\n  text \"Mon\" width=48", false),
    ]);
    let src = "component App\n  state on = true\n  view\n    box width=48 display=(on ? \"block\" : \"none\")\n      text \"Mon\" width=48\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 5)]);
}

#[test]
fn text_width_diagnoses_absolute_boxes_without_counting_them_in_ancestors() {
    let src = "component App\n  view\n    row width=48\n      box position=\"absolute\" width=24 flex=1\n        text \"Mon\"\n";
    assert_eq!(warned(src), [("lower-text-px-width".into(), 4)]);
}

#[test]
fn text_width_names_the_bound_to_repair() {
    for (attrs, bound) in [
        ("width=48 max-width=24", "max-width"),
        ("max-width=24", "max-width"),
        ("width=48 min-width=96", "min-width"),
        ("min-width=96 max-width=24", "min-width"),
    ] {
        let src = format!("component App\n  view\n    text \"Mon\" {attrs}\n");
        let (compiled, warnings) = contract::compile_warned(&src);
        assert!(compiled.is_ok(), "{compiled:?}");
        let warning = warnings
            .iter()
            .find(|w| w.id == "lower-text-px-width")
            .unwrap();
        assert!(warning.message.contains(&format!("`{bound}`")), "{warning}");
        let repaired = src
            .replace("=48", "=\"3rem\"")
            .replace("=24", "=\"1.5rem\"")
            .replace("=96", "=\"6rem\"");
        assert_eq!(warned(&repaired), [], "{repaired}");
    }
}

#[test]
fn text_width_uses_parents_inserted_by_lowering() {
    let mut failures = Vec::new();
    for (view, expected) in [
        ("list height=200\n  for n in [1] key=n\n    row delete=remove(n) width=48 flex=\"0 0 8rem\"\n      text \"Mon\"", false),
        ("list height=200\n  if on\n    row delete=remove(1) width=48 flex=1\n      text \"Mon\"", false),
        ("list height=200 appearance=\"auto\"\n  section\n    for n in [1] key=n\n      row delete=remove(n) width=48 flex=\"0 0 8rem\"\n        text \"Mon\"", false),
        ("list height=200 appearance=\"auto\"\n  section\n    match some(1)\n      case some(n)\n        row delete=remove(n) width=48 flex=1\n          text \"Mon\"\n      case none\n        text \"Empty\"", false),
        ("list height=200\n  row delete=remove(1) max-width=48 flex=\"0 0 8rem\"\n    text \"Mon\"", true),
        ("list height=200 appearance=\"auto\"\n  section display=\"flex\" flex-direction=\"row\"\n    row width=48 flex=\"0 0 8rem\"\n      text \"Mon\"", true),
        ("list height=200 appearance=\"auto\"\n  section display=(on ? \"block\" : \"flex\")\n    row width=48 flex=1\n      text \"Mon\"", true),
    ] {
        let view: String = view.lines().map(|l| format!("    {l}\n")).collect();
        let src = format!("component App\n  state on = true\n  state removed = 0\n  action remove(n: number)\n    removed = n\n  view\n{view}");
        let (result, warnings) = contract::compile_warned(&src);
        let got = warnings.iter().any(|w| w.id == "lower-text-px-width");
        if result.is_err() || got != expected {
            failures.push(format!("{src}\nexpected warning: {expected}, got {got}; errors: {:?}; warnings: {warnings:?}", result.err()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn text_width_initial_minimum_preserves_a_fixed_ceiling() {
    cases(&[
        ("text \"Mon\" max-width=24 min-width=\"auto\"", true),
        ("text \"Mon\" max-width=24 min-width=\"unset\"", true),
        ("text \"Mon\" width=48 min-width=\"auto\"", true),
        (
            "row\n  text \"Mon\" max-width=24 min-width=\"auto\" flex=1",
            true,
        ),
        ("text \"Mon\" min-width=\"auto\"", false),
        ("text \"Mon\" max-width=24 min-width=\"3em\"", false),
        ("text \"Mon\" max-width=24 min-width=\"3rem\"", false),
        (
            "text \"Mon\" max-width=24 min-width=(on ? \"auto\" : \"3rem\")",
            false,
        ),
    ]);
    let src = "style Floor\n  min-width=\"3rem\"\ncomponent App\n  view\n    box\n      text \"Mon\" max-width=24 class=Floor min-width=\"unset\"\n      text \"Tue\" max-width=24 class=Floor min-width=\"auto\"\n      text \"Wed\" max-width=24 class=Floor min-width=\"\"\n      text \"Thu\" max-width=24 class=Floor\n";
    assert_eq!(
        warned(src),
        [
            ("lower-text-px-width".into(), 6),
            ("lower-text-px-width".into(), 7),
            ("lower-text-px-width".into(), 8)
        ]
    );
    cases(&[
        ("text \"Mon\" max-width=24 min-width=\"\"", true),
        ("text \"Mon\" max-width=24 min-width=\"   \"", true),
        ("text \"Mon\" min-width=\"\"", false),
    ]);
}

#[test]
fn text_width_preserves_shared_slot_styles_when_only_the_parent_axis_differs() {
    let mut failures = Vec::new();
    for parents in [
        &["row", "column"][..],
        &["column", "row"][..],
        &["row", "column", "row"][..],
        &["column", "row", "column"][..],
    ] {
        let placements: String = parents
            .iter()
            .map(|tag| format!("      {tag}\n        children\n"))
            .collect();
        for (attrs, outer_warns) in [
            ("width=48", true),
            ("width=48 flex=1", false),
            ("max-width=48 flex=1", true),
        ] {
            let src = format!("component App\n  view\n    Shell()\n      column {attrs}\n        text \"Title\"\n        text \"Body\" width=40\ncomponent Shell\n  slot\n  view\n    box\n{placements}");
            let got = warned(&src);
            let mut expected = vec![("lower-text-px-width".into(), 6)];
            if outer_warns {
                expected.push(("lower-text-px-width".into(), 4));
            }
            if got != expected {
                failures.push(format!("{src}\nexpected {expected:?}, got {got:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
