//! A grouped list's section is laid out by the platform: UIKit's list draws
//! its rows one under another across the card. Lowering refuses a section's
//! own layout other than the space above and below it and whether it shows,
//! written on it or given by its class.

/// `body` under a 402-point column, with `style` before the component.
fn app(style: &str, body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "{style}component App\n  state on = true\n  view\n    column width=402 height=874\n{body}"
    )
}

#[test]
fn a_sections_layout_is_the_platforms() {
    for (style, section, named) in [
        (
            "",
            "section display=\"flex\" flex-direction=\"row\"",
            "display",
        ),
        ("", "section display=(on ? \"block\" : \"flex\")", "display"),
        ("", "section height=300", "height"),
        ("", "section padding=12", "padding"),
        ("", "section margin=8", "margin"),
        (
            "style Wide\n  width=\"50%\"\n",
            "section class=Wide",
            "width",
        ),
    ] {
        let src = app(style, &format!("list appearance=\"auto\" flex=1\n  {section}\n    row min-height=\"50%\"\n      text \"Mon\""));
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, "lower-grouped-list", "{section}: {}", e.message);
        assert!(
            e.message.contains(&format!("`{named}`")),
            "{section}: {}",
            e.message
        );
    }
    for section in [
        "section margin-top=20 margin-bottom=0",
        "section display=\"none\"",
        "section display=(on ? \"block\" : \"none\")",
        "section font-size=13 color=\"gray\"",
        "section class=(on ? Hidden : Quiet)",
    ] {
        let src = app(
            "style Hidden\n  display=\"none\"\nstyle Quiet\n  color=\"gray\"\n",
            &format!("list appearance=\"auto\" flex=1\n  {section}\n    row\n      text \"Mon\""),
        );
        assert!(contract::compile(&src).is_ok(), "{section}");
    }
}
