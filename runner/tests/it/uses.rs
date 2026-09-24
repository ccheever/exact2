//! @ref LLP 1047 D2 — what a plan uses is read from its rows, never declared.

use exact_runner::{uses, Capability, Uses};

fn used(source: &str) -> Uses {
    uses(&contract::compile(source).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn markdown_is_a_markup_prop_that_can_be_markdown() {
    let plain = used("component A\n  view\n    text \"**b**\"\n");
    assert_eq!(plain, Uses::NONE);
    let constant = used("component A\n  view\n    text \"**b**\" markup=\"markdown\"\n");
    assert!(constant.has(Capability::Markdown));
    assert_eq!(constant.to_string(), "markdown");
    // Another constant selects nothing; a computed one might be Markdown.
    let other = used("component A\n  view\n    text \"**b**\" markup=\"plain\"\n");
    assert_eq!(other, Uses::NONE);
    let computed = used(
        "component A\n  state rich = true\n  view\n    text \"**b**\" markup=(rich ? \"markdown\" : \"plain\")\n",
    );
    assert!(computed.has(Capability::Markdown));
}

#[test]
fn a_set_names_what_it_holds_beyond_another() {
    let markdown = Uses::NONE.with(Capability::Markdown);
    assert!(markdown.beyond(markdown).is_empty());
    assert_eq!(markdown.beyond(Uses::NONE), markdown);
    assert!(Uses::NONE.beyond(markdown).is_empty());
    assert_eq!(Uses::NONE.to_string(), "");
}
